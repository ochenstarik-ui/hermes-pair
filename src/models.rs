use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairingPayloadV1 {
    pub v: u32, // 1
    #[serde(rename = "type")]
    pub payload_type: String, // "hermes-pair"
    pub host_id: String, // UUIDv4 string
    pub name: String, // Display name / computer name
    pub host: String, // Reachable IPv4 or hostname
    pub port: u16, // Port (e.g. 9119)
    pub scheme: String, // "http" or "https"
    pub expires_at: u64, // Unix timestamp in seconds
    pub nonce: String, // Base64URL-encoded cryptographically secure random 16 bytes
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct HermesStatusResponse {
    #[serde(default)]
    pub status: String,
    #[serde(rename = "authRequired", default)]
    pub auth_required: bool,
    #[serde(rename = "authProviders", default)]
    pub auth_providers: Vec<String>,
    #[serde(rename = "authFlows", default)]
    pub auth_flows: Vec<String>,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkInterfaceInfo {
    pub name: String,
    pub ip: Ipv4Addr,
    pub is_loopback: bool,
    pub is_virtual: bool,
}
