mod api;
mod backend;
mod config;
mod crypto;
mod engine;
mod engine_manager;
mod history;
mod model_manager;
mod paths;
mod pipeline;
mod runtime;
mod session;
mod storage;
mod translation_memory;

use std::{env, process::Command, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use api::AppState;
use config::LaunchOptions;
use crypto::VaultCrypto;
use engine_manager::EngineManager;
use paths::data_dir;
use reqwest::{Client, redirect::Policy};
use session::{SessionGuard, SessionRole};
use storage::StorageWorker;

#[tokio::main]
async fn main() {
    let arguments = model_manager::arguments_for_executable(
        env::args().skip(1).collect::<Vec<_>>(),
        env::current_exe().ok().as_deref(),
    );
    let model_command = model_manager::is_model_command(&arguments);
    if let Err(error) = run(&arguments, model_command).await {
        eprintln!("{error:#}");
        let interactive_launch = !model_command && !arguments.iter().any(|arg| arg == "--no-open");
        if interactive_launch {
            show_startup_error(&error);
        }
        std::process::exit(1);
    }
}

async fn run(arguments: &[String], model_command: bool) -> Result<()> {
    if model_command {
        model_manager::run(arguments).await?;
        return Ok(());
    }

    // Parse command-line options before taking the instance lock so `--help`
    // and invalid configuration never disturb an already-running session.
    let mut options = LaunchOptions::from_env_and_args()?;
    let data_dir = data_dir()?;
    let primary_session = match SessionGuard::acquire(&data_dir)? {
        SessionRole::Primary(primary) => primary,
        SessionRole::Secondary(session) => {
            if options.open_browser {
                open_browser(&session.browser_url());
            }
            return Ok(());
        }
    };
    // Secure the public listener before opening the vault or starting an
    // engine. This makes bind failure side-effect free for private data.
    let listener = tokio::net::TcpListener::bind(&options.config.bind)
        .await
        .with_context(|| format!("Could not bind the local address: {}", options.config.bind))?;
    let host = listener
        .local_addr()
        .context("Could not inspect the bound local address")?
        .to_string();
    let origin = format!("http://{host}");

    let crypto = VaultCrypto::load_or_create(&data_dir)?;
    let storage = StorageWorker::start(&data_dir.join("history.db"), crypto)?;
    let client = build_engine_client()?;
    let _local_engine = runtime::LocalEngine::ensure(&mut options.config, &client).await?;
    let engines = Arc::new(EngineManager::new(&options.config, client));
    let security = Arc::new(api::SecurityContext::new(
        primary_session.token().to_owned(),
        host,
        origin.clone(),
    ));
    let state = AppState {
        config: Arc::new(options.config.clone()),
        storage,
        engines,
        security,
    };
    let app = api::router(state);
    let published_session = primary_session.publish(&origin)?;

    println!("{}", options.config.display_name);
    println!("Profile: {}", options.config.profile);
    println!("Encrypted vault: {}", data_dir.display());
    println!("Browser address: {origin}");
    println!("Source text and translations are never written to application logs.");

    if options.open_browser {
        open_browser(&published_session.browser_url());
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("The local translation server stopped unexpectedly")?;
    Ok(())
}

fn build_engine_client() -> Result<Client> {
    Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(120))
        .no_proxy()
        // A model endpoint is trusted only at its validated address. Following
        // a 307/308 could replay the translation POST body to an unvalidated
        // public host selected by a compromised private engine.
        .redirect(Policy::none())
        .build()
        .context("Could not create the local translation HTTP client")
}

#[cfg(windows)]
fn show_startup_error(error: &anyhow::Error) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MessageBoxW,
    };

    let message = format!(
        "Private Translator could not start.\r\n\r\n{error:#}\r\n\r\nIf the translation model is not installed, run Install-Model.exe once and then start the app again.\r\n\r\nYour existing history was not deleted."
    );
    let message = wide_null(&message);
    let title = wide_null("Private Translator - Startup problem");
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
        );
    }
}

#[cfg(windows)]
fn wide_null(value: &str) -> Vec<u16> {
    value
        .encode_utf16()
        .filter(|unit| *unit != 0)
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(not(windows))]
fn show_startup_error(_: &anyhow::Error) {}

fn open_browser(url: &str) {
    #[cfg(windows)]
    let result = Command::new("rundll32")
        .arg("url.dll,FileProtocolHandler")
        .arg(url)
        .spawn();

    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg(url).spawn();

    #[cfg(all(unix, not(target_os = "macos")))]
    let result = Command::new("xdg-open").arg(url).spawn();

    if let Err(error) = result {
        eprintln!("Could not open the default browser automatically: {error}");
    }
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use axum::{
        Router,
        body::Body,
        extract::{Path, State},
        http::{Response, StatusCode, header},
        routing::post,
    };

    use super::*;

    #[tokio::test]
    async fn engine_client_never_replays_translation_bodies_across_redirects() {
        async fn capture(State(hits): State<Arc<AtomicUsize>>, body: String) -> StatusCode {
            assert_eq!(body, "private translation source");
            hits.fetch_add(1, Ordering::SeqCst);
            StatusCode::NO_CONTENT
        }

        async fn redirect(
            Path(status): Path<u16>,
            State(location): State<String>,
        ) -> Response<Body> {
            Response::builder()
                .status(status)
                .header(header::LOCATION, location)
                .body(Body::empty())
                .unwrap()
        }

        let sink_hits = Arc::new(AtomicUsize::new(0));
        let sink = Router::new()
            .route("/capture", post(capture))
            .with_state(Arc::clone(&sink_hits));
        let sink_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let sink_address = sink_listener.local_addr().unwrap();
        let sink_server = tokio::spawn(async move {
            axum::serve(sink_listener, sink).await.unwrap();
        });

        let redirector = Router::new()
            .route("/{status}", post(redirect))
            .with_state(format!("http://{sink_address}/capture"));
        let redirect_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let redirect_address = redirect_listener.local_addr().unwrap();
        let redirect_server = tokio::spawn(async move {
            axum::serve(redirect_listener, redirector).await.unwrap();
        });

        let client = build_engine_client().unwrap();
        for expected in [
            StatusCode::TEMPORARY_REDIRECT,
            StatusCode::PERMANENT_REDIRECT,
        ] {
            let response = client
                .post(format!("http://{redirect_address}/{}", expected.as_u16()))
                .body("private translation source")
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
        }
        assert_eq!(sink_hits.load(Ordering::SeqCst), 0);

        redirect_server.abort();
        sink_server.abort();
    }
}
