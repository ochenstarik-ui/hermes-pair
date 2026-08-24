pub use crate::models::NetworkInterfaceInfo;
use std::net::Ipv4Addr;

pub fn is_loopback(ip: &Ipv4Addr) -> bool {
    ip.is_loopback() || ip.octets()[0] == 127
}

pub fn is_link_local(ip: &Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 169 && octets[1] == 254
}

pub fn is_tailscale_ip(ip: &Ipv4Addr) -> bool {
    let octets = ip.octets();
    // CGNAT range 100.64.0.0/10 commonly used by Tailscale / WireGuard overlays
    octets[0] == 100 && (octets[1] >= 64 && octets[1] <= 127)
}

pub fn is_virtual_adapter(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("docker")
        || lower.contains("veth")
        || lower.contains("virbr")
        || lower.contains("vbox")
        || lower.contains("vmnet")
        || lower.contains("hyper-v")
        || lower.contains("wsl")
        || lower.contains("vethernet")
        || lower.contains("virtual")
        || lower.contains("tap")
        || lower.contains("tun")
}

fn interface_priority(info: &NetworkInterfaceInfo) -> u32 {
    let is_priv = info.ip.is_private();
    let is_ts = is_tailscale_ip(&info.ip);

    match (info.is_virtual, is_priv, is_ts) {
        (false, true, _) => 100,      // Physical LAN (192.168.x.x, 10.x.x.x, 172.16-31.x.x)
        (false, false, true) => 80,   // Tailscale / Overlay
        (false, false, false) => 60,  // Other physical (e.g. public or custom)
        (true, true, _) => 40,        // Virtual LAN (e.g. WSL, Hyper-V virtual switch)
        (true, false, true) => 30,    // Virtual Tailscale
        (true, false, false) => 20,   // Other virtual
    }
}

pub fn filter_and_sort_interfaces(
    interfaces: impl IntoIterator<Item = NetworkInterfaceInfo>,
) -> Vec<NetworkInterfaceInfo> {
    let mut filtered: Vec<NetworkInterfaceInfo> = interfaces
        .into_iter()
        .filter(|iface| !iface.is_loopback && !is_loopback(&iface.ip) && !is_link_local(&iface.ip))
        .collect();

    filtered.sort_by(|a, b| {
        let prio_a = interface_priority(a);
        let prio_b = interface_priority(b);
        prio_b
            .cmp(&prio_a)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.ip.cmp(&b.ip))
    });

    filtered
}

pub fn discover_network_interfaces() -> Result<Vec<NetworkInterfaceInfo>, std::io::Error> {
    let if_addrs_list = if_addrs::get_if_addrs()?;

    let mut raw_interfaces = Vec::new();
    for iface in if_addrs_list {
        if let if_addrs::IfAddr::V4(ref v4_addr) = iface.addr {
            let is_virt = is_virtual_adapter(&iface.name);
            let loopback = iface.is_loopback() || is_loopback(&v4_addr.ip);
            raw_interfaces.push(NetworkInterfaceInfo {
                name: iface.name,
                ip: v4_addr.ip,
                is_loopback: loopback,
                is_virtual: is_virt,
            });
        }
    }

    Ok(filter_and_sort_interfaces(raw_interfaces))
}
