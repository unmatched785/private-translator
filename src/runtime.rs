use std::{
    collections::BTreeSet,
    env,
    fs::{self, File},
    io::{BufReader, Read},
    path::{Component, Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use reqwest::{Client, Url};
use serde::Deserialize;
use tokio::time::sleep;

use crate::config::{AppConfig, LocalEngineConfig};

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

#[derive(Debug, Deserialize)]
struct TrustedModel {
    id: String,
    file: String,
    bytes: u64,
    sha256: String,
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
    pub async fn ensure(config: &AppConfig, client: &Client) -> Result<Self> {
        let Some(engine) = &config.local_engine else {
            return Ok(Self::external());
        };
        let model = config
            .model(&engine.model_id)
            .context("자동 시작할 로컬 모델을 찾을 수 없습니다")?;
        let endpoint = Url::parse(&model.endpoint).context("로컬 모델 주소가 올바르지 않습니다")?;
        let health_url = health_url(&endpoint);
        let models_url = models_url(&endpoint);

        if probe_model_identity(client, &health_url, &models_url, &model.api_model).await? {
            println!(
                "로컬 번역 엔진: 이미 실행 중인 확인된 모델을 사용합니다 ({}).",
                model.api_model
            );
            return Ok(Self::external());
        }

        let bundle_root = bundle_root().context("신뢰할 수 있는 번역기 묶음을 찾을 수 없습니다")?;
        let executable = resolve_bundle_path(&bundle_root, &engine.executable)
            .with_context(|| format!("로컬 엔진을 찾을 수 없습니다: {}", engine.executable))?;
        let model_path = resolve_bundle_path(&bundle_root, &engine.model_path)
            .with_context(|| format!("로컬 모델을 찾을 수 없습니다: {}", engine.model_path))?;

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
        .context("로컬 번역 파일 검증 작업이 중단되었습니다")??;
        println!("로컬 번역 파일 무결성: 확인됨");

        let host = endpoint
            .host_str()
            .context("로컬 모델 주소에 호스트가 없습니다")?;
        let port = endpoint
            .port()
            .context("로컬 모델 주소에는 포트를 명시해야 합니다")?;
        let threads = engine_threads(engine);

        let mut command = Command::new(&executable);
        command.current_dir(
            executable
                .parent()
                .context("로컬 엔진 폴더를 확인할 수 없습니다")?,
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
                "로컬 번역 엔진을 시작할 수 없습니다: {}",
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
            match probe_model_identity(client, &health_url, &models_url, &model.api_model).await {
                Ok(true) => {
                    println!(
                        "로컬 번역 엔진: 준비됨 (모델 {}, {threads} threads)",
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
            if let Some(status) = child
                .try_wait()
                .context("로컬 번역 엔진 상태를 확인할 수 없습니다")?
            {
                bail!("로컬 번역 엔진이 시작 중 종료되었습니다: {status}");
            }
            sleep(Duration::from_millis(250)).await;
        }

        let _ = child.kill();
        let _ = child.wait();
        bail!("로컬 번역 엔진이 45초 안에 준비되지 않았습니다");
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
                "로컬 엔진 종료 보호를 만들 수 없습니다: {}",
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
                "로컬 엔진 종료 보호를 설정할 수 없습니다: {}",
                std::io::Error::last_os_error()
            );
        }

        let assigned =
            unsafe { AssignProcessToJobObject(job.handle, child.as_raw_handle().cast()) };
        if assigned == 0 {
            bail!(
                "로컬 엔진을 종료 보호에 연결할 수 없습니다: {}",
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
) -> Result<bool> {
    let health_response = match client
        .get(health_url.clone())
        .timeout(Duration::from_millis(700))
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) if error.is_connect() || error.is_timeout() => return Ok(false),
        Err(error) => return Err(error).context("로컬 번역 엔진 상태를 확인할 수 없습니다"),
    };

    if !health_response.status().is_success() {
        return Ok(false);
    }

    let response = client
        .get(models_url.clone())
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .context("실행 중인 로컬 엔진의 모델 정보를 읽을 수 없습니다")?;
    if !response.status().is_success() {
        bail!(
            "로컬 엔진 포트는 사용 중이지만 모델 확인 요청이 실패했습니다: HTTP {}",
            response.status()
        );
    }

    let identities: ModelsResponse = response
        .json()
        .await
        .context("실행 중인 로컬 엔진의 모델 정보 형식이 올바르지 않습니다")?;
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
    bail!("로컬 엔진 포트에서 다른 모델이 확인되었습니다 (예상: {expected_model}, 실제: {actual})")
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

fn bundle_root() -> Result<PathBuf> {
    let root = if let Some(explicit_root) = env::var_os("TRANSLATOR_BUNDLE_DIR") {
        PathBuf::from(explicit_root)
    } else {
        env::current_exe()
            .context("실행 파일 위치를 확인할 수 없습니다")?
            .parent()
            .context("실행 파일 폴더를 확인할 수 없습니다")?
            .to_path_buf()
    };
    let root = fs::canonicalize(&root)
        .with_context(|| format!("번역기 묶음 경로가 존재하지 않습니다: {}", root.display()))?;
    if !root.is_dir() {
        bail!("번역기 묶음 경로가 폴더가 아닙니다: {}", root.display());
    }
    Ok(root)
}

fn resolve_bundle_path(root: &Path, raw: &str) -> Result<PathBuf> {
    let relative = Path::new(raw);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!("번역기 묶음 안의 상대 경로만 허용됩니다");
    }

    let candidate = fs::canonicalize(root.join(relative)).context("파일이 존재하지 않습니다")?;
    if !candidate.starts_with(root) || !candidate.is_file() {
        bail!("번역기 묶음 밖의 파일은 사용할 수 없습니다");
    }
    Ok(candidate)
}

fn trusted_artifacts() -> Result<TrustedArtifacts> {
    let manifest: TrustedArtifacts =
        serde_json::from_str(include_str!("../packaging/trusted-artifacts.json"))
            .context("내장된 신뢰 파일 목록이 올바르지 않습니다")?;
    if manifest.schema_version != 1 {
        bail!(
            "지원하지 않는 신뢰 파일 목록 버전입니다: {}",
            manifest.schema_version
        );
    }
    Ok(manifest)
}

fn verify_trusted_artifacts(
    executable: &Path,
    model_path: &Path,
    engine: &LocalEngineConfig,
) -> Result<()> {
    let manifest = trusted_artifacts()?;
    if manifest.runtime.id != engine.runtime_id {
        bail!(
            "로컬 엔진 종류가 신뢰 목록과 다릅니다 (설정: {}, 신뢰 목록: {})",
            engine.runtime_id,
            manifest.runtime.id
        );
    }

    let model = manifest
        .models
        .iter()
        .find(|model| model.id == engine.model_id)
        .with_context(|| format!("신뢰 목록에 없는 모델입니다: {}", engine.model_id))?;
    let actual_model_name = model_path
        .file_name()
        .context("모델 파일 이름을 확인할 수 없습니다")?
        .to_string_lossy();
    if !actual_model_name.eq_ignore_ascii_case(&model.file) {
        bail!(
            "모델 파일 이름이 신뢰 목록과 다릅니다 (예상: {}, 실제: {})",
            model.file,
            actual_model_name
        );
    }
    verify_file(model_path, model.bytes, &model.sha256)
        .with_context(|| format!("모델 무결성 검증 실패: {}", model_path.display()))?;

    let runtime_dir = executable
        .parent()
        .context("로컬 엔진 폴더를 확인할 수 없습니다")?;
    let expected_names = manifest
        .runtime
        .files
        .iter()
        .map(|file| file.path.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let executable_name = executable
        .file_name()
        .context("로컬 엔진 파일 이름을 확인할 수 없습니다")?
        .to_string_lossy()
        .to_ascii_lowercase();
    if !expected_names.contains(&executable_name) {
        bail!("신뢰 목록에 없는 로컬 엔진 실행 파일입니다: {executable_name}");
    }

    let actual_names = fs::read_dir(runtime_dir)
        .context("로컬 엔진 폴더를 읽을 수 없습니다")?
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
            "로컬 엔진 파일 구성이 신뢰 목록과 다릅니다 (누락: [{}], 추가: [{}])",
            missing,
            extra
        );
    }

    for artifact in &manifest.runtime.files {
        let path = resolve_bundle_path(runtime_dir, &artifact.path)
            .with_context(|| format!("신뢰 런타임 파일을 찾을 수 없습니다: {}", artifact.path))?;
        verify_file(&path, artifact.bytes, &artifact.sha256)
            .with_context(|| format!("런타임 무결성 검증 실패: {}", artifact.path))?;
    }
    Ok(())
}

fn verify_file(path: &Path, expected_bytes: u64, expected_sha256: &str) -> Result<()> {
    if expected_sha256.len() != 64 || !expected_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        bail!("신뢰 목록의 SHA-256 값이 올바르지 않습니다");
    }
    let metadata = fs::metadata(path).context("파일 정보를 읽을 수 없습니다")?;
    if metadata.len() != expected_bytes {
        bail!(
            "파일 크기가 다릅니다 (예상: {expected_bytes}, 실제: {})",
            metadata.len()
        );
    }

    let actual_sha256 = sha256_file(path)?;
    if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
        bail!("SHA-256이 다릅니다 (예상: {expected_sha256}, 실제: {actual_sha256})");
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
            bail!("Windows SHA-256 {operation} 실패: NTSTATUS 0x{status:08x}");
        }
        Ok(())
    }

    let mut algorithm = ptr::null_mut();
    check_status(
        unsafe {
            BCryptOpenAlgorithmProvider(&raw mut algorithm, BCRYPT_SHA256_ALGORITHM, ptr::null(), 0)
        },
        "초기화",
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
        "해시 생성",
    )?;
    let hash = Hash(hash);

    let file = File::open(path).context("파일을 열 수 없습니다")?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .context("파일을 읽을 수 없습니다")?;
        if read == 0 {
            break;
        }
        check_status(
            unsafe { BCryptHashData(hash.0, buffer.as_ptr(), read as u32, 0) },
            "파일 처리",
        )?;
    }

    let mut output = [0_u8; 32];
    check_status(
        unsafe { BCryptFinishHash(hash.0, output.as_mut_ptr(), output.len() as u32, 0) },
        "완료",
    )?;
    Ok(output.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(not(windows))]
fn sha256_file(_path: &Path) -> Result<String> {
    bail!("신뢰 파일 SHA-256 검증은 Windows 패키지에서 지원됩니다")
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
        assert!(
            manifest
                .models
                .iter()
                .any(|model| model.id == "hy-mt2-1.8b-q4")
        );
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
