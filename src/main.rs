mod app;
mod network;
mod ui;

use app::{App, AppMessage, InputMode, Tab};
use color_eyre::eyre::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, sync::Arc, time::Duration};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal).await;

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = App::new();

    let (msg_tx, mut msg_rx) = mpsc::channel::<AppMessage>(128);

    // ── Initial background fetches ─────────────────────────────────────────────
    {
        let tx = msg_tx.clone();
        tokio::spawn(async move {
            let ip = network::fetch_public_ip().await;
            let _ = tx.send(AppMessage::PublicIp(ip)).await;
        });
    }
    {
        let tx = msg_tx.clone();
        tokio::spawn(async move {
            let info = network::fetch_network_info().await;
            let _ = tx.send(AppMessage::NetworkInfo(info)).await;
        });
    }
    {
        let tx = msg_tx.clone();
        tokio::spawn(async move {
            let installed = network::check_speedtest_installed().await;
            let _ = tx.send(AppMessage::SpeedtestInstalled(installed)).await;
        });
    }

    // ── Crossterm event thread ─────────────────────────────────────────────────
    let (ev_tx, mut ev_rx) = mpsc::channel::<Event>(32);
    std::thread::spawn(move || {
        loop {
            match event::poll(Duration::from_millis(100)) {
                Ok(true) => match event::read() {
                    Ok(ev) => {
                        if ev_tx.blocking_send(ev).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                },
                Ok(false) => {}
                Err(_) => break,
            }
        }
    });

    // ── Periodic network-info refresh (detects Wi-Fi on/off) ──────────────────
    let mut net_ticker = tokio::time::interval(Duration::from_secs(5));
    net_ticker.tick().await; // consume the immediate first tick

    // ── Spinner ticker (200 ms) ─────────────────────────────────────────────
    let mut spin_ticker = tokio::time::interval(Duration::from_millis(200));
    spin_ticker.tick().await;

    // ── Main loop ──────────────────────────────────────────────────────────────
    loop {
        terminal.draw(|f| ui::render(f, &app))?;

        tokio::select! {
            Some(event) = ev_rx.recv() => {
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
                        handle_key(&mut app, key, &msg_tx).await;
                    }
                }
            }
            Some(msg) = msg_rx.recv() => {
                app.apply_message(msg);
            }
            _ = net_ticker.tick() => {
                let tx = msg_tx.clone();
                tokio::spawn(async move {
                    let info = network::fetch_network_info().await;
                    let _ = tx.send(AppMessage::NetworkInfo(info)).await;
                });
            }
            _ = spin_ticker.tick() => {
                app.spin_tick = app.spin_tick.wrapping_add(1);
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

async fn handle_key(app: &mut App, key: event::KeyEvent, msg_tx: &mpsc::Sender<AppMessage>) {
    match app.input_mode {
        // ── Normal mode ──────────────────────────────────────────────────────
        InputMode::Normal => match key.code {
            KeyCode::Char('q') => app.should_quit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.should_quit = true;
            }

            // Tab cycling
            KeyCode::Tab => {
                app.active_tab = match app.active_tab {
                    Tab::Dashboard if app.speedtest_visible() => Tab::Speedtest,
                    Tab::Dashboard => Tab::Ping,
                    Tab::Speedtest => Tab::Ping,
                    Tab::Ping => Tab::Traceroute,
                    Tab::Traceroute => Tab::Dns,
                    Tab::Dns => Tab::Dashboard,
                };
            }
            KeyCode::Char('1') => app.active_tab = Tab::Dashboard,
            // When speedtest is hidden, shift: 2=Ping, 3=Traceroute, 4=DNS
            KeyCode::Char('2') => {
                app.active_tab = if app.speedtest_visible() {
                    Tab::Speedtest
                } else {
                    Tab::Ping
                };
            }
            KeyCode::Char('3') => {
                app.active_tab = if app.speedtest_visible() {
                    Tab::Ping
                } else {
                    Tab::Traceroute
                };
            }
            KeyCode::Char('4') => {
                app.active_tab = if app.speedtest_visible() {
                    Tab::Traceroute
                } else {
                    Tab::Dns
                };
            }
            KeyCode::Char('5') if app.speedtest_visible() => app.active_tab = Tab::Dns,

            // Dashboard: refresh on 'r'
            KeyCode::Char('r') if app.active_tab == Tab::Dashboard => {
                app.public_ip = "Fetching…".to_string();
                app.network_info = None;
                let tx = msg_tx.clone();
                tokio::spawn(async move {
                    let ip = network::fetch_public_ip().await;
                    let _ = tx.send(AppMessage::PublicIp(ip)).await;
                });
                let tx = msg_tx.clone();
                tokio::spawn(async move {
                    let info = network::fetch_network_info().await;
                    let _ = tx.send(AppMessage::NetworkInfo(info)).await;
                });
            }

            // Ping: +/- to adjust interval
            KeyCode::Char('+') | KeyCode::Char('=') if app.active_tab == Tab::Ping => {
                app.ping_interval_increase();
            }
            KeyCode::Char('-') if app.active_tab == Tab::Ping => {
                app.ping_interval_decrease();
            }

            // Ping: stop if running, enter edit mode if idle
            KeyCode::Char('s') if app.active_tab == Tab::Ping && app.ping_running => {
                if let Some(tx) = app.ping_cancel_tx.take() {
                    let _ = tx.send(());
                }
            }
            KeyCode::Enter | KeyCode::Char('i')
                if app.active_tab == Tab::Ping && !app.ping_running =>
            {
                app.input_mode = InputMode::Editing;
            }

            // Traceroute: stop if running, enter edit mode if idle
            KeyCode::Char('s') if app.active_tab == Tab::Traceroute && app.traceroute_running => {
                if let Some(tx) = app.traceroute_cancel_tx.take() {
                    let _ = tx.send(());
                }
            }
            KeyCode::Enter | KeyCode::Char('i')
                if app.active_tab == Tab::Traceroute && !app.traceroute_running =>
            {
                app.input_mode = InputMode::Editing;
            }

            // DNS: cycle IP filter
            KeyCode::Char('f') if app.active_tab == Tab::Dns => {
                app.dns_ip_filter = app.dns_ip_filter.cycle();
            }

            // DNS: enter edit mode
            KeyCode::Enter | KeyCode::Char('i') if app.active_tab == Tab::Dns => {
                app.input_mode = InputMode::Editing;
            }

            // Speedtest: toggle start/stop
            KeyCode::Enter | KeyCode::Char('s') if app.active_tab == Tab::Speedtest => {
                dispatch_speedtest(app, msg_tx).await;
            }

            _ => {}
        },

        // ── Editing mode ─────────────────────────────────────────────────────
        InputMode::Editing => match key.code {
            KeyCode::Esc => {
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Enter => {
                app.input_mode = InputMode::Normal;
                dispatch_action(app, msg_tx).await;
            }
            KeyCode::Char(c) => app.handle_char(c),
            KeyCode::Backspace => app.handle_backspace(),
            KeyCode::Left => app.move_cursor_left(),
            KeyCode::Right => app.move_cursor_right(),
            KeyCode::Home => app.move_cursor_home(),
            KeyCode::End => app.move_cursor_end(),
            _ => {}
        },
    }
}

/// Dispatches after the user presses Enter in editing mode (Ping / DNS).
async fn dispatch_action(app: &mut App, msg_tx: &mpsc::Sender<AppMessage>) {
    match app.active_tab {
        Tab::Ping => {
            let host = app.ping_input.trim().to_string();
            if host.is_empty() || app.ping_running {
                return;
            }
            app.ping_running = true;
            app.ping_results.clear();
            app.ping_rtts.clear();
            app.ping_received = 0;
            app.ping_timeouts = 0;

            let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
            app.ping_cancel_tx = Some(cancel_tx);

            let tx = msg_tx.clone();
            let interval = Arc::clone(&app.ping_interval_ms);
            tokio::spawn(async move {
                network::stream_ping(host, tx, cancel_rx, interval).await;
            });
        }
        Tab::Traceroute => {
            let host = app.traceroute_input.trim().to_string();
            if host.is_empty() || app.traceroute_running {
                return;
            }
            app.traceroute_running = true;
            app.traceroute_results.clear();

            let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
            app.traceroute_cancel_tx = Some(cancel_tx);

            let tx = msg_tx.clone();
            tokio::spawn(async move {
                network::stream_traceroute(host, tx, cancel_rx).await;
            });
        }
        Tab::Dns => {
            let host = app.dns_input.trim().to_string();
            if host.is_empty() || app.dns_running {
                return;
            }
            app.dns_running = true;
            app.dns_results.clear();
            app.dns_latency_ms = None;
            let tx = msg_tx.clone();
            tokio::spawn(async move {
                let (ips, latency_ms) = network::run_dns(&host).await;
                let _ = tx.send(AppMessage::DnsResult(ips, latency_ms)).await;
                let _ = tx.send(AppMessage::DnsDone).await;
            });
        }
        Tab::Dashboard | Tab::Speedtest => {}
    }
}

/// Toggles speedtest start/stop.
async fn dispatch_speedtest(app: &mut App, msg_tx: &mpsc::Sender<AppMessage>) {
    if app.speedtest_installed != Some(true) {
        return;
    }
    if app.speedtest_running {
        if let Some(tx) = app.speedtest_cancel_tx.take() {
            let _ = tx.send(());
        }
    } else {
        app.speedtest_running = true;
        app.speedtest_lines.clear();

        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
        app.speedtest_cancel_tx = Some(cancel_tx);

        let tx = msg_tx.clone();
        tokio::spawn(async move {
            network::stream_speedtest(tx, cancel_rx).await;
        });
    }
}
