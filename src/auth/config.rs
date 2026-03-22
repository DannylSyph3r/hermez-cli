use crate::error::HermezError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const API_URL: &str = "https://staging.api.hermez.one";
pub const TUNNEL_URL: &str = "wss://staging.hermez.online";

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub api_key: String,
    /// Display-only: cached from the server at login time, the server always validates from the token itself.
    pub user: UserInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub tier: String,
}

pub fn config_dir() -> Result<PathBuf, HermezError> {
    let home = dirs::home_dir()
        .ok_or_else(|| HermezError::Config("Could not determine home directory".to_string()))?;
    Ok(home.join(".hermez"))
}

pub fn config_path() -> Result<PathBuf, HermezError> {
    Ok(config_dir()?.join("config.json"))
}

pub fn load_config() -> Result<Option<Config>, HermezError> {
    let path = config_path()?;

    if !path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(&path)?;
    let config = serde_json::from_str(&contents)
        .map_err(|e| HermezError::Config(format!("Failed to parse config: {}", e)))?;

    Ok(Some(config))
}

pub fn save_config(config: &Config) -> Result<(), HermezError> {
    let dir = config_dir()?;
    let path = config_path()?;

    fs::create_dir_all(&dir)?;

    let contents = serde_json::to_string_pretty(config)
        .map_err(|e| HermezError::Config(format!("Failed to serialize config: {}", e)))?;

    fs::write(&path, contents)?;

    // Restrict file to owner read/write only on Unix (Linux + macOS)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

pub fn delete_config() -> Result<(), HermezError> {
    let path = config_path()?;

    if path.exists() {
        fs::remove_file(&path)?;
    }

    Ok(())
}
