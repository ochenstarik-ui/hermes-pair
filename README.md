# Hermes Pair (`hermes-pair`)

[![Build and Test](https://github.com/ochenstarik-ui/hermes-pair/actions/workflows/build.yml/badge.svg)](https://github.com/ochenstarik-ui/hermes-pair/actions/workflows/build.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Fast, lightweight, cross-platform pairing helper for **Hermes Agent** and the **Hermes Android App**.

`hermes-pair` bridges your host computer (running Hermes) with the mobile companion app by generating high-contrast QR codes and standard pairing URIs (`hermes://pair?data=...`) containing host network information, authentication state, and cryptographically secure random nonces.

---

## Features

- 🖥️ **Native GUI & Terminal UI Modes**: Runs as a lightweight native GUI window (`eframe`/`egui`) on desktop or an interactive/single-shot ANSI terminal interface on headless servers.
- 🔍 **Smart Network Interface Discovery**: Automatically discovers and prioritizes physical LAN interfaces (`192.168.x.x`, `10.x.x.x`, `172.16-31.x.x`) and Tailscale overlays (`100.x.x.x`), while filtering out loopback and link-local adapters.
- ⚡ **Real-Time Hermes Status Probing**: Connects to `http://127.0.0.1:<port>/api/status` and `http://<lan_ip>:<port>/api/status` to detect whether Hermes is active, verify version and auth requirements, and warn if Hermes is mistakenly bound only to loopback (`127.0.0.1`).
- 🛡️ **Built-in Security Safeguards**:
  - Automatically warns if Hermes is exposed over LAN without authentication.
  - Generates single-use, 16-byte cryptographically secure random nonces.
  - Enforces configurable TTL expiry (default: 120 seconds).
  - Validates payload structure, versioning, and UUID integrity.
- 💾 **Persistent Host Identity**: Manages a persistent UUIDv4 `host_id` saved atomically to `%APPDATA%\HermesPair\config.json` (Windows) or `~/.config/hermes-pair/config.json` (Linux).

---

## Pairing Protocol Specification (`v1`)

The generated QR code encodes a custom URI:
```text
hermes://pair?data=<BASE64URL_ENCODED_JSON>
```

### Decoded JSON Payload (`PairingPayloadV1`)

```json
{
  "v": 1,
  "type": "hermes-pair",
  "host_id": "9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d",
  "name": "Gaming-PC",
  "host": "192.168.1.34",
  "port": 9119,
  "scheme": "http",
  "expires_at": 1756012800,
  "nonce": "k7a_QW9jRz1M..."
}
```

| Field | Type | Description |
|---|---|---|
| `v` | `u32` | Protocol schema version (`1`). |
| `type` | `string` | Payload identifier (`hermes-pair`). |
| `host_id` | `string` | Persistent UUIDv4 identifying the host machine. |
| `name` | `string` | Human-readable computer or host display name. |
| `host` | `string` | Reachable IPv4 address or hostname for the mobile client. |
| `port` | `u16` | HTTP port on which Hermes Agent is listening (e.g. `9119`). |
| `scheme` | `string` | Connection scheme (`http` or `https`). |
| `expires_at` | `u64` | Unix epoch timestamp (seconds) after which this payload is rejected. |
| `nonce` | `string` | 16 cryptographically random bytes, Base64URL-encoded. |

---

## Installation & Build

### Prerequisites
- [Rust](https://rustup.rs/) 1.80+ (tested on Rust 1.98.0)

### Building from Source

```bash
# Clone the repository
git clone https://github.com/ochenstarik-ui/hermes-pair.git
cd hermes-pair

# Run tests
cargo test

# Build release binary
cargo build --release
```

Release binaries are produced at:
- **Windows**: `target/release/hermes-pair.exe`
- **Linux**: `target/release/hermes-pair`

---

## Usage

### 1. Native GUI Mode (Default)

Simply execute `hermes-pair` without flags to open the native desktop window:

```bash
hermes-pair
```

#### GUI Controls:
- **Status Badge**: Shows Hermes connectivity (Running, Offline, Loopback Only) and auth status.
- **Network Interface Dropdown**: Switch between Wi-Fi, Ethernet, Tailscale, or virtual adapters.
- **[ 🔄 Regenerate QR ]**: Generates a new payload with a fresh nonce and resets the countdown.
- **[ 📋 Copy Link ]**: Copies `hermes://pair?data=...` directly to the system clipboard.
- **[ 🔄 Check ]**: Retries probing the Hermes status endpoint immediately.

### 2. Interactive Terminal UI Mode

For remote SSH sessions or terminals without a display server:

```bash
hermes-pair --terminal
# or short flag:
hermes-pair -t
```

Output:
```text
Hermes: Running (v1.2.0, Auth: Required)
Host: Gaming-PC
Address: http://192.168.1.34:9119
Host ID: 7b31d044...
Expires in: 01:58

██████████████████████████████
██          ██  ██          ██
██  ██████  ██  ██  ██████  ██
██  ██████  ██  ██  ██████  ██
██          ██  ██          ██
██████████████████████████████
...
Pairing Link: hermes://pair?data=eyJ2Ijox...
```

### 3. Headless / Single-Shot Script Mode

To print the QR code once to stdout (useful for automation, provisioning scripts, or terminal output piping):

```bash
hermes-pair qr
# or
hermes-pair --no-gui
```

### 4. Command Line Options

```text
Usage: hermes-pair [OPTIONS] [COMMAND]

Commands:
  qr    Output QR code once to stdout and exit
  help  Print this message or the help of the given subcommand(s)

Options:
  -t, --terminal               Run interactive terminal mode with periodic refresh
      --no-gui                 Print QR once to stdout and exit (headless/script mode)
      --port <PORT>            Port of Hermes Agent [default: 9119]
      --hermes-url <URL>       Hermes status API URL (e.g. http://127.0.0.1:9119)
  -i, --interface <INTERFACE>  Specific network interface name or IPv4 address to advertise
      --ttl <TTL>              Pairing QR validity TTL in seconds [default: 120]
  -h, --help                   Print help
  -V, --version                Print version
```

---

## Configuration & Storage

`hermes-pair` generates a persistent `host_id` on first launch and stores it in JSON format:

- **Windows**: `%APPDATA%\HermesPair\config.json`
- **Linux / macOS**: `~/.config/hermes-pair/config.json`

Example configuration:
```json
{
  "host_id": "7b31d044-cf64-4e78-9df2-bb58a8f5e1a1",
  "display_name": "Studio-Workstation"
}
```

---

## Troubleshooting & Warnings

### Warning: Loopback Only
If Hermes is started with default `127.0.0.1` binding, other devices on the LAN cannot connect.
Start Hermes binding to all interfaces:
```bash
hermes serve --host 0.0.0.0 --port 9119
```

### Warning: Unauthenticated Network Access
If Hermes has no authentication configured, any device on your local network could access the API. Consider enabling token or password authentication for production usage.

---

## License

Licensed under the [MIT License](LICENSE).
