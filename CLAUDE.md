# fisherman — Developer Notes

macOS network analysis TUI utility built with **ratatui** + **crossterm** on a **tokio** async runtime.

---

## Architecture

```
src/
  main.rs      Entry point, terminal setup/teardown, tokio runtime, event loop
  app.rs       All application state, message types, input handling helpers
  network.rs   Async network operations (public IP, interface detection, ping, DNS)
  ui.rs        All ratatui rendering (tab-bar, dashboard, ping, DNS, footer)
```

### Event / message flow

```
┌─ OS thread ──────────────────────────────────────────────────────────────┐
│  crossterm::event::poll / read (blocking)  →  mpsc::channel<Event>       │
└──────────────────────────────────────────────────────────────────────────┘
                    │
         tokio::select! in main loop
                    │
┌─ tokio tasks ───────────────────────────────────────────────────────────┐
│  fetch_public_ip()        →  AppMessage::PublicIp                        │
│  fetch_network_info()     →  AppMessage::NetworkInfo                     │
│  run_ping() (on demand)   →  AppMessage::PingLine × N + PingDone         │
│  run_dns()  (on demand)   →  AppMessage::DnsResult + DnsDone             │
└──────────────────────────────────────────────────────────────────────────┘
```

Background tasks send `AppMessage` values over a `tokio::sync::mpsc` channel. The
main loop applies them to `App` state before the next draw call.

---

## Features implemented

### Dashboard tab

- **Public IP** — GET `https://api.ipify.org` with a 5 s reqwest timeout. Displayed
  immediately on startup; shows "Fetching…" while pending and "Unavailable" on failure.
- **Network type & name** — macOS-specific:
  1. `route get default` → active interface name (e.g. `en0`)
  2. `networksetup -listallhardwareports` → classify as Wi-Fi / Ethernet / Other
  3. For Wi-Fi: `networksetup -getairportnetwork <iface>` → SSID
  4. For Ethernet: uses the hardware port label (e.g. "Thunderbolt Ethernet")

### Ping tab (`3` or `Tab`)

- Press `i` or `Enter` to enter edit mode; type a host or IP; `Enter` to **start
  continuous ping** (no `-c` flag — runs forever until stopped).
- Press `s` in Normal mode while ping is running to **stop** it.
- Uses **surge-ping** (raw ICMP via `surge_ping::Client`) — no OS subprocess. Resolves
  the hostname with `tokio::net::lookup_host` then sends ICMP echo requests in a loop.
- Each reply gives an RTT `Duration` directly; lines are formatted as
  `"N bytes from IP: icmp_seq=S ttl=T time=R ms"` so the stats parser can reuse them.
- **Statistics** panel shows live: Sent / Received / Timeouts / Loss% and
  Min / Max / Avg / StdDev (ms).
- Reply log: capped ring-buffer (`VecDeque`, 200 lines), auto-scrolls to bottom.
- Line coloring: green = reply, red = timeout/error, cyan = statistics summary.

### DNS tab (`4` or `Tab`)

- Same edit-mode flow as Ping.
- Uses `trust-dns-resolver 0.23` (`TokioAsyncResolver`) with default system resolver
  config; returns all A + AAAA records.
- IPv4 shown in green, IPv6 in magenta, errors in red.

### Speedtest tab (`2` or `Tab`)

- On startup, checks whether `speedtest` is on `$PATH` via `which speedtest`.
- If **not installed**: shows a red error message with the install URL
  `https://www.speedtest.net/apps/cli` and a brew hint.
- If **installed**: press `Enter` or `s` to start; press `s` or `Enter` again to stop.
- Output streams line-by-line (same subprocess streaming pattern as Ping).
- Line coloring: Download → green, Upload → cyan, Latency/Ping → yellow,
  Result URL → blue underline, errors → red.

---

## Keybindings

| Key                   | Context                    | Action                                           |
| --------------------- | -------------------------- | ------------------------------------------------ |
| `Tab`                 | Normal                     | Cycle through Dashboard → Speedtest → Ping → DNS |
| `1` / `2` / `3` / `4` | Normal                     | Jump to tab directly                             |
| `i` or `Enter`        | Normal (Ping, not running) | Enter editing mode                               |
| `s`                   | Normal (Ping, running)     | Stop continuous ping                             |
| `i` or `Enter`        | Normal (DNS)               | Enter editing mode                               |
| `Enter` or `s`        | Normal (Speedtest)         | Start / stop speedtest                           |
| `Esc`                 | Editing                    | Cancel / return to normal                        |
| `Enter`               | Editing                    | Execute ping or DNS resolve                      |
| `←` `→`               | Editing                    | Move cursor left/right                           |
| `Home` / `End`        | Editing                    | Jump to start/end of input                       |
| `Backspace`           | Editing                    | Delete character before cursor                   |
| `q`                   | Normal                     | Quit                                             |
| `Ctrl-C`              | Any                        | Quit                                             |

---

## Dependencies

| Crate                | Version           | Purpose                              |
| -------------------- | ----------------- | ------------------------------------ |
| `ratatui`            | 0.30              | TUI framework                        |
| `crossterm`          | 0.29              | Terminal backend + raw-mode          |
| `tokio`              | 1.52 (full)       | Async runtime, process spawn, timers |
| `reqwest`            | 0.13 (rustls-tls) | HTTP for ipify public-IP lookup      |
| `trust-dns-resolver` | 0.23              | Async DNS resolution                 |
| `surge-ping`         | 0.8               | Raw ICMP echo (no OS subprocess)     |
| `color-eyre`         | 0.6               | Error formatting                     |

---

## Subprocess streaming pattern

Both Ping and Speedtest use the same pattern for streaming subprocess output:

1. Spawn child with `stdout(Stdio::piped())` and `stderr(Stdio::piped())`.
2. Read stderr in a background tokio task (merged into the same message channel).
3. Read stdout line-by-line with `BufReader::lines()` in a `tokio::select!` loop.
4. The other select branch watches a `oneshot::Receiver<()>` — when it fires, call
   `child.kill().await` and break.
5. Always send `PingDone` / `SpeedtestDone` on exit so the UI clears the running flag.

Cancellation is triggered by taking `app.ping_cancel_tx` / `app.speedtest_cancel_tx`
out of the `Option` and calling `sender.send(())`.

---

## Known platform notes

- All network detection (`route`, `networksetup`) is **macOS-only**. On Linux, the
  dashboard's network section will show "Detecting…" permanently (graceful fallback).
- Ping uses the system `ping` binary; the output format is macOS BSD ping.
- Ping runs indefinitely (no `-c` flag). Press `s` to stop it.
- surge-ping creates a raw ICMP socket. On macOS this may require `sudo` if the system
  denies raw socket creation to regular users. The error `"Socket error: Operation not\n  permitted"` will appear in the reply log if permissions are insufficient.
- `speedtest` must be the Ookla CLI (`speedtest` binary), not `speedtest-cli` (the Python one).
  Both work but their output formats differ slightly.

---

## Extending the app

**Add a new tab:**

1. Add a variant to `Tab` in `app.rs`.
2. Add any new state fields to `App`.
3. Add a new `AppMessage` variant if background work is needed.
4. Add a render function in `ui.rs` and wire it up in `render()`.
5. Update the tab-bar labels in `render_tabbar()`.
6. Handle tab switching in `handle_key()` in `main.rs`.

**Add traceroute:**

- Run `traceroute -n <host>` via `tokio::process::Command`, stream lines the same way
  ping lines are streamed (one `AppMessage::TraceLine` per line).

**Persistent history:**

- `ping_results` is a `VecDeque<String>` — easy to serialize/persist between runs if
  needed.

**Tests:**

- `network.rs` functions are pure async; they can be tested with `#[tokio::test]` by
  mocking the subprocess output or running against localhost.
