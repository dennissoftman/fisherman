# fisherman — Developer Notes

Network analysis TUI utility for macOS and Linux, built with **ratatui** + **crossterm** on a **tokio** async runtime.

---

## Architecture

```
src/
  main.rs      Entry point, terminal setup/teardown, tokio runtime, event loop
  app.rs       All application state, message types, input handling helpers
  network.rs   Async network operations (public IP, interface detection, ping, DNS, traceroute)
  ui.rs        All ratatui rendering (tab-bar, dashboard, ping, DNS, traceroute, footer)
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
│  run_ping() (on demand)        →  AppMessage::PingLine × N + PingDone    │
│  stream_traceroute() (on demand) →  AppMessage::TracerouteLine × N + Done │
│  run_dns()  (on demand)        →  AppMessage::DnsResult + DnsDone         │
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

### Traceroute tab (`4` or `Tab`)

- Same edit-mode flow as Ping.
- Runs **continuous MTR** (`stream_mtr`): repeatedly spawns `traceroute -n -w 1 -q 1 <host>` in a loop (2 s between rounds), accumulating per-hop statistics.
- Each hop tracks: TTL, IP, sent/received probes, loss %, and a ring-buffer of RTT samples used to render a sparkline.
- Table columns: `TTL | ADDRESS | LOSS% | AVG | BEST | LAST | HISTORY (sparkline)`.
- Press `s` to stop mid-run.
- Press `y` to copy the current hop table as a Markdown table to the clipboard (shows a brief toast confirmation).
- Uses `AppMessage::MtrHopUpdate { ttl, ip, rtt }` and `AppMessage::MtrDone`.

### DNS tab (`5` or `Tab`)

- Same edit-mode flow as Ping.
- Uses `hickory-resolver 0.26` (`TokioResolver`) with default system resolver
  config; returns all A + AAAA records.
- Tristate filter: IPv4 only / IPv6 only / both (press `f` to cycle).
- IPv4 shown in green, IPv6 in magenta, errors in red.

### Speedtest tab (`2` or `Tab`)

- On startup, checks whether `speedtest` is on `$PATH` via `which speedtest`.
- If **not installed**: shows a red error message with the install URL
  `https://www.speedtest.net/apps/cli` and a brew hint.
- If **installed**: press `Enter` or `s` to start; press `s` or `Enter` again to stop.
- Output streams line-by-line (same subprocess streaming pattern as Traceroute).
- Line coloring: Download → green, Upload → cyan, Latency/Ping → yellow,
  Result URL → blue underline, errors → red.

---

## Keybindings

| Key                        | Context                         | Action                                                          |
| -------------------------- | ------------------------------- | --------------------------------------------------------------- |
| `Tab`                      | Normal                          | Cycle through Dashboard → Speedtest → Ping → Traceroute → DNS   |
| `1` / `2` / `3` / `4` / `5` | Normal                         | Jump to tab directly                                            |
| `r`                        | Normal (Dashboard)              | Force refresh public IP and network info                        |
| `i` or `Enter`             | Normal (Ping, not running)      | Enter editing mode                                              |
| `s`                        | Normal (Ping, running)          | Stop continuous ping                                            |
| `+` / `=`                  | Normal (Ping)                   | Increase ping interval                                          |
| `-`                        | Normal (Ping)                   | Decrease ping interval                                          |
| `i` or `Enter`             | Normal (Traceroute, not running) | Enter editing mode                                             |
| `s`                        | Normal (Traceroute, running)    | Stop traceroute                                                 |
| `y`                        | Normal (any tab, results available) | Copy current tab results to clipboard           |
| `i` or `Enter`             | Normal (DNS)                    | Enter editing mode                                              |
| `f`                        | Normal (DNS)                    | Cycle IP filter: IPv4 only → IPv6 only → both                   |
| `Enter` or `s`             | Normal (Speedtest)              | Start / stop speedtest                                          |
| `Esc`                      | Editing                         | Cancel / return to normal                                       |
| `Enter`                    | Editing                         | Execute action (ping / traceroute / DNS resolve)                |
| `←` `→`                   | Editing                         | Move cursor left/right                                          |
| `Home` / `End`             | Editing                         | Jump to start/end of input                                      |
| `Backspace`                | Editing                         | Delete character before cursor                                  |
| `q`                        | Normal                          | Quit                                                            |
| `Ctrl-C`                   | Any                             | Quit                                                            |

---

## Dependencies

| Crate                | Version           | Purpose                              |
| -------------------- | ----------------- | ------------------------------------ |
| `ratatui`            | 0.30              | TUI framework                        |
| `crossterm`          | 0.29              | Terminal backend + raw-mode          |
| `tokio`              | 1.52 (full)       | Async runtime, process spawn, timers |
| `reqwest`            | 0.13 (rustls-tls) | HTTP for ipify public-IP lookup      |
| `hickory-resolver`   | 0.26              | Async DNS resolution                 |
| `surge-ping`         | 0.8               | Raw ICMP echo (no OS subprocess)     |
| `clap`               | 4 (derive)        | CLI argument parsing (`-P` flag)     |
| `color-eyre`         | 0.6               | Error formatting                     |

---

## Subprocess streaming pattern

Traceroute and Speedtest use the same pattern for streaming subprocess output:

1. Spawn child with `stdout(Stdio::piped())` and `stderr(Stdio::piped())`.
2. Read stderr in a background tokio task (merged into the same message channel).
3. Read stdout line-by-line with `BufReader::lines()` in a `tokio::select!` loop.
4. The other select branch watches a `oneshot::Receiver<()>` — when it fires, call
   `child.kill().await` and break.
5. Always send `TracerouteDone` / `SpeedtestDone` on exit so the UI clears the running flag.

Cancellation is triggered by taking `app.traceroute_cancel_tx` / `app.speedtest_cancel_tx`
out of the `Option` and calling `sender.send(())`.

Note: Ping does **not** use a subprocess — it uses **surge-ping** (raw ICMP) directly.

---

## Known platform notes

- Network detection uses macOS commands (`route`, `networksetup`) on macOS and Linux equivalents (`ip route`, `iw`) on Linux.
- Ping uses **surge-ping** (raw ICMP via `surge_ping::Client`) — no OS subprocess. Requires raw socket permissions (see below).
- Ping runs indefinitely (no `-c` flag). Press `s` to stop it.
- surge-ping creates a raw ICMP socket. On macOS this may require `sudo` if the system
  denies raw socket creation to regular users. The error `"Socket error: Operation not\n  permitted"` will appear in the reply log if permissions are insufficient.
- `speedtest` must be the Ookla CLI (`speedtest` binary), not `speedtest-cli` (the Python one).
  Both work but their output formats differ slightly.
- `-P` / `--private` flag: redacts public and private IP addresses in the UI, replacing each
  digit with a deterministic shade block (`█` / `▓`), always shown as `aaa.bbb.ccc.ddd`
  regardless of actual digit count. Useful for demos and screenshots.

---

## Extending the app

**Add a new tab:**

1. Add a variant to `Tab` in `app.rs`.
2. Add any new state fields to `App`.
3. Add a new `AppMessage` variant if background work is needed.
4. Add a render function in `ui.rs` and wire it up in `render()`.
5. Update the tab-bar labels in `render_tabbar()`.
6. Handle tab switching in `handle_key()` in `main.rs`.

**Persistent history:**

- `ping_results` is a `VecDeque<String>` — easy to serialize/persist between runs if
  needed.

**Tests:**

- `network.rs` functions are pure async; they can be tested with `#[tokio::test]` by
  mocking the subprocess output or running against localhost.
