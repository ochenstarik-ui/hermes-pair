pub use crate::models::PairingPayloadV1;
use base64::engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use rand::RngCore;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingError {
    InvalidUriScheme(String),
    InvalidUriFormat(String),
    MissingDataParameter,
    Base64DecodeError(String),
    JsonDecodeError(String),
    UnsupportedVersion(u32),
    InvalidPayloadType(String),
    InvalidHostId(String),
    EmptyHost,
    InvalidPort(u16),
    PayloadExpired { expires_at: u64, now: u64 },
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
            PairingError::EmptyHost => write!(f, "Host address cannot be empty"),
            PairingError::InvalidPort(p) => write!(f, "Invalid port number: {}", p),
            PairingError::PayloadExpired { expires_at, now } => {
                write!(
                    f,
                    "Pairing payload expired at timestamp {} (current time: {})",
                    expires_at, now
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
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
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
    let expires_at = now + ttl_seconds;
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

    let payload: PairingPayloadV1 = serde_json::from_slice(&decoded_bytes)
        .map_err(|e| PairingError::JsonDecodeError(e.to_string()))?;

    // Validations
    if payload.v != 1 {
        return Err(PairingError::UnsupportedVersion(payload.v));
    }

    if payload.payload_type != "hermes-pair" {
        return Err(PairingError::InvalidPayloadType(payload.payload_type));
    }

    if Uuid::parse_str(&payload.host_id).is_err() {
        return Err(PairingError::InvalidHostId(payload.host_id));
    }

    if payload.host.trim().is_empty() {
        return Err(PairingError::EmptyHost);
    }

    if payload.port == 0 {
        return Err(PairingError::InvalidPort(payload.port));
    }

    if payload.expires_at <= current_time {
        return Err(PairingError::PayloadExpired {
            expires_at: payload.expires_at,
            now: current_time,
        });
    }

    Ok(payload)
}

pub fn decode_pairing_uri(uri: &str) -> Result<PairingPayloadV1, PairingError> {
    let now = current_unix_timestamp();
    decode_pairing_uri_at_time(uri, now)
}
