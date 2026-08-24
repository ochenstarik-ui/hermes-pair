pub use crate::models::PairingPayloadV1;
use base64::engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use rand::RngCore;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};
use url::Url;
use uuid::Uuid;

pub const MIN_TTL_SECONDS: u64 = 10;
pub const MAX_TTL_SECONDS: u64 = 600;
pub const DEFAULT_TTL_SECONDS: u64 = 120;
pub const MAX_CLOCK_SKEW_SECONDS: u64 = 30;
pub const MAX_ENCODED_URI_BYTES: usize = 4096;
pub const MAX_DECODED_JSON_BYTES: usize = 2048;
pub const MAX_NAME_LENGTH: usize = 128;
pub const MIN_NONCE_BYTES: usize = 16;
pub const MAX_NONCE_BYTES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingError {
    InvalidUriScheme(String),
    InvalidUriFormat(String),
    MissingDataParameter,
    PayloadTooLarge { size: usize, max: usize },
    Base64DecodeError(String),
    JsonDecodeError(String),
    UnsupportedVersion(u32),
    InvalidPayloadType(String),
    InvalidHostId(String),
    InvalidName(String),
    EmptyHost,
    InvalidHost(String),
    InvalidPort(u16),
    InvalidScheme(String),
    InvalidNonce(String),
    PayloadExpired { expires_at: u64, now: u64 },
    TtlExceedsMaximum { expires_at: u64, max_allowed: u64 },
    InvalidTtl { ttl: u64, min: u64, max: u64 },
}

impl fmt::Display for PairingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PairingError::InvalidUriScheme(s) => {
                write!(f, "Invalid URI scheme '{}', expected 'hermes'", s)
            }
            PairingError::InvalidUriFormat(s) => write!(f, "Invalid pairing URI format: {}", s),
            PairingError::MissingDataParameter => {
                write!(f, "Missing 'data' query parameter in pairing URI")
            }
            PairingError::PayloadTooLarge { size, max } => {
                write!(
                    f,
                    "Payload size ({} bytes) exceeds maximum limit of {} bytes",
                    size, max
                )
            }
            PairingError::Base64DecodeError(e) => {
                write!(f, "Failed to decode Base64URL payload: {}", e)
            }
            PairingError::JsonDecodeError(e) => write!(f, "Failed to parse JSON payload: {}", e),
            PairingError::UnsupportedVersion(v) => {
                write!(f, "Unsupported payload version {}, expected 1", v)
            }
            PairingError::InvalidPayloadType(t) => {
                write!(f, "Invalid payload type '{}', expected 'hermes-pair'", t)
            }
            PairingError::InvalidHostId(id) => write!(f, "Invalid host UUID: '{}'", id),
            PairingError::InvalidName(msg) => write!(f, "Invalid host display name: {}", msg),
            PairingError::EmptyHost => write!(f, "Host address cannot be empty"),
            PairingError::InvalidHost(msg) => write!(f, "Invalid host address: {}", msg),
            PairingError::InvalidPort(p) => write!(f, "Invalid port number: {}", p),
            PairingError::InvalidScheme(s) => {
                write!(f, "Invalid scheme '{}', expected 'http' or 'https'", s)
            }
            PairingError::InvalidNonce(msg) => write!(f, "Invalid nonce: {}", msg),
            PairingError::PayloadExpired { expires_at, now } => {
                write!(
                    f,
                    "Pairing payload expired at timestamp {} (current time: {})",
                    expires_at, now
                )
            }
            PairingError::TtlExceedsMaximum {
                expires_at,
                max_allowed,
            } => {
                write!(
                    f,
                    "Pairing payload expiry timestamp {} exceeds maximum allowed {}",
                    expires_at, max_allowed
                )
            }
            PairingError::InvalidTtl { ttl, min, max } => {
                write!(
                    f,
                    "Invalid TTL {}s: TTL must be between {} and {} seconds",
                    ttl, min, max
                )
            }
        }
    }
}

impl std::error::Error for PairingError {}

pub fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn generate_nonce() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn validate_ttl(ttl: u64) -> Result<(), PairingError> {
    if !(MIN_TTL_SECONDS..=MAX_TTL_SECONDS).contains(&ttl) {
        return Err(PairingError::InvalidTtl {
            ttl,
            min: MIN_TTL_SECONDS,
            max: MAX_TTL_SECONDS,
        });
    }
    Ok(())
}

pub fn validate_payload(payload: &PairingPayloadV1, current_time: u64) -> Result<(), PairingError> {
    // 1. Version must be 1
    if payload.v != 1 {
        return Err(PairingError::UnsupportedVersion(payload.v));
    }

    // 2. Payload type must be "hermes-pair"
    if payload.payload_type != "hermes-pair" {
        return Err(PairingError::InvalidPayloadType(
            payload.payload_type.clone(),
        ));
    }

    // 3. Host ID must be a valid UUIDv4 string
    let host_uuid = Uuid::parse_str(&payload.host_id).map_err(|_| {
        PairingError::InvalidHostId(format!("'{}' is not a valid UUID", payload.host_id))
    })?;
    if host_uuid.get_version_num() != 4 {
        return Err(PairingError::InvalidHostId(format!(
            "UUID must be version 4 (random), got version {}",
            host_uuid.get_version_num()
        )));
    }

    // 4. Name: not blank, trimmed <= 128 chars, no control characters
    let trimmed_name = payload.name.trim();
    if trimmed_name.is_empty() {
        return Err(PairingError::InvalidName(
            "Host display name cannot be blank".into(),
        ));
    }
    if trimmed_name.chars().count() > MAX_NAME_LENGTH {
        return Err(PairingError::InvalidName(format!(
            "Host display name length ({}) exceeds maximum allowed {}",
            trimmed_name.chars().count(),
            MAX_NAME_LENGTH
        )));
    }
    if payload
        .name
        .chars()
        .any(|c| (c as u32) < 0x20 || (c as u32) == 0x7F)
    {
        return Err(PairingError::InvalidName(
            "Host display name contains forbidden control characters".into(),
        ));
    }

    // 5. Host: not blank, no whitespace, no forbidden chars: / \ ? # @ : control chars
    let trimmed_host = payload.host.trim();
    if trimmed_host.is_empty() {
        return Err(PairingError::EmptyHost);
    }
    if payload.host.chars().any(|c| {
        c.is_whitespace()
            || ['/', '\\', '?', '#', '@', ':'].contains(&c)
            || (c as u32) < 0x20
            || (c as u32) == 0x7F
    }) {
        return Err(PairingError::InvalidHost(format!(
            "Host '{}' contains forbidden characters (whitespace, delimiters, or control characters)",
            payload.host
        )));
    }

    // 6. Port: 1..=65535 (u16 is <= 65535, port 0 is invalid)
    if payload.port == 0 {
        return Err(PairingError::InvalidPort(0));
    }

    // 7. Scheme: "http" or "https"
    if payload.scheme != "http" && payload.scheme != "https" {
        return Err(PairingError::InvalidScheme(format!(
            "Invalid scheme '{}', must be 'http' or 'https'",
            payload.scheme
        )));
    }

    // 8. Nonce: Base64URL-encoded, decodes to >= 16 bytes and <= 64 bytes
    let trimmed_nonce = payload.nonce.trim();
    if trimmed_nonce.is_empty() {
        return Err(PairingError::InvalidNonce("Nonce cannot be empty".into()));
    }
    let decoded_nonce = URL_SAFE_NO_PAD
        .decode(trimmed_nonce.as_bytes())
        .or_else(|_| URL_SAFE.decode(trimmed_nonce.as_bytes()))
        .or_else(|_| STANDARD.decode(trimmed_nonce.as_bytes()))
        .map_err(|e| PairingError::InvalidNonce(format!("Nonce Base64 decode failed: {}", e)))?;

    if decoded_nonce.len() < MIN_NONCE_BYTES {
        return Err(PairingError::InvalidNonce(format!(
            "Nonce length {} bytes is below minimum {} bytes (128 bits)",
            decoded_nonce.len(),
            MIN_NONCE_BYTES
        )));
    }
    if decoded_nonce.len() > MAX_NONCE_BYTES {
        return Err(PairingError::InvalidNonce(format!(
            "Nonce length {} bytes exceeds maximum {} bytes",
            decoded_nonce.len(),
            MAX_NONCE_BYTES
        )));
    }

    // 9. Expires at: now - 30 <= expires_at <= now + 600
    let min_allowed_expiry = current_time.saturating_sub(MAX_CLOCK_SKEW_SECONDS);
    if payload.expires_at < min_allowed_expiry {
        return Err(PairingError::PayloadExpired {
            expires_at: payload.expires_at,
            now: current_time,
        });
    }
    let max_allowed_expiry = current_time + MAX_TTL_SECONDS;
    if payload.expires_at > max_allowed_expiry {
        return Err(PairingError::TtlExceedsMaximum {
            expires_at: payload.expires_at,
            max_allowed: max_allowed_expiry,
        });
    }

    Ok(())
}

pub fn create_pairing_payload(
    host_id: String,
    name: String,
    host: String,
    port: u16,
    scheme: String,
    ttl_seconds: u64,
) -> PairingPayloadV1 {
    let now = current_unix_timestamp();
    let ttl = ttl_seconds.clamp(MIN_TTL_SECONDS, MAX_TTL_SECONDS);
    let expires_at = now + ttl;
    let nonce = generate_nonce();

    PairingPayloadV1 {
        v: 1,
        payload_type: "hermes-pair".to_string(),
        host_id,
        name,
        host,
        port,
        scheme,
        expires_at,
        nonce,
    }
}

pub fn encode_pairing_uri(payload: &PairingPayloadV1) -> String {
    let json = serde_json::to_string(payload)
        .expect("Serialization of PairingPayloadV1 should never fail");
    let encoded = URL_SAFE_NO_PAD.encode(json.as_bytes());
    format!("hermes://pair?data={}", encoded)
}

pub fn decode_pairing_uri_at_time(
    uri: &str,
    current_time: u64,
) -> Result<PairingPayloadV1, PairingError> {
    // Check encoded URI length bound
    if uri.len() > MAX_ENCODED_URI_BYTES {
        return Err(PairingError::PayloadTooLarge {
            size: uri.len(),
            max: MAX_ENCODED_URI_BYTES,
        });
    }

    let data_str = if let Ok(url) = Url::parse(uri) {
        if url.scheme() != "hermes" {
            return Err(PairingError::InvalidUriScheme(url.scheme().to_string()));
        }
        let host = url.host_str().unwrap_or_default();
        if host != "pair" && url.path() != "pair" && url.path() != "/pair" {
            return Err(PairingError::InvalidUriFormat(uri.to_string()));
        }

        url.query_pairs()
            .find(|(k, _)| k == "data")
            .map(|(_, v)| v.into_owned())
            .ok_or(PairingError::MissingDataParameter)?
    } else {
        // Fallback simple parsing for custom hermes:// URIs
        if !uri.starts_with("hermes://") && !uri.starts_with("hermes:") {
            return Err(PairingError::InvalidUriScheme("unknown".to_string()));
        }
        let query_part = uri
            .split_once('?')
            .map(|x| x.1)
            .ok_or(PairingError::MissingDataParameter)?;
        let mut found = None;
        for pair in query_part.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                if k == "data" {
                    found = Some(v.to_string());
                    break;
                }
            }
        }
        found.ok_or(PairingError::MissingDataParameter)?
    };

    // Decode Base64 (supporting URL_SAFE_NO_PAD, URL_SAFE, and STANDARD)
    let decoded_bytes = URL_SAFE_NO_PAD
        .decode(data_str.as_bytes())
        .or_else(|_| URL_SAFE.decode(data_str.as_bytes()))
        .or_else(|_| STANDARD.decode(data_str.as_bytes()))
        .map_err(|e| PairingError::Base64DecodeError(e.to_string()))?;

    // Check decoded payload size limit
    if decoded_bytes.len() > MAX_DECODED_JSON_BYTES {
        return Err(PairingError::PayloadTooLarge {
            size: decoded_bytes.len(),
            max: MAX_DECODED_JSON_BYTES,
        });
    }

    let payload: PairingPayloadV1 = serde_json::from_slice(&decoded_bytes)
        .map_err(|e| PairingError::JsonDecodeError(e.to_string()))?;

    validate_payload(&payload, current_time)?;

    Ok(payload)
}

pub fn decode_pairing_uri(uri: &str) -> Result<PairingPayloadV1, PairingError> {
    let now = current_unix_timestamp();
    decode_pairing_uri_at_time(uri, now)
}
