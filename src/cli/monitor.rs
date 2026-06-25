use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Sparkline};
use ratatui::Terminal;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::io;
use std::time::{Duration, Instant};

use neurogate_limit_watch::{self as ng, VERSION};

use super::accounts::AccountConfig;
use super::args::{Args, Preset};
use super::constants;
use super::notify::Notifier;
use super::theme::{Palette, Theme};
use super::trends::TrendStore;

const SPARKLINE_LEN: usize = 20;

#[derive(Debug, Clone)]
pub struct PanelState {
    pub show_header: bool,
    pub show_quota: bool,
    pub show_alerts: bool,
    pub show_agents: bool,
    pub show_footer: bool,
    pub show_help: bool,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            show_header: true,
            show_quota: true,
            show_alerts: true,
            show_agents: true,
            show_footer: true,
            show_help: false,
        }
    }
}

impl PanelState {
    pub fn toggle(&mut self, panel: u8) {
        match panel {
            1 => self.show_header = !self.show_header,
            2 => self.show_quota = !self.show_quota,
            3 => self.show_alerts = !self.show_alerts,
            4 => self.show_agents = !self.show_agents,
            5 => self.show_footer = !self.show_footer,
            _ => {}
        }
    }
}

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

pub fn run_monitor(
    args: &Args,
    notifier: &mut Notifier,
    account_names: &[String],
    account_configs: &[AccountConfig],
    initial_account_idx: usize,
    trends: Option<&TrendStore>,
) -> Result<i32, String> {
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

    let has_accounts = !account_names.is_empty();
    let total_accounts = account_names.len();
    let mut cur_account = if has_accounts {
        initial_account_idx.min(total_accounts - 1)
    } else {
        0
    };

    let mut last_snapshot = None::<StatusSnapshot>;
    let mut last_error = None::<String>;
    let mut force_refresh = true;
    let mut next_refresh = Instant::now();
    let mut window_history: HashMap<&str, WindowHistory> = HashMap::new();
    let http = ng::HttpClient::new(ng::USER_AGENT)?;
    let mut panels = PanelState::default();

    loop {
        let now = Instant::now();
        if force_refresh || now >= next_refresh {
            let account = if has_accounts && total_accounts > 1 {
                Some(&account_configs[cur_account])
            } else {
                None
            };
            match load_config(args, account).and_then(|config| collect_status(args, &config, &http))
            {
                Ok(snapshot) => {
                    if let Some(store) = trends {
                        let _ = store.save_snapshot(&snapshot.windows, snapshot.fetched_at);
                    }
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

        let current_account_name = if has_accounts && total_accounts > 1 {
            Some(account_names[cur_account].as_str())
        } else {
            None
        };

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
                    args.theme,
                    &panels,
                    current_account_name,
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
                    KeyCode::Char('?') => panels.show_help = !panels.show_help,
                    KeyCode::Char('1') => panels.toggle(1),
                    KeyCode::Char('2') => panels.toggle(2),
                    KeyCode::Char('3') => panels.toggle(3),
                    KeyCode::Char('4') => panels.toggle(4),
                    KeyCode::Char('5') => panels.toggle(5),
                    KeyCode::Tab if has_accounts && total_accounts > 1 => {
                        cur_account = (cur_account + 1) % total_accounts;
                        force_refresh = true;
                    }
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
    theme: Theme,
    panels: &PanelState,
    current_account: Option<&str>,
) {
    if panels.show_help {
        draw_help_overlay(frame, theme);
        return;
    }

    let area = frame.area();
    let pal = theme.palette();
    let (header_len, footer_len) = match preset {
        Preset::Mini => (1, 1),
        Preset::Compact => (2, 1),
        Preset::Full => (3, 3),
    };

    let mut constraints = Vec::new();
    if panels.show_header {
        constraints.push(Constraint::Length(header_len));
    }
    constraints.push(Constraint::Min(4));
    if panels.show_footer {
        constraints.push(Constraint::Length(footer_len));
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut idx = 0;
    if panels.show_header {
        draw_header(frame, chunks[idx], snapshot, &pal, current_account);
        idx += 1;
    }
    if panels.show_quota {
        draw_body(
            frame,
            chunks[idx],
            snapshot,
            error,
            with_abtop,
            warning_threshold,
            window_history,
            preset,
            &pal,
            panels,
        );
        idx += 1;
    }
    if panels.show_footer {
        draw_footer(
            frame,
            chunks[idx],
            interval_secs,
            next_refresh_secs,
            &pal,
            current_account,
        );
    }
}

fn draw_header(
    frame: &mut ratatui::Frame,
    area: Rect,
    snapshot: Option<&StatusSnapshot>,
    pal: &Palette,
    current_account: Option<&str>,
) {
    let account_prefix = current_account
        .map(|name| format!(" [{name}]"))
        .unwrap_or_default();
    let (title, style) = match snapshot {
        Some(s) => {
            let level = ng::worst_level(&s.windows);
            let peak = ng::peak_percent_all(&s.windows)
                .map(|v| format!("{v:.0}%"))
                .unwrap_or_else(|| "n/a".into());
            let label = if level == "danger" {
                "DANGER"
            } else if level == "warning" {
                "WARNING"
            } else {
                "OK"
            };
            (
                format!(" NeuroGate v{VERSION}{account_prefix} | {label} | peak {peak} "),
                pal.bold_level_style(level),
            )
        }
        None => (
            format!(" NeuroGate v{VERSION}{account_prefix} | waiting for data... "),
            Style::default().fg(pal.muted),
        ),
    };

    let header = Paragraph::new(Line::from(Span::styled(title, style))).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(pal.border_style()),
    );
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
    pal: &Palette,
    panels: &PanelState,
) {
    let (alerts_height, cols) = match preset {
        Preset::Mini => (1, 1),
        Preset::Compact => (2, 1),
        Preset::Full => (5, 2),
    };

    let mut body_constraints = Vec::new();
    body_constraints.push(Constraint::Min(4));
    if panels.show_alerts {
        body_constraints.push(Constraint::Length(alerts_height));
    }
    if panels.show_agents {
        body_constraints.push(Constraint::Length(alerts_height));
    }

    let body_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(body_constraints)
        .split(area);

    let windows_area = body_chunks[0];
    let mut idx = 1;

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
                pal,
            );
        }
    } else {
        let waiting = Paragraph::new(Span::styled(
            "Collecting NeuroGate status...",
            pal.muted_style(),
        ))
        .block(
            Block::default()
                .title("Limits")
                .borders(Borders::ALL)
                .border_style(pal.border_style()),
        );
        frame.render_widget(waiting, windows_area);
    }

    if panels.show_alerts {
        let alerts_area = body_chunks[idx];
        idx += 1;
        draw_alerts(frame, alerts_area, snapshot, warning_threshold, pal);
    }

    if panels.show_agents {
        let agents_area = body_chunks[idx];
        draw_agents_panel(frame, agents_area, snapshot, pal);
    }

    if let Some(error) = error {
        let error_area = Rect {
            y: area.y + area.height.saturating_sub(1),
            height: 1,
            ..area
        };
        let err_widget = Paragraph::new(Span::styled(
            format!(" ! {error}"),
            Style::default().fg(pal.danger),
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
    pal: &Palette,
) {
    let peak = ng::peak_percent(window.credits.as_ref(), window.requests.as_ref()).unwrap_or(0.0);

    let title_style = pal.bold_level_style(&window.level);

    let reset_text = ng::format_duration_opt(window.reset_in_seconds);

    match preset {
        Preset::Mini => {
            let line = Line::from(vec![
                Span::styled(format!("{} ", window.key), title_style),
                Span::styled(format!("{:.0}%", peak), pal.level_style(&window.level)),
                Span::styled(format!(" reset {}", reset_text), pal.muted_style()),
            ]);
            let widget = Paragraph::new(line).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(pal.border_style()),
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
                        .border_style(pal.border_style()),
                )
                .gauge_style(pal.gauge_style(&window.level))
                .ratio((peak / 100.0) as f64);
            frame.render_widget(gauge, inner[0]);

            let credit_line = match &window.credits {
                Some(m) => Line::from(vec![
                    Span::styled("cr ", pal.muted_style()),
                    Span::raw(format!(
                        "{}/{} ({:.0}%)",
                        ng::short_number(m.used),
                        ng::short_number(m.limit),
                        m.percent
                    )),
                ]),
                None => Line::from(Span::styled("cr n/a", pal.muted_style())),
            };
            let request_line = match &window.requests {
                Some(m) => Line::from(vec![
                    Span::styled("rq ", pal.muted_style()),
                    Span::raw(format!(
                        "{}/{} ({:.0}%)",
                        ng::short_number(m.used),
                        ng::short_number(m.limit),
                        m.percent
                    )),
                ]),
                None => Line::from(Span::styled("rq n/a", pal.muted_style())),
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
                        .border_style(pal.border_style()),
                )
                .gauge_style(pal.gauge_style(&window.level))
                .ratio((peak / 100.0) as f64);
            frame.render_widget(gauge, inner[0]);

            let credit_line = match &window.credits {
                Some(m) => Line::from(vec![
                    Span::styled("cr ", pal.muted_style()),
                    Span::raw(format!(
                        "{}/{} ({:.0}%)",
                        ng::short_number(m.used),
                        ng::short_number(m.limit),
                        m.percent
                    )),
                ]),
                None => Line::from(Span::styled("cr n/a", pal.muted_style())),
            };
            let request_line = match &window.requests {
                Some(m) => Line::from(vec![
                    Span::styled("rq ", pal.muted_style()),
                    Span::raw(format!(
                        "{}/{} ({:.0}%)",
                        ng::short_number(m.used),
                        ng::short_number(m.limit),
                        m.percent
                    )),
                ]),
                None => Line::from(Span::styled("rq n/a", pal.muted_style())),
            };
            let metrics = Paragraph::new(vec![credit_line, request_line]);
            frame.render_widget(metrics, inner[1]);

            if let Some(hist) = history {
                let values = hist.sparkline_values();
                if !values.is_empty() {
                    let spark = Sparkline::default()
                        .block(
                            Block::default()
                                .title("history")
                                .border_style(pal.border_style()),
                        )
                        .data(&values)
                        .style(pal.sparkline_style());
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
    pal: &Palette,
) {
    let alerts = match snapshot {
        Some(s) => monitor_alerts(&s.windows, warning_threshold),
        None => vec!["waiting for data".into()],
    };

    let block = Block::default()
        .title(" alerts ")
        .borders(Borders::ALL)
        .border_style(pal.border_style());

    if alerts.is_empty() {
        let ok = Paragraph::new(Span::styled(
            "all windows below threshold",
            Style::default().fg(pal.ok),
        ))
        .block(block);
        frame.render_widget(ok, area);
    } else {
        let lines: Vec<Line> = alerts
            .iter()
            .map(|alert| {
                let style = if alert.starts_with("danger") {
                    Style::default().fg(pal.danger)
                } else if alert.starts_with("warning") {
                    Style::default().fg(pal.warning)
                } else {
                    pal.muted_style()
                };
                Line::from(Span::styled(alert.clone(), style))
            })
            .collect();
        let widget = Paragraph::new(lines).block(block);
        frame.render_widget(widget, area);
    }
}

fn draw_footer(
    frame: &mut ratatui::Frame,
    area: Rect,
    interval_secs: u64,
    next_refresh_secs: u64,
    pal: &Palette,
    current_account: Option<&str>,
) {
    let has_multi_account = current_account.is_some();
    let height = area.height;
    let footer = if height <= 1 {
        let mut text = format!("q quit | r refresh | {interval_secs}s/{next_refresh_secs}s");
        if has_multi_account {
            text = format!("Tab acct | {text}");
        }
        Paragraph::new(Line::from(vec![Span::styled(text, pal.muted_style())]))
    } else {
        let mut spans = vec![
            Span::styled(" q ", pal.key_binding_style()),
            Span::raw("quit  "),
            Span::styled(" r ", pal.key_binding_style()),
            Span::raw(format!(
                "refresh  auto {interval_secs}s  next {next_refresh_secs}s"
            )),
        ];
        if has_multi_account {
            let mut with_tab = vec![
                Span::styled(" Tab ", pal.key_binding_style()),
                Span::raw("account  "),
            ];
            with_tab.extend(spans);
            spans = with_tab;
        }
        Paragraph::new(Line::from(spans)).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(pal.border_style()),
        )
    };
    frame.render_widget(footer, area);
}

fn load_config(args: &Args, account: Option<&AccountConfig>) -> Result<ng::RuntimeConfig, String> {
    let (api_base, api_key_env) = match account {
        Some(acct) => (
            acct.api_base.clone().or_else(|| args.api_base.clone()),
            acct.api_key_env.as_deref().unwrap_or(&args.api_key_env),
        ),
        None => (args.api_base.clone(), args.api_key_env.as_str()),
    };
    ng::RuntimeConfig::from_dotenv(api_base, api_key_env, args.env_file.as_ref())
}

fn draw_agents_panel(
    frame: &mut ratatui::Frame,
    area: Rect,
    snapshot: Option<&StatusSnapshot>,
    pal: &Palette,
) {
    let block = Block::default()
        .title(" agents ")
        .borders(Borders::ALL)
        .border_style(pal.border_style());

    let content = match snapshot {
        Some(s) => {
            if let Some(abtop) = &s.abtop {
                let token_rate = abtop
                    .get("token_rate")
                    .and_then(ng::to_number)
                    .map(|v| format!("{v:.1}/min"))
                    .unwrap_or_else(|| "n/a".into());
                let sessions =
                    ng::value_string(abtop.get("sessions_total")).unwrap_or_else(|| "?".into());
                let active =
                    ng::value_string(abtop.get("sessions_active")).unwrap_or_else(|| "?".into());
                Line::from(Span::styled(
                    format!("token rate {token_rate} | sessions {sessions} active {active}"),
                    pal.accent_style(),
                ))
            } else {
                Line::from(Span::styled(
                    "run with --with-abtop for agent data",
                    pal.muted_style(),
                ))
            }
        }
        None => Line::from(Span::styled("waiting for data", pal.muted_style())),
    };

    let widget = Paragraph::new(content).block(block);
    frame.render_widget(widget, area);
}

fn draw_help_overlay(frame: &mut ratatui::Frame, theme: Theme) {
    let pal = theme.palette();
    let area = frame.area();

    let help_text = vec![
        Line::from(Span::styled(
            "  NeuroGate Monitor - Keybindings",
            pal.bold_level_style("ok"),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  q / Esc", pal.key_binding_style()),
            Span::raw("    Quit"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+C", pal.key_binding_style()),
            Span::raw("      Force quit"),
        ]),
        Line::from(vec![
            Span::styled("  r", pal.key_binding_style()),
            Span::raw("            Force refresh"),
        ]),
        Line::from(vec![
            Span::styled("  ?", pal.key_binding_style()),
            Span::raw("            Toggle this help"),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Panel Toggles:", pal.accent_style())),
        Line::from(vec![
            Span::styled("  1", pal.key_binding_style()),
            Span::raw("            Toggle header"),
        ]),
        Line::from(vec![
            Span::styled("  2", pal.key_binding_style()),
            Span::raw("            Toggle quota cards"),
        ]),
        Line::from(vec![
            Span::styled("  3", pal.key_binding_style()),
            Span::raw("            Toggle alerts"),
        ]),
        Line::from(vec![
            Span::styled("  4", pal.key_binding_style()),
            Span::raw("            Toggle agents"),
        ]),
        Line::from(vec![
            Span::styled("  5", pal.key_binding_style()),
            Span::raw("            Toggle footer"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Press ?", pal.muted_style()),
            Span::raw(" to close this overlay"),
        ]),
    ];

    let help_widget = Paragraph::new(help_text)
        .block(
            Block::default()
                .title(" help ")
                .borders(Borders::ALL)
                .border_style(pal.border_style()),
        )
        .style(pal.header_style());

    let popup_area = Rect {
        x: area.width.saturating_sub(constants::HELP_WIDTH) / 2,
        y: area.height.saturating_sub(constants::HELP_HEIGHT) / 2,
        width: constants::HELP_WIDTH.min(area.width),
        height: constants::HELP_HEIGHT.min(area.height),
    };

    frame.render_widget(help_widget, popup_area);
}

fn monitor_interval(args: &Args) -> u64 {
    if args.watch == 0 {
        constants::DEFAULT_WATCH_INTERVAL
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

    // ── snapshot tests for ratatui TUI ─────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    fn render_tui_to_string(
        snapshot: Option<&StatusSnapshot>,
        error: Option<&str>,
        interval_secs: u64,
        next_refresh_secs: u64,
        with_abtop: bool,
        warning_threshold: f64,
        window_history: &HashMap<&str, WindowHistory>,
        preset: Preset,
        width: u16,
        height: u16,
    ) -> String {
        render_tui_to_string_themed(
            snapshot,
            error,
            interval_secs,
            next_refresh_secs,
            with_abtop,
            warning_threshold,
            window_history,
            preset,
            Theme::Btop,
            width,
            height,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_tui_to_string_themed(
        snapshot: Option<&StatusSnapshot>,
        error: Option<&str>,
        interval_secs: u64,
        next_refresh_secs: u64,
        with_abtop: bool,
        warning_threshold: f64,
        window_history: &HashMap<&str, WindowHistory>,
        preset: Preset,
        theme: Theme,
        width: u16,
        height: u16,
    ) -> String {
        use ratatui::backend::TestBackend;
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let panels = PanelState::default();
        terminal
            .draw(|frame| {
                draw_frame(
                    frame,
                    snapshot,
                    error,
                    interval_secs,
                    next_refresh_secs,
                    with_abtop,
                    warning_threshold,
                    window_history,
                    preset,
                    theme,
                    &panels,
                    None,
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..height {
            for x in 0..width {
                let cell = &buffer[(x, y)];
                out.push_str(cell.symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn tui_snapshot_full_preset() {
        let snapshot = test_snapshot();
        let mut history = HashMap::new();
        history.insert("5h", {
            let mut h = WindowHistory::new();
            h.record(78.0);
            h
        });
        history.insert("24h", {
            let mut h = WindowHistory::new();
            h.record(45.0);
            h
        });
        history.insert("7d", {
            let mut h = WindowHistory::new();
            h.record(30.0);
            h
        });
        history.insert("30d", {
            let mut h = WindowHistory::new();
            h.record(12.0);
            h
        });

        let output = render_tui_to_string(
            Some(&snapshot),
            None,
            5,
            3,
            true,
            75.0,
            &history,
            Preset::Full,
            120,
            40,
        );
        insta::assert_snapshot!(output);
    }

    #[test]
    fn tui_snapshot_compact_preset() {
        let snapshot = test_snapshot();
        let mut history = HashMap::new();
        history.insert("5h", {
            let mut h = WindowHistory::new();
            h.record(78.0);
            h
        });

        let output = render_tui_to_string(
            Some(&snapshot),
            None,
            5,
            3,
            true,
            75.0,
            &history,
            Preset::Compact,
            80,
            25,
        );
        insta::assert_snapshot!(output);
    }

    #[test]
    fn tui_snapshot_mini_preset() {
        let snapshot = test_snapshot();
        let mut history = HashMap::new();
        history.insert("5h", {
            let mut h = WindowHistory::new();
            h.record(78.0);
            h
        });

        let output = render_tui_to_string(
            Some(&snapshot),
            None,
            5,
            3,
            true,
            75.0,
            &history,
            Preset::Mini,
            60,
            15,
        );
        insta::assert_snapshot!(output);
    }

    #[test]
    fn tui_snapshot_with_error() {
        let snapshot = test_snapshot();
        let history = HashMap::new();

        let output = render_tui_to_string(
            Some(&snapshot),
            Some("connection timeout"),
            5,
            3,
            true,
            75.0,
            &history,
            Preset::Full,
            100,
            30,
        );
        insta::assert_snapshot!(output);
    }

    #[test]
    fn tui_snapshot_waiting() {
        let history = HashMap::new();

        let output = render_tui_to_string(
            None,
            None,
            5,
            3,
            true,
            75.0,
            &history,
            Preset::Full,
            100,
            30,
        );
        insta::assert_snapshot!(output);
    }

    #[test]
    fn tui_snapshot_dracula_theme() {
        let snapshot = test_snapshot();
        let mut history = HashMap::new();
        history.insert("5h", {
            let mut h = WindowHistory::new();
            h.record(78.0);
            h
        });

        let output = render_tui_to_string_themed(
            Some(&snapshot),
            None,
            5,
            3,
            true,
            75.0,
            &history,
            Preset::Full,
            Theme::Dracula,
            120,
            40,
        );
        insta::assert_snapshot!(output);
    }

    #[test]
    fn tui_snapshot_high_contrast_theme() {
        let snapshot = test_snapshot();
        let mut history = HashMap::new();
        history.insert("5h", {
            let mut h = WindowHistory::new();
            h.record(78.0);
            h
        });

        let output = render_tui_to_string_themed(
            Some(&snapshot),
            None,
            5,
            3,
            true,
            75.0,
            &history,
            Preset::Full,
            Theme::HighContrast,
            120,
            40,
        );
        insta::assert_snapshot!(output);
    }
}
