use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Bar, BarChart, BarGroup, Block, Borders, Cell, Paragraph, Row, Sparkline, Table, Tabs, Wrap,
};

use crate::app::{App, ConnFilter, Tab};
use crate::network::{format_bytes, format_rate};

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tabs
            Constraint::Min(0),   // body
            Constraint::Length(1), // status bar
        ])
        .split(frame.area());

    draw_tabs(frame, app, chunks[0]);

    match app.tab {
        Tab::Interfaces => draw_interfaces(frame, app, chunks[1]),
        Tab::Connections => draw_connections(frame, app, chunks[1]),
        Tab::Bandwidth => draw_bandwidth(frame, app, chunks[1]),
    }

    draw_status_bar(frame, app, chunks[2]);
}

fn draw_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = Tab::ALL
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let num = format!(" {} ", i + 1);
            Line::from(vec![
                Span::styled(num, Style::default().fg(Color::Yellow).bold()),
                Span::raw(t.title()),
                Span::raw(" "),
            ])
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" netstat "),
        )
        .select(app.tab.index())
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::raw("|"));

    frame.render_widget(tabs, area);
}

fn draw_interfaces(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(10)])
        .split(area);

    // Interface table.
    let header_cells = [
        "Interface", "RX Bytes", "TX Bytes", "RX Pkts", "TX Pkts", "RX Err", "TX Err", "RX Drop",
        "TX Drop", "RX Rate", "TX Rate",
    ]
    .iter()
    .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow).bold()));

    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows: Vec<Row> = app
        .data
        .interfaces
        .iter()
        .enumerate()
        .map(|(i, iface)| {
            let rate = app
                .data
                .rates
                .get(&iface.name)
                .cloned()
                .unwrap_or_default();

            let style = if i == app.iface_scroll {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else if i % 2 == 0 {
                Style::default()
            } else {
                Style::default().fg(Color::White)
            };

            Row::new(vec![
                Cell::from(iface.name.clone()),
                Cell::from(format_bytes(iface.rx_bytes)),
                Cell::from(format_bytes(iface.tx_bytes)),
                Cell::from(iface.rx_packets.to_string()),
                Cell::from(iface.tx_packets.to_string()),
                Cell::from(iface.rx_errors.to_string()),
                Cell::from(iface.tx_errors.to_string()),
                Cell::from(iface.rx_dropped.to_string()),
                Cell::from(iface.tx_dropped.to_string()),
                Cell::from(format_rate(rate.rx_bps)),
                Cell::from(format_rate(rate.tx_bps)),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Network Interfaces "),
    );

    frame.render_widget(table, chunks[0]);

    // Summary bar chart of traffic by interface.
    let bars: Vec<Bar> = app
        .data
        .interfaces
        .iter()
        .filter(|iface| iface.name != "lo")
        .map(|iface| {
            let total = iface.rx_bytes.saturating_add(iface.tx_bytes);
            let label = iface.name.clone();
            Bar::default()
                .label(Line::from(label))
                .value(total / 1024) // show in KB
                .style(Style::default().fg(Color::Green))
        })
        .collect();

    if !bars.is_empty() {
        let chart = BarChart::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Total Traffic (KB) "),
            )
            .data(BarGroup::default().bars(&bars))
            .bar_width(8)
            .bar_gap(2)
            .value_style(Style::default().fg(Color::Yellow).bold());

        frame.render_widget(chart, chunks[1]);
    }
}

fn draw_connections(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(area);

    // TCP state summary.
    let state_text = if app.data.tcp_state_counts.is_empty() {
        vec![Line::from("  No TCP connections")]
    } else {
        let mut pairs: Vec<(&String, &usize)> = app.data.tcp_state_counts.iter().collect();
        pairs.sort_by_key(|(k, _)| (*k).clone());
        let spans: Vec<Span> = pairs
            .iter()
            .flat_map(|(state, count)| {
                let color = match state.as_str() {
                    "ESTABLISHED" => Color::Green,
                    "LISTEN" => Color::Cyan,
                    "TIME_WAIT" => Color::Yellow,
                    "CLOSE_WAIT" => Color::Red,
                    "SYN_SENT" => Color::Magenta,
                    _ => Color::White,
                };
                vec![
                    Span::styled(
                        format!("  {state}: "),
                        Style::default().fg(color).bold(),
                    ),
                    Span::raw(format!("{count}")),
                    Span::raw("  "),
                ]
            })
            .collect();
        vec![Line::from(spans)]
    };

    let filter_label = match app.conn_filter {
        ConnFilter::All => "ALL",
        ConnFilter::Tcp => "TCP",
        ConnFilter::Udp => "UDP",
    };

    let summary = Paragraph::new(state_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" TCP State Summary  [f] Filter: {filter_label} ")),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(summary, chunks[0]);

    // Connection table.
    let header_cells = ["Proto", "Local Address", "Remote Address", "State", "UID", "Inode"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow).bold()));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let mut rows: Vec<Row> = Vec::new();

    if app.conn_filter != ConnFilter::Udp {
        for conn in &app.data.tcp_conns {
            let state_color = match conn.state.as_str() {
                "ESTABLISHED" => Color::Green,
                "LISTEN" => Color::Cyan,
                "TIME_WAIT" => Color::Yellow,
                "CLOSE_WAIT" => Color::Red,
                _ => Color::White,
            };

            rows.push(Row::new(vec![
                Cell::from("TCP"),
                Cell::from(conn.local_addr.clone()),
                Cell::from(conn.remote_addr.clone()),
                Cell::from(conn.state.clone()).style(Style::default().fg(state_color)),
                Cell::from(conn.uid.to_string()),
                Cell::from(conn.inode.to_string()),
            ]));
        }
    }

    if app.conn_filter != ConnFilter::Tcp {
        for sock in &app.data.udp_sockets {
            rows.push(Row::new(vec![
                Cell::from("UDP"),
                Cell::from(sock.local_addr.clone()),
                Cell::from(sock.remote_addr.clone()),
                Cell::from("-"),
                Cell::from(sock.uid.to_string()),
                Cell::from(sock.inode.to_string()),
            ]));
        }
    }

    let visible_rows: Vec<Row> = rows.into_iter().skip(app.conn_scroll).collect();
    let conn_count_label = match app.conn_filter {
        ConnFilter::All => {
            format!(
                "TCP: {} / UDP: {}",
                app.data.tcp_conns.len(),
                app.data.udp_sockets.len()
            )
        }
        ConnFilter::Tcp => format!("TCP: {}", app.data.tcp_conns.len()),
        ConnFilter::Udp => format!("UDP: {}", app.data.udp_sockets.len()),
    };

    let table = Table::new(
        visible_rows,
        [
            Constraint::Length(6),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Length(14),
            Constraint::Length(8),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Connections ({conn_count_label}) ")),
    );

    frame.render_widget(table, chunks[1]);
}

fn draw_bandwidth(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Total rate summary.
    let summary = Paragraph::new(Line::from(vec![
        Span::styled("  Total RX: ", Style::default().fg(Color::Green).bold()),
        Span::raw(format_rate(app.data.total_rx_bps)),
        Span::raw("    "),
        Span::styled("  Total TX: ", Style::default().fg(Color::Blue).bold()),
        Span::raw(format_rate(app.data.total_tx_bps)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Bandwidth Summary "),
    );

    frame.render_widget(summary, chunks[0]);

    // Sparklines for each interface.
    let iface_names: Vec<String> = app.data.interfaces.iter().map(|i| i.name.clone()).collect();
    if iface_names.is_empty() {
        return;
    }

    let selected = app.selected_iface.min(iface_names.len() - 1);
    let constraints: Vec<Constraint> = iface_names
        .iter()
        .map(|_| Constraint::Min(4))
        .collect();

    let iface_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(chunks[1]);

    for (i, name) in iface_names.iter().enumerate() {
        if i >= iface_chunks.len() {
            break;
        }

        let history = app.data.bandwidth_history.get(name);

        let rx_data: Vec<u64> = history
            .map(|h| h.iter().map(|(rx, _)| *rx as u64).collect())
            .unwrap_or_default();

        let border_style = if i == selected {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };

        let rate = app.data.rates.get(name).cloned().unwrap_or_default();
        let label = format!(
            " {name}  RX: {}  TX: {} ",
            format_rate(rate.rx_bps),
            format_rate(rate.tx_bps)
        );

        let inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(iface_chunks[i]);

        let rx_sparkline = Sparkline::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(label),
            )
            .data(&rx_data)
            .style(Style::default().fg(Color::Green));

        let tx_data: Vec<u64> = history
            .map(|h| h.iter().map(|(_, tx)| *tx as u64).collect())
            .unwrap_or_default();

        let tx_sparkline = Sparkline::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(format!(" {name} TX ")),
            )
            .data(&tx_data)
            .style(Style::default().fg(Color::Blue));

        frame.render_widget(rx_sparkline, inner[0]);
        frame.render_widget(tx_sparkline, inner[1]);
    }
}

fn draw_status_bar(frame: &mut Frame, _app: &App, area: Rect) {
    let text = Line::from(vec![
        Span::styled(" q", Style::default().fg(Color::Yellow).bold()),
        Span::raw(" Quit  "),
        Span::styled("Tab/1-3", Style::default().fg(Color::Yellow).bold()),
        Span::raw(" Switch Tab  "),
        Span::styled("\u{2191}\u{2193}/j/k", Style::default().fg(Color::Yellow).bold()),
        Span::raw(" Scroll  "),
        Span::styled("f", Style::default().fg(Color::Yellow).bold()),
        Span::raw(" Filter  "),
    ]);

    let bar = Paragraph::new(text).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(bar, area);
}
