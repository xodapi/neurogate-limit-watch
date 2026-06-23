use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Sparkline};
use ratatui::Terminal;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::io;
use std::time::{Duration, Instant};

use neurogate_limit_watch::{self as ng, VERSION};

use super::args::{Args, Preset};
use super::notify::Notifier;

const SPARKLINE_LEN: usize = 20;

#[allow(dead_code)]
pub struct StatusSnapshot {
    pub windows: Vec<ng::WindowState>,
    pub abtop: Option<Value>,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
}

pub fn collect_status(
    args: &Args,
    config: &ng::RuntimeConfig,
    http: &ng::HttpClient,
) -> Result<StatusSnapshot, String> {
    let payload = if args.demo {
        ng::demo_payload()
    } else if let Some(path) = &args.mock {
        ng::load_mock(path)?
    } else {
        http.fetch_me(&config.api_key, &config.api_base)?
    };

    let windows = ng::summarize_me_with_thresholds(
        &payload,
        args.warning_threshold,
        args.danger_threshold,
        &args.window_thresholds,
    );
    let abtop = if args.with_abtop {
        ng::read_abtop_status(&config.abtop_bin)
    } else {
        None
    };

    Ok(StatusSnapshot {
        windows,
        abtop,
        fetched_at: chrono::Utc::now(),
    })
}

pub fn run_monitor(args: &Args, notifier: &mut Notifier) -> Result<i32, String> {
    let interval_secs = monitor_interval(args);
    let interval = Duration::from_secs(interval_secs);

    crossterm::terminal::enable_raw_mode()
        .map_err(|error| format!("cannot enable terminal raw mode: {error}"))?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)
        .map_err(|error| format!("cannot enter alternate screen: {error}"))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal =
        Terminal::new(backend).map_err(|error| format!("cannot initialize terminal: {error}"))?;

    let _guard = TerminalGuard;

    let mut last_snapshot = None::<StatusSnapshot>;
    let mut last_error = None::<String>;
    let mut force_refresh = true;
    let mut next_refresh = Instant::now();
    let mut window_history: HashMap<&str, WindowHistory> = HashMap::new();
    let http = ng::HttpClient::new(ng::USER_AGENT)?;

    loop {
        let now = Instant::now();
        if force_refresh || now >= next_refresh {
            match load_config(args).and_then(|config| collect_status(args, &config, &http)) {
                Ok(snapshot) => {
                    for window in &snapshot.windows {
                        let hist = window_history
                            .entry(window.key)
                            .or_insert_with(WindowHistory::new);
                        let peak =
                            ng::peak_percent(window.credits.as_ref(), window.requests.as_ref())
                                .unwrap_or(0.0);
                        hist.record(peak);
                    }
                    notifier.check_windows(&snapshot.windows);
                    last_snapshot = Some(snapshot);
                    last_error = None;
                }
                Err(error) => {
                    last_error = Some(error);
                }
            }
            next_refresh = Instant::now() + interval;
            force_refresh = false;
        }

        let next_refresh_secs = next_refresh
            .saturating_duration_since(Instant::now())
            .as_secs();

        terminal
            .draw(|frame| {
                draw_frame(
                    frame,
                    last_snapshot.as_ref(),
                    last_error.as_deref(),
                    interval_secs,
                    next_refresh_secs,
                    args.with_abtop,
                    args.warning_threshold,
                    &window_history,
                    args.preset,
                );
            })
            .map_err(|error| format!("cannot draw terminal frame: {error}"))?;

        if event::poll(Duration::from_millis(200))
            .map_err(|error| format!("cannot read terminal events: {error}"))?
        {
            match event::read().map_err(|error| format!("cannot read terminal event: {error}"))? {
                Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(0),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(130);
                    }
                    KeyCode::Char('r') => force_refresh = true,
                    _ => {}
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_frame(
    frame: &mut ratatui::Frame,
    snapshot: Option<&StatusSnapshot>,
    error: Option<&str>,
    interval_secs: u64,
    next_refresh_secs: u64,
    with_abtop: bool,
    warning_threshold: f64,
    window_history: &HashMap<&str, WindowHistory>,
    preset: Preset,
) {
    let area = frame.area();
    let (header_len, footer_len) = match preset {
        Preset::Mini => (1, 1),
        Preset::Compact => (2, 1),
        Preset::Full => (3, 3),
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_len),
            Constraint::Min(4),
            Constraint::Length(footer_len),
        ])
        .split(area);

    draw_header(frame, chunks[0], snapshot);
    draw_body(
        frame,
        chunks[1],
        snapshot,
        error,
        with_abtop,
        warning_threshold,
        window_history,
        preset,
    );
    draw_footer(frame, chunks[2], interval_secs, next_refresh_secs);
}

fn draw_header(frame: &mut ratatui::Frame, area: Rect, snapshot: Option<&StatusSnapshot>) {
    let (title, style) = match snapshot {
        Some(s) => {
            let level = ng::worst_level(&s.windows);
            let peak = ng::peak_percent_all(&s.windows)
                .map(|v| format!("{v:.0}%"))
                .unwrap_or_else(|| "n/a".into());
            let (color, label) = if level == "danger" {
                (Color::Red, "DANGER")
            } else if level == "warning" {
                (Color::Yellow, "WARNING")
            } else {
                (Color::Green, "OK")
            };
            (
                format!(" NeuroGate v{VERSION} | {label} | peak {peak} "),
                Style::default().fg(color),
            )
        }
        None => (
            format!(" NeuroGate v{VERSION} | waiting for data... "),
            Style::default().fg(Color::DarkGray),
        ),
    };

    let header = Paragraph::new(Line::from(Span::styled(title, style)))
        .block(Block::default().borders(Borders::ALL).border_style(style));
    frame.render_widget(header, area);
}

#[allow(clippy::too_many_arguments)]
fn draw_body(
    frame: &mut ratatui::Frame,
    area: Rect,
    snapshot: Option<&StatusSnapshot>,
    error: Option<&str>,
    _with_abtop: bool,
    warning_threshold: f64,
    window_history: &HashMap<&str, WindowHistory>,
    preset: Preset,
) {
    let (alerts_height, cols) = match preset {
        Preset::Mini => (1, 1),
        Preset::Compact => (2, 1),
        Preset::Full => (5, 2),
    };
    let body_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(alerts_height)])
        .split(area);

    let windows_area = body_chunks[0];
    let alerts_area = body_chunks[1];

    if let Some(snapshot) = snapshot {
        let rows = (snapshot.windows.len() as u16).div_ceil(cols).max(1);
        let mut row_constraints = Vec::new();
        for _ in 0..rows {
            row_constraints.push(Constraint::Percentage(100 / rows));
        }

        let grid = Layout::default()
            .direction(Direction::Vertical)
            .constraints(row_constraints)
            .split(windows_area);

        for (i, window) in snapshot.windows.iter().enumerate() {
            let row = (i as u16) / cols;
            let col = (i as u16) % cols;
            let cell = if cols == 1 {
                grid[row as usize]
            } else {
                let col_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50); 2])
                    .split(grid[row as usize]);
                col_chunks[col as usize]
            };
            draw_window_card(
                frame,
                cell,
                window,
                window_history.get(window.key),
                warning_threshold,
                preset,
            );
        }
    } else {
        let waiting = Paragraph::new("Collecting NeuroGate status...")
            .block(Block::default().title("Limits").borders(Borders::ALL));
        frame.render_widget(waiting, windows_area);
    }

    draw_alerts(frame, alerts_area, snapshot, warning_threshold);

    if let Some(error) = error {
        let error_area = Rect {
            y: area.y + area.height.saturating_sub(1),
            height: 1,
            ..area
        };
        let err_widget = Paragraph::new(Span::styled(
            format!(" ! {error}"),
            Style::default().fg(Color::Red),
        ));
        frame.render_widget(err_widget, error_area);
    }
}

fn draw_window_card(
    frame: &mut ratatui::Frame,
    area: Rect,
    window: &ng::WindowState,
    history: Option<&WindowHistory>,
    _warning_threshold: f64,
    preset: Preset,
) {
    let peak = ng::peak_percent(window.credits.as_ref(), window.requests.as_ref()).unwrap_or(0.0);

    let (border_color, title_style) = match window.level.as_str() {
        "danger" => (
            Color::Red,
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        "warning" => (
            Color::Yellow,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        _ => (Color::Green, Style::default().fg(Color::Green)),
    };

    let reset_text = ng::format_duration_opt(window.reset_in_seconds);

    match preset {
        Preset::Mini => {
            let line = Line::from(vec![
                Span::styled(
                    format!("{} ", window.key),
                    Style::default()
                        .fg(border_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{:.0}%", peak), Style::default().fg(border_color)),
                Span::raw(format!(" reset {}", reset_text)),
            ]);
            let widget = Paragraph::new(line).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color)),
            );
            frame.render_widget(widget, area);
        }
        Preset::Compact => {
            let title = format!(" {} | {} ", window.key, window.level);
            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(1)])
                .margin(1)
                .split(area);

            let gauge = Gauge::default()
                .block(
                    Block::default()
                        .title(title)
                        .title_style(title_style)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(border_color)),
                )
                .gauge_style(Style::default().fg(border_color))
                .ratio((peak / 100.0) as f64);
            frame.render_widget(gauge, inner[0]);

            let credit_line = match &window.credits {
                Some(m) => Line::from(Span::raw(format!(
                    "cr {}/{} ({:.0}%)",
                    ng::short_number(m.used),
                    ng::short_number(m.limit),
                    m.percent
                ))),
                None => Line::from(Span::raw("cr n/a")),
            };
            let request_line = match &window.requests {
                Some(m) => Line::from(Span::raw(format!(
                    "rq {}/{} ({:.0}%)",
                    ng::short_number(m.used),
                    ng::short_number(m.limit),
                    m.percent
                ))),
                None => Line::from(Span::raw("rq n/a")),
            };
            let metrics = Paragraph::new(vec![credit_line, request_line]);
            frame.render_widget(metrics, inner[1]);
        }
        Preset::Full => {
            let title = format!(" {} | {} | reset {} ", window.key, window.level, reset_text);

            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(2),
                    Constraint::Min(2),
                ])
                .margin(1)
                .split(area);

            let gauge = Gauge::default()
                .block(
                    Block::default()
                        .title(title)
                        .title_style(title_style)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(border_color)),
                )
                .gauge_style(Style::default().fg(border_color))
                .ratio((peak / 100.0) as f64);
            frame.render_widget(gauge, inner[0]);

            let credit_line = match &window.credits {
                Some(m) => Line::from(vec![
                    Span::styled("cr ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!(
                        "{}/{} ({:.0}%)",
                        ng::short_number(m.used),
                        ng::short_number(m.limit),
                        m.percent
                    )),
                ]),
                None => Line::from(Span::styled("cr n/a", Style::default().fg(Color::DarkGray))),
            };
            let request_line = match &window.requests {
                Some(m) => Line::from(vec![
                    Span::styled("rq ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!(
                        "{}/{} ({:.0}%)",
                        ng::short_number(m.used),
                        ng::short_number(m.limit),
                        m.percent
                    )),
                ]),
                None => Line::from(Span::styled("rq n/a", Style::default().fg(Color::DarkGray))),
            };
            let metrics = Paragraph::new(vec![credit_line, request_line]);
            frame.render_widget(metrics, inner[1]);

            if let Some(hist) = history {
                let values = hist.sparkline_values();
                if !values.is_empty() {
                    let spark = Sparkline::default()
                        .block(Block::default().title("history"))
                        .data(&values)
                        .style(Style::default().fg(border_color));
                    frame.render_widget(spark, inner[2]);
                }
            }
        }
    }
}

fn draw_alerts(
    frame: &mut ratatui::Frame,
    area: Rect,
    snapshot: Option<&StatusSnapshot>,
    warning_threshold: f64,
) {
    let alerts = match snapshot {
        Some(s) => monitor_alerts(&s.windows, warning_threshold),
        None => vec!["waiting for data".into()],
    };

    let block = Block::default()
        .title(" alerts ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    if alerts.is_empty() {
        let ok = Paragraph::new(Span::styled(
            "all windows below threshold",
            Style::default().fg(Color::Green),
        ))
        .block(block);
        frame.render_widget(ok, area);
    } else {
        let lines: Vec<Line> = alerts
            .iter()
            .map(|alert| {
                let style = if alert.starts_with("danger") {
                    Style::default().fg(Color::Red)
                } else if alert.starts_with("warning") {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };
                Line::from(Span::styled(alert.clone(), style))
            })
            .collect();
        let widget = Paragraph::new(lines).block(block);
        frame.render_widget(widget, area);
    }
}

fn draw_footer(frame: &mut ratatui::Frame, area: Rect, interval_secs: u64, next_refresh_secs: u64) {
    let height = area.height;
    let footer = if height <= 1 {
        Paragraph::new(Line::from(vec![Span::raw(format!(
            "q quit | r refresh | {interval_secs}s/{next_refresh_secs}s"
        ))]))
    } else {
        Paragraph::new(Line::from(vec![
            Span::styled(
                " q ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("quit  "),
            Span::styled(
                " r ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "refresh  auto {interval_secs}s  next {next_refresh_secs}s"
            )),
        ]))
        .block(Block::default().borders(Borders::ALL))
    };
    frame.render_widget(footer, area);
}

fn load_config(args: &Args) -> Result<ng::RuntimeConfig, String> {
    ng::RuntimeConfig::from_dotenv(
        args.api_base.clone(),
        &args.api_key_env,
        args.env_file.as_ref(),
    )
}

fn monitor_interval(args: &Args) -> u64 {
    if args.watch == 0 {
        5
    } else {
        args.watch.max(1)
    }
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen);
    }
}

struct WindowHistory {
    values: VecDeque<f64>,
}

impl WindowHistory {
    fn new() -> Self {
        Self {
            values: VecDeque::with_capacity(SPARKLINE_LEN),
        }
    }

    fn record(&mut self, peak: f64) {
        if self.values.len() >= SPARKLINE_LEN {
            self.values.pop_front();
        }
        self.values.push_back(peak);
    }

    fn sparkline_values(&self) -> Vec<u64> {
        self.values.iter().map(|v| *v as u64).collect()
    }
}

// ── plain-text rendering (used by tests and --once mode) ──────────────

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub fn render_monitor(
    snapshot: Option<&StatusSnapshot>,
    error: Option<&str>,
    width: u16,
    height: u16,
    interval_secs: u64,
    next_refresh_secs: u64,
    with_abtop: bool,
    warning_threshold: f64,
) -> String {
    render_monitor_lines(
        snapshot,
        error,
        width,
        height,
        interval_secs,
        next_refresh_secs,
        with_abtop,
        warning_threshold,
    )
    .join("\r\n")
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub fn render_monitor_lines(
    snapshot: Option<&StatusSnapshot>,
    error: Option<&str>,
    width: u16,
    height: u16,
    interval_secs: u64,
    next_refresh_secs: u64,
    with_abtop: bool,
    warning_threshold: f64,
) -> Vec<String> {
    let width = usize::from(width.max(20));
    let max_lines = usize::from(height.max(10));
    let mut lines = Vec::<String>::new();
    let now = chrono::Utc::now().format("%H:%M:%S UTC");
    let title = match snapshot {
        Some(snapshot) => {
            let level = ng::worst_level(&snapshot.windows);
            let peak = ng::peak_percent_all(&snapshot.windows)
                .map(|value| format!("{value:.0}%"))
                .unwrap_or_else(|| "n/a".to_string());
            let agents = abtop_agent_summary(snapshot.abtop.as_ref());
            format!(
                "nglimit v{VERSION}  NeuroGate monitor  quota:{level} peak:{peak}  {agents}  {now}"
            )
        }
        None => format!("nglimit v{VERSION}  NeuroGate monitor  waiting for first refresh  {now}"),
    };
    lines.push(fit_text(&title, width));

    if let Some(error) = error {
        lines.push(panel_top("last error", width));
        lines.push(panel_line(error, width));
        lines.push(panel_line(
            "keeping the dashboard open; next refresh may recover",
            width,
        ));
        lines.push(panel_bottom(width));
    }

    lines.push(panel_top("neurogate quota", width));
    if let Some(snapshot) = snapshot {
        lines.push(panel_line(
            &format!(
                "fetched {} | refresh every {}s | next in {}s",
                snapshot.fetched_at.format("%H:%M:%S UTC"),
                interval_secs,
                next_refresh_secs
            ),
            width,
        ));
        if snapshot.windows.is_empty() {
            lines.push(panel_line("usage rows not found in /v1/me response", width));
        }
        for window in &snapshot.windows {
            let peak = ng::peak_percent(window.credits.as_ref(), window.requests.as_ref())
                .map(|value| format!("{value:.1}%"))
                .unwrap_or_else(|| "n/a".to_string());
            let bar = hbar(
                ng::peak_percent(window.credits.as_ref(), window.requests.as_ref()).unwrap_or(0.0),
                bar_width(width),
            );
            lines.push(panel_line(
                &format!(
                    "{:<4} {:<7} {:<30} peak {:>6} reset {}",
                    window.key,
                    window.level,
                    bar,
                    peak,
                    ng::format_duration_opt(window.reset_in_seconds)
                ),
                width,
            ));
            lines.push(panel_line(
                &format!(
                    "     {} | {}",
                    monitor_metric("credits", window.credits.as_ref()),
                    monitor_metric("requests", window.requests.as_ref())
                ),
                width,
            ));
        }
    } else {
        lines.push(panel_line("collecting NeuroGate status...", width));
    }
    lines.push(panel_bottom(width));

    lines.push(panel_top("alerts", width));
    match snapshot {
        Some(snapshot) => {
            let alerts = monitor_alerts(&snapshot.windows, warning_threshold);
            if alerts.is_empty() {
                lines.push(panel_line(
                    "all monitored windows are below the warning threshold",
                    width,
                ));
            } else {
                for alert in alerts {
                    lines.push(panel_line(&alert, width));
                }
            }
        }
        None => lines.push(panel_line("waiting for data", width)),
    }
    lines.push(panel_bottom(width));

    lines.push(panel_top("local agents", width));
    if with_abtop {
        match snapshot.and_then(|snapshot| snapshot.abtop.as_ref()) {
            Some(abtop) => {
                let token_rate = abtop
                    .get("token_rate")
                    .and_then(ng::to_number)
                    .map(|value| format!("{value:.1}/min"))
                    .unwrap_or_else(|| "n/a".to_string());
                let sessions_total = ng::value_string(abtop.get("sessions_total"))
                    .unwrap_or_else(|| "?".to_string());
                let sessions_active = ng::value_string(abtop.get("sessions_active"))
                    .unwrap_or_else(|| "?".to_string());
                lines.push(panel_line(
                    &format!(
                        "source abtop --status-json | token rate {token_rate} | sessions {sessions_total} active {sessions_active}"
                    ),
                    width,
                ));
                if let Some(agents) = abtop.get("agents").and_then(Value::as_array) {
                    if agents.is_empty() {
                        lines.push(panel_line(
                            "no active local agents reported by abtop",
                            width,
                        ));
                    } else {
                        lines.push(panel_line(
                            "CLI      sessions active waiting ctx-max total-tokens active-tokens turns",
                            width,
                        ));
                        for agent in agents {
                            lines.push(panel_line(&monitor_agent(agent), width));
                        }
                    }
                } else {
                    lines.push(panel_line("abtop payload does not include agents[]", width));
                }
            }
            None => lines.push(panel_line(
                "abtop status is not available; set ABTOP_BIN or remove --with-abtop",
                width,
            )),
        }
    } else {
        lines.push(panel_line(
            "run with --with-abtop to add Codex/Claude session context from abtop",
            width,
        ));
    }
    lines.push(panel_bottom(width));

    lines.push(fit_text(
        &format!(
            "q quit | Esc quit | r refresh now | auto {}s | next {}s | .env next to binary supported",
            interval_secs, next_refresh_secs
        ),
        width,
    ));

    lines
        .into_iter()
        .take(max_lines)
        .map(|line| fit_text(&line, width))
        .collect::<Vec<_>>()
}

#[cfg(test)]
pub fn panel_top(title: &str, width: usize) -> String {
    let label = format!(" {title} ");
    let inner = width.saturating_sub(2);
    format!(
        "+{}{}+",
        label,
        "-".repeat(inner.saturating_sub(label.len()))
    )
}

#[cfg(test)]
pub fn panel_bottom(width: usize) -> String {
    format!("+{}+", "-".repeat(width.saturating_sub(2)))
}

#[cfg(test)]
pub fn panel_line(text: &str, width: usize) -> String {
    let inner = width.saturating_sub(4);
    let fitted = fit_text(text, inner);
    let padding = inner.saturating_sub(fitted.chars().count());
    format!("| {}{} |", fitted, " ".repeat(padding))
}

#[cfg(test)]
pub fn fit_text(text: &str, width: usize) -> String {
    let mut chars = text.chars();
    let mut out = String::new();
    for _ in 0..width {
        let Some(ch) = chars.next() else {
            return out;
        };
        out.push(ch);
    }
    if chars.next().is_some() && width > 1 {
        out.pop();
        out.push('~');
    }
    out
}

#[cfg(test)]
fn bar_width(width: usize) -> usize {
    if width >= 120 {
        28
    } else if width >= 90 {
        22
    } else {
        16
    }
}

#[cfg(test)]
pub fn hbar(percent: f64, width: usize) -> String {
    let percent = percent.clamp(0.0, 100.0);
    let filled = ((percent / 100.0) * width as f64).round() as usize;
    format!(
        "[{}{}]",
        "#".repeat(filled.min(width)),
        "-".repeat(width.saturating_sub(filled))
    )
}

#[cfg(test)]
fn monitor_metric(label: &str, metric: Option<&ng::Metric>) -> String {
    match metric {
        Some(metric) => format!(
            "{label} {}/{} {:.1}% left {}",
            ng::short_number(metric.used),
            ng::short_number(metric.limit),
            metric.percent,
            ng::short_number(metric.remaining)
        ),
        None => format!("{label} n/a"),
    }
}

fn monitor_alerts(windows: &[ng::WindowState], warning_threshold: f64) -> Vec<String> {
    let mut alerts = Vec::new();
    for window in windows {
        for (label, metric) in [
            ("credits", window.credits.as_ref()),
            ("requests", window.requests.as_ref()),
        ] {
            let Some(metric) = metric else {
                continue;
            };
            if matches!(window.level.as_str(), "warning" | "danger")
                && metric.percent >= warning_threshold
            {
                alerts.push(format!(
                    "{} {}/{} at {:.1}%: {} left, reset {}",
                    window.level,
                    window.key,
                    label,
                    metric.percent,
                    ng::short_number(metric.remaining),
                    ng::format_duration_opt(window.reset_in_seconds)
                ));
            }
        }
    }
    alerts
}

#[cfg(test)]
fn monitor_agent(agent: &Value) -> String {
    let agent_cli = agent
        .get("agent_cli")
        .and_then(Value::as_str)
        .unwrap_or("agent");
    let sessions = ng::value_string(agent.get("sessions")).unwrap_or_else(|| "?".to_string());
    let active = ng::value_string(agent.get("active")).unwrap_or_else(|| "?".to_string());
    let waiting = ng::value_string(agent.get("waiting")).unwrap_or_else(|| "?".to_string());
    let total_tokens = agent
        .get("total_tokens")
        .and_then(ng::to_number)
        .map(ng::short_number)
        .unwrap_or_else(|| "?".to_string());
    let active_tokens = agent
        .get("active_tokens")
        .and_then(ng::to_number)
        .map(ng::short_number)
        .unwrap_or_else(|| "?".to_string());
    let context = agent
        .get("max_context_pct")
        .and_then(ng::to_number)
        .map(|value| format!("{value:.0}%"))
        .unwrap_or_else(|| "n/a".to_string());
    let turns = ng::value_string(agent.get("max_turn_count")).unwrap_or_else(|| "?".to_string());
    format!(
        "{agent_cli:<8} {sessions:>8} {active:>6} {waiting:>7} {context:>7} {total_tokens:>12} {active_tokens:>13} {turns:>5}"
    )
}

#[cfg(test)]
fn abtop_agent_summary(abtop: Option<&Value>) -> String {
    let Some(abtop) = abtop else {
        return "agents:n/a".to_string();
    };
    let sessions = ng::value_string(abtop.get("sessions_total")).unwrap_or_else(|| "?".to_string());
    let active = ng::value_string(abtop.get("sessions_active")).unwrap_or_else(|| "?".to_string());
    let ctx = abtop
        .get("agents")
        .and_then(Value::as_array)
        .and_then(|agents| {
            agents
                .iter()
                .filter_map(|agent| agent.get("max_context_pct").and_then(ng::to_number))
                .fold(None, |peak: Option<f64>, value| {
                    Some(peak.map_or(value, |peak| peak.max(value)))
                })
        })
        .map(|value| format!("{value:.0}%"))
        .unwrap_or_else(|| "n/a".to_string());
    format!("sessions:{sessions} active:{active} ctx:{ctx}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn test_snapshot() -> StatusSnapshot {
        StatusSnapshot {
            windows: ng::summarize_me(&ng::demo_payload(), 75.0, 90.0),
            abtop: Some(serde_json::json!({
                "token_rate": 42.0,
                "sessions_total": 2,
                "sessions_active": 1,
                "agents": [{
                    "agent_cli": "codex",
                    "sessions": 2,
                    "active": 1,
                    "waiting": 1,
                    "total_tokens": 1000,
                    "active_tokens": 500,
                    "max_context_pct": 27.0,
                    "max_turn_count": 12
                }]
            })),
            fetched_at: chrono::Utc.timestamp_opt(0, 0).single().unwrap(),
        }
    }

    #[test]
    fn monitor_output_has_dashboard_sections() {
        let snapshot = test_snapshot();
        let rendered = render_monitor(Some(&snapshot), None, 100, 30, 5, 4, true, 75.0);

        assert!(rendered.contains("neurogate quota"));
        assert!(rendered.contains("alerts"));
        assert!(rendered.contains("local agents"));
        assert!(rendered.contains("codex"));
    }

    #[test]
    fn hbar_renders_correctly() {
        assert_eq!(hbar(50.0, 10), "[#####-----]");
        assert_eq!(hbar(0.0, 10), "[----------]");
        assert_eq!(hbar(100.0, 10), "[##########]");
    }

    #[test]
    fn fit_text_truncates_with_tilde() {
        assert_eq!(fit_text("hello", 10), "hello");
        assert_eq!(fit_text("hello world", 8), "hello w~");
    }

    #[test]
    fn panel_dimensions_are_correct() {
        let top = panel_top("test", 20);
        assert_eq!(top.len(), 20);
        assert!(top.starts_with('+'));
        assert!(top.ends_with('+'));

        let bottom = panel_bottom(20);
        assert_eq!(bottom.len(), 20);
    }

    #[test]
    fn window_history_sparkline() {
        let mut hist = WindowHistory::new();
        assert!(hist.sparkline_values().is_empty());

        hist.record(50.0);
        hist.record(75.0);
        assert_eq!(hist.sparkline_values(), vec![50, 75]);

        for i in 0..25 {
            hist.record(i as f64);
        }
        assert_eq!(hist.sparkline_values().len(), SPARKLINE_LEN);
    }
}
