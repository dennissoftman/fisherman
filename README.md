# 🎣 fisherman

A terminal UI for network diagnostics — because debugging networks is a lot like fishing in the dark.

Built with [ratatui](https://github.com/ratatui-org/ratatui) and [tokio](https://tokio.rs) on macOS and Linux.

[![CI](https://github.com/dennissoftman/fisherman/actions/workflows/rust.yml/badge.svg)](https://github.com/dennissoftman/fisherman/actions/workflows/rust.yml)
![Rust](https://img.shields.io/badge/rust-2024_edition-orange?logo=rust)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-lightgrey?logo=apple)
![License](https://img.shields.io/badge/license-MIT-blue)

---

## Features

| Tab | What it does |
|-----|--------------|
| **Dashboard** | Public IP, network type (Wi‑Fi / Ethernet), SSID, interface, private (LAN) IP, gateway, DNS servers. Auto-refreshes every 5 s; press `r` to force refresh. |
| **Speedtest** | Streams output from the [Ookla speedtest CLI](https://www.speedtest.net/apps/cli) in real time. *(optional — see below)* |
| **Ping** | Continuous ICMP ping with live statistics (sent / received / loss % / min / max / avg / stddev). Adjustable interval from 100 ms to 5 s. |
| **Traceroute** | Streams `traceroute -n` output hop-by-hop to diagnose routing issues. |
| **DNS** | Resolves A + AAAA records with latency measurement. Tristate filter: IPv4 only / IPv6 only / both. |

---

## Installation

### Prerequisites

- macOS or Linux
- [Rust toolchain](https://rustup.rs) (stable, 2024 edition)

```bash
git clone https://github.com/your-username/fisherman
cd fisherman
cargo build --release
./target/release/fisherman
```

> **Raw ICMP note:** The Ping tab uses raw ICMP sockets. If you see `Socket error: Operation not permitted`, run with `sudo` (or on Linux: `sudo setcap cap_net_raw+ep ./target/release/fisherman`).

> **Linux extras:** `iw` is needed for Wi-Fi SSID detection (`apt install iw` / `dnf install iw`). `traceroute` is usually pre-installed; if not: `apt install traceroute`.

### Optional: Ookla Speedtest CLI

The Speedtest tab requires the official [Ookla CLI](https://www.speedtest.net/apps/cli) (not the Python `speedtest-cli` package):

```bash
# Homebrew (recommended)
brew tap teamookla/speedtest
brew install speedtest

# or direct download from https://www.speedtest.net/apps/cli
```

If the binary is not found, the tab shows an install prompt instead of crashing.

---

## Keybindings

### Global

| Key | Action |
|-----|--------|
| `Tab` | Cycle tabs: Dashboard → Speedtest → Ping → Traceroute → DNS |
| `1` – `5` | Jump directly to a tab (Speedtest tab is hidden and numbers shift down if not installed) |
| `q` / `Ctrl-C` | Quit |

### Dashboard

| Key | Action |
|-----|--------|
| `r` | Refresh public IP and network info |

### Ping

| Key | Action |
|-----|--------|
| `i` / `Enter` | Enter host input |
| `Enter` *(in input)* | Start continuous ping |
| `s` | Stop ping |
| `+` / `=` | Increase interval (100 ms → 200 → 500 → 1 s → 2 s → 3 s → 5 s) |
| `-` | Decrease interval |
| `Esc` | Cancel input |

### Traceroute

| Key | Action |
|-----|--------|
| `i` / `Enter` | Enter host input |
| `Enter` *(in input)* | Start traceroute |
| `s` | Stop traceroute |
| `Esc` | Cancel input |

### DNS

| Key | Action |
|-----|--------|
| `i` / `Enter` | Enter domain input |
| `Enter` *(in input)* | Resolve |
| `f` | Cycle IP filter: **IPv4 only** → IPv6 only → IPv4 + IPv6 |
| `Esc` | Cancel input |

### Speedtest

| Key | Action |
|-----|--------|
| `Enter` / `s` | Start / stop speedtest |

---

## Project structure

```
src/
  main.rs      Entry point, terminal setup, tokio runtime, event loop
  app.rs       Application state, message types, input helpers
  network.rs   Async network ops (public IP, interfaces, ping, DNS, traceroute)
  ui.rs        ratatui rendering (all tabs, tabbar, footer)
```

---

## License

MIT
