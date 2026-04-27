use crate::app::{App, InputMode, MtrHop, NetType, Tab};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Sparkline, Tabs},
};

// ── IP redaction ───────────────────────────────────────────────────────────────

/// Replace an IP address with `aaa.bbb.ccc.ddd` where each `a/b/c/d` is a
/// randomly-looking shade block (█ ▓). The pattern is deterministic per
/// input character so it stays stable across redraws without needing `rand`.
fn redact_ip(ip: &str) -> String {
    const SHADES: [char; 2] = ['█', '▓'];
    let mut out = String::with_capacity(15);
    for (seg_idx, _seg) in ip.split('.').enumerate() {
        if seg_idx > 0 {
            out.push('.');
        }
        for char_idx in 0..3usize {
            // cheap deterministic "random": mix segment and char index
            let pick = (seg_idx * 7 + char_idx * 3 + seg_idx ^ char_idx) % 2;
            out.push(SHADES[pick]);
        }
    }
    out
}

// ── Toast helper ─────────────────────────────────────────────────────────────────

/// Returns "  ✓ Copied!" if the copy toast is active, empty string otherwise.
fn toast_str(app: &App) -> &'static str {
    if app
        .copy_toast
        .map(|t| t.elapsed().as_secs() < 2)
        .unwrap_or(false)
    {
        "  ✓ Copied!"
    } else {
        ""
    }
}

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
                .title(format!(" 🎣 fisherman v{} ", env!("CARGO_PKG_VERSION")))
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

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

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
            Span::styled(
                if app.hide_private {
                    redact_ip(&app.public_ip)
                } else {
                    app.public_ip.clone()
                },
                val_green,
            ),
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
            Span::styled(
                if app.hide_private {
                    redact_ip(&private_ip_str)
                } else {
                    private_ip_str
                },
                val_white,
            ),
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

    f.render_widget(Paragraph::new(lines), chunks[0]);

    f.render_widget(
        Paragraph::new(toast_str(app)).style(Style::default().fg(Color::Green)),
        chunks[1],
    );
}

// ── Ping ───────────────────────────────────────────────────────────────────────

fn render_ping(f: &mut Frame, app: &App, area: Rect) {
    let outer = Block::default().borders(Borders::ALL).title(" Ping ");
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    // Layout: input(3) | hint(1) | stats(4) | sparkline(5) | log(rest)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(4),
            Constraint::Length(5),
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
    let toast = toast_str(app);
    let hint_text: String = if app.ping_running {
        format!(
            " ⟳  Pinging ({interval_label})   ·   s to stop   ·   +/-: adjust interval   ·   y to copy{toast}"
        )
    } else if editing {
        " Enter → start continuous ping   ·   Esc → cancel".to_string()
    } else if !app.ping_results.is_empty() {
        format!(
            " Press i or Enter to type a host, then Enter to start   ·   interval: {interval_label} (+/-)   ·   y to copy{toast}"
        )
    } else {
        format!(
            " Press i or Enter to type a host, then Enter to start   ·   interval: {interval_label} (+/-)"
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

    // RTT sparkline
    render_ping_sparkline(f, app, chunks[3]);

    // Log
    render_ping_log(f, app, chunks[4]);
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

fn render_ping_sparkline(f: &mut Frame, app: &App, area: Rect) {
    // Trim to the number of bars that actually fit (inner width = area - 2 borders)
    let bar_capacity = area.width.saturating_sub(2) as usize;
    let data: Vec<u64> = app
        .ping_rtt_sparkline
        .iter()
        .copied()
        .rev()
        .take(bar_capacity)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    // Determine a sensible ceiling (at least 10 ms, or 150% of the visible max)
    let max_val = data.iter().copied().max().unwrap_or(0).max(10);
    let ceiling = (max_val as f64 * 1.5).ceil() as u64;

    // Color shifts based on avg of visible data
    let avg = if data.is_empty() {
        0.0
    } else {
        data.iter().sum::<u64>() as f64 / data.len() as f64
    };
    let spark_color = if avg <= 30.0 {
        Color::Green
    } else if avg <= 100.0 {
        Color::Yellow
    } else {
        Color::Red
    };

    let title = if data.is_empty() {
        " RTT History ".to_string()
    } else {
        format!(" RTT History  │  ceiling: {ceiling} ms ")
    };

    let sparkline = Sparkline::default()
        .block(Block::default().borders(Borders::ALL).title(title))
        .data(&data)
        .max(ceiling)
        .style(Style::default().fg(spark_color));

    f.render_widget(sparkline, area);
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

    let dns_toast = toast_str(app);
    let dns_hint_owned: String;
    let hint_text: &str = if app.dns_running {
        " ⟳  Resolving…"
    } else if editing {
        " Enter → resolve   ·   Esc → cancel"
    } else if !app.dns_results.is_empty() {
        dns_hint_owned = format!(
            " Press i or Enter to edit   ·   f: cycle IP filter   ·   y to copy{dns_toast}"
        );
        &dns_hint_owned
    } else {
        dns_hint_owned = " Press i or Enter to edit   ·   f: cycle IP filter".to_string();
        &dns_hint_owned
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

    let toast = toast_str(app);
    let hint_text: String = if app.traceroute_running {
        format!(" ⧳  Running continuous MTR…   ·   s to stop   ·   y to copy markdown{toast}")
    } else if editing {
        " Enter → start   ·   Esc → cancel".to_string()
    } else if !app.mtr_hops.is_empty() {
        format!(
            " Press i or Enter to type a host, then Enter to start   ·   y to copy markdown{toast}"
        )
    } else {
        " Press i or Enter to type a host, then Enter to start".to_string()
    };
    let hint_color = if app.traceroute_running {
        Color::Yellow
    } else {
        Color::DarkGray
    };
    f.render_widget(
        Paragraph::new(hint_text.as_str()).style(Style::default().fg(hint_color)),
        chunks[1],
    );

    // Hop visualization
    render_traceroute_hops(f, app, chunks[2]);
}

/// Render the MTR hop table with per-hop inline sparklines.
fn render_traceroute_hops(f: &mut Frame, app: &App, area: Rect) {
    let title = if app.traceroute_running {
        " Continuous MTR (running…) "
    } else {
        " Continuous MTR "
    };

    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.mtr_hops.is_empty() {
        f.render_widget(
            Paragraph::new("No output yet — enter a host above and press Enter.")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    // Fixed columns: TTL(5) + IP(23) + LOSS(8) + AVG(10) + BEST(10) + LAST(10) = 66
    let spark_width = inner.width.saturating_sub(66).max(8) as usize;

    let header = Line::from(Span::styled(
        format!(
            " {:<3}  {:<22} {:>6}  {:>8}  {:>8}  {:>8}  HISTORY",
            "TTL", "ADDRESS", "LOSS", "AVG", "BEST", "LAST"
        ),
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    ));

    let mut lines: Vec<Line> = vec![header];
    let redact_ip = if app.hide_private {
        Some(app.public_ip.as_str())
    } else {
        None
    };
    for hop in &app.mtr_hops {
        lines.push(render_mtr_hop_line(hop, spark_width, redact_ip));
    }

    if app.traceroute_running {
        lines.push(Line::from(Span::styled(
            " …",
            Style::default().fg(Color::Yellow),
        )));
    }

    let max = inner.height as usize;
    let skip = lines.len().saturating_sub(max);
    f.render_widget(
        Paragraph::new(lines.into_iter().skip(skip).collect::<Vec<_>>()),
        inner,
    );
}

fn render_mtr_hop_line(hop: &MtrHop, spark_width: usize, redact_ip: Option<&str>) -> Line<'static> {
    let loss_pct = hop.loss_pct();

    let ttl_span = Span::styled(
        format!(" {:>3}  ", hop.ttl),
        Style::default().fg(Color::DarkGray),
    );

    let raw_ip = hop.ip.as_deref().unwrap_or("*");
    let ip_str = match redact_ip {
        Some(pub_ip) if raw_ip == pub_ip => self::redact_ip(raw_ip),
        _ => raw_ip.to_string(),
    };
    let ip_color = if hop.received == 0 && hop.sent > 0 {
        Color::DarkGray
    } else {
        Color::Cyan
    };
    let ip_span = Span::styled(format!("{:<22} ", ip_str), Style::default().fg(ip_color));

    let loss_color = if loss_pct == 0.0 {
        Color::Green
    } else if loss_pct < 50.0 {
        Color::Yellow
    } else {
        Color::Red
    };
    let loss_span = Span::styled(
        format!("{:>5.1}%  ", loss_pct),
        Style::default().fg(loss_color),
    );

    let fmt_rtt = |v: Option<f64>| -> String {
        v.map(|ms| format!("{:>6.2}ms", ms))
            .unwrap_or_else(|| format!("{:>8}", "-"))
    };

    let avg = hop.avg_rtt();
    let avg_color = avg.map(rtt_color).unwrap_or(Color::DarkGray);
    let avg_span = Span::styled(
        format!("{}  ", fmt_rtt(avg)),
        Style::default().fg(avg_color),
    );
    let best_span = Span::styled(
        format!("{}  ", fmt_rtt(hop.best_rtt())),
        Style::default().fg(Color::Green),
    );
    let last_span = Span::styled(
        format!("{}  ", fmt_rtt(hop.last_rtt())),
        Style::default().fg(avg.map(rtt_color).unwrap_or(Color::DarkGray)),
    );

    // Inline sparkline using block elements ▁▂▃▄▅▆▇█
    let spark = hop.sparkline_data(spark_width);
    let max_val = spark.iter().copied().max().unwrap_or(1).max(1);
    let bar_chars = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let spark_str: String = spark
        .iter()
        .map(|&v| {
            if v == 0 {
                '░' // timeout / no data
            } else {
                let idx = ((v as f64 / max_val as f64) * 8.0).round() as usize;
                bar_chars[idx.min(8)]
            }
        })
        .collect();
    let spark_span = Span::styled(
        spark_str,
        Style::default().fg(avg.map(rtt_color).unwrap_or(Color::DarkGray)),
    );

    Line::from(vec![
        ttl_span, ip_span, loss_span, avg_span, best_span, last_span, spark_span,
    ])
}

fn rtt_color(ms: f64) -> Color {
    if ms < 20.0 {
        Color::Green
    } else if ms < 100.0 {
        Color::Yellow
    } else {
        Color::Red
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

            let speedtest_toast = toast_str(app);
            let hint_text = if app.speedtest_running {
                const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
                let frame = SPINNER[(app.spin_tick as usize) % SPINNER.len()];
                format!(
                    " {frame}  Running speedtest…   ·   s or Enter to stop   ·   y to copy{speedtest_toast}"
                )
            } else if !app.speedtest_lines.is_empty() {
                format!(" Press Enter or s to run speedtest   ·   y to copy{speedtest_toast}")
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
            format!(" r: refresh  │  y: copy  │  {tabs_hint}  │  q / Ctrl-C: quit ")
        }
        (_, Tab::Ping) => format!(
            " +/-: interval ({interval_label})  │  i / Enter: start ping{}  │  {tabs_hint}  │  q: quit ",
            if app.ping_results.is_empty() {
                ""
            } else {
                "  │  y: copy"
            }
        ),
        (_, Tab::Dns) => format!(
            " f: cycle filter ({})  │  i / Enter: edit{}  │  {tabs_hint}  │  q: quit ",
            app.dns_ip_filter.label(),
            if app.dns_results.is_empty() {
                ""
            } else {
                "  │  y: copy"
            }
        ),
        (_, Tab::Traceroute) if !app.traceroute_running => {
            format!(
                " i / Enter: start traceroute{}  │  {tabs_hint}  │  q: quit ",
                if app.mtr_hops.is_empty() {
                    ""
                } else {
                    "  │  y: copy"
                }
            )
        }
        (_, Tab::Traceroute) => {
            format!(" s: stop  │  y: copy  │  {tabs_hint}  │  q: quit ")
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
