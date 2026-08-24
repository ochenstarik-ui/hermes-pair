use crate::config::AppConfig;

/// Returns the persistent host_id from configuration.
pub fn get_host_id(config: &AppConfig) -> String {
    config.host_id.clone()
}

/// Detects the system hostname from environment variables or sensible fallbacks.
pub fn detect_system_hostname() -> String {
    if let Ok(name) = std::env::var("COMPUTERNAME") {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    if let Ok(name) = std::env::var("HOSTNAME") {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    if let Ok(name) = std::env::var("HOST") {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    // Try reading /etc/hostname on Unix
    #[cfg(unix)]
    {
        if let Ok(name) = std::fs::read_to_string("/etc/hostname") {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    "Hermes-Host".to_string()
}

/// Returns the configured display name, or the detected system hostname if not set.
pub fn get_display_name(config: &AppConfig) -> String {
    if let Some(ref name) = config.display_name {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    detect_system_hostname()
}
