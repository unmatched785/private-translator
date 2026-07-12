use std::{env, path::PathBuf};

use anyhow::{Context, Result};

pub fn data_dir() -> Result<PathBuf> {
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

pub fn model_dir() -> Result<PathBuf> {
    if let Some(path) = env::var_os("TRANSLATOR_MODEL_DIR") {
        return Ok(PathBuf::from(path));
    }
    Ok(data_dir()?.join("models"))
}
