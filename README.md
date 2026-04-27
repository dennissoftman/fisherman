# 🎣 fisherman

A terminal UI for network diagnostics — because debugging networks is a lot like fishing in the dark.

Built with [ratatui](https://github.com/ratatui-org/ratatui) and [tokio](https://tokio.rs) on macOS and Linux.

[![CI](https://github.com/dennissoftman/fisherman/actions/workflows/rust.yml/badge.svg)](https://github.com/dennissoftman/fisherman/actions/workflows/rust.yml)
![Rust](https://img.shields.io/badge/rust-2024_edition-orange?logo=rust)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-lightgrey?logo=apple)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

---

<table>
  <tr>
    <td align="center"><b>Ping</b></td>
    <td align="center"><b>DNS</b></td>
  </tr>
  <tr>
    <td><img src="demos/gifs/ping.gif" alt="Ping demo"/></td>
    <td><img src="demos/gifs/dns.gif" alt="DNS demo"/></td>
  </tr>
</table>

**Traceroute**
![Traceroute demo](demos/gifs/traceroute.gif)

---

## Features

| Tab | What it does |
|-----|--------------|
| **Dashboard** | Public IP, network type (Wi‑Fi / Ethernet), SSID, interface, private (LAN) IP, gateway, DNS servers. Auto-refreshes every 5 s; press `r` to force refresh. |
| **Speedtest** | Streams output from the [Ookla speedtest CLI](https://www.speedtest.net/apps/cli) in real time. *(optional — see below)* |
| **Ping** | Continuous ICMP ping with live statistics (sent / received / loss % / min / max / avg / stddev). Adjustable interval from 100 ms to 5 s. |
| **Traceroute** | Continuous MTR — repeatedly runs `traceroute -n` and accumulates per-hop stats (loss %, avg/best/last RTT, sparkline history). |
| **DNS** | Resolves A + AAAA records with latency measurement. Tristate filter: IPv4 only / IPv6 only / both. |

---

## Installation

### Homebrew (recommended)

```bash
brew tap dennissoftman/fisherman
brew install fisherman
```

To upgrade later: `brew upgrade fisherman`

### Build from source

Requires the [Rust toolchain](https://rustup.rs) (stable, 2024 edition):

```bash
git clone https://github.com/dennissoftman/fisherman
cd fisherman
cargo build --release
./target/release/fisherman
```

### Platform notes

> **Raw ICMP (Ping tab):** uses raw ICMP sockets. If you see `Socket error: Operation not permitted`, run with `sudo` or on Linux: `sudo setcap cap_net_raw+ep ./target/release/fisherman`.

> **Linux:** `iw` is needed for Wi-Fi SSID detection (`apt install iw` / `dnf install iw`). `traceroute` is usually pre-installed; if not: `apt install traceroute`.

### Optional: Ookla Speedtest CLI

The Speedtest tab requires the official [Ookla CLI](https://www.speedtest.net/apps/cli) (not the Python `speedtest-cli` package):

```bash
brew tap teamookla/speedtest
brew install speedtest
```

If the binary is not found, the tab shows an install prompt instead of crashing.

---

## Keybindings

The footer shows context-sensitive hints at runtime. Quick reference:

| Key | Context | Action |
|-----|---------|--------|
| `Tab` / `1`–`5` | Any | Cycle tabs / jump to tab |
| `q` / `Ctrl-C` | Any | Quit |
| `r` | Dashboard | Refresh |
| `i` / `Enter` | Ping, Traceroute, DNS (idle) | Enter input |
| `y` | Any (with results) | Copy current tab results to clipboard |
| `Enter` | Input mode | Execute |
| `s` | Ping, Traceroute (running) | Stop |
| `+` / `-` | Ping | Increase / decrease interval |
| `f` | DNS | Cycle filter: IPv4 → IPv6 → both |
| `Enter` / `s` | Speedtest | Start / stop |
| `Esc` | Input mode | Cancel |

---

## Contributing

Issues and PRs are welcome. See [CLAUDE.md](CLAUDE.md) for architecture notes and how to extend the app.
