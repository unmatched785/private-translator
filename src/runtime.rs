use std::{
    collections::BTreeSet,
    env,
    fs::{self, File},
    io::{BufReader, Read},
    net::{IpAddr, Ipv4Addr, TcpListener},
    path::{Component, Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use reqwest::{Client, Url};
use serde::Deserialize;
use tokio::time::sleep;

use crate::{
    config::{AppConfig, LocalEngineConfig},
    paths,
};

pub struct LocalEngine {
    child: Option<Child>,
    #[cfg(windows)]
    _job: Option<KillOnCloseJob>,
}

#[derive(Debug, Deserialize)]
struct TrustedArtifacts {
    schema_version: u32,
    runtime: TrustedRuntime,
    models: Vec<TrustedModel>,
}

#[derive(Debug, Deserialize)]
struct TrustedRuntime {
    id: String,
    files: Vec<TrustedFile>,
}

#[derive(Debug, Deserialize)]
struct TrustedFile {
    path: String,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TrustedModel {
    pub(crate) id: String,
    pub(crate) channel: String,
    pub(crate) file: String,
    pub(crate) bytes: u64,
    pub(crate) sha256: String,
    pub(crate) source: String,
    pub(crate) revision: String,
    pub(crate) download_url: String,
    pub(crate) license: String,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelIdentity>,
}

#[derive(Debug, Deserialize)]
struct ModelIdentity {
    id: String,
}

impl LocalEngine {
    pub async fn ensure(config: &mut AppConfig, client: &Client) -> Result<Self> {
        let Some(engine) = config.local_engine.clone() else {
            return Ok(Self::external());
        };
        let model_index = config
            .models
            .iter()
            .position(|model| model.id == engine.model_id)
            .context("The configured local model was not found")?;
        let model = config.models[model_index].clone();
        let bundle_root = bundle_root().context("Could not locate a trusted translator bundle")?;
        let executable = resolve_bundle_path(&bundle_root, &engine.executable)
            .with_context(|| format!("Could not find the local engine: {}", engine.executable))?;
        let model_path = resolve_model_path(&bundle_root, &engine)
            .with_context(|| format!("Could not find the local model: {}", engine.model_path))?;

        let executable_for_verification = executable.clone();
        let model_for_verification = model_path.clone();
        let engine_for_verification = engine.clone();
        tokio::task::spawn_blocking(move || {
            verify_trusted_artifacts(
                &executable_for_verification,
                &model_for_verification,
                &engine_for_verification,
            )
        })
        .await
        .context("The local translation artifact verification task stopped")??;
        println!("Local translation artifact integrity: verified");

        // Choose the private endpoint only after the potentially long artifact
        // verification, keeping the bind-to-spawn race window as small as the
        // process API permits.
        let mut endpoint =
            Url::parse(&model.endpoint).context("The local model endpoint is invalid")?;
        let configured_host = endpoint
            .host_str()
            .context("The local model endpoint has no host")?;
        let bind_ip = if configured_host.eq_ignore_ascii_case("localhost") {
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        } else {
            configured_host
                .parse::<IpAddr>()
                .context("The managed local model endpoint must use a loopback IP address")?
        };
        let reservation = TcpListener::bind((bind_ip, 0))
            .context("Could not reserve a private port for the local translation engine")?;
        let port = reservation
            .local_addr()
            .context("Could not inspect the reserved local engine port")?
            .port();
        drop(reservation);
        endpoint
            .set_port(Some(port))
            .map_err(|_| anyhow::anyhow!("Could not assign the private local engine port"))?;
        let api_key = random_api_key()?;
        config.models[model_index].endpoint = endpoint.to_string();
        config.models[model_index].runtime_api_key = Some(api_key.clone());
        let health_url = health_url(&endpoint);
        let models_url = models_url(&endpoint);
        let host = endpoint
            .host_str()
            .context("The local model endpoint has no host")?;
        let threads = engine_threads(&engine);

        let mut command = Command::new(&executable);
        command.current_dir(
            executable
                .parent()
                .context("Could not resolve the local engine directory")?,
        );
        command
            .arg("-m")
            .arg(&model_path)
            .arg("--alias")
            .arg(&model.api_model)
            .arg("--host")
            .arg(host)
            .arg("--port")
            .arg(port.to_string())
            .arg("--api-key")
            .arg(&api_key)
            .arg("-c")
            .arg(engine.context_size.to_string())
            .arg("-t")
            .arg(threads.to_string())
            .arg("--parallel")
            .arg("1")
            .arg("--jinja")
            .arg("--log-disable")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = command.spawn().with_context(|| {
            format!(
                "Could not start the local translation engine: {}",
                executable.display()
            )
        })?;

        #[cfg(windows)]
        let job = match KillOnCloseJob::attach(&child) {
            Ok(job) => job,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };

        for _ in 0..180 {
            if let Some(status) = child
                .try_wait()
                .context("Could not inspect the local translation engine status")?
            {
                bail!("The local translation engine exited during startup: {status}");
            }
            match probe_model_identity(
                client,
                &health_url,
                &models_url,
                &model.api_model,
                Some(&api_key),
            )
            .await
            {
                Ok(true) => {
                    println!(
                        "Local translation engine: ready (model {}, {threads} threads)",
                        model.api_model
                    );
                    return Ok(Self {
                        child: Some(child),
                        #[cfg(windows)]
                        _job: Some(job),
                    });
                }
                Ok(false) => {}
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(error);
                }
            }
            sleep(Duration::from_millis(250)).await;
        }

        let _ = child.kill();
        let _ = child.wait();
        bail!("The local translation engine was not ready within 45 seconds");
    }

    fn external() -> Self {
        Self {
            child: None,
            #[cfg(windows)]
            _job: None,
        }
    }
}

impl Drop for LocalEngine {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(windows)]
struct KillOnCloseJob {
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
impl KillOnCloseJob {
    fn attach(child: &Child) -> Result<Self> {
        use std::{mem, os::windows::io::AsRawHandle, ptr};

        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        };

        let handle = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if handle.is_null() {
            bail!(
                "Could not create local engine termination protection: {}",
                std::io::Error::last_os_error()
            );
        }
        let job = Self { handle };
        let mut information: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { mem::zeroed() };
        information.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        let configured = unsafe {
            SetInformationJobObject(
                job.handle,
                JobObjectExtendedLimitInformation,
                (&raw const information).cast(),
                mem::size_of_val(&information) as u32,
            )
        };
        if configured == 0 {
            bail!(
                "Could not configure local engine termination protection: {}",
                std::io::Error::last_os_error()
            );
        }

        let assigned =
            unsafe { AssignProcessToJobObject(job.handle, child.as_raw_handle().cast()) };
        if assigned == 0 {
            bail!(
                "Could not attach the local engine to termination protection: {}",
                std::io::Error::last_os_error()
            );
        }
        Ok(job)
    }
}

#[cfg(windows)]
impl Drop for KillOnCloseJob {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::CloseHandle;

        unsafe {
            CloseHandle(self.handle);
        }
    }
}

fn health_url(endpoint: &Url) -> Url {
    let mut url = endpoint.clone();
    url.set_path("/health");
    url.set_query(None);
    url.set_fragment(None);
    url
}

fn models_url(endpoint: &Url) -> Url {
    let mut url = endpoint.clone();
    let path = format!("{}/models", endpoint.path().trim_end_matches('/'));
    url.set_path(&path);
    url.set_query(None);
    url.set_fragment(None);
    url
}

async fn probe_model_identity(
    client: &Client,
    health_url: &Url,
    models_url: &Url,
    expected_model: &str,
    api_key: Option<&str>,
) -> Result<bool> {
    let mut health_request = client
        .get(health_url.clone())
        .timeout(Duration::from_millis(700));
    if let Some(api_key) = api_key {
        health_request = health_request.bearer_auth(api_key);
    }
    let health_response = match health_request.send().await {
        Ok(response) => response,
        Err(error) if error.is_connect() || error.is_timeout() => return Ok(false),
        Err(error) => {
            return Err(error).context("Could not inspect the local translation engine status");
        }
    };

    if !health_response.status().is_success() {
        return Ok(false);
    }

    let mut models_request = client
        .get(models_url.clone())
        .timeout(Duration::from_secs(2));
    if let Some(api_key) = api_key {
        models_request = models_request.bearer_auth(api_key);
    }
    let response = models_request
        .send()
        .await
        .context("Could not read model information from the running local engine")?;
    if !response.status().is_success() {
        bail!(
            "The local engine port is in use, but model verification failed: HTTP {}",
            response.status()
        );
    }

    let identities: ModelsResponse = response
        .json()
        .await
        .context("The running local engine returned invalid model information")?;
    if identities
        .data
        .iter()
        .any(|identity| identity.id == expected_model)
    {
        return Ok(true);
    }

    let actual = identities
        .data
        .iter()
        .map(|identity| identity.id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    bail!(
        "A different model is running on the local engine port (expected: {expected_model}, actual: {actual})"
    )
}

fn random_api_key() -> Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|error| {
        anyhow::anyhow!("Could not generate local engine authentication: {error}")
    })?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn engine_threads(config: &LocalEngineConfig) -> usize {
    if config.threads > 0 {
        return config.threads;
    }
    std::thread::available_parallelism()
        .map(|threads| threads.get())
        .unwrap_or(4)
        .clamp(1, 8)
}

pub(crate) fn bundle_root() -> Result<PathBuf> {
    let root = if let Some(explicit_root) = env::var_os("TRANSLATOR_BUNDLE_DIR") {
        PathBuf::from(explicit_root)
    } else {
        env::current_exe()
            .context("Could not resolve the executable path")?
            .parent()
            .context("Could not resolve the executable directory")?
            .to_path_buf()
    };
    let root = fs::canonicalize(&root).with_context(|| {
        format!(
            "The translator bundle path does not exist: {}",
            root.display()
        )
    })?;
    if !root.is_dir() {
        bail!(
            "The translator bundle path is not a directory: {}",
            root.display()
        );
    }
    Ok(root)
}

fn resolve_model_path(root: &Path, engine: &LocalEngineConfig) -> Result<PathBuf> {
    let relative = validated_relative_path(&engine.model_path)?;
    let portable_candidate = root.join(&relative);
    if portable_candidate.is_file() {
        return resolve_bundle_path(root, &engine.model_path);
    }

    let model = trusted_model(&engine.model_id)?;
    let model_root = paths::model_dir()?;
    let user_candidate = model_root.join(&model.file);
    if user_candidate.is_file() {
        let canonical_root = fs::canonicalize(&model_root).with_context(|| {
            format!(
                "Could not resolve the model directory: {}",
                model_root.display()
            )
        })?;
        let canonical_model = fs::canonicalize(&user_candidate).with_context(|| {
            format!(
                "Could not resolve the installed model: {}",
                user_candidate.display()
            )
        })?;
        if canonical_model.starts_with(&canonical_root) && canonical_model.is_file() {
            return Ok(canonical_model);
        }
        bail!("The installed model path escapes the configured model directory");
    }

    let executable = env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "PrivateTranslator-Lite.exe".into());
    bail!(
        "The verified Hy-MT2 model is not installed. Run:\n  \"{executable}\" setup\n\nThe one-time model download is about 1.13 GB."
    )
}

fn validated_relative_path(raw: &str) -> Result<PathBuf> {
    let relative = Path::new(raw);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!("Only relative paths inside the translator bundle are allowed");
    }
    Ok(relative.to_path_buf())
}

fn resolve_bundle_path(root: &Path, raw: &str) -> Result<PathBuf> {
    let relative = validated_relative_path(raw)?;

    let candidate = fs::canonicalize(root.join(relative)).context("The file does not exist")?;
    if !candidate.starts_with(root) || !candidate.is_file() {
        bail!("Files outside the translator bundle cannot be used");
    }
    Ok(candidate)
}

fn trusted_artifacts() -> Result<TrustedArtifacts> {
    let manifest: TrustedArtifacts =
        serde_json::from_str(include_str!("../packaging/trusted-artifacts.json"))
            .context("The embedded trusted artifact manifest is invalid")?;
    if manifest.schema_version != 1 {
        bail!(
            "Unsupported trusted artifact manifest version: {}",
            manifest.schema_version
        );
    }
    Ok(manifest)
}

pub(crate) fn trusted_model(id: &str) -> Result<TrustedModel> {
    trusted_artifacts()?
        .models
        .into_iter()
        .find(|model| model.id == id)
        .with_context(|| format!("The model is not in the trust manifest: {id}"))
}

fn verify_trusted_artifacts(
    executable: &Path,
    model_path: &Path,
    engine: &LocalEngineConfig,
) -> Result<()> {
    let manifest = trusted_artifacts()?;
    if manifest.runtime.id != engine.runtime_id {
        bail!(
            "The local engine type differs from the trust manifest (config: {}, manifest: {})",
            engine.runtime_id,
            manifest.runtime.id
        );
    }

    let model = manifest
        .models
        .iter()
        .find(|model| model.id == engine.model_id)
        .with_context(|| {
            format!(
                "The model is not in the trust manifest: {}",
                engine.model_id
            )
        })?;
    let actual_model_name = model_path
        .file_name()
        .context("Could not read the model file name")?
        .to_string_lossy();
    if !actual_model_name.eq_ignore_ascii_case(&model.file) {
        bail!(
            "The model file name differs from the trust manifest (expected: {}, actual: {})",
            model.file,
            actual_model_name
        );
    }
    verify_file(model_path, model.bytes, &model.sha256).with_context(|| {
        format!(
            "Model integrity verification failed: {}",
            model_path.display()
        )
    })?;

    let runtime_dir = executable
        .parent()
        .context("Could not resolve the local engine directory")?;
    let expected_names = manifest
        .runtime
        .files
        .iter()
        .map(|file| file.path.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let executable_name = executable
        .file_name()
        .context("Could not read the local engine file name")?
        .to_string_lossy()
        .to_ascii_lowercase();
    if !expected_names.contains(&executable_name) {
        bail!("The local engine executable is not in the trust manifest: {executable_name}");
    }

    let actual_names = fs::read_dir(runtime_dir)
        .context("Could not read the local engine directory")?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let extension = path.extension()?.to_string_lossy();
            if !extension.eq_ignore_ascii_case("exe") && !extension.eq_ignore_ascii_case("dll") {
                return None;
            }
            Some(entry.file_name().to_string_lossy().to_ascii_lowercase())
        })
        .collect::<BTreeSet<_>>();
    if actual_names != expected_names {
        let missing = expected_names
            .difference(&actual_names)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        let extra = actual_names
            .difference(&expected_names)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        bail!(
            "The local engine inventory differs from the trust manifest (missing: [{}], extra: [{}])",
            missing,
            extra
        );
    }

    for artifact in &manifest.runtime.files {
        let path = resolve_bundle_path(runtime_dir, &artifact.path)
            .with_context(|| format!("A trusted runtime file is missing: {}", artifact.path))?;
        verify_file(&path, artifact.bytes, &artifact.sha256)
            .with_context(|| format!("Runtime integrity verification failed: {}", artifact.path))?;
    }
    Ok(())
}

pub(crate) fn verify_file(path: &Path, expected_bytes: u64, expected_sha256: &str) -> Result<()> {
    if expected_sha256.len() != 64 || !expected_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        bail!("The SHA-256 value in the trust manifest is invalid");
    }
    let metadata = fs::metadata(path).context("Could not read file metadata")?;
    if metadata.len() != expected_bytes {
        bail!(
            "File size differs (expected: {expected_bytes}, actual: {})",
            metadata.len()
        );
    }

    let actual_sha256 = sha256_file(path)?;
    if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
        bail!("SHA-256 differs (expected: {expected_sha256}, actual: {actual_sha256})");
    }
    Ok(())
}

#[cfg(windows)]
fn sha256_file(path: &Path) -> Result<String> {
    use std::ptr;

    use windows_sys::Win32::Security::Cryptography::{
        BCRYPT_ALG_HANDLE, BCRYPT_HASH_HANDLE, BCRYPT_SHA256_ALGORITHM,
        BCryptCloseAlgorithmProvider, BCryptCreateHash, BCryptDestroyHash, BCryptFinishHash,
        BCryptHashData, BCryptOpenAlgorithmProvider,
    };

    struct Algorithm(BCRYPT_ALG_HANDLE);
    impl Drop for Algorithm {
        fn drop(&mut self) {
            unsafe {
                BCryptCloseAlgorithmProvider(self.0, 0);
            }
        }
    }

    struct Hash(BCRYPT_HASH_HANDLE);
    impl Drop for Hash {
        fn drop(&mut self) {
            unsafe {
                BCryptDestroyHash(self.0);
            }
        }
    }

    fn check_status(status: i32, operation: &str) -> Result<()> {
        if status < 0 {
            bail!("Windows SHA-256 {operation} failed: NTSTATUS 0x{status:08x}");
        }
        Ok(())
    }

    let mut algorithm = ptr::null_mut();
    check_status(
        unsafe {
            BCryptOpenAlgorithmProvider(&raw mut algorithm, BCRYPT_SHA256_ALGORITHM, ptr::null(), 0)
        },
        "initialization",
    )?;
    let algorithm = Algorithm(algorithm);

    let mut hash = ptr::null_mut();
    check_status(
        unsafe {
            BCryptCreateHash(
                algorithm.0,
                &raw mut hash,
                ptr::null_mut(),
                0,
                ptr::null(),
                0,
                0,
            )
        },
        "hash creation",
    )?;
    let hash = Hash(hash);

    let file = File::open(path).context("Could not open file")?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = reader.read(&mut buffer).context("Could not read file")?;
        if read == 0 {
            break;
        }
        check_status(
            unsafe { BCryptHashData(hash.0, buffer.as_ptr(), read as u32, 0) },
            "file processing",
        )?;
    }

    let mut output = [0_u8; 32];
    check_status(
        unsafe { BCryptFinishHash(hash.0, output.as_mut_ptr(), output.len() as u32, 0) },
        "completion",
    )?;
    Ok(output.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(not(windows))]
fn sha256_file(_path: &Path) -> Result<String> {
    bail!("Trusted artifact SHA-256 verification is supported by the Windows package")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_url_uses_same_private_origin() {
        let endpoint = Url::parse("http://127.0.0.1:8080/v1").unwrap();
        assert_eq!(
            health_url(&endpoint).as_str(),
            "http://127.0.0.1:8080/health"
        );
    }

    #[test]
    fn models_url_uses_endpoint_prefix() {
        let endpoint = Url::parse("http://127.0.0.1:8080/v1/").unwrap();
        assert_eq!(
            models_url(&endpoint).as_str(),
            "http://127.0.0.1:8080/v1/models"
        );
    }

    #[test]
    fn embedded_trust_manifest_contains_configured_artifacts() {
        let manifest = trusted_artifacts().unwrap();
        assert_eq!(manifest.runtime.id, "llama.cpp-b9966-win-cpu-x64");
        assert!(
            manifest
                .runtime
                .files
                .iter()
                .any(|file| file.path == "llama-server.exe")
        );
        let model = manifest
            .models
            .iter()
            .find(|model| model.id == "hy-mt2-1.8b-q4")
            .unwrap();
        assert_eq!(model.channel, "stable");
        assert_eq!(model.revision, "1cd5208700acedef4ef93019b6cfc148b8522d45");
        assert!(model.download_url.contains(&model.revision));
        assert_eq!(model.license, "Apache-2.0");
    }

    #[test]
    fn resolve_bundle_path_rejects_escape_and_absolute_paths() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("bundle");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("inside.bin"), b"safe").unwrap();
        fs::write(temporary.path().join("outside.bin"), b"unsafe").unwrap();
        let root = fs::canonicalize(root).unwrap();

        assert!(resolve_bundle_path(&root, "inside.bin").is_ok());
        assert!(resolve_bundle_path(&root, "../outside.bin").is_err());
        assert!(
            resolve_bundle_path(
                &root,
                temporary.path().join("outside.bin").to_str().unwrap()
            )
            .is_err()
        );
    }

    #[test]
    fn verify_file_rejects_changed_content() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("artifact.bin");
        fs::write(&path, b"abc").unwrap();
        let sha256 = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        verify_file(&path, 3, sha256).unwrap();

        fs::write(&path, b"abd").unwrap();
        assert!(verify_file(&path, 3, sha256).is_err());
    }
}
