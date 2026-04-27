use crate::app::{AppMessage, NetType, NetworkInfo};
use hickory_resolver::net::runtime::TokioRuntimeProvider;
use std::net::IpAddr;
use std::process::Stdio;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;
use surge_ping::{Client, Config, ICMP, IcmpPacket, PingIdentifier, PingSequence, SurgeError};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};

// ── Public IP ──────────────────────────────────────────────────────────────────

/// Fetches the machine's public IP via ipify. Timeout: 5 s.
pub async fn fetch_public_ip() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok()?;

    let text = client
        .get("https://api.ipify.org")
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;

    let ip = text.trim().to_string();
    if ip.is_empty() { None } else { Some(ip) }
}

// ── Network interface info ─────────────────────────────────────────────────────

pub async fn fetch_network_info() -> Option<NetworkInfo> {
    #[cfg(target_os = "macos")]
    return fetch_network_info_macos().await;

    #[cfg(target_os = "linux")]
    return fetch_network_info_linux().await;

    #[allow(unreachable_code)]
    None
}

#[cfg(target_os = "macos")]
async fn fetch_network_info_macos() -> Option<NetworkInfo> {
    let route_out = Command::new("route")
        .args(["get", "default"])
        .output()
        .await
        .ok()?;

    let route_str = String::from_utf8_lossy(&route_out.stdout);
    let interface = route_str
        .lines()
        .find(|l| l.trim().starts_with("interface:"))?
        .split_whitespace()
        .nth(1)?
        .to_string();

    let gateway_ip = route_str
        .lines()
        .find(|l| l.trim().starts_with("gateway:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .map(String::from);

    let interface_clone = interface.clone();

    let hw_out = Command::new("networksetup")
        .args(["-listallhardwareports"])
        .output()
        .await
        .ok()?;

    let hw_str = String::from_utf8_lossy(&hw_out.stdout);
    let mut current_port = String::new();
    let mut net_type = NetType::Unknown;
    let mut name = interface.clone();

    for line in hw_str.lines() {
        let line = line.trim();
        if let Some(port) = line.strip_prefix("Hardware Port:") {
            current_port = port.trim().to_string();
        } else if let Some(dev) = line.strip_prefix("Device:") {
            if dev.trim() == interface {
                let lc = current_port.to_lowercase();
                if lc.contains("wi-fi") || lc.contains("wifi") || lc.contains("airport") {
                    net_type = NetType::Wifi;
                    if let Ok(ssid_out) = Command::new("networksetup")
                        .args(["-getairportnetwork", &interface])
                        .output()
                        .await
                    {
                        let ssid_str = String::from_utf8_lossy(&ssid_out.stdout);
                        if let Some(ssid_part) = ssid_str.splitn(2, ':').nth(1) {
                            let ssid = ssid_part.trim();
                            if !ssid.is_empty() && !ssid.to_lowercase().contains("not associated") {
                                name = ssid.to_string();
                            }
                        }
                    }
                } else if lc.contains("ethernet")
                    || lc.contains("thunderbolt")
                    || lc.contains("usb")
                {
                    net_type = NetType::Ethernet;
                    name = current_port.clone();
                } else {
                    net_type = NetType::Other(current_port.clone());
                    name = current_port.clone();
                }
                break;
            }
        }
    }

    Some(NetworkInfo {
        interface,
        net_type,
        name,
        private_ip: fetch_private_ip(&interface_clone).await,
        gateway_ip,
        dns_servers: fetch_dns_servers().await,
    })
}

#[cfg(target_os = "linux")]
async fn fetch_network_info_linux() -> Option<NetworkInfo> {
    // `ip route show default` — parse interface and gateway
    let route_out = Command::new("ip")
        .args(["route", "show", "default"])
        .output()
        .await
        .ok()?;
    let route_str = String::from_utf8_lossy(&route_out.stdout);

    // Example line: "default via 192.168.1.1 dev eth0 proto dhcp ..."
    let first = route_str.lines().next()?;
    let interface = first
        .split_whitespace()
        .skip_while(|&t| t != "dev")
        .nth(1)?
        .to_string();
    let gateway_ip = first
        .split_whitespace()
        .skip_while(|&t| t != "via")
        .nth(1)
        .map(String::from);

    let interface_clone = interface.clone();

    // Classify: check /sys/class/net/<iface>/wireless — exists only for Wi-Fi
    let wireless_path = format!("/sys/class/net/{interface}/wireless");
    let (net_type, name) = if tokio::fs::metadata(&wireless_path).await.is_ok() {
        // Try to get SSID via `iw dev <iface> link`
        let ssid = Command::new("iw")
            .args(["dev", &interface, "link"])
            .output()
            .await
            .ok()
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .find(|l| l.trim().starts_with("SSID:"))
                    .and_then(|l| l.splitn(2, ':').nth(1))
                    .map(|s| s.trim().to_string())
            })
            .unwrap_or_default();
        let label = if ssid.is_empty() {
            interface.clone()
        } else {
            ssid
        };
        (NetType::Wifi, label)
    } else {
        // Ethernet or other — use the interface name as label
        let lc = interface.to_lowercase();
        if lc.starts_with('e') || lc.starts_with("en") {
            (NetType::Ethernet, interface.clone())
        } else {
            (NetType::Other(interface.clone()), interface.clone())
        }
    };

    Some(NetworkInfo {
        interface,
        net_type,
        name,
        private_ip: fetch_private_ip(&interface_clone).await,
        gateway_ip,
        dns_servers: fetch_dns_servers().await,
    })
}

async fn fetch_private_ip(interface: &str) -> Option<String> {
    // Try ifconfig first (works on macOS and most Linux distros)
    let out = Command::new("ifconfig").arg(interface).output().await;
    if let Ok(out) = out {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("inet ") {
                let ip = rest.split_whitespace().next()?.to_string();
                if ip != "127.0.0.1" {
                    return Some(ip);
                }
            }
        }
    }

    // Fallback: `ip -4 addr show <iface>` (Linux)
    #[cfg(target_os = "linux")]
    if let Ok(out) = Command::new("ip")
        .args(["-4", "addr", "show", interface])
        .output()
        .await
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("inet ") {
                if let Some(addr) = rest.split_whitespace().next() {
                    let ip = addr.split('/').next().unwrap_or(addr);
                    if ip != "127.0.0.1" {
                        return Some(ip.to_string());
                    }
                }
            }
        }
    }

    None
}

async fn fetch_dns_servers() -> Vec<String> {
    if let Ok(content) = tokio::fs::read_to_string("/etc/resolv.conf").await {
        let servers: Vec<String> = content
            .lines()
            .filter(|l| l.trim_start().starts_with("nameserver"))
            .filter_map(|l| l.split_whitespace().nth(1).map(String::from))
            .collect();
        if !servers.is_empty() {
            return servers;
        }
    }
    Vec::new()
}

// ── Continuous ping (via surge-ping) ──────────────────────────────────────────

/// Resolves a hostname or IP string to the first `IpAddr`.
async fn resolve_host(host: &str) -> Result<IpAddr, String> {
    // Already an IP?
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(ip);
    }
    // DNS lookup via tokio
    let mut addrs = tokio::net::lookup_host(format!("{host}:0"))
        .await
        .map_err(|e| e.to_string())?;
    addrs
        .next()
        .map(|sa| sa.ip())
        .ok_or_else(|| "No addresses found".to_string())
}

/// Sends continuous ICMP pings using surge-ping (no OS subprocess).
/// Streams one `AppMessage::PingLine` per echo reply or timeout.
/// Stops when `cancel_rx` fires.
/// The `interval` atomic controls delay between pings (ms, 100–5000).
pub async fn stream_ping(
    host: String,
    tx: mpsc::Sender<AppMessage>,
    mut cancel_rx: oneshot::Receiver<()>,
    interval: Arc<AtomicU64>,
) {
    // Resolve hostname → IP
    let addr = match resolve_host(&host).await {
        Ok(a) => a,
        Err(e) => {
            let _ = tx
                .send(AppMessage::PingLine(format!(
                    "Cannot resolve '{host}': {e}"
                )))
                .await;
            let _ = tx.send(AppMessage::PingDone).await;
            return;
        }
    };
    let _ = tx
        .send(AppMessage::PingLine(format!("PING {host} ({addr})")))
        .await;

    // Build client — choose ICMPv4 or ICMPv6 based on resolved address
    let kind = if addr.is_ipv6() { ICMP::V6 } else { ICMP::V4 };
    let config = Config::builder().kind(kind).build();
    let client = match Client::new(&config) {
        Ok(c) => c,
        Err(e) => {
            let _ = tx
                .send(AppMessage::PingLine(format!(
                    "Socket error: {e}  (may need sudo for raw ICMP)"
                )))
                .await;
            let _ = tx.send(AppMessage::PingDone).await;
            return;
        }
    };

    let ident = PingIdentifier(std::process::id() as u16);
    let mut pinger = client.pinger(addr, ident).await;
    pinger.timeout(Duration::from_secs(2));

    let payload = [0u8; 56];
    let mut seq: u16 = 0;

    loop {
        // Send one ping, or stop on cancel
        tokio::select! {
            biased;
            _ = &mut cancel_rx => break,
            result = pinger.ping(PingSequence(seq), &payload) => {
                let line = match result {
                    Ok((IcmpPacket::V4(pkt), dur)) => {
                        let rtt = dur.as_secs_f64() * 1000.0;
                        let ttl = pkt.get_ttl().map(|t| t.to_string()).unwrap_or_else(|| "?".into());
                        format!(
                            "{} bytes from {}: icmp_seq={} ttl={} time={:.3} ms",
                            pkt.get_size(), pkt.get_source(), seq, ttl, rtt
                        )
                    }
                    Ok((IcmpPacket::V6(pkt), dur)) => {
                        let rtt = dur.as_secs_f64() * 1000.0;
                        format!(
                            "{} bytes from {}: icmp_seq={} hop_limit={} time={:.3} ms",
                            pkt.get_size(), pkt.get_source(), seq, pkt.get_max_hop_limit(), rtt
                        )
                    }
                    Err(SurgeError::Timeout { .. }) => {
                        format!("Request timeout for icmp_seq {seq}")
                    }
                    Err(e) => format!("Error: {e}"),
                };
                let _ = tx.send(AppMessage::PingLine(line)).await;
                seq = seq.wrapping_add(1);
            }
        }

        // Wait configurable time between pings, cancellable
        let sleep_ms = interval.load(Ordering::Relaxed);
        tokio::select! {
            biased;
            _ = &mut cancel_rx => break,
            _ = tokio::time::sleep(Duration::from_millis(sleep_ms)) => {}
        }
    }

    let _ = tx.send(AppMessage::PingDone).await;
}

// ── DNS resolve ────────────────────────────────────────────────────────────────

pub async fn run_dns(host: &str) -> (Vec<String>, f64) {
    use hickory_resolver::TokioResolver;
    use hickory_resolver::config::{ResolverConfig, ResolverOpts};
    use hickory_resolver::system_conf::read_system_conf;

    let start = std::time::Instant::now();

    let (config, opts) =
        read_system_conf().unwrap_or_else(|_| (ResolverConfig::default(), ResolverOpts::default()));

    let resolver = TokioResolver::builder_with_config(config, TokioRuntimeProvider::default())
        .with_options(opts)
        .build()
        .unwrap();

    let results = match resolver.lookup_ip(host).await {
        Ok(response) => {
            let ips: Vec<String> = response
                .iter()
                .map(|a: std::net::IpAddr| a.to_string())
                .collect();
            if ips.is_empty() {
                vec!["No records found".to_string()]
            } else {
                ips
            }
        }
        Err(e) => vec![format!("DNS error: {e}")],
    };
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    (results, latency_ms)
}

// ── Continuous MTR (repeated traceroute subprocess) ───────────────────────────

/// Pause between full traceroute rounds in ms.
const MTR_ROUND_INTERVAL_MS: u64 = 2000;

/// Parses a traceroute hop line into (ttl, ip, rtt_ms).
/// Returns None for lines that aren't hop lines.
fn parse_mtr_line(line: &str) -> Option<(u8, Option<String>, Option<f64>)> {
    let trimmed = line.trim();
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let ttl: u8 = parts.next()?.trim().parse().ok()?;
    let rest = parts.next()?.trim();

    if rest.starts_with('*') {
        return Some((ttl, None, None));
    }

    let mut tokens = rest.split_whitespace();
    let ip = tokens.next()?.to_string();
    // Find the first numeric token (RTT value), skip the "ms" after it
    let rtt = tokens.find_map(|t| t.parse::<f64>().ok());

    Some((ttl, Some(ip), rtt))
}

/// Continuously runs `traceroute -n -w 1 -q 1 <host>` in a loop.
/// Each hop line is sent as an MtrHopUpdate. Stops on cancel.
pub async fn stream_mtr(
    host: String,
    tx: mpsc::Sender<AppMessage>,
    mut cancel_rx: oneshot::Receiver<()>,
) {
    'outer: loop {
        let mut child = match Command::new("traceroute")
            .args(["-n", "-w", "1", "-q", "1", &host])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = tx
                    .send(AppMessage::MtrHopUpdate {
                        ttl: 1,
                        ip: Some(format!("Failed to run traceroute: {e}")),
                        rtt: None,
                    })
                    .await;
                let _ = tx.send(AppMessage::MtrDone).await;
                return;
            }
        };

        let stdout = child.stdout.take().expect("piped stdout");
        let mut lines = BufReader::new(stdout).lines();

        loop {
            tokio::select! {
                biased;
                _ = &mut cancel_rx => {
                    let _ = child.kill().await;
                    break 'outer;
                }
                result = lines.next_line() => {
                    match result {
                        Ok(Some(line)) => {
                            if let Some((ttl, ip, rtt)) = parse_mtr_line(&line) {
                                let _ = tx.send(AppMessage::MtrHopUpdate { ttl, ip, rtt }).await;
                            }
                        }
                        _ => break, // process ended, start next round
                    }
                }
            }
        }

        let _ = child.wait().await;

        // Wait between rounds, cancellable
        tokio::select! {
            biased;
            _ = &mut cancel_rx => break,
            _ = tokio::time::sleep(Duration::from_millis(MTR_ROUND_INTERVAL_MS)) => {}
        }
    }

    let _ = tx.send(AppMessage::MtrDone).await;
}

// ── Clipboard helper ──────────────────────────────────────────────────────────

/// Copies `text` to the system clipboard.  Returns true on success.
pub async fn copy_to_clipboard(text: String) -> bool {
    #[cfg(target_os = "macos")]
    {
        if let Ok(mut child) = Command::new("pbcopy").stdin(Stdio::piped()).spawn() {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes()).await;
            }
            return child.wait().await.map(|s| s.success()).unwrap_or(false);
        }
    }

    // Linux: try wl-copy (Wayland), then xclip, then xsel
    for args in [
        vec!["wl-copy"],
        vec!["xclip", "-selection", "clipboard"],
        vec!["xsel", "--clipboard", "--input"],
    ] {
        if let Ok(mut child) = Command::new(args[0])
            .args(&args[1..])
            .stdin(Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes()).await;
            }
            if child.wait().await.map(|s| s.success()).unwrap_or(false) {
                return true;
            }
        }
    }
    false
}

// ── Speedtest ─────────────────────────────────────────────────────────────────

/// Returns true if the `speedtest` binary is on $PATH.
pub async fn check_speedtest_installed() -> bool {
    Command::new("which")
        .arg("speedtest")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Runs `speedtest`, streaming each output line as `AppMessage::SpeedtestLine`.
/// Stops when `cancel_rx` fires or the process finishes.
pub async fn stream_speedtest(tx: mpsc::Sender<AppMessage>, mut cancel_rx: oneshot::Receiver<()>) {
    let mut child = match Command::new("speedtest")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = tx
                .send(AppMessage::SpeedtestLine(format!(
                    "Failed to run speedtest: {e}"
                )))
                .await;
            let _ = tx.send(AppMessage::SpeedtestDone).await;
            return;
        }
    };

    if let Some(stderr) = child.stderr.take() {
        let tx2 = tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                let _ = tx2.send(AppMessage::SpeedtestLine(l)).await;
            }
        });
    }

    let stdout = child.stdout.take().expect("piped stdout");
    let mut lines = BufReader::new(stdout).lines();

    loop {
        tokio::select! {
            biased;
            _ = &mut cancel_rx => {
                let _ = child.kill().await;
                break;
            }
            result = lines.next_line() => {
                match result {
                    Ok(Some(l)) => { let _ = tx.send(AppMessage::SpeedtestLine(l)).await; }
                    _ => break,
                }
            }
        }
    }

    let _ = tx.send(AppMessage::SpeedtestDone).await;
}
