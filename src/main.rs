mod api;
mod backend;
mod config;
mod crypto;
mod engine;
mod engine_manager;
mod history;
mod pipeline;
mod runtime;
mod storage;
mod translation_memory;

use std::{env, path::PathBuf, process::Command, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use api::AppState;
use config::LaunchOptions;
use crypto::VaultCrypto;
use engine_manager::EngineManager;
use reqwest::Client;
use storage::StorageWorker;

#[tokio::main]
async fn main() -> Result<()> {
    let options = LaunchOptions::from_env_and_args()?;
    let data_dir = data_dir()?;
    let crypto = VaultCrypto::load_or_create(&data_dir)?;
    let storage = StorageWorker::start(&data_dir.join("history.db"), crypto)?;
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(120))
        .no_proxy()
        .build()
        .context("Could not create the local translation HTTP client")?;
    let _local_engine = runtime::LocalEngine::ensure(&options.config, &client).await?;
    let engines = Arc::new(EngineManager::new(&options.config, client));
    let state = AppState {
        config: Arc::new(options.config.clone()),
        storage,
        engines,
    };
    let app = api::router(state);
    let listener = tokio::net::TcpListener::bind(&options.config.bind)
        .await
        .with_context(|| format!("Could not bind the local address: {}", options.config.bind))?;
    let url = format!("http://{}", options.config.bind);

    println!("{}", options.config.display_name);
    println!("Profile: {}", options.config.profile);
    println!("Encrypted vault: {}", data_dir.display());
    println!("Browser address: {url}");
    println!("Source text and translations are never written to application logs.");

    if options.open_browser {
        open_browser(&url);
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("The local translation server stopped unexpectedly")?;
    Ok(())
}

fn data_dir() -> Result<PathBuf> {
    if let Some(path) = env::var_os("TRANSLATOR_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }

    #[cfg(windows)]
    {
        let local_app_data = env::var_os("LOCALAPPDATA")
            .context("The LOCALAPPDATA environment variable is not available")?;
        Ok(PathBuf::from(local_app_data).join("PrivateTranslator"))
    }

    #[cfg(not(windows))]
    {
        if let Some(xdg) = env::var_os("XDG_DATA_HOME") {
            return Ok(PathBuf::from(xdg).join("private-translator"));
        }
        let home = env::var_os("HOME").context("The HOME environment variable is not available")?;
        Ok(PathBuf::from(home).join(".local/share/private-translator"))
    }
}

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
