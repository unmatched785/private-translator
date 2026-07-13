use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use reqwest::{
    Client, StatusCode,
    header::{CONTENT_RANGE, RANGE},
};

use crate::{paths, runtime};

const DEFAULT_MODEL_ID: &str = "hy-mt2-1.8b-q4";

#[derive(Debug, Default, PartialEq, Eq)]
struct InstallOptions {
    force: bool,
    portable: bool,
    source: Option<PathBuf>,
}

pub fn is_model_command(arguments: &[String]) -> bool {
    matches!(
        arguments.first().map(String::as_str),
        Some("setup" | "model")
    )
}

/// The release packages a byte-for-byte copy of this binary as
/// `Install-Model.exe`. Only an argument-free launch of that exact filename is
/// converted to the explicit setup command; ordinary launchers and all
/// user-supplied arguments retain their normal behavior.
pub fn arguments_for_executable(arguments: Vec<String>, executable: Option<&Path>) -> Vec<String> {
    if !arguments.is_empty() {
        return arguments;
    }
    let is_installer = executable
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("Install-Model.exe"));
    if is_installer {
        vec!["setup".to_owned()]
    } else {
        arguments
    }
}

pub async fn run(arguments: &[String]) -> Result<()> {
    match arguments.first().map(String::as_str) {
        Some("setup") => install(parse_install_options(&arguments[1..])?).await,
        Some("model") => match arguments.get(1).map(String::as_str) {
            Some("install") => install(parse_install_options(&arguments[2..])?).await,
            Some("status") => {
                require_no_extra_arguments(&arguments[2..], "model status")?;
                status().await
            }
            Some("verify") => {
                require_no_extra_arguments(&arguments[2..], "model verify")?;
                verify().await
            }
            Some("path") => {
                require_no_extra_arguments(&arguments[2..], "model path")?;
                println!("{}", paths::model_dir()?.display());
                Ok(())
            }
            Some("help" | "--help" | "-h") | None => {
                print_help();
                Ok(())
            }
            Some(command) => {
                bail!("Unknown model command: {command}\n\nRun `model help` for usage.")
            }
        },
        _ => bail!("Unknown model command"),
    }
}

fn print_help() {
    println!(
        "Private Translator model commands\n\n\
         setup [--portable] [--force] [--from <gguf>]\n\
         model install [--portable] [--force] [--from <gguf>]\n\
         model status\n\
         model verify\n\
         model path\n\n\
         setup is a shortcut for model install. Downloads are resumable and the\n\
         model is accepted only after its pinned size and SHA-256 are verified."
    );
}

fn parse_install_options(arguments: &[String]) -> Result<InstallOptions> {
    let mut options = InstallOptions::default();
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--force" => options.force = true,
            "--portable" => options.portable = true,
            "--from" => {
                index += 1;
                let source = arguments
                    .get(index)
                    .context("--from requires a GGUF path")?;
                options.source = Some(PathBuf::from(source));
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => bail!("Unknown setup option: {other}"),
        }
        index += 1;
    }
    Ok(options)
}

fn require_no_extra_arguments(arguments: &[String], command: &str) -> Result<()> {
    if let Some(argument) = arguments.first() {
        bail!("{command} does not accept an argument: {argument}");
    }
    Ok(())
}

async fn install(options: InstallOptions) -> Result<()> {
    let model = runtime::trusted_model(DEFAULT_MODEL_ID)?;
    let destination = install_destination(&model.file, options.portable)?;
    fs::create_dir_all(
        destination
            .parent()
            .context("Could not resolve the model installation directory")?,
    )
    .with_context(|| {
        format!(
            "Could not create the model installation directory: {}",
            destination.display()
        )
    })?;

    if destination.is_file() {
        match verify_model_file(&destination, &model).await {
            Ok(()) if !options.force => {
                println!("Model is already installed and verified.");
                print_model_details(&model, &destination);
                return Ok(());
            }
            Ok(()) => println!("Replacing the existing verified model because --force was used."),
            Err(error) if !options.force => {
                bail!(
                    "The installed model is not valid: {error:#}\nRun setup --force to replace it."
                );
            }
            Err(_) => println!("Replacing an invalid model because --force was used."),
        }
    }

    println!("Installing the verified translation model.");
    print_model_details(&model, &destination);
    let partial = partial_path(&destination)?;

    if let Some(source) = options.source {
        let source = fs::canonicalize(&source)
            .with_context(|| format!("Could not open the source model: {}", source.display()))?;
        println!("Verifying local source: {}", source.display());
        verify_model_file(&source, &model)
            .await
            .context("The local source model is not the pinned trusted model")?;
        fs::copy(&source, &partial).with_context(|| {
            format!(
                "Could not copy the local model to the installation directory: {}",
                partial.display()
            )
        })?;
        sync_file(&partial)?;
    } else {
        let client = Client::builder()
            .user_agent(concat!("PrivateTranslator/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(24 * 60 * 60))
            .build()
            .context("Could not create the model download client")?;
        download_to_partial(&client, &model, &partial).await?;
    }

    println!("Verifying downloaded model (size and SHA-256)...");
    verify_model_file(&partial, &model)
        .await
        .context("The downloaded model did not match the pinned trusted model")?;
    finalize_install(&partial, &destination)?;

    println!("Model installation complete.");
    print_model_details(&model, &destination);
    Ok(())
}

async fn status() -> Result<()> {
    let model = runtime::trusted_model(DEFAULT_MODEL_ID)?;
    println!("Trusted model: {}", model.id);
    println!("Channel: {}", model.channel);
    println!("Revision: {}", model.revision);

    let locations = model_locations(&model.file)?;
    let mut installed = false;
    for (label, path) in locations {
        if !path.is_file() {
            println!("{label}: not installed ({})", path.display());
            continue;
        }
        installed = true;
        match verify_model_file(&path, &model).await {
            Ok(()) => println!("{label}: installed and verified ({})", path.display()),
            Err(error) => println!("{label}: INVALID ({})\n  {error:#}", path.display()),
        }
    }

    if !installed {
        println!(
            "\nRun `setup` for the one-time {:.2} GB download.",
            decimal_gigabytes(model.bytes)
        );
    }
    Ok(())
}

async fn verify() -> Result<()> {
    let model = runtime::trusted_model(DEFAULT_MODEL_ID)?;
    for (_, path) in model_locations(&model.file)? {
        if path.is_file() {
            println!("Verifying {}...", path.display());
            verify_model_file(&path, &model).await?;
            println!("Model verification passed.");
            print_model_details(&model, &path);
            return Ok(());
        }
    }
    bail!("The model is not installed. Run `setup` first.")
}

fn install_destination(file_name: &str, portable: bool) -> Result<PathBuf> {
    if portable {
        return Ok(runtime::bundle_root()?.join("models").join(file_name));
    }
    Ok(paths::model_dir()?.join(file_name))
}

fn model_locations(file_name: &str) -> Result<Vec<(&'static str, PathBuf)>> {
    let portable = runtime::bundle_root()?.join("models").join(file_name);
    let user = paths::model_dir()?.join(file_name);
    let mut locations = vec![("Portable bundle", portable.clone())];
    if user != portable {
        locations.push(("Current user", user));
    }
    Ok(locations)
}

fn partial_path(destination: &Path) -> Result<PathBuf> {
    let file_name = destination
        .file_name()
        .context("Could not determine the model file name")?
        .to_string_lossy();
    Ok(destination.with_file_name(format!("{file_name}.partial")))
}

async fn download_to_partial(
    client: &Client,
    model: &runtime::TrustedModel,
    partial: &Path,
) -> Result<()> {
    let mut resume_at = match fs::metadata(partial) {
        Ok(metadata) if metadata.len() < model.bytes => metadata.len(),
        Ok(metadata) if metadata.len() == model.bytes => {
            if verify_model_file(partial, model).await.is_ok() {
                println!("Existing download is complete; reusing it.");
                return Ok(());
            }
            0
        }
        Ok(_) => 0,
        Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
        Err(error) => return Err(error).context("Could not inspect the partial model download"),
    };

    if resume_at > 0 {
        println!(
            "Resuming download at {:.1}% ({:.2} GB of {:.2} GB).",
            percentage(resume_at, model.bytes),
            decimal_gigabytes(resume_at),
            decimal_gigabytes(model.bytes)
        );
    } else {
        println!(
            "Downloading {:.2} GB from the pinned official source.",
            decimal_gigabytes(model.bytes)
        );
    }

    let mut request = client.get(&model.download_url);
    if resume_at > 0 {
        request = request.header(RANGE, format!("bytes={resume_at}-"));
    }
    let mut response = request
        .send()
        .await
        .context("Could not download the model from the official source")?;
    let status = response.status();
    if !status.is_success() {
        response
            .error_for_status_ref()
            .context("The official model source rejected the download request")?;
    }

    let append = if status == StatusCode::PARTIAL_CONTENT {
        validate_content_range(&response, resume_at, model.bytes)?;
        resume_at > 0
    } else {
        if resume_at > 0 {
            println!("The server did not accept resume; restarting the download safely.");
        }
        resume_at = 0;
        false
    };

    let mut file = if append {
        OpenOptions::new().create(true).append(true).open(partial)
    } else {
        File::create(partial)
    }
    .with_context(|| format!("Could not open the partial download: {}", partial.display()))?;

    let mut downloaded = resume_at;
    let mut last_progress = Instant::now();
    while let Some(chunk) = response
        .chunk()
        .await
        .context("The model download was interrupted; run setup again to resume")?
    {
        downloaded = downloaded
            .checked_add(chunk.len() as u64)
            .context("The model download size overflowed")?;
        if downloaded > model.bytes {
            bail!("The model source returned more data than the trusted size");
        }
        file.write_all(&chunk)
            .context("Could not write the partial model download")?;
        if last_progress.elapsed() >= Duration::from_secs(1) {
            print!(
                "\rDownloading: {:5.1}% ({:.2}/{:.2} GB)",
                percentage(downloaded, model.bytes),
                decimal_gigabytes(downloaded),
                decimal_gigabytes(model.bytes)
            );
            io::stdout().flush().ok();
            last_progress = Instant::now();
        }
    }
    file.sync_all()
        .context("Could not flush the partial model download")?;
    println!(
        "\rDownloading: {:5.1}% ({:.2}/{:.2} GB)",
        percentage(downloaded, model.bytes),
        decimal_gigabytes(downloaded),
        decimal_gigabytes(model.bytes)
    );

    if downloaded != model.bytes {
        bail!(
            "The model download is incomplete (expected {} bytes, received {}). Run setup again to resume.",
            model.bytes,
            downloaded
        );
    }
    Ok(())
}

fn validate_content_range(
    response: &reqwest::Response,
    resume_at: u64,
    expected_bytes: u64,
) -> Result<()> {
    let value = response
        .headers()
        .get(CONTENT_RANGE)
        .context("The model source returned a partial response without Content-Range")?
        .to_str()
        .context("The model source returned an invalid Content-Range header")?;
    let expected_prefix = format!("bytes {resume_at}-");
    let expected_suffix = format!("/{expected_bytes}");
    if !value.starts_with(&expected_prefix) || !value.ends_with(&expected_suffix) {
        bail!("The model source returned an unexpected Content-Range: {value}");
    }
    Ok(())
}

async fn verify_model_file(path: &Path, model: &runtime::TrustedModel) -> Result<()> {
    let path = path.to_path_buf();
    let expected_bytes = model.bytes;
    let expected_sha256 = model.sha256.clone();
    tokio::task::spawn_blocking(move || {
        runtime::verify_file(&path, expected_bytes, &expected_sha256)
    })
    .await
    .context("The model verification task stopped")?
}

fn sync_file(path: &Path) -> Result<()> {
    OpenOptions::new()
        .write(true)
        .open(path)
        .with_context(|| format!("Could not reopen the copied model: {}", path.display()))?
        .sync_all()
        .context("Could not flush the copied model")
}

fn finalize_install(partial: &Path, destination: &Path) -> Result<()> {
    if !destination.exists() {
        return fs::rename(partial, destination).with_context(|| {
            format!(
                "Could not finalize the model installation: {}",
                destination.display()
            )
        });
    }

    let backup = destination.with_extension("gguf.previous");
    if backup.exists() {
        fs::remove_file(&backup).with_context(|| {
            format!("Could not remove an old model backup: {}", backup.display())
        })?;
    }
    fs::rename(destination, &backup).with_context(|| {
        format!(
            "Could not prepare the existing model for replacement: {}",
            destination.display()
        )
    })?;
    if let Err(error) = fs::rename(partial, destination) {
        let _ = fs::rename(&backup, destination);
        return Err(error).context("Could not finalize the replacement model");
    }
    fs::remove_file(&backup).with_context(|| {
        format!(
            "Could not remove the old model backup: {}",
            backup.display()
        )
    })?;
    Ok(())
}

fn print_model_details(model: &runtime::TrustedModel, destination: &Path) {
    println!("Model: {}", model.id);
    println!("Channel: {}", model.channel);
    println!("Revision: {}", model.revision);
    println!("License: {}", model.license);
    println!("Source: {}", model.source);
    println!("Path: {}", destination.display());
}

fn decimal_gigabytes(bytes: u64) -> f64 {
    bytes as f64 / 1_000_000_000.0
}

fn percentage(current: u64, total: u64) -> f64 {
    current as f64 * 100.0 / total as f64
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use axum::{
        Router,
        body::Body,
        http::{HeaderMap, Response},
        routing::get,
    };

    use super::*;

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn recognizes_setup_and_model_commands_only() {
        assert!(is_model_command(&arguments(&["setup"])));
        assert!(is_model_command(&arguments(&["model", "status"])));
        assert!(!is_model_command(&arguments(&["--profile", "lite"])));
    }

    #[test]
    fn argument_free_install_model_executable_enters_setup_mode() {
        assert_eq!(
            arguments_for_executable(Vec::new(), Some(Path::new("Install-Model.exe"))),
            arguments(&["setup"])
        );
        assert_eq!(
            arguments_for_executable(Vec::new(), Some(Path::new("install-model.EXE"))),
            arguments(&["setup"])
        );
    }

    #[test]
    fn installer_filename_never_overrides_explicit_arguments_or_near_matches() {
        let explicit = arguments(&["model", "status"]);
        assert_eq!(
            arguments_for_executable(explicit.clone(), Some(Path::new("Install-Model.exe"))),
            explicit
        );
        assert!(
            arguments_for_executable(Vec::new(), Some(Path::new("Install-Model-copy.exe")))
                .is_empty()
        );
        assert!(
            arguments_for_executable(Vec::new(), Some(Path::new("PrivateTranslator-Lite.exe")))
                .is_empty()
        );
        assert!(arguments_for_executable(Vec::new(), None).is_empty());
    }

    #[test]
    fn parses_install_options() {
        assert_eq!(
            parse_install_options(&arguments(&[
                "--portable",
                "--force",
                "--from",
                "model.gguf"
            ]))
            .unwrap(),
            InstallOptions {
                force: true,
                portable: true,
                source: Some(PathBuf::from("model.gguf")),
            }
        );
    }

    #[test]
    fn rejects_missing_from_path_and_unknown_options() {
        assert!(parse_install_options(&arguments(&["--from"])).is_err());
        assert!(parse_install_options(&arguments(&["--latest"])).is_err());
    }

    #[test]
    fn copied_model_can_be_flushed_on_windows() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("model.gguf.partial");
        fs::write(&path, b"model").unwrap();
        sync_file(&path).unwrap();
    }

    #[test]
    fn validates_resumed_content_range() {
        let response = reqwest::Response::from(
            axum::http::Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(CONTENT_RANGE, "bytes 3-5/6")
                .body("")
                .unwrap(),
        );
        validate_content_range(&response, 3, 6).unwrap();
        assert!(validate_content_range(&response, 2, 6).is_err());
    }

    #[tokio::test]
    async fn resumes_a_partial_download_and_verifies_the_result() {
        let range_seen = Arc::new(AtomicBool::new(false));
        let handler_range_seen = Arc::clone(&range_seen);
        let app = Router::new().route(
            "/",
            get(move |headers: HeaderMap| {
                let handler_range_seen = Arc::clone(&handler_range_seen);
                async move {
                    if headers.get(RANGE).and_then(|value| value.to_str().ok()) == Some("bytes=3-")
                    {
                        handler_range_seen.store(true, Ordering::SeqCst);
                    }
                    Response::builder()
                        .status(StatusCode::PARTIAL_CONTENT)
                        .header(CONTENT_RANGE, "bytes 3-5/6")
                        .body(Body::from("def"))
                        .unwrap()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let temporary = tempfile::tempdir().unwrap();
        let partial = temporary.path().join("model.gguf.partial");
        fs::write(&partial, b"abc").unwrap();
        let model = runtime::TrustedModel {
            id: "test-model".into(),
            channel: "test".into(),
            file: "model.gguf".into(),
            bytes: 6,
            sha256: "bef57ec7f53a6d40beb640a780a639c83bc29ac8a9816f1fc6c5c6dcd93c4721".into(),
            source: "local test".into(),
            revision: "test".into(),
            download_url: format!("http://{address}/"),
            license: "test".into(),
        };
        let client = Client::new();

        download_to_partial(&client, &model, &partial)
            .await
            .unwrap();

        assert!(range_seen.load(Ordering::SeqCst));
        assert_eq!(fs::read(&partial).unwrap(), b"abcdef");
        verify_model_file(&partial, &model).await.unwrap();
        server.abort();
    }
}
