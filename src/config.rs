use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    pub host_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            host_id: Uuid::new_v4().to_string(),
            display_name: None,
        }
    }
}

/// Resolves configuration path according to platform conventions:
/// - Windows: `%APPDATA%\HermesPair\config.json`
/// - Linux/Unix: `~/.config/hermes-pair/config.json`
pub fn get_config_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("HermesPair").join("config.json");
        }
        if let Some(config_dir) = dirs::config_dir() {
            return config_dir.join("HermesPair").join("config.json");
        }
        PathBuf::from("config.json")
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("hermes-pair").join("config.json");
        }
        if let Some(config_dir) = dirs::config_dir() {
            return config_dir.join("hermes-pair").join("config.json");
        }
        if let Some(home_dir) = dirs::home_dir() {
            return home_dir.join(".config").join("hermes-pair").join("config.json");
        }
        PathBuf::from("config.json")
    }
}

/// Saves the configuration atomically by writing to a temporary file and renaming it.
pub fn save_config_to_path(config: &AppConfig, path: &Path) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json_bytes = serde_json::to_vec_pretty(config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let tmp_file_name = format!(
        "{}.tmp.{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("config"),
        Uuid::new_v4()
    );
    let tmp_path = match path.parent() {
        Some(p) => p.join(tmp_file_name),
        None => PathBuf::from(tmp_file_name),
    };

    fs::write(&tmp_path, json_bytes)?;

    // On Windows and Unix, fs::rename replaces the destination atomically if in the same directory.
    if let Err(_err) = fs::rename(&tmp_path, path) {
        // Fallback in case rename fails due to cross-platform replacement edge cases
        let _ = fs::remove_file(path);
        if let Err(fallback_err) = fs::rename(&tmp_path, path) {
            let _ = fs::remove_file(&tmp_path);
            return Err(fallback_err.into());
        }
    }

    Ok(())
}

/// Loads an existing config or generates a new one with a persistent UUIDv4 `host_id` and saves it.
pub fn load_or_create_config_from_path(path: &Path) -> Result<AppConfig, std::io::Error> {
    if path.exists() {
        let content = fs::read_to_string(path)?;
        if let Ok(mut config) = serde_json::from_str::<AppConfig>(&content) {
            if config.host_id.trim().is_empty() {
                config.host_id = Uuid::new_v4().to_string();
                save_config_to_path(&config, path)?;
            }
            return Ok(config);
        }
    }

    let new_config = AppConfig::default();
    save_config_to_path(&new_config, path)?;
    Ok(new_config)
}

/// Convenience function to load or create configuration at the default system location.
pub fn load_or_create_config() -> Result<AppConfig, std::io::Error> {
    let path = get_config_path();
    load_or_create_config_from_path(&path)
}
