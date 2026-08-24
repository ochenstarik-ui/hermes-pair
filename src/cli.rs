use crate::config::AppConfig;
use crate::hermes::{HermesProbeClient, ProbeState};
use crate::identity::{get_display_name, get_host_id};
use crate::models::NetworkInterfaceInfo;
use crate::network::discover_network_interfaces;
use crate::pairing::{create_pairing_payload, encode_pairing_uri};
use crate::qr::render_terminal_qr;
use clap::{Args, Parser, Subcommand};
use std::net::Ipv4Addr;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "hermes-pair",
    version = "0.1.0",
    author = "ochenstarik-ui",
    about = "Fast, secure QR onboarding helper for Hermes Agent"
)]
pub struct CliArgs {
    /// Run interactive terminal mode with periodic refresh
    #[arg(long, short = 't')]
    pub terminal: bool,

    /// Print QR once to stdout and exit (headless/script mode)
    #[arg(long = "no-gui")]
    pub no_gui: bool,

    /// Port of Hermes Agent (default 9119)
    #[arg(long, default_value = "9119")]
    pub port: u16,

    /// Hermes status API URL (e.g. http://127.0.0.1:9119)
    #[arg(long = "hermes-url")]
    pub hermes_url: Option<String>,

    /// Specific network interface name or IPv4 address to advertise
    #[arg(long, short = 'i')]
    pub interface: Option<String>,

    /// Pairing QR validity TTL in seconds (default 120)
    #[arg(long, default_value = "120")]
    pub ttl: u64,

    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum CliCommand {
    /// Output QR code once to stdout and exit
    Qr(QrArgs),
}

#[derive(Args, Debug, Clone)]
pub struct QrArgs {
    /// Port of Hermes Agent
    #[arg(long)]
    pub port: Option<u16>,

    /// Specific network interface name or IPv4 address
    #[arg(long, short = 'i')]
    pub interface: Option<String>,

    /// Pairing QR validity TTL in seconds
    #[arg(long)]
    pub ttl: Option<u64>,
}

/// Resolves the selected IPv4 address based on user input or automatic interface detection.
pub fn resolve_selected_ip(
    explicit_interface: Option<&str>,
    interfaces: &[NetworkInterfaceInfo],
) -> (String, Ipv4Addr) {
    if let Some(target) = explicit_interface {
        if let Ok(ip) = Ipv4Addr::from_str(target) {
            return (format!("Manual ({})", ip), ip);
        }

        let lower = target.to_lowercase();
        if let Some(matched) = interfaces.iter().find(|i| {
            i.name.to_lowercase().contains(&lower) || i.ip.to_string() == target
        }) {
            return (matched.name.clone(), matched.ip);
        }
    }

    if let Some(first) = interfaces.first() {
        return (first.name.clone(), first.ip);
    }

    ("Loopback".to_string(), Ipv4Addr::new(127, 0, 0, 1))
}

/// Runs single-shot terminal output mode.
pub async fn run_once(
    config: &AppConfig,
    port: u16,
    explicit_interface: Option<&str>,
    ttl: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let interfaces = discover_network_interfaces().unwrap_or_default();
    let (_iface_name, host_ip) = resolve_selected_ip(explicit_interface, &interfaces);

    let client = HermesProbeClient::new();
    let probe_state = client.probe(port, Some(host_ip)).await;

    let host_id = get_host_id(config);
    let display_name = get_display_name(config);
    let scheme = "http".to_string();

    let payload = create_pairing_payload(
        host_id.clone(),
        display_name.clone(),
        host_ip.to_string(),
        port,
        scheme,
        ttl,
    );

    let uri = encode_pairing_uri(&payload);
    let qr_rendered = render_terminal_qr(&uri).map_err(|e| format!("QR Render Error: {}", e))?;

    // Format Hermes status line
    match &probe_state {
        ProbeState::Online(status) => {
            let ver = status.version.as_deref().unwrap_or("unknown");
            let auth = if status.auth_required {
                "Required"
            } else {
                "None"
            };
            println!("Hermes: Running (v{}, Auth: {})", ver, auth);
        }
        ProbeState::LoopbackOnly { lan_error, .. } => {
            println!("Hermes: Running locally (127.0.0.1), but LAN unreachable: {}", lan_error);
            println!("⚠️  Warning: Hermes is bound to loopback only. Start Hermes with --host 0.0.0.0");
        }
        ProbeState::Offline(err) => {
            println!("Hermes: Offline ({})", err);
        }
    }

    let short_id = if host_id.len() >= 8 {
        format!("{}...", &host_id[..8])
    } else {
        host_id.clone()
    };

    println!("Host: {}", display_name);
    println!("Address: http://{}:{}", host_ip, port);
    println!("Host ID: {}", short_id);
    println!("Expires in: {:02}:{:02}", ttl / 60, ttl % 60);
    println!("\n{}", qr_rendered);
    println!("Pairing Link: {}", uri);

    Ok(())
}

/// Runs interactive terminal UI mode that updates in a loop.
pub async fn run_terminal_loop(
    config: &AppConfig,
    port: u16,
    explicit_interface: Option<&str>,
    ttl: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut last_generated_at = std::time::Instant::now();
    let mut current_payload = {
        let interfaces = discover_network_interfaces().unwrap_or_default();
        let (_iface_name, host_ip) = resolve_selected_ip(explicit_interface, &interfaces);
        create_pairing_payload(
            get_host_id(config),
            get_display_name(config),
            host_ip.to_string(),
            port,
            "http".to_string(),
            ttl,
        )
    };

    let client = HermesProbeClient::new();

    loop {
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(last_generated_at).as_secs();

        let interfaces = discover_network_interfaces().unwrap_or_default();
        let (_iface_name, host_ip) = resolve_selected_ip(explicit_interface, &interfaces);

        // Regenerate if expired
        if elapsed >= ttl {
            current_payload = create_pairing_payload(
                get_host_id(config),
                get_display_name(config),
                host_ip.to_string(),
                port,
                "http".to_string(),
                ttl,
            );
            last_generated_at = std::time::Instant::now();
        }

        let remaining = ttl.saturating_sub(now.duration_since(last_generated_at).as_secs());
        let probe_state = client.probe(port, Some(host_ip)).await;

        let uri = encode_pairing_uri(&current_payload);
        let qr_rendered = render_terminal_qr(&uri).unwrap_or_default();

        // Clear terminal screen (cross-platform ANSI)
        print!("\x1B[2J\x1B[1;1H");

        match &probe_state {
            ProbeState::Online(status) => {
                let ver = status.version.as_deref().unwrap_or("unknown");
                let auth = if status.auth_required {
                    "Required"
                } else {
                    "None"
                };
                println!("Hermes: Running (v{}, Auth: {})", ver, auth);
            }
            ProbeState::LoopbackOnly { lan_error, .. } => {
                println!("Hermes: Loopback Only (127.0.0.1) [LAN Error: {}]", lan_error);
                println!("⚠️  Run Hermes with: hermes serve --host 0.0.0.0 --port {}", port);
            }
            ProbeState::Offline(err) => {
                println!("Hermes: Offline ({})", err);
            }
        }

        let host_id = &current_payload.host_id;
        let short_id = if host_id.len() >= 8 {
            format!("{}...", &host_id[..8])
        } else {
            host_id.clone()
        };

        println!("Host: {}", current_payload.name);
        println!("Address: http://{}:{}", current_payload.host, current_payload.port);
        println!("Host ID: {}", short_id);
        println!("Expires in: {:02}:{:02}", remaining / 60, remaining % 60);
        println!("\n{}", qr_rendered);
        println!("Pairing Link: {}", uri);
        println!("\nPress Ctrl+C to exit.");

        sleep(Duration::from_secs(1)).await;
    }
}
