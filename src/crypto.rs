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
        fs::create_dir_all(data_dir).context("Could not create the local vault directory")?;
        let key = load_or_create_key(data_dir)?;
        Self::from_key(&key)
    }

    pub fn from_key(key: &[u8]) -> Result<Self> {
        if key.len() != KEY_BYTES {
            bail!("The encryption key length is invalid");
        }
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|_| anyhow::anyhow!("Could not initialize the encryption key"))?;
        Ok(Self { cipher })
    }

    pub fn encrypt(&self, plaintext: &[u8], aad: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        let mut nonce = [0_u8; NONCE_BYTES];
        getrandom::fill(&mut nonce)
            .map_err(|error| anyhow::anyhow!("Could not generate secure random data: {error}"))?;
        let ciphertext = self
            .cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| anyhow::anyhow!("Could not encrypt the vault record"))?;
        Ok((nonce.to_vec(), ciphertext))
    }

    pub fn decrypt(&self, nonce: &[u8], ciphertext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
        if nonce.len() != NONCE_BYTES {
            bail!("The encryption nonce is invalid");
        }
        self.cipher
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad,
                },
            )
            .map_err(|_| anyhow::anyhow!("Could not decrypt the vault record"))
    }
}

#[cfg(windows)]
fn random_key() -> Result<[u8; KEY_BYTES]> {
    let mut key = [0_u8; KEY_BYTES];
    getrandom::fill(&mut key)
        .map_err(|error| anyhow::anyhow!("Could not generate a secure encryption key: {error}"))?;
    Ok(key)
}

#[cfg(windows)]
fn load_or_create_key(data_dir: &Path) -> Result<[u8; KEY_BYTES]> {
    let path = data_dir.join("vault.key");
    if path.exists() {
        let wrapped = fs::read(&path).context("Could not read the protected vault key")?;
        let plain = dpapi::unprotect(&wrapped)?;
        return plain
            .try_into()
            .map_err(|_| anyhow::anyhow!("The protected vault key is corrupted"));
    }

    let key = random_key()?;
    let wrapped = dpapi::protect(&key)?;
    fs::write(&path, wrapped).context("Could not save the protected vault key")?;
    Ok(key)
}

#[cfg(not(windows))]
fn load_or_create_key(data_dir: &Path) -> Result<[u8; KEY_BYTES]> {
    let passphrase = env::var("TRANSLATOR_VAULT_PASSPHRASE")
        .context("TRANSLATOR_VAULT_PASSPHRASE is required outside Windows")?;
    let salt_path = data_dir.join("vault.salt");
    let salt = if salt_path.exists() {
        fs::read(&salt_path).context("Could not read the vault encryption salt")?
    } else {
        let mut salt = [0_u8; 16];
        getrandom::fill(&mut salt).map_err(|error| {
            anyhow::anyhow!("Could not generate the vault encryption salt: {error}")
        })?;
        fs::write(&salt_path, salt).context("Could not save the vault encryption salt")?;
        salt.to_vec()
    };
    let mut key = [0_u8; KEY_BYTES];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), &salt, &mut key)
        .map_err(|_| anyhow::anyhow!("Could not derive the vault encryption key"))?;
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
            .context("The encryption key data is too large")?;
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
            bail!("Could not protect the vault key with the current Windows account");
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
