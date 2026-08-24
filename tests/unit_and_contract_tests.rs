use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hermes_pair::cli::{parse_hermes_url, resolve_cli_endpoint};
use hermes_pair::config::load_or_create_config_from_path;
use hermes_pair::hermes::HermesProbeClient;
use hermes_pair::models::{NetworkInterfaceInfo, PairingPayloadV1};
use hermes_pair::network::filter_and_sort_interfaces;
use hermes_pair::pairing::{
    create_pairing_payload, decode_pairing_uri, decode_pairing_uri_at_time, encode_pairing_uri,
    validate_payload, validate_ttl, PairingError, MAX_DECODED_JSON_BYTES, MAX_ENCODED_URI_BYTES,
};
use std::net::Ipv4Addr;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use uuid::Uuid;

#[test]
fn test_canonical_cross_contract_fixture() {
    let payload = PairingPayloadV1 {
        v: 1,
        payload_type: "hermes-pair".to_string(),
        host_id: "58af1471-a0a2-4e2b-9426-5068f2a2deab".to_string(),
        name: "Office-PC".to_string(),
        host: "192.168.1.150".to_string(),
        port: 9119,
        scheme: "http".to_string(),
        expires_at: 1800000000,
        nonce: "QUJDREVGR0hJSktMTU5PUHFyc3R1dnd4eXoxMjM0NTY".to_string(),
    };

    let json_str = serde_json::to_string(&payload).expect("Serialization failed");
    assert!(json_str.contains("\"v\":1"));
    assert!(json_str.contains("\"type\":\"hermes-pair\""));
    assert!(json_str.contains("\"host_id\":\"58af1471-a0a2-4e2b-9426-5068f2a2deab\""));
    assert!(json_str.contains("\"name\":\"Office-PC\""));
    assert!(json_str.contains("\"host\":\"192.168.1.150\""));
    assert!(json_str.contains("\"port\":9119"));
    assert!(json_str.contains("\"scheme\":\"http\""));
    assert!(json_str.contains("\"expires_at\":1800000000"));
    assert!(json_str.contains("\"nonce\":\"QUJDREVGR0hJSktMTU5PUHFyc3R1dnd4eXoxMjM0NTY\""));

    let uri = encode_pairing_uri(&payload);
    assert!(uri.starts_with("hermes://pair?data="));

    // Decode at current_time = expires_at - 60 (well within validity window)
    let decode_time = 1800000000 - 60;
    let decoded = decode_pairing_uri_at_time(&uri, decode_time)
        .expect("Canonical fixture must decode successfully");

    assert_eq!(decoded.v, 1);
    assert_eq!(decoded.payload_type, "hermes-pair");
    assert_eq!(decoded.host_id, "58af1471-a0a2-4e2b-9426-5068f2a2deab");
    assert_eq!(decoded.name, "Office-PC");
    assert_eq!(decoded.host, "192.168.1.150");
    assert_eq!(decoded.port, 9119);
    assert_eq!(decoded.scheme, "http");
    assert_eq!(decoded.expires_at, 1800000000);
    assert_eq!(decoded.nonce, "QUJDREVGR0hJSktMTU5PUHFyc3R1dnd4eXoxMjM0NTY");
}

#[test]
fn test_config_persistence() {
    let tmp_dir = std::env::temp_dir();
    let config_path: PathBuf = tmp_dir.join(format!("hermes_test_config_{}.json", Uuid::new_v4()));

    let _ = std::fs::remove_file(&config_path);

    let config1 = load_or_create_config_from_path(&config_path).expect("Should create new config");
    assert!(!config1.host_id.is_empty());
    assert!(Uuid::parse_str(&config1.host_id).is_ok());

    let config2 =
        load_or_create_config_from_path(&config_path).expect("Should load existing config");
    assert_eq!(config1.host_id, config2.host_id);

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
        nonce: "QUJDREVGR0hJSktMTU5PUHFyc3R1dnd4eXoxMjM0NTY".to_string(),
    };

    let json = serde_json::to_string(&payload).expect("Serialization failed");
    let deserialized: PairingPayloadV1 =
        serde_json::from_str(&json).expect("Deserialization failed");
    assert_eq!(payload, deserialized);

    let b64 = URL_SAFE_NO_PAD.encode(json.as_bytes());
    let decoded_bytes = URL_SAFE_NO_PAD
        .decode(b64.as_bytes())
        .expect("B64 decode failed");
    let from_b64: PairingPayloadV1 =
        serde_json::from_slice(&decoded_bytes).expect("JSON from b64 failed");
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
fn test_ttl_validation_bounds() {
    assert!(validate_ttl(0).is_err());
    assert!(validate_ttl(5).is_err());
    assert!(validate_ttl(9).is_err());
    assert!(validate_ttl(10).is_ok());
    assert!(validate_ttl(120).is_ok());
    assert!(validate_ttl(600).is_ok());
    assert!(validate_ttl(601).is_err());
    assert!(validate_ttl(1000).is_err());
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
        nonce: "QUJDREVGR0hJSktMTU5PUHFyc3R1dnd4eXoxMjM0NTY".to_string(),
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
fn test_excessive_future_ttl_rejection() {
    let host_id = Uuid::new_v4().to_string();
    let payload = PairingPayloadV1 {
        v: 1,
        payload_type: "hermes-pair".to_string(),
        host_id,
        name: "Future-Node".to_string(),
        host: "10.0.0.5".to_string(),
        port: 9119,
        scheme: "http".to_string(),
        expires_at: 1000 + 700, // Exceeds now + 600
        nonce: "QUJDREVGR0hJSktMTU5PUHFyc3R1dnd4eXoxMjM0NTY".to_string(),
    };

    let uri = encode_pairing_uri(&payload);
    let result = decode_pairing_uri_at_time(&uri, 1000);

    match result {
        Err(PairingError::TtlExceedsMaximum { .. }) => {}
        other => panic!("Expected TtlExceedsMaximum error, got {:?}", other),
    }
}

#[test]
fn test_invalid_version_rejection() {
    let host_id = Uuid::new_v4().to_string();
    let payload = PairingPayloadV1 {
        v: 2,
        payload_type: "hermes-pair".to_string(),
        host_id,
        name: "Future-Node".to_string(),
        host: "10.0.0.5".to_string(),
        port: 9119,
        scheme: "http".to_string(),
        expires_at: 1100,
        nonce: "QUJDREVGR0hJSktMTU5PUHFyc3R1dnd4eXoxMjM0NTY".to_string(),
    };

    let uri = encode_pairing_uri(&payload);
    let result = decode_pairing_uri_at_time(&uri, 1000);

    match result {
        Err(PairingError::UnsupportedVersion(v)) => {
            assert_eq!(v, 2);
        }
        other => panic!("Expected UnsupportedVersion error, got {:?}", other),
    }
}

#[test]
fn test_invalid_payload_type_rejection() {
    let host_id = Uuid::new_v4().to_string();
    let payload = PairingPayloadV1 {
        v: 1,
        payload_type: "hermes-auth".to_string(),
        host_id,
        name: "Bad-Type-Node".to_string(),
        host: "10.0.0.5".to_string(),
        port: 9119,
        scheme: "http".to_string(),
        expires_at: 1100,
        nonce: "QUJDREVGR0hJSktMTU5PUHFyc3R1dnd4eXoxMjM0NTY".to_string(),
    };

    let uri = encode_pairing_uri(&payload);
    let result = decode_pairing_uri_at_time(&uri, 1000);

    match result {
        Err(PairingError::InvalidPayloadType(t)) => {
            assert_eq!(t, "hermes-auth");
        }
        other => panic!("Expected InvalidPayloadType error, got {:?}", other),
    }
}

#[test]
fn test_invalid_uuid_rejection() {
    let payload = PairingPayloadV1 {
        v: 1,
        payload_type: "hermes-pair".to_string(),
        host_id: "not-a-valid-uuid".to_string(),
        name: "Bad-UUID-Node".to_string(),
        host: "10.0.0.5".to_string(),
        port: 9119,
        scheme: "http".to_string(),
        expires_at: 1100,
        nonce: "QUJDREVGR0hJSktMTU5PUHFyc3R1dnd4eXoxMjM0NTY".to_string(),
    };

    let uri = encode_pairing_uri(&payload);
    let result = decode_pairing_uri_at_time(&uri, 1000);

    match result {
        Err(PairingError::InvalidHostId(_)) => {}
        other => panic!("Expected InvalidHostId error, got {:?}", other),
    }
}

#[test]
fn test_blank_or_oversized_name_rejection() {
    let host_id = Uuid::new_v4().to_string();

    // Blank name
    let mut payload = PairingPayloadV1 {
        v: 1,
        payload_type: "hermes-pair".to_string(),
        host_id: host_id.clone(),
        name: "   ".to_string(),
        host: "10.0.0.5".to_string(),
        port: 9119,
        scheme: "http".to_string(),
        expires_at: 1100,
        nonce: "QUJDREVGR0hJSktMTU5PUHFyc3R1dnd4eXoxMjM0NTY".to_string(),
    };
    assert!(validate_payload(&payload, 1000).is_err());

    // Control characters in name
    payload.name = "Office\x00PC".to_string();
    assert!(validate_payload(&payload, 1000).is_err());
    payload.name = "Office\nPC".to_string();
    assert!(validate_payload(&payload, 1000).is_err());

    // Oversized name (>128 chars)
    payload.name = "A".repeat(129);
    assert!(validate_payload(&payload, 1000).is_err());

    // Valid 128-char name
    payload.name = "A".repeat(128);
    assert!(validate_payload(&payload, 1000).is_ok());
}

#[test]
fn test_malicious_host_rejection() {
    let host_id = Uuid::new_v4().to_string();
    let base_payload = PairingPayloadV1 {
        v: 1,
        payload_type: "hermes-pair".to_string(),
        host_id,
        name: "Node".to_string(),
        host: "".to_string(),
        port: 9119,
        scheme: "http".to_string(),
        expires_at: 1100,
        nonce: "QUJDREVGR0hJSktMTU5PUHFyc3R1dnd4eXoxMjM0NTY".to_string(),
    };

    let forbidden_hosts = vec![
        "",
        "   ",
        "user@evil.com",
        "evil.com/path",
        "evil.com\\path",
        "evil.com?param=1",
        "evil.com#frag",
        "192.168.1.1:9119",
        "192.168.1.1 evil.com",
        "192.168.1.1\x00",
    ];

    for bad_host in forbidden_hosts {
        let mut p = base_payload.clone();
        p.host = bad_host.to_string();
        assert!(
            validate_payload(&p, 1000).is_err(),
            "Host '{}' should be rejected",
            bad_host
        );
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
        port: 0,
        scheme: "http".to_string(),
        expires_at: 1100,
        nonce: "QUJDREVGR0hJSktMTU5PUHFyc3R1dnd4eXoxMjM0NTY".to_string(),
    };

    let uri = encode_pairing_uri(&payload);
    let result = decode_pairing_uri_at_time(&uri, 1000);

    match result {
        Err(PairingError::InvalidPort(p)) => {
            assert_eq!(p, 0);
        }
        other => panic!("Expected InvalidPort error, got {:?}", other),
    }
}

#[test]
fn test_invalid_scheme_rejection() {
    let host_id = Uuid::new_v4().to_string();
    let mut payload = PairingPayloadV1 {
        v: 1,
        payload_type: "hermes-pair".to_string(),
        host_id,
        name: "Node".to_string(),
        host: "192.168.1.1".to_string(),
        port: 9119,
        scheme: "ftp".to_string(),
        expires_at: 1100,
        nonce: "QUJDREVGR0hJSktMTU5PUHFyc3R1dnd4eXoxMjM0NTY".to_string(),
    };

    assert!(validate_payload(&payload, 1000).is_err());

    payload.scheme = "ws".to_string();
    assert!(validate_payload(&payload, 1000).is_err());

    payload.scheme = "http".to_string();
    assert!(validate_payload(&payload, 1000).is_ok());

    payload.scheme = "https".to_string();
    assert!(validate_payload(&payload, 1000).is_ok());
}

#[test]
fn test_nonce_validation() {
    let host_id = Uuid::new_v4().to_string();
    let mut payload = PairingPayloadV1 {
        v: 1,
        payload_type: "hermes-pair".to_string(),
        host_id,
        name: "Node".to_string(),
        host: "192.168.1.1".to_string(),
        port: 9119,
        scheme: "http".to_string(),
        expires_at: 1100,
        nonce: "".to_string(),
    };

    // Empty nonce rejected
    assert!(validate_payload(&payload, 1000).is_err());

    // Nonce too short (< 16 bytes decoded)
    let short_bytes = [1u8; 15];
    payload.nonce = URL_SAFE_NO_PAD.encode(short_bytes);
    assert!(validate_payload(&payload, 1000).is_err());

    // Valid 16-byte nonce
    let valid_16 = [1u8; 16];
    payload.nonce = URL_SAFE_NO_PAD.encode(valid_16);
    assert!(validate_payload(&payload, 1000).is_ok());

    // Valid 32-byte nonce
    let valid_32 = [1u8; 32];
    payload.nonce = URL_SAFE_NO_PAD.encode(valid_32);
    assert!(validate_payload(&payload, 1000).is_ok());

    // Valid 64-byte nonce
    let valid_64 = [1u8; 64];
    payload.nonce = URL_SAFE_NO_PAD.encode(valid_64);
    assert!(validate_payload(&payload, 1000).is_ok());

    // Nonce too long (> 64 bytes decoded)
    let too_long = [1u8; 65];
    payload.nonce = URL_SAFE_NO_PAD.encode(too_long);
    assert!(validate_payload(&payload, 1000).is_err());

    // Invalid base64 characters
    payload.nonce = "not-valid-base64!@#$%".to_string();
    assert!(validate_payload(&payload, 1000).is_err());
}

#[test]
fn test_oversized_payload_rejection() {
    let huge_uri = format!(
        "hermes://pair?data={}",
        "A".repeat(MAX_ENCODED_URI_BYTES + 10)
    );
    let result = decode_pairing_uri(&huge_uri);
    match result {
        Err(PairingError::PayloadTooLarge { .. }) => {}
        other => panic!("Expected PayloadTooLarge error, got {:?}", other),
    }

    // Huge JSON decoded payload
    let huge_data = vec![b' '; MAX_DECODED_JSON_BYTES + 100];
    let encoded = URL_SAFE_NO_PAD.encode(&huge_data);
    let uri = format!("hermes://pair?data={}", encoded);
    let result = decode_pairing_uri(&uri);
    match result {
        Err(PairingError::PayloadTooLarge { .. }) => {}
        other => panic!("Expected PayloadTooLarge error, got {:?}", other),
    }
}

#[test]
fn test_cli_parse_hermes_url_and_endpoint_resolution() {
    let (scheme, host, port) =
        parse_hermes_url("http://127.0.0.1:9222").expect("Should parse hermes url");
    assert_eq!(scheme, "http");
    assert_eq!(host, "127.0.0.1");
    assert_eq!(port, 9222);

    let (scheme, host, port) =
        parse_hermes_url("https://localhost:8443").expect("Should parse https hermes url");
    assert_eq!(scheme, "https");
    assert_eq!(host, "localhost");
    assert_eq!(port, 8443);

    assert!(parse_hermes_url("ftp://127.0.0.1:9119").is_err());

    // Endpoint resolution
    let (s, p) = resolve_cli_endpoint(Some("http://127.0.0.1:9222"), None).unwrap();
    assert_eq!(s, "http");
    assert_eq!(p, 9222);

    let (s, p) = resolve_cli_endpoint(Some("https://127.0.0.1:9222"), Some(8888)).unwrap();
    assert_eq!(s, "https");
    assert_eq!(p, 8888);

    let (s, p) = resolve_cli_endpoint(None, None).unwrap();
    assert_eq!(s, "http");
    assert_eq!(p, 9119);

    let (s, p) = resolve_cli_endpoint(None, Some(9555)).unwrap();
    assert_eq!(s, "http");
    assert_eq!(p, 9555);
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
    assert!(!sorted
        .iter()
        .any(|i| i.is_loopback || i.ip == Ipv4Addr::new(127, 0, 0, 1)));
    assert!(!sorted
        .iter()
        .any(|i| i.ip == Ipv4Addr::new(169, 254, 10, 20)));

    // Order: Physical LAN (eth0 192.168.1.10) -> Tailscale (100.80.5.6) -> Virtual LAN (docker0 172.17.0.1)
    assert_eq!(sorted.len(), 3);
    assert_eq!(sorted[0].name, "eth0");
    assert_eq!(sorted[1].name, "tailscale0");
    assert_eq!(sorted[2].name, "docker0");
}

#[tokio::test]
async fn test_mock_hermes_probe() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind mock listener");
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

    assert!(
        status_res.is_ok(),
        "Probe should succeed against mock server"
    );
    let status = status_res.unwrap();
    assert_eq!(status.status, "running");
    assert!(status.auth_required);
    assert_eq!(
        status.auth_providers,
        vec!["bearer".to_string(), "oauth2".to_string()]
    );
    assert_eq!(status.auth_flows, vec!["token".to_string()]);
    assert_eq!(status.version, Some("1.2.0".to_string()));

    let _ = server_task.await;
}
