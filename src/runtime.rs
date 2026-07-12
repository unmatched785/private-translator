use std::{
    env,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use reqwest::{Client, Url};
use tokio::time::sleep;

use crate::config::{AppConfig, LocalEngineConfig};

pub struct LocalEngine {
    child: Option<Child>,
    #[cfg(windows)]
    _job: Option<KillOnCloseJob>,
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

        if is_ready(client, &health_url).await {
            println!("로컬 번역 엔진: 이미 실행 중인 엔진을 사용합니다.");
            return Ok(Self::external());
        }

        let executable = resolve_bundle_path(&engine.executable)
            .with_context(|| format!("로컬 엔진을 찾을 수 없습니다: {}", engine.executable))?;
        let model_path = resolve_bundle_path(&engine.model_path)
            .with_context(|| format!("로컬 모델을 찾을 수 없습니다: {}", engine.model_path))?;
        let host = endpoint
            .host_str()
            .context("로컬 모델 주소에 호스트가 없습니다")?;
        let port = endpoint
            .port()
            .context("로컬 모델 주소에는 포트를 명시해야 합니다")?;
        let threads = engine_threads(engine);

        let mut command = Command::new(&executable);
        command
            .arg("-m")
            .arg(&model_path)
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
            if is_ready(client, &health_url).await {
                println!("로컬 번역 엔진: 준비됨 ({threads} threads)");
                return Ok(Self {
                    child: Some(child),
                    #[cfg(windows)]
                    _job: Some(job),
                });
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

async fn is_ready(client: &Client, url: &Url) -> bool {
    client
        .get(url.clone())
        .timeout(Duration::from_millis(700))
        .send()
        .await
        .is_ok_and(|response| response.status().is_success())
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

fn resolve_bundle_path(raw: &str) -> Result<PathBuf> {
    let path = Path::new(raw);
    if path.is_absolute() && path.is_file() {
        return Ok(path.to_path_buf());
    }

    if let Ok(current_dir) = env::current_dir() {
        let candidate = current_dir.join(path);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    if let Some(executable_dir) = env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(Path::to_path_buf))
    {
        let candidate = executable_dir.join(path);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    bail!("파일이 존재하지 않습니다")
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
}
