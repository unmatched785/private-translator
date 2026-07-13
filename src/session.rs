use std::{fmt::Write as _, fs, path::Path};

#[cfg(windows)]
use std::{
    fs::{File, OpenOptions},
    io::Write as _,
    os::windows::fs::OpenOptionsExt,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[cfg(windows)]
use crate::crypto::dpapi;

const TOKEN_BYTES: usize = 32;
#[cfg(windows)]
const SECONDARY_WAIT: Duration = Duration::from_secs(60);
#[cfg(windows)]
const SECONDARY_RETRY: Duration = Duration::from_millis(100);

pub enum SessionRole {
    Primary(SessionGuard),
    Secondary(SessionRecord),
}

pub struct SessionGuard {
    #[cfg(windows)]
    record_path: PathBuf,
    token: String,
    #[cfg(windows)]
    _lock: File,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    url: String,
    token: String,
}

impl SessionGuard {
    pub fn acquire(data_dir: &Path) -> Result<SessionRole> {
        fs::create_dir_all(data_dir).context("Could not create the local data directory")?;
        #[cfg(windows)]
        {
            let record_path = data_dir.join("active-session.dat");
            let lock_path = data_dir.join("instance.lock");
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .share_mode(0)
                .open(&lock_path);

            match lock {
                Ok(lock) => {
                    remove_stale_record(&record_path)?;
                    Ok(SessionRole::Primary(Self {
                        record_path,
                        token: generate_token()?,
                        _lock: lock,
                    }))
                }
                Err(error) if error.raw_os_error() == Some(32) => {
                    Ok(SessionRole::Secondary(wait_for_record(&record_path)?))
                }
                Err(error) => Err(error).context("Could not acquire the application instance lock"),
            }
        }

        #[cfg(not(windows))]
        Ok(SessionRole::Primary(Self {
            token: generate_token()?,
        }))
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn publish(&self, url: &str) -> Result<SessionRecord> {
        let record = SessionRecord::new(url, &self.token)?;

        #[cfg(windows)]
        write_record(&self.record_path, &record)?;

        Ok(record)
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            // Drop runs while `_lock` is still open, so a replacement primary
            // cannot publish a new record before this cleanup finishes.
            let _ = fs::remove_file(&self.record_path);
        }
    }
}

impl SessionRecord {
    fn new(url: &str, token: &str) -> Result<Self> {
        validate_url(url)?;
        validate_token(token)?;
        Ok(Self {
            url: url.to_owned(),
            token: token.to_owned(),
        })
    }

    pub fn browser_url(&self) -> String {
        format!("{}/#token={}", self.url.trim_end_matches('/'), self.token)
    }
}

fn generate_token() -> Result<String> {
    let mut bytes = [0_u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes)
        .map_err(|error| anyhow::anyhow!("Could not generate the local session token: {error}"))?;
    let mut token = String::with_capacity(TOKEN_BYTES * 2);
    for byte in bytes {
        write!(&mut token, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(token)
}

fn validate_token(token: &str) -> Result<()> {
    if token.len() != TOKEN_BYTES * 2 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("The local session token is invalid");
    }
    Ok(())
}

fn validate_url(url: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(url).context("The local session URL is invalid")?;
    let local_host = matches!(parsed.host_str(), Some("127.0.0.1" | "localhost" | "::1"));
    if parsed.scheme() != "http"
        || !local_host
        || parsed.port().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.path() != "/"
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        bail!("The local session URL is not a permitted loopback address");
    }
    Ok(())
}

#[cfg(windows)]
fn remove_stale_record(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("Could not remove the stale local session record"),
    }
}

#[cfg(windows)]
fn write_record(path: &Path, record: &SessionRecord) -> Result<()> {
    let serialized = serde_json::to_vec(record).context("Could not encode the local session")?;
    let protected = dpapi::protect(&serialized)?;
    let temporary = path.with_extension(format!(
        "tmp-{}-{}",
        std::process::id(),
        &record.token[..16]
    ));

    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .context("Could not create the protected local session record")?;
        file.write_all(&protected)
            .context("Could not save the protected local session record")?;
        file.sync_all()
            .context("Could not flush the protected local session record")?;
        drop(file);
        fs::rename(&temporary, path)
            .context("Could not publish the protected local session record")?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(windows)]
fn wait_for_record(path: &Path) -> Result<SessionRecord> {
    let started = Instant::now();
    loop {
        match fs::read(path) {
            Ok(protected) => {
                let serialized = dpapi::unprotect(&protected)
                    .context("Could not open the active local session")?;
                let record: SessionRecord = serde_json::from_slice(&serialized)
                    .context("The active local session record is corrupted")?;
                return SessionRecord::new(&record.url, &record.token);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if started.elapsed() >= SECONDARY_WAIT {
                    bail!(
                        "Private Translator is already starting, but its browser session was not ready within 60 seconds"
                    );
                }
                thread::sleep(SECONDARY_RETRY);
            }
            Err(error) => {
                return Err(error).context("Could not read the active local session record");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_have_256_bits_encoded_as_hex() {
        let first = generate_token().unwrap();
        let second = generate_token().unwrap();
        assert_eq!(first.len(), 64);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(first, second);
    }

    #[test]
    fn session_record_accepts_only_loopback_urls_and_strong_tokens() {
        let token = "a".repeat(64);
        let record = SessionRecord::new("http://127.0.0.1:8173", &token).unwrap();
        assert_eq!(
            record.browser_url(),
            format!("http://127.0.0.1:8173/#token={token}")
        );
        assert!(SessionRecord::new("https://127.0.0.1:8173", &token).is_err());
        assert!(SessionRecord::new("http://example.com:8173", &token).is_err());
        assert!(SessionRecord::new("http://127.0.0.1:8173", "short").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn second_instance_reads_the_protected_primary_session() {
        let temporary = tempfile::tempdir().unwrap();
        let SessionRole::Primary(primary) = SessionGuard::acquire(temporary.path()).unwrap() else {
            panic!("first instance was not primary");
        };
        let published = primary.publish("http://127.0.0.1:8173").unwrap();

        let SessionRole::Secondary(secondary) = SessionGuard::acquire(temporary.path()).unwrap()
        else {
            panic!("second instance was not secondary");
        };
        assert_eq!(secondary.browser_url(), published.browser_url());
        assert_ne!(
            fs::read(temporary.path().join("active-session.dat")).unwrap(),
            serde_json::to_vec(&published).unwrap()
        );

        drop(primary);
        assert!(!temporary.path().join("active-session.dat").exists());
    }
}
