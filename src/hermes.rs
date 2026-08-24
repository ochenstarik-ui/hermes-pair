use crate::models::HermesStatusResponse;
use std::net::Ipv4Addr;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeState {
    Online(HermesStatusResponse),
    LoopbackOnly {
        local_status: HermesStatusResponse,
        lan_error: String,
    },
    Offline(String),
}

impl ProbeState {
    pub fn is_online(&self) -> bool {
        matches!(self, ProbeState::Online(_))
    }

    pub fn is_loopback_only(&self) -> bool {
        matches!(self, ProbeState::LoopbackOnly { .. })
    }

    pub fn status_response(&self) -> Option<&HermesStatusResponse> {
        match self {
            ProbeState::Online(ref resp) => Some(resp),
            ProbeState::LoopbackOnly {
                ref local_status, ..
            } => Some(local_status),
            ProbeState::Offline(_) => None,
        }
    }
}

#[derive(Clone)]
pub struct HermesProbeClient {
    client: reqwest::Client,
}

impl Default for HermesProbeClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HermesProbeClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }

    pub async fn fetch_status(&self, base_url: &str) -> Result<HermesStatusResponse, String> {
        let url = if base_url.ends_with('/') {
            format!("{}api/status", base_url)
        } else {
            format!("{}/api/status", base_url)
        };

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Connection error: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("HTTP status {}", response.status()));
        }

        let body = response
            .json::<HermesStatusResponse>()
            .await
            .map_err(|e| format!("Failed to parse status response: {}", e))?;

        Ok(body)
    }

    pub async fn probe(&self, port: u16, lan_ip: Option<Ipv4Addr>) -> ProbeState {
        let local_url = format!("http://127.0.0.1:{}", port);
        let local_result = self.fetch_status(&local_url).await;

        match (local_result, lan_ip) {
            (Ok(local_status), Some(lan)) => {
                let lan_url = format!("http://{}:{}", lan, port);
                match self.fetch_status(&lan_url).await {
                    Ok(lan_status) => ProbeState::Online(lan_status),
                    Err(lan_err) => ProbeState::LoopbackOnly {
                        local_status,
                        lan_error: lan_err,
                    },
                }
            }
            (Ok(local_status), None) => ProbeState::Online(local_status),
            (Err(local_err), Some(lan)) => {
                let lan_url = format!("http://{}:{}", lan, port);
                match self.fetch_status(&lan_url).await {
                    Ok(lan_status) => ProbeState::Online(lan_status),
                    Err(_) => ProbeState::Offline(local_err),
                }
            }
            (Err(local_err), None) => ProbeState::Offline(local_err),
        }
    }
}

pub async fn probe_hermes_status(base_url: &str) -> Result<HermesStatusResponse, String> {
    let client = HermesProbeClient::new();
    client.fetch_status(base_url).await
}

pub async fn probe_hermes(port: u16, lan_ip: Option<Ipv4Addr>) -> ProbeState {
    let client = HermesProbeClient::new();
    client.probe(port, lan_ip).await
}
