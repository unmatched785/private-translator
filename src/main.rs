mod api;
mod config;
mod crypto;
mod engine;
mod history;
mod runtime;

use std::{env, path::PathBuf, process::Command, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use api::AppState;
use config::LaunchOptions;
use crypto::VaultCrypto;
use history::HistoryStore;
use reqwest::Client;

#[tokio::main]
async fn main() -> Result<()> {
    let options = LaunchOptions::from_env_and_args()?;
    let data_dir = data_dir()?;
    let crypto = VaultCrypto::load_or_create(&data_dir)?;
    let history = Arc::new(HistoryStore::open(&data_dir.join("history.db"), crypto)?);
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(120))
        .no_proxy()
        .build()
        .context("로컬 번역 HTTP 클라이언트를 만들 수 없습니다")?;
    let _local_engine = runtime::LocalEngine::ensure(&options.config, &client).await?;
    let state = AppState {
        config: Arc::new(options.config.clone()),
        history,
        client,
    };
    let app = api::router(state);
    let listener = tokio::net::TcpListener::bind(&options.config.bind)
        .await
        .with_context(|| format!("로컬 주소를 열 수 없습니다: {}", options.config.bind))?;
    let url = format!("http://{}", options.config.bind);

    println!("{}", options.config.display_name);
    println!("프로필: {}", options.config.profile);
    println!("암호화 기록고: {}", data_dir.display());
    println!("브라우저 주소: {url}");
    println!("번역 원문과 번역문은 로그에 기록하지 않습니다.");

    if options.open_browser {
        open_browser(&url);
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("로컬 번역 서버가 중단되었습니다")?;
    Ok(())
}

fn data_dir() -> Result<PathBuf> {
    if let Some(path) = env::var_os("TRANSLATOR_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }

    #[cfg(windows)]
    {
        let local_app_data =
            env::var_os("LOCALAPPDATA").context("LOCALAPPDATA 환경 변수를 찾을 수 없습니다")?;
        Ok(PathBuf::from(local_app_data).join("PrivateTranslator"))
    }

    #[cfg(not(windows))]
    {
        if let Some(xdg) = env::var_os("XDG_DATA_HOME") {
            return Ok(PathBuf::from(xdg).join("private-translator"));
        }
        let home = env::var_os("HOME").context("HOME 환경 변수를 찾을 수 없습니다")?;
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
        eprintln!("브라우저를 자동으로 열지 못했습니다: {error}");
    }
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
