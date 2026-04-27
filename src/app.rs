use std::collections::VecDeque;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::oneshot;

// ── Tab selection ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    Dashboard,
    Speedtest,
    Ping,
    Traceroute,
    Dns,
}

// ── Input mode ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Editing,
}

// ── Network info types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum NetType {
    Wifi,
    Ethernet,
    Other(String),
    #[allow(dead_code)]
    Unknown,
}

impl std::fmt::Display for NetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetType::Wifi => write!(f, "Wi-Fi"),
            NetType::Ethernet => write!(f, "Ethernet"),
            NetType::Other(s) => write!(f, "{s}"),
            NetType::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetworkInfo {
    pub interface: String,
    pub net_type: NetType,
    /// SSID for Wi-Fi, port label for Ethernet, interface name otherwise
    pub name: String,
    pub private_ip: Option<String>,
    pub gateway_ip: Option<String>,
    pub dns_servers: Vec<String>,
}

// ── DNS IP filter ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum DnsIpFilter {
    V4Only,
    V6Only,
    Both,
}

impl DnsIpFilter {
    pub fn cycle(&self) -> Self {
        match self {
            DnsIpFilter::V4Only => DnsIpFilter::V6Only,
            DnsIpFilter::V6Only => DnsIpFilter::Both,
            DnsIpFilter::Both => DnsIpFilter::V4Only,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            DnsIpFilter::V4Only => "IPv4 only",
            DnsIpFilter::V6Only => "IPv6 only",
            DnsIpFilter::Both => "IPv4 + IPv6",
        }
    }

    pub fn keeps_v4(&self) -> bool {
        matches!(self, DnsIpFilter::V4Only | DnsIpFilter::Both)
    }

    pub fn keeps_v6(&self) -> bool {
        matches!(self, DnsIpFilter::V6Only | DnsIpFilter::Both)
    }
}

// ── Messages sent from background tasks ───────────────────────────────────────

#[derive(Debug)]
pub enum AppMessage {
    PublicIp(Option<String>),
    NetworkInfo(Option<NetworkInfo>),
    // Ping
    PingLine(String),
    PingDone,
    // DNS
    DnsResult(Vec<String>, f64), // results, latency_ms
    DnsDone,
    // Traceroute
    TracerouteLine(String),
    TracerouteDone,
    // Speedtest
    SpeedtestInstalled(bool),
    SpeedtestLine(String),
    SpeedtestDone,
}

// ── Application state ──────────────────────────────────────────────────────────

pub struct App {
    pub active_tab: Tab,
    pub input_mode: InputMode,
    pub should_quit: bool,
    pub spin_tick: u64,

    // ── Dashboard ──
    pub public_ip: String,
    pub network_info: Option<NetworkInfo>,

    // ── Ping tab ──
    pub ping_input: String,
    pub ping_cursor: usize,
    pub ping_results: VecDeque<String>,
    pub ping_running: bool,
    /// All successfully parsed RTT values (ms) for stats computation.
    pub ping_rtts: Vec<f64>,
    pub ping_received: u32,
    pub ping_timeouts: u32,
    pub ping_cancel_tx: Option<oneshot::Sender<()>>,

    // ── Ping tab interval ──
    pub ping_interval_ms: Arc<AtomicU64>,

    // ── Traceroute tab ──
    pub traceroute_input: String,
    pub traceroute_cursor: usize,
    pub traceroute_results: VecDeque<String>,
    pub traceroute_running: bool,
    pub traceroute_cancel_tx: Option<oneshot::Sender<()>>,

    // ── DNS tab ──
    pub dns_input: String,
    pub dns_cursor: usize,
    pub dns_results: Vec<String>,
    pub dns_running: bool,
    pub dns_latency_ms: Option<f64>,
    pub dns_ip_filter: DnsIpFilter,

    // ── Speedtest tab ──
    /// None = checking, Some(false) = not installed, Some(true) = installed.
    pub speedtest_installed: Option<bool>,
    pub speedtest_running: bool,
    pub speedtest_lines: VecDeque<String>,
    pub speedtest_cancel_tx: Option<oneshot::Sender<()>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            active_tab: Tab::Dashboard,
            input_mode: InputMode::Normal,
            should_quit: false,
            spin_tick: 0,
            public_ip: "Fetching…".to_string(),
            network_info: None,
            ping_input: String::new(),
            ping_cursor: 0,
            ping_results: VecDeque::new(),
            ping_running: false,
            ping_rtts: Vec::new(),
            ping_received: 0,
            ping_timeouts: 0,
            ping_cancel_tx: None,
            ping_interval_ms: Arc::new(AtomicU64::new(1000)),
            traceroute_input: String::new(),
            traceroute_cursor: 0,
            traceroute_results: VecDeque::new(),
            traceroute_running: false,
            traceroute_cancel_tx: None,
            dns_input: String::new(),
            dns_cursor: 0,
            dns_results: Vec::new(),
            dns_running: false,
            dns_latency_ms: None,
            dns_ip_filter: DnsIpFilter::V4Only,
            speedtest_installed: None,
            speedtest_running: false,
            speedtest_lines: VecDeque::new(),
            speedtest_cancel_tx: None,
        }
    }

    // ── Derived ping stats ─────────────────────────────────────────────────────

    /// Returns false only once we know speedtest is not installed.
    /// While still checking (None) or confirmed installed (Some(true)) → true.
    pub fn speedtest_visible(&self) -> bool {
        self.speedtest_installed != Some(false)
    }

    /// Returns (min, max, avg, stddev) in ms, or None if no RTT data.
    pub fn ping_stats(&self) -> Option<(f64, f64, f64, f64)> {
        let rtts = &self.ping_rtts;
        if rtts.is_empty() {
            return None;
        }
        let n = rtts.len() as f64;
        let min = rtts.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = rtts.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let avg = rtts.iter().sum::<f64>() / n;
        let variance = rtts.iter().map(|&r| (r - avg).powi(2)).sum::<f64>() / n;
        Some((min, max, avg, variance.sqrt()))
    }

    pub fn ping_sent(&self) -> u32 {
        self.ping_received + self.ping_timeouts
    }

    pub fn get_ping_interval_ms(&self) -> u64 {
        self.ping_interval_ms.load(Ordering::Relaxed)
    }

    pub fn ping_interval_increase(&self) {
        const LEVELS: &[u64] = &[100, 200, 500, 1000, 2000, 3000, 5000];
        let cur = self.ping_interval_ms.load(Ordering::Relaxed);
        if let Some(&next) = LEVELS.iter().find(|&&v| v > cur) {
            self.ping_interval_ms.store(next, Ordering::Relaxed);
        }
    }

    pub fn ping_interval_decrease(&self) {
        const LEVELS: &[u64] = &[100, 200, 500, 1000, 2000, 3000, 5000];
        let cur = self.ping_interval_ms.load(Ordering::Relaxed);
        if let Some(&prev) = LEVELS.iter().rev().find(|&&v| v < cur) {
            self.ping_interval_ms.store(prev, Ordering::Relaxed);
        }
    }

    pub fn ping_loss_pct(&self) -> f64 {
        let sent = self.ping_sent();
        if sent == 0 {
            0.0
        } else {
            self.ping_timeouts as f64 / sent as f64 * 100.0
        }
    }

    // ── Input handling ─────────────────────────────────────────────────────────

    pub fn handle_char(&mut self, c: char) {
        match self.active_tab {
            Tab::Ping => {
                self.ping_input.insert(self.ping_cursor, c);
                self.ping_cursor += c.len_utf8();
            }
            Tab::Traceroute => {
                self.traceroute_input.insert(self.traceroute_cursor, c);
                self.traceroute_cursor += c.len_utf8();
            }
            Tab::Dns => {
                self.dns_input.insert(self.dns_cursor, c);
                self.dns_cursor += c.len_utf8();
            }
            Tab::Dashboard | Tab::Speedtest => {}
        }
    }

    pub fn handle_backspace(&mut self) {
        match self.active_tab {
            Tab::Ping => {
                if self.ping_cursor > 0 {
                    let prev = prev_char_boundary(&self.ping_input, self.ping_cursor);
                    self.ping_input.drain(prev..self.ping_cursor);
                    self.ping_cursor = prev;
                }
            }
            Tab::Traceroute => {
                if self.traceroute_cursor > 0 {
                    let prev = prev_char_boundary(&self.traceroute_input, self.traceroute_cursor);
                    self.traceroute_input.drain(prev..self.traceroute_cursor);
                    self.traceroute_cursor = prev;
                }
            }
            Tab::Dns => {
                if self.dns_cursor > 0 {
                    let prev = prev_char_boundary(&self.dns_input, self.dns_cursor);
                    self.dns_input.drain(prev..self.dns_cursor);
                    self.dns_cursor = prev;
                }
            }
            Tab::Dashboard | Tab::Speedtest => {}
        }
    }

    pub fn move_cursor_left(&mut self) {
        match self.active_tab {
            Tab::Ping => {
                self.ping_cursor = prev_char_boundary(&self.ping_input, self.ping_cursor);
            }
            Tab::Traceroute => {
                self.traceroute_cursor =
                    prev_char_boundary(&self.traceroute_input, self.traceroute_cursor);
            }
            Tab::Dns => {
                self.dns_cursor = prev_char_boundary(&self.dns_input, self.dns_cursor);
            }
            Tab::Dashboard | Tab::Speedtest => {}
        }
    }

    pub fn move_cursor_right(&mut self) {
        match self.active_tab {
            Tab::Ping => {
                self.ping_cursor = next_char_boundary(&self.ping_input, self.ping_cursor);
            }
            Tab::Traceroute => {
                self.traceroute_cursor =
                    next_char_boundary(&self.traceroute_input, self.traceroute_cursor);
            }
            Tab::Dns => {
                self.dns_cursor = next_char_boundary(&self.dns_input, self.dns_cursor);
            }
            Tab::Dashboard | Tab::Speedtest => {}
        }
    }

    pub fn move_cursor_home(&mut self) {
        match self.active_tab {
            Tab::Ping => self.ping_cursor = 0,
            Tab::Traceroute => self.traceroute_cursor = 0,
            Tab::Dns => self.dns_cursor = 0,
            Tab::Dashboard | Tab::Speedtest => {}
        }
    }

    pub fn move_cursor_end(&mut self) {
        match self.active_tab {
            Tab::Ping => self.ping_cursor = self.ping_input.len(),
            Tab::Traceroute => self.traceroute_cursor = self.traceroute_input.len(),
            Tab::Dns => self.dns_cursor = self.dns_input.len(),
            Tab::Dashboard | Tab::Speedtest => {}
        }
    }

    // ── Message application ────────────────────────────────────────────────────

    pub fn apply_message(&mut self, msg: AppMessage) {
        match msg {
            AppMessage::PublicIp(ip) => {
                self.public_ip = ip.unwrap_or_else(|| "Unavailable".to_string());
            }
            AppMessage::NetworkInfo(info) => {
                self.network_info = info;
            }
            AppMessage::PingLine(ref line) => {
                // Parse reply stats
                if line.contains("bytes from") && !line.contains("(DUP!)") {
                    if let Some(rtt) = parse_rtt(line) {
                        self.ping_rtts.push(rtt);
                        self.ping_received += 1;
                    }
                } else if line.to_lowercase().contains("request timeout")
                    || line.contains("No route")
                    || line.contains("100% packet loss")
                {
                    self.ping_timeouts += 1;
                }
                // Add to log
                if !line.trim().is_empty() {
                    if self.ping_results.len() >= 200 {
                        self.ping_results.pop_front();
                    }
                    self.ping_results.push_back(line.clone());
                }
            }
            AppMessage::PingDone => {
                self.ping_running = false;
                self.ping_cancel_tx = None;
            }
            AppMessage::DnsResult(ips, latency_ms) => {
                self.dns_results = ips;
                self.dns_latency_ms = Some(latency_ms);
            }
            AppMessage::DnsDone => {
                self.dns_running = false;
            }
            AppMessage::TracerouteLine(line) => {
                if !line.trim().is_empty() {
                    if self.traceroute_results.len() >= 64 {
                        self.traceroute_results.pop_front();
                    }
                    self.traceroute_results.push_back(line);
                }
            }
            AppMessage::TracerouteDone => {
                self.traceroute_running = false;
                self.traceroute_cancel_tx = None;
            }
            AppMessage::SpeedtestInstalled(v) => {
                self.speedtest_installed = Some(v);
            }
            AppMessage::SpeedtestLine(line) => {
                if !line.trim().is_empty() {
                    if self.speedtest_lines.len() >= 200 {
                        self.speedtest_lines.pop_front();
                    }
                    self.speedtest_lines.push_back(line);
                }
            }
            AppMessage::SpeedtestDone => {
                self.speedtest_running = false;
                self.speedtest_cancel_tx = None;
            }
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Parse RTT from lines like: "64 bytes from 8.8.8.8: icmp_seq=5 ttl=119 time=12.345 ms"
fn parse_rtt(line: &str) -> Option<f64> {
    let idx = line.find("time=")?;
    let rest = &line[idx + 5..];
    rest.split_whitespace()
        .next()?
        .trim_end_matches("ms")
        .parse()
        .ok()
}

fn prev_char_boundary(s: &str, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    s[..pos]
        .char_indices()
        .next_back()
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn next_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    s[pos..]
        .char_indices()
        .nth(1)
        .map(|(i, _)| pos + i)
        .unwrap_or(s.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── DnsIpFilter ────────────────────────────────────────────────────────

    #[test]
    fn dns_filter_cycle_wraps() {
        assert_eq!(DnsIpFilter::V4Only.cycle(), DnsIpFilter::V6Only);
        assert_eq!(DnsIpFilter::V6Only.cycle(), DnsIpFilter::Both);
        assert_eq!(DnsIpFilter::Both.cycle(), DnsIpFilter::V4Only);
    }

    #[test]
    fn dns_filter_keeps_flags() {
        assert!(DnsIpFilter::V4Only.keeps_v4());
        assert!(!DnsIpFilter::V4Only.keeps_v6());
        assert!(!DnsIpFilter::V6Only.keeps_v4());
        assert!(DnsIpFilter::V6Only.keeps_v6());
        assert!(DnsIpFilter::Both.keeps_v4());
        assert!(DnsIpFilter::Both.keeps_v6());
    }

    #[test]
    fn dns_filter_labels_are_nonempty() {
        for f in [DnsIpFilter::V4Only, DnsIpFilter::V6Only, DnsIpFilter::Both] {
            assert!(!f.label().is_empty());
        }
    }

    // ── NetType display ────────────────────────────────────────────────────

    #[test]
    fn net_type_display() {
        assert_eq!(NetType::Wifi.to_string(), "Wi-Fi");
        assert_eq!(NetType::Ethernet.to_string(), "Ethernet");
        assert_eq!(NetType::Other("VPN".into()).to_string(), "VPN");
        assert_eq!(NetType::Unknown.to_string(), "Unknown");
    }

    // ── ping_stats ─────────────────────────────────────────────────────────

    #[test]
    fn ping_stats_none_when_empty() {
        let app = App::new();
        assert!(app.ping_stats().is_none());
    }

    #[test]
    fn ping_stats_single_value() {
        let mut app = App::new();
        app.ping_rtts = vec![42.0];
        let (min, max, avg, stddev) = app.ping_stats().unwrap();
        assert_eq!(min, 42.0);
        assert_eq!(max, 42.0);
        assert_eq!(avg, 42.0);
        assert_eq!(stddev, 0.0);
    }

    #[test]
    fn ping_stats_known_values() {
        let mut app = App::new();
        // min=10, max=30, avg=20, stddev=~8.165
        app.ping_rtts = vec![10.0, 20.0, 30.0];
        let (min, max, avg, stddev) = app.ping_stats().unwrap();
        assert_eq!(min, 10.0);
        assert_eq!(max, 30.0);
        assert!((avg - 20.0).abs() < 1e-9);
        assert!((stddev - (200.0_f64 / 3.0).sqrt()).abs() < 1e-9);
    }

    // ── ping_loss_pct ──────────────────────────────────────────────────────

    #[test]
    fn ping_loss_zero_when_no_packets_sent() {
        let app = App::new();
        assert_eq!(app.ping_loss_pct(), 0.0);
    }

    #[test]
    fn ping_loss_full() {
        let mut app = App::new();
        app.ping_timeouts = 4;
        assert_eq!(app.ping_loss_pct(), 100.0);
    }

    #[test]
    fn ping_loss_partial() {
        let mut app = App::new();
        app.ping_received = 3;
        app.ping_timeouts = 1;
        assert!((app.ping_loss_pct() - 25.0).abs() < 1e-9);
    }

    // ── ping_interval stepping ─────────────────────────────────────────────

    #[test]
    fn ping_interval_increase_steps_up() {
        let app = App::new(); // starts at 1000 ms
        app.ping_interval_increase();
        assert_eq!(app.get_ping_interval_ms(), 2000);
    }

    #[test]
    fn ping_interval_decrease_steps_down() {
        let app = App::new(); // starts at 1000 ms
        app.ping_interval_decrease();
        assert_eq!(app.get_ping_interval_ms(), 500);
    }

    #[test]
    fn ping_interval_does_not_exceed_max() {
        let app = App::new();
        for _ in 0..10 {
            app.ping_interval_increase();
        }
        assert_eq!(app.get_ping_interval_ms(), 5000);
    }

    #[test]
    fn ping_interval_does_not_go_below_min() {
        let app = App::new();
        for _ in 0..10 {
            app.ping_interval_decrease();
        }
        assert_eq!(app.get_ping_interval_ms(), 100);
    }

    // ── speedtest_visible ──────────────────────────────────────────────────

    #[test]
    fn speedtest_visible_while_checking() {
        let app = App::new(); // speedtest_installed = None
        assert!(app.speedtest_visible());
    }

    #[test]
    fn speedtest_visible_when_installed() {
        let mut app = App::new();
        app.speedtest_installed = Some(true);
        assert!(app.speedtest_visible());
    }

    #[test]
    fn speedtest_hidden_when_not_installed() {
        let mut app = App::new();
        app.speedtest_installed = Some(false);
        assert!(!app.speedtest_visible());
    }
}
