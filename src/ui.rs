use crate::app::{App, InputMode, NetType, Tab};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
};

// ── Entry point ────────────────────────────────────────────────────────────────

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    render_tabbar(f, app, root[0]);

    match app.active_tab {
        Tab::Dashboard => render_dashboard(f, app, root[1]),
        Tab::Ping => render_ping(f, app, root[1]),
        Tab::Traceroute => render_traceroute(f, app, root[1]),
        Tab::Dns => render_dns(f, app, root[1]),
        Tab::Speedtest => render_speedtest(f, app, root[1]),
    }

    render_footer(f, app, root[2]);
}

// ── Tab bar ────────────────────────────────────────────────────────────────────

fn render_tabbar(f: &mut Frame, app: &App, area: Rect) {
    let speedtest_visible = app.speedtest_visible();

    let idx = if speedtest_visible {
        match app.active_tab {
            Tab::Dashboard => 0,
            Tab::Speedtest => 1,
            Tab::Ping => 2,
            Tab::Traceroute => 3,
            Tab::Dns => 4,
        }
    } else {
        match app.active_tab {
            Tab::Dashboard => 0,
            Tab::Speedtest => 0, // shouldn't happen, fall back
            Tab::Ping => 1,
            Tab::Traceroute => 2,
            Tab::Dns => 3,
        }
    };

    let tab_labels: Vec<&str> = if speedtest_visible {
        vec![
            "  Dashboard  ",
            "  Speedtest  ",
            "  Ping  ",
            "  Traceroute  ",
            "  DNS  ",
        ]
    } else {
        vec!["  Dashboard  ", "  Ping  ", "  Traceroute  ", "  DNS  "]
    };

    let tabs = Tabs::new(tab_labels)
        .select(idx)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 🎣 fisherman ")
                .title_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, area);
}

// ── Dashboard ──────────────────────────────────────────────────────────────────

fn render_dashboard(f: &mut Frame, app: &App, area: Rect) {
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" Network Status ");
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let lbl = Style::default().fg(Color::DarkGray);
    let val_green = Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD);
    let val_cyan = Style::default().fg(Color::Cyan);
    let val_yellow = Style::default().fg(Color::Yellow);
    let val_white = Style::default().fg(Color::White);
    let val_gray = Style::default().fg(Color::DarkGray);

    let (type_str, name_str, iface_str, private_ip_str, gateway_str, dns_str) =
        match &app.network_info {
            Some(info) => (
                format!("{}", info.net_type),
                info.name.clone(),
                info.interface.clone(),
                info.private_ip.clone().unwrap_or_else(|| "—".into()),
                info.gateway_ip.clone().unwrap_or_else(|| "—".into()),
                if info.dns_servers.is_empty() {
                    "—".to_string()
                } else {
                    info.dns_servers.join(", ")
                },
            ),
            None => (
                "Detecting…".into(),
                "—".into(),
                "—".into(),
                "—".into(),
                "—".into(),
                "—".into(),
            ),
        };

    let type_icon = match app.network_info.as_ref().map(|i| &i.net_type) {
        Some(NetType::Wifi) => "  ",
        Some(NetType::Ethernet) => "  ",
        _ => "  ",
    };

    let lines: Vec<Line> = vec![
        Line::default(),
        Line::from(vec![
            Span::styled("  Public IP    : ", lbl),
            Span::styled(app.public_ip.as_str(), val_green),
            Span::styled("   (r to refresh)", val_gray),
        ]),
        Line::default(),
        Line::from(vec![
            Span::styled("  Network Type : ", lbl),
            Span::styled(type_icon, val_cyan),
            Span::styled(type_str.as_str(), val_cyan),
        ]),
        Line::from(vec![
            Span::styled("  Name / SSID  : ", lbl),
            Span::styled(name_str.as_str(), val_yellow),
        ]),
        Line::from(vec![
            Span::styled("  Interface    : ", lbl),
            Span::styled(iface_str.as_str(), val_white),
        ]),
        Line::from(vec![
            Span::styled("  Private IP   : ", lbl),
            Span::styled(private_ip_str.as_str(), val_white),
        ]),
        Line::from(vec![
            Span::styled("  Gateway      : ", lbl),
            Span::styled(gateway_str.as_str(), val_white),
        ]),
        Line::from(vec![
            Span::styled("  DNS Servers  : ", lbl),
            Span::styled(dns_str.as_str(), val_white),
        ]),
        Line::default(),
        Line::from(vec![
            Span::styled("  Speedtest    : ", lbl),
            match app.speedtest_installed {
                None => Span::styled("checking…", val_gray),
                Some(true) => Span::styled("installed  (tab 2)", val_green),
                Some(false) => Span::styled(
                    "not installed — brew tap teamookla/speedtest && brew install speedtest",
                    Style::default().fg(Color::Red),
                ),
            },
        ]),
    ];

    f.render_widget(Paragraph::new(lines), inner);
}

// ── Ping ───────────────────────────────────────────────────────────────────────

fn render_ping(f: &mut Frame, app: &App, area: Rect) {
    let outer = Block::default().borders(Borders::ALL).title(" Ping ");
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    // Layout: input(3) | hint(1) | stats(4) | log(rest)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(4),
            Constraint::Min(3),
        ])
        .split(inner);

    let editing = app.input_mode == InputMode::Editing && app.active_tab == Tab::Ping;
    let border_style = if editing {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // Input field
    let input_block = Block::default()
        .borders(Borders::ALL)
        .title(" Host / IP ")
        .border_style(border_style);
    f.render_widget(
        Paragraph::new(app.ping_input.as_str())
            .block(input_block)
            .style(Style::default().fg(Color::White)),
        chunks[0],
    );

    // Cursor
    if editing {
        let cx =
            (chunks[0].x + 1 + app.ping_cursor as u16).min(chunks[0].right().saturating_sub(2));
        f.set_cursor_position((cx, chunks[0].y + 1));
    }

    // Hint line
    let interval_label = format_interval(app.get_ping_interval_ms());
    let hint_text: String = if app.ping_running {
        format!(" ⟳  Pinging ({interval_label})   ·   Press s to stop   ·   +/-: adjust interval")
    } else if editing {
        " Enter → start continuous ping   ·   Esc → cancel".to_string()
    } else {
        format!(
            " Press i or Enter to type a host, then Enter to start   ·   interval: {interval_label} (+/-) "
        )
    };
    let hint_color = if app.ping_running {
        Color::Yellow
    } else {
        Color::DarkGray
    };
    f.render_widget(
        Paragraph::new(hint_text.as_str()).style(Style::default().fg(hint_color)),
        chunks[1],
    );

    // Stats block
    render_ping_stats(f, app, chunks[2]);

    // Log
    render_ping_log(f, app, chunks[3]);
}

fn render_ping_stats(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" Statistics ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let lbl = Style::default().fg(Color::DarkGray);
    let val = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let green = Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD);
    let red = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
    let cyan = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    let sent = app.ping_sent();
    let loss_pct = app.ping_loss_pct();
    let loss_style = if loss_pct > 10.0 { red } else { green };

    let row1 = Line::from(vec![
        Span::styled("  Sent: ", lbl),
        Span::styled(sent.to_string(), val),
        Span::styled("   Received: ", lbl),
        Span::styled(app.ping_received.to_string(), green),
        Span::styled("   Timeouts: ", lbl),
        Span::styled(
            app.ping_timeouts.to_string(),
            if app.ping_timeouts > 0 { red } else { val },
        ),
        Span::styled("   Loss: ", lbl),
        Span::styled(format!("{:.1}%", loss_pct), loss_style),
    ]);

    let row2 = if let Some((min, max, avg, stddev)) = app.ping_stats() {
        Line::from(vec![
            Span::styled("  Min: ", lbl),
            Span::styled(format!("{min:.2} ms"), cyan),
            Span::styled("   Max: ", lbl),
            Span::styled(format!("{max:.2} ms"), cyan),
            Span::styled("   Avg: ", lbl),
            Span::styled(format!("{avg:.2} ms"), cyan),
            Span::styled("   StdDev: ", lbl),
            Span::styled(format!("{stddev:.2} ms"), cyan),
        ])
    } else {
        Line::from(Span::styled(
            "  No RTT data yet",
            Style::default().fg(Color::DarkGray),
        ))
    };

    f.render_widget(Paragraph::new(vec![row1, row2]), inner);
}

fn render_ping_log(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" Reply Log ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.ping_results.is_empty() {
        f.render_widget(
            Paragraph::new("No output yet — enter a host above and press Enter.")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    let max = inner.height as usize;
    let skip = app.ping_results.len().saturating_sub(max);
    let items: Vec<ListItem> = app
        .ping_results
        .iter()
        .skip(skip)
        .map(|l| ListItem::new(l.as_str()).style(ping_line_style(l)))
        .collect();

    f.render_widget(List::new(items), inner);
}

fn ping_line_style(line: &str) -> Style {
    if line.contains("bytes from") {
        Style::default().fg(Color::Green)
    } else if line.contains("Request timeout")
        || line.contains("Unreachable")
        || line.to_lowercase().contains("error")
        || line.contains("No route")
    {
        Style::default().fg(Color::Red)
    } else if line.contains("round-trip") || line.contains("rtt") || line.contains("---") {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::White)
    }
}

// ── DNS ────────────────────────────────────────────────────────────────────────

fn render_dns(f: &mut Frame, app: &App, area: Rect) {
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" DNS Resolve ");
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(3),
        ])
        .split(inner);

    let editing = app.input_mode == InputMode::Editing && app.active_tab == Tab::Dns;
    let border_style = if editing {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let input_block = Block::default()
        .borders(Borders::ALL)
        .title(" Domain / Host ")
        .border_style(border_style);
    f.render_widget(
        Paragraph::new(app.dns_input.as_str())
            .block(input_block)
            .style(Style::default().fg(Color::White)),
        chunks[0],
    );

    if editing {
        let cx = (chunks[0].x + 1 + app.dns_cursor as u16).min(chunks[0].right().saturating_sub(2));
        f.set_cursor_position((cx, chunks[0].y + 1));
    }

    let hint_text: &str = if app.dns_running {
        " ⟳  Resolving…"
    } else if editing {
        " Enter → resolve   ·   Esc → cancel"
    } else {
        " Press i or Enter to edit   ·   f: cycle IP filter"
    };
    let hint_color = if app.dns_running {
        Color::Yellow
    } else {
        Color::DarkGray
    };
    f.render_widget(
        Paragraph::new(hint_text).style(Style::default().fg(hint_color)),
        chunks[1],
    );

    render_dns_results(f, app, chunks[2]);
}

fn render_dns_results(f: &mut Frame, app: &App, area: Rect) {
    let filter_label = app.dns_ip_filter.label();
    let title = if app.dns_running {
        " Resolving… ".to_string()
    } else if let Some(lat) = app.dns_latency_ms {
        format!(" Resolved IPs  ({lat:.0} ms)  │  filter: {filter_label} (f) ")
    } else {
        format!(" Resolved IPs  │  filter: {filter_label} (f) ")
    };
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.dns_results.is_empty() {
        let msg = if app.dns_running {
            "Resolving…"
        } else {
            "No results yet — enter a domain above and press Enter."
        };
        f.render_widget(
            Paragraph::new(msg)
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    let items: Vec<ListItem> = app
        .dns_results
        .iter()
        .filter(|ip| {
            let is_err = ip.to_lowercase().contains("error") || ip.starts_with("No records");
            if is_err {
                return true; // always show errors
            }
            let is_v6 = ip.contains(':');
            if is_v6 {
                app.dns_ip_filter.keeps_v6()
            } else {
                app.dns_ip_filter.keeps_v4()
            }
        })
        .map(|ip| {
            let is_v6 = ip.contains(':');
            let is_err = ip.to_lowercase().contains("error") || ip.starts_with("No records");
            if is_err {
                ListItem::new(format!("  {ip}")).style(Style::default().fg(Color::Red))
            } else if is_v6 {
                ListItem::new(format!("  ● {ip}  (IPv6)"))
                    .style(Style::default().fg(Color::Magenta))
            } else {
                ListItem::new(format!("  ● {ip}")).style(Style::default().fg(Color::Green))
            }
        })
        .collect();

    if items.is_empty() {
        f.render_widget(
            Paragraph::new(format!(
                "No {} addresses returned — press f to change filter",
                app.dns_ip_filter.label()
            ))
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    f.render_widget(List::new(items), inner);
}

// ── Traceroute ─────────────────────────────────────────────────────────────────

fn render_traceroute(f: &mut Frame, app: &App, area: Rect) {
    let outer = Block::default().borders(Borders::ALL).title(" Traceroute ");
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    // Layout: input(3) | hint(1) | output(rest)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(3),
        ])
        .split(inner);

    let editing = app.input_mode == InputMode::Editing && app.active_tab == Tab::Traceroute;
    let border_style = if editing {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let input_block = Block::default()
        .borders(Borders::ALL)
        .title(" Host / IP ")
        .border_style(border_style);
    f.render_widget(
        Paragraph::new(app.traceroute_input.as_str())
            .block(input_block)
            .style(Style::default().fg(Color::White)),
        chunks[0],
    );

    if editing {
        let cx = (chunks[0].x + 1 + app.traceroute_cursor as u16)
            .min(chunks[0].right().saturating_sub(2));
        f.set_cursor_position((cx, chunks[0].y + 1));
    }

    let hint_text: &str = if app.traceroute_running {
        " ⟳  Running traceroute…   ·   Press s to stop"
    } else if editing {
        " Enter → start traceroute   ·   Esc → cancel"
    } else {
        " Press i or Enter to type a host, then Enter to start"
    };
    let hint_color = if app.traceroute_running {
        Color::Yellow
    } else {
        Color::DarkGray
    };
    f.render_widget(
        Paragraph::new(hint_text).style(Style::default().fg(hint_color)),
        chunks[1],
    );

    // Output log
    let title = if app.traceroute_running {
        " Hops (running) "
    } else {
        " Hops "
    };
    let log_block = Block::default().borders(Borders::ALL).title(title);
    let log_inner = log_block.inner(chunks[2]);
    f.render_widget(log_block, chunks[2]);

    if app.traceroute_results.is_empty() {
        f.render_widget(
            Paragraph::new("No output yet — enter a host above and press Enter.")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center),
            log_inner,
        );
        return;
    }

    let max = log_inner.height as usize;
    let skip = app.traceroute_results.len().saturating_sub(max);
    let items: Vec<ListItem> = app
        .traceroute_results
        .iter()
        .skip(skip)
        .map(|l| ListItem::new(l.as_str()).style(traceroute_line_style(l)))
        .collect();

    f.render_widget(List::new(items), log_inner);
}

fn traceroute_line_style(line: &str) -> Style {
    let lc = line.to_lowercase();
    if lc.contains("traceroute to") || lc.starts_with("traceroute") {
        Style::default().fg(Color::Cyan)
    } else if lc.contains("* * *") || lc.contains("request timeout") {
        Style::default().fg(Color::Red)
    } else if lc.contains("error") || lc.contains("failed") || lc.contains("no route") {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        // Hop lines: dim the hop number, highlight the IP/time in white/green
        Style::default().fg(Color::Green)
    }
}

// ── Speedtest ─────────────────────────────────────────────────────────────────

fn render_speedtest(f: &mut Frame, app: &App, area: Rect) {
    let outer = Block::default().borders(Borders::ALL).title(" Speedtest ");
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    match app.speedtest_installed {
        // Still checking
        None => {
            f.render_widget(
                Paragraph::new("Checking for speedtest…")
                    .style(Style::default().fg(Color::DarkGray))
                    .alignment(Alignment::Center),
                inner,
            );
        }

        // Not installed
        Some(false) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(0),
                    Constraint::Length(5),
                    Constraint::Min(0),
                ])
                .split(inner);

            let msg_lines: Vec<Line> = vec![
                Line::from(Span::styled(
                    "  speedtest is not installed",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )),
                Line::default(),
                Line::from(vec![
                    Span::styled("  Install it from: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        "https://www.speedtest.net/apps/cli",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::UNDERLINED),
                    ),
                ]),
                Line::default(),
                Line::from(Span::styled(
                    "  (macOS: brew install speedtest  or  brew tap teamookla/speedtest && brew install speedtest)",
                    Style::default().fg(Color::DarkGray),
                )),
            ];

            f.render_widget(
                Paragraph::new(msg_lines).alignment(Alignment::Left),
                chunks[1],
            );
        }

        // Installed
        Some(true) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(3)])
                .split(inner);

            let hint_text = if app.speedtest_running {
                const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
                let frame = SPINNER[(app.spin_tick as usize) % SPINNER.len()];
                format!(" {frame}  Running speedtest…   ·   Press s or Enter to stop")
            } else {
                " Press Enter or s to run speedtest".to_string()
            };
            let hint_color = if app.speedtest_running {
                Color::Yellow
            } else {
                Color::DarkGray
            };
            f.render_widget(
                Paragraph::new(hint_text.as_str()).style(Style::default().fg(hint_color)),
                chunks[0],
            );

            render_speedtest_output(f, app, chunks[1]);
        }
    }
}

fn render_speedtest_output(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" Output ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.speedtest_lines.is_empty() {
        f.render_widget(
            Paragraph::new("No output yet — press Enter or s to run a speedtest.")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    let max = inner.height as usize;
    let skip = app.speedtest_lines.len().saturating_sub(max);
    let items: Vec<ListItem> = app
        .speedtest_lines
        .iter()
        .skip(skip)
        .map(|l| ListItem::new(l.as_str()).style(speedtest_line_style(l)))
        .collect();

    f.render_widget(List::new(items), inner);
}

fn speedtest_line_style(line: &str) -> Style {
    let lc = line.to_lowercase();
    if lc.contains("download") {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else if lc.contains("upload") {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else if lc.contains("latency") || lc.contains("ping") {
        Style::default().fg(Color::Yellow)
    } else if lc.contains("result url") || lc.contains("result:") {
        Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::UNDERLINED)
    } else if lc.contains("error") || lc.contains("failed") || lc.contains("cannot") {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::White)
    }
}

// ── Footer / help bar ──────────────────────────────────────────────────────────

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let interval_label = format_interval(app.get_ping_interval_ms());
    let tabs_hint = if app.speedtest_visible() {
        "Tab / 1-5: switch"
    } else {
        "Tab / 1-4: switch"
    };
    let ping_running_hint =
        format!(" s: stop ping  │  +/-: interval ({interval_label})  │  {tabs_hint}  │  q: quit ");
    let help: String = match (&app.input_mode, &app.active_tab) {
        (InputMode::Editing, _) => {
            " Enter: run  │  Esc: cancel  │  ←→: cursor  │  Home/End: jump ".to_string()
        }
        (_, Tab::Ping) if app.ping_running => ping_running_hint,
        (_, Tab::Speedtest) if app.speedtest_running => {
            format!(" s / Enter: stop speedtest  │  {tabs_hint}  │  q: quit ")
        }
        (_, Tab::Dashboard) => {
            format!(" r: refresh  │  {tabs_hint}  │  q / Ctrl-C: quit ")
        }
        (_, Tab::Ping) => format!(
            " +/-: interval ({interval_label})  │  i / Enter: start ping  │  {tabs_hint}  │  q: quit "
        ),
        (_, Tab::Dns) => format!(
            " f: cycle filter ({})  │  i / Enter: edit  │  {tabs_hint}  │  q: quit ",
            app.dns_ip_filter.label()
        ),
        (_, Tab::Traceroute) if !app.traceroute_running => {
            format!(" i / Enter: start traceroute  │  {tabs_hint}  │  q: quit ")
        }
        (_, Tab::Traceroute) => {
            format!(" s: stop  │  {tabs_hint}  │  q: quit ")
        }
        _ => format!(" {tabs_hint}  │  i / Enter: edit input  │  q: quit "),
    };

    f.render_widget(
        Paragraph::new(help.as_str())
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        area,
    );
}

pub(crate) fn format_interval(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms % 1000 == 0 {
        format!("{}s", ms / 1000)
    } else {
        format!("{:.1}s", ms as f64 / 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::format_interval;

    #[test]
    fn format_interval_milliseconds() {
        assert_eq!(format_interval(100), "100ms");
        assert_eq!(format_interval(500), "500ms");
        assert_eq!(format_interval(999), "999ms");
    }

    #[test]
    fn format_interval_whole_seconds() {
        assert_eq!(format_interval(1000), "1s");
        assert_eq!(format_interval(2000), "2s");
        assert_eq!(format_interval(5000), "5s");
    }

    #[test]
    fn format_interval_fractional_seconds() {
        assert_eq!(format_interval(1500), "1.5s");
        assert_eq!(format_interval(2500), "2.5s");
    }
}
