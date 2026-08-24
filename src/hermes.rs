use crate::models::HermesStatusResponse;
use crate::network::is_loopback;
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

    pub fn is_offline(&self) -> bool {
        matches!(self, ProbeState::Offline(_))
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
        let trimmed = base_url.trim().trim_end_matches('/');
        let url = if trimmed.ends_with("/api/status") {
            trimmed.to_string()
        } else {
            format!("{}/api/status", trimmed)
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

        if body.status.trim().is_empty() {
            return Err("Invalid status response: missing 'status' field".to_string());
        }

        Ok(body)
    }

    pub async fn probe(
        &self,
        hermes_url: Option<&str>,
        scheme: &str,
        port: u16,
        lan_ip: Option<Ipv4Addr>,
    ) -> ProbeState {
        if let Some(url_str) = hermes_url {
            let direct_res = self.fetch_status(url_str).await;

            if is_url_loopback(url_str) {
                match direct_res {
                    Ok(local_status) => {
                        if let Some(lan) = lan_ip {
                            if !is_loopback(&lan) {
                                let lan_url = format!("{}://{}:{}", scheme, lan, port);
                                match self.fetch_status(&lan_url).await {
                                    Ok(lan_status) => ProbeState::Online(lan_status),
                                    Err(lan_err) => ProbeState::LoopbackOnly {
                                        local_status,
                                        lan_error: lan_err,
                                    },
                                }
                            } else {
                                ProbeState::Online(local_status)
                            }
                        } else {
                            ProbeState::Online(local_status)
                        }
                    }
                    Err(err) => {
                        if let Some(lan) = lan_ip {
                            if !is_loopback(&lan) {
                                let lan_url = format!("{}://{}:{}", scheme, lan, port);
                                match self.fetch_status(&lan_url).await {
                                    Ok(lan_status) => ProbeState::Online(lan_status),
                                    Err(_) => ProbeState::Offline(err),
                                }
                            } else {
                                ProbeState::Offline(err)
                            }
                        } else {
                            ProbeState::Offline(err)
                        }
                    }
                }
            } else {
                match direct_res {
                    Ok(status) => ProbeState::Online(status),
                    Err(err) => ProbeState::Offline(err),
                }
            }
        } else {
            let local_url = format!("{}://127.0.0.1:{}", scheme, port);
            let local_result = self.fetch_status(&local_url).await;

            match (local_result, lan_ip) {
                (Ok(local_status), Some(lan)) if !is_loopback(&lan) => {
                    let lan_url = format!("{}://{}:{}", scheme, lan, port);
                    match self.fetch_status(&lan_url).await {
                        Ok(lan_status) => ProbeState::Online(lan_status),
                        Err(lan_err) => ProbeState::LoopbackOnly {
                            local_status,
                            lan_error: lan_err,
                        },
                    }
                }
                (Ok(local_status), _) => ProbeState::Online(local_status),
                (Err(local_err), Some(lan)) if !is_loopback(&lan) => {
                    let lan_url = format!("{}://{}:{}", scheme, lan, port);
                    match self.fetch_status(&lan_url).await {
                        Ok(lan_status) => ProbeState::Online(lan_status),
                        Err(_) => ProbeState::Offline(local_err),
                    }
                }
                (Err(local_err), _) => ProbeState::Offline(local_err),
            }
        }
    }
}

fn is_url_loopback(url_or_host: &str) -> bool {
    let host = if let Ok(parsed) = url::Url::parse(url_or_host) {
        parsed.host_str().unwrap_or("").to_string()
    } else {
        url_or_host.to_string()
    };
    let host_lower = host.trim().to_lowercase();
    host_lower == "127.0.0.1"
        || host_lower == "localhost"
        || host_lower == "::1"
        || host_lower == "[::1]"
        || host_lower.starts_with("127.")
}

pub async fn probe_hermes_status(base_url: &str) -> Result<HermesStatusResponse, String> {
    let client = HermesProbeClient::new();
    client.fetch_status(base_url).await
}

pub async fn probe_hermes(
    hermes_url: Option<&str>,
    scheme: &str,
    port: u16,
    lan_ip: Option<Ipv4Addr>,
) -> ProbeState {
    let client = HermesProbeClient::new();
    client.probe(hermes_url, scheme, port, lan_ip).await
}
