use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hermes_pair::config::load_or_create_config_from_path;
use hermes_pair::hermes::HermesProbeClient;
use hermes_pair::models::{NetworkInterfaceInfo, PairingPayloadV1};
use hermes_pair::network::filter_and_sort_interfaces;
use hermes_pair::pairing::{
    create_pairing_payload, decode_pairing_uri, decode_pairing_uri_at_time, encode_pairing_uri,
    PairingError,
};
use std::net::Ipv4Addr;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use uuid::Uuid;

#[test]
fn test_config_persistence() {
    let tmp_dir = std::env::temp_dir();
    let config_path: PathBuf = tmp_dir.join(format!("hermes_test_config_{}.json", Uuid::new_v4()));

    // Ensure clean state
    let _ = std::fs::remove_file(&config_path);

    // First load -> generates new config and persists it
    let config1 = load_or_create_config_from_path(&config_path).expect("Should create new config");
    assert!(!config1.host_id.is_empty());
    assert!(Uuid::parse_str(&config1.host_id).is_ok());

    // Second load -> loads existing config and keeps same host_id
    let config2 = load_or_create_config_from_path(&config_path).expect("Should load existing config");
    assert_eq!(config1.host_id, config2.host_id);

    // Clean up
    let _ = std::fs::remove_file(&config_path);
}

#[test]
fn test_pairing_payload_serde() {
    let host_id = Uuid::new_v4().to_string();
    let payload = PairingPayloadV1 {
        v: 1,
        payload_type: "hermes-pair".to_string(),
        host_id: host_id.clone(),
        name: "Test-Rig".to_string(),
        host: "192.168.1.100".to_string(),
        port: 9119,
        scheme: "http".to_string(),
        expires_at: 1800000000,
        nonce: "test_nonce_1234".to_string(),
    };

    let json = serde_json::to_string(&payload).expect("Serialization failed");
    let deserialized: PairingPayloadV1 = serde_json::from_str(&json).expect("Deserialization failed");
    assert_eq!(payload, deserialized);

    // Verify Base64URL round-trip
    let b64 = URL_SAFE_NO_PAD.encode(json.as_bytes());
    let decoded_bytes = URL_SAFE_NO_PAD.decode(b64.as_bytes()).expect("B64 decode failed");
    let from_b64: PairingPayloadV1 = serde_json::from_slice(&decoded_bytes).expect("JSON from b64 failed");
    assert_eq!(payload, from_b64);
}

#[test]
fn test_pairing_uri_encoding_and_validation() {
    let host_id = Uuid::new_v4().to_string();
    let payload = create_pairing_payload(
        host_id.clone(),
        "Studio-PC".to_string(),
        "192.168.0.50".to_string(),
        9119,
        "http".to_string(),
        300,
    );

    let uri = encode_pairing_uri(&payload);
    assert!(uri.starts_with("hermes://pair?data="));

    let decoded = decode_pairing_uri(&uri).expect("Decoding valid pairing URI must succeed");
    assert_eq!(decoded.v, 1);
    assert_eq!(decoded.payload_type, "hermes-pair");
    assert_eq!(decoded.host_id, host_id);
    assert_eq!(decoded.name, "Studio-PC");
    assert_eq!(decoded.host, "192.168.0.50");
    assert_eq!(decoded.port, 9119);
    assert_eq!(decoded.scheme, "http");
}

#[test]
fn test_expired_payload_rejection() {
    let host_id = Uuid::new_v4().to_string();
    let payload = PairingPayloadV1 {
        v: 1,
        payload_type: "hermes-pair".to_string(),
        host_id,
        name: "Old-Node".to_string(),
        host: "10.0.0.5".to_string(),
        port: 9119,
        scheme: "http".to_string(),
        expires_at: 1000,
        nonce: "test_nonce".to_string(),
    };

    let uri = encode_pairing_uri(&payload);
    let result = decode_pairing_uri_at_time(&uri, 2000);

    match result {
        Err(PairingError::PayloadExpired { expires_at, now }) => {
            assert_eq!(expires_at, 1000);
            assert_eq!(now, 2000);
        }
        other => panic!("Expected PayloadExpired error, got {:?}", other),
    }
}

#[test]
fn test_invalid_version_rejection() {
    let host_id = Uuid::new_v4().to_string();
    let payload = PairingPayloadV1 {
        v: 2, // Unsupported version
        payload_type: "hermes-pair".to_string(),
        host_id,
        name: "Future-Node".to_string(),
        host: "10.0.0.5".to_string(),
        port: 9119,
        scheme: "http".to_string(),
        expires_at: 3000000000,
        nonce: "test_nonce".to_string(),
    };

    let uri = encode_pairing_uri(&payload);
    let result = decode_pairing_uri_at_time(&uri, 10000);

    match result {
        Err(PairingError::UnsupportedVersion(v)) => {
            assert_eq!(v, 2);
        }
        other => panic!("Expected UnsupportedVersion error, got {:?}", other),
    }
}

#[test]
fn test_invalid_port_rejection() {
    let host_id = Uuid::new_v4().to_string();
    let payload = PairingPayloadV1 {
        v: 1,
        payload_type: "hermes-pair".to_string(),
        host_id,
        name: "Invalid-Port-Node".to_string(),
        host: "192.168.1.1".to_string(),
        port: 0, // Port 0 is invalid
        scheme: "http".to_string(),
        expires_at: 3000000000,
        nonce: "test_nonce".to_string(),
    };

    let uri = encode_pairing_uri(&payload);
    let result = decode_pairing_uri_at_time(&uri, 10000);

    match result {
        Err(PairingError::InvalidPort(p)) => {
            assert_eq!(p, 0);
        }
        other => panic!("Expected InvalidPort error, got {:?}", other),
    }
}

#[test]
fn test_network_interface_filtering() {
    let test_interfaces = vec![
        NetworkInterfaceInfo {
            name: "lo".to_string(),
            ip: Ipv4Addr::new(127, 0, 0, 1),
            is_loopback: true,
            is_virtual: false,
        },
        NetworkInterfaceInfo {
            name: "link-local".to_string(),
            ip: Ipv4Addr::new(169, 254, 10, 20),
            is_loopback: false,
            is_virtual: false,
        },
        NetworkInterfaceInfo {
            name: "docker0".to_string(),
            ip: Ipv4Addr::new(172, 17, 0, 1),
            is_loopback: false,
            is_virtual: true,
        },
        NetworkInterfaceInfo {
            name: "tailscale0".to_string(),
            ip: Ipv4Addr::new(100, 80, 5, 6),
            is_loopback: false,
            is_virtual: false,
        },
        NetworkInterfaceInfo {
            name: "eth0".to_string(),
            ip: Ipv4Addr::new(192, 168, 1, 10),
            is_loopback: false,
            is_virtual: false,
        },
    ];

    let sorted = filter_and_sort_interfaces(test_interfaces);

    // Loopback and link-local must be eliminated
    assert!(!sorted.iter().any(|i| i.is_loopback || i.ip == Ipv4Addr::new(127, 0, 0, 1)));
    assert!(!sorted.iter().any(|i| i.ip == Ipv4Addr::new(169, 254, 10, 20)));

    // Order: Physical LAN (eth0 192.168.1.10) -> Tailscale (100.80.5.6) -> Virtual LAN (docker0 172.17.0.1)
    assert_eq!(sorted.len(), 3);
    assert_eq!(sorted[0].name, "eth0");
    assert_eq!(sorted[1].name, "tailscale0");
    assert_eq!(sorted[2].name, "docker0");
}

#[tokio::test]
async fn test_mock_hermes_probe() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("Failed to bind mock listener");
    let port = listener.local_addr().unwrap().port();

    let server_task = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await;

            let response_body = serde_json::json!({
                "status": "running",
                "authRequired": true,
                "authProviders": ["bearer", "oauth2"],
                "authFlows": ["token"],
                "version": "1.2.0"
            })
            .to_string();

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            let _ = socket.write_all(response.as_bytes()).await;
        }
    });

    let client = HermesProbeClient::new();
    let base_url = format!("http://127.0.0.1:{}", port);
    let status_res = client.fetch_status(&base_url).await;

    assert!(status_res.is_ok(), "Probe should succeed against mock server");
    let status = status_res.unwrap();
    assert_eq!(status.status, "running");
    assert!(status.auth_required);
    assert_eq!(status.auth_providers, vec!["bearer".to_string(), "oauth2".to_string()]);
    assert_eq!(status.auth_flows, vec!["token".to_string()]);
    assert_eq!(status.version, Some("1.2.0".to_string()));

    let _ = server_task.await;
}
