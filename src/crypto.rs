use std::{fs, path::Path};

#[cfg(not(windows))]
use std::env;

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use anyhow::{Context, Result, bail};
#[cfg(not(windows))]
use argon2::Argon2;

const KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 12;

#[derive(Clone)]
pub struct VaultCrypto {
    cipher: Aes256Gcm,
}

impl VaultCrypto {
    pub fn load_or_create(data_dir: &Path) -> Result<Self> {
        fs::create_dir_all(data_dir).context("로컬 기록 폴더를 만들 수 없습니다")?;
        let key = load_or_create_key(data_dir)?;
        Self::from_key(&key)
    }

    pub fn from_key(key: &[u8]) -> Result<Self> {
        if key.len() != KEY_BYTES {
            bail!("암호키 길이가 올바르지 않습니다");
        }
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|_| anyhow::anyhow!("암호키를 초기화할 수 없습니다"))?;
        Ok(Self { cipher })
    }

    pub fn encrypt(&self, plaintext: &[u8], aad: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        let mut nonce = [0_u8; NONCE_BYTES];
        getrandom::fill(&mut nonce)
            .map_err(|error| anyhow::anyhow!("보안 난수를 만들 수 없습니다: {error}"))?;
        let ciphertext = self
            .cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| anyhow::anyhow!("기록을 암호화할 수 없습니다"))?;
        Ok((nonce.to_vec(), ciphertext))
    }

    pub fn decrypt(&self, nonce: &[u8], ciphertext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
        if nonce.len() != NONCE_BYTES {
            bail!("암호화 nonce가 올바르지 않습니다");
        }
        self.cipher
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad,
                },
            )
            .map_err(|_| anyhow::anyhow!("기록을 복호화할 수 없습니다"))
    }
}

#[cfg(windows)]
fn random_key() -> Result<[u8; KEY_BYTES]> {
    let mut key = [0_u8; KEY_BYTES];
    getrandom::fill(&mut key)
        .map_err(|error| anyhow::anyhow!("보안 암호키를 만들 수 없습니다: {error}"))?;
    Ok(key)
}

#[cfg(windows)]
fn load_or_create_key(data_dir: &Path) -> Result<[u8; KEY_BYTES]> {
    let path = data_dir.join("vault.key");
    if path.exists() {
        let wrapped = fs::read(&path).context("보호된 기록 암호키를 읽을 수 없습니다")?;
        let plain = dpapi::unprotect(&wrapped)?;
        return plain
            .try_into()
            .map_err(|_| anyhow::anyhow!("보호된 기록 암호키가 손상되었습니다"));
    }

    let key = random_key()?;
    let wrapped = dpapi::protect(&key)?;
    fs::write(&path, wrapped).context("보호된 기록 암호키를 저장할 수 없습니다")?;
    Ok(key)
}

#[cfg(not(windows))]
fn load_or_create_key(data_dir: &Path) -> Result<[u8; KEY_BYTES]> {
    let passphrase = env::var("TRANSLATOR_VAULT_PASSPHRASE")
        .context("Windows 외 환경에서는 TRANSLATOR_VAULT_PASSPHRASE가 반드시 필요합니다")?;
    let salt_path = data_dir.join("vault.salt");
    let salt = if salt_path.exists() {
        fs::read(&salt_path).context("기록 암호화 salt를 읽을 수 없습니다")?
    } else {
        let mut salt = [0_u8; 16];
        getrandom::fill(&mut salt)
            .map_err(|error| anyhow::anyhow!("기록 암호화 salt를 만들 수 없습니다: {error}"))?;
        fs::write(&salt_path, salt).context("기록 암호화 salt를 저장할 수 없습니다")?;
        salt.to_vec()
    };
    let mut key = [0_u8; KEY_BYTES];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), &salt, &mut key)
        .map_err(|_| anyhow::anyhow!("기록 암호키를 파생할 수 없습니다"))?;
    Ok(key)
}

#[cfg(windows)]
mod dpapi {
    use std::{ffi::c_void, ptr, slice};

    use anyhow::{Context, Result, bail};

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

    #[repr(C)]
    struct DataBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    #[link(name = "Crypt32")]
    unsafe extern "system" {
        fn CryptProtectData(
            data_in: *const DataBlob,
            description: *const u16,
            optional_entropy: *const DataBlob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;

        fn CryptUnprotectData(
            data_in: *const DataBlob,
            description: *mut *mut u16,
            optional_entropy: *const DataBlob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }

    pub fn protect(input: &[u8]) -> Result<Vec<u8>> {
        transform(input, true)
    }

    pub fn unprotect(input: &[u8]) -> Result<Vec<u8>> {
        transform(input, false)
    }

    fn transform(input: &[u8], protect: bool) -> Result<Vec<u8>> {
        let input_len: u32 = input
            .len()
            .try_into()
            .context("암호키 데이터가 너무 큽니다")?;
        let input_blob = DataBlob {
            cb_data: input_len,
            pb_data: input.as_ptr() as *mut u8,
        };
        let mut output_blob = DataBlob {
            cb_data: 0,
            pb_data: ptr::null_mut(),
        };

        let ok = unsafe {
            if protect {
                CryptProtectData(
                    &input_blob,
                    ptr::null(),
                    ptr::null(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output_blob,
                )
            } else {
                CryptUnprotectData(
                    &input_blob,
                    ptr::null_mut(),
                    ptr::null(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output_blob,
                )
            }
        };

        if ok == 0 {
            bail!("Windows 사용자 계정으로 기록 암호키를 보호할 수 없습니다");
        }

        let output = unsafe {
            let bytes = slice::from_raw_parts(output_blob.pb_data, output_blob.cb_data as usize);
            let copied = bytes.to_vec();
            LocalFree(output_blob.pb_data.cast());
            copied
        };
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_payload_round_trips_and_checks_aad() {
        let crypto = VaultCrypto::from_key(&[7_u8; KEY_BYTES]).unwrap();
        let (nonce, ciphertext) = crypto.encrypt(b"private text", b"row-1").unwrap();
        assert_ne!(ciphertext, b"private text");
        assert_eq!(
            crypto.decrypt(&nonce, &ciphertext, b"row-1").unwrap(),
            b"private text"
        );
        assert!(crypto.decrypt(&nonce, &ciphertext, b"row-2").is_err());
    }
}
