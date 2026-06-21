use chrono::{DateTime, SecondsFormat, TimeZone, Utc};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_API_BASE: &str = "https://api.neurogate.space";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const USER_AGENT: &str = concat!("neurogate-limit-watch/", env!("CARGO_PKG_VERSION"));

const WINDOWS: [(&str, &str, &str); 4] = [
    ("5h", "5Hours", "window5HoursEndsAt"),
    ("24h", "24Hours", "window24HoursEndsAt"),
    ("7d", "7Days", "window7DaysEndsAt"),
    ("30d", "30Days", "window30DaysEndsAt"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailOn {
    Never,
    Warning,
    Danger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputMode {
    Human,
    Json,
    Compact,
}

#[derive(Debug)]
struct Args {
    api_base: Option<String>,
    api_key_env: String,
    env_file: Option<PathBuf>,
    demo: bool,
    mock: Option<String>,
    output: OutputMode,
    monitor: bool,
    with_abtop: bool,
    watch: u64,
    fail_on: FailOn,
    warning_threshold: f64,
    danger_threshold: f64,
    help: bool,
    version: bool,
}

#[derive(Debug, Clone)]
struct MetricSummary {
    used: f64,
    limit: f64,
    remaining: f64,
    percent: f64,
}

#[derive(Debug, Clone)]
struct WindowSummary {
    key: &'static str,
    credits: Option<MetricSummary>,
    requests: Option<MetricSummary>,
    reset_at: Option<String>,
    reset_in_seconds: Option<i64>,
    level: &'static str,
}

#[derive(Debug)]
struct RuntimeConfig {
    api_base: String,
    api_key: String,
    abtop_bin: String,
}

#[derive(Debug)]
struct StatusSnapshot {
    windows: Vec<WindowSummary>,
    abtop: Option<Value>,
    fetched_at: DateTime<Utc>,
}

fn main() {
    let code = match real_main() {
        Ok(code) => code,
        Err(message) => {
            eprintln!("nglimit: {message}");
            2
        }
    };
    pause_before_exit_if_own_console();
    std::process::exit(code);
}

#[cfg(windows)]
fn pause_before_exit_if_own_console() {
    if windows_console_process_count() <= 1 {
        eprintln!();
        eprint!("Press Enter to exit...");
        let _ = io::stderr().flush();
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);
    }
}

#[cfg(not(windows))]
fn pause_before_exit_if_own_console() {}

#[cfg(windows)]
fn windows_console_process_count() -> u32 {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetConsoleProcessList(process_list: *mut u32, process_count: u32) -> u32;
    }

    let mut processes = [0_u32; 8];
    unsafe { GetConsoleProcessList(processes.as_mut_ptr(), processes.len() as u32) }
}

fn real_main() -> Result<i32, String> {
    let args = parse_args(env::args().skip(1))?;
    if args.help {
        print_help();
        return Ok(0);
    }
    if args.version {
        println!("nglimit {VERSION}");
        return Ok(0);
    }
    if args.monitor {
        return run_monitor(&args);
    }

    loop {
        let dotenv = load_dotenv_for_args(&args)?;
        let config = runtime_config(&args, &dotenv);
        let code = run_once(&args, &config)?;
        if args.watch == 0 {
            return Ok(code);
        }
        if args.fail_on != FailOn::Never && code != 0 {
            return Ok(code);
        }
        thread::sleep(Duration::from_secs(args.watch));
    }
}

fn parse_args<I>(args: I) -> Result<Args, String>
where
    I: IntoIterator<Item = String>,
{
    let mut parsed = Args {
        api_base: None,
        api_key_env: "NEUROGATE_API_KEY".to_string(),
        env_file: None,
        demo: false,
        mock: None,
        output: OutputMode::Human,
        monitor: false,
        with_abtop: false,
        watch: 0,
        fail_on: FailOn::Never,
        warning_threshold: 75.0,
        danger_threshold: 90.0,
        help: false,
        version: false,
    };

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => parsed.help = true,
            "-V" | "--version" => parsed.version = true,
            "--demo" => parsed.demo = true,
            "--json" => parsed.output = set_output_mode(parsed.output, OutputMode::Json)?,
            "--compact" => parsed.output = set_output_mode(parsed.output, OutputMode::Compact)?,
            "--monitor" => parsed.monitor = true,
            "--with-abtop" => parsed.with_abtop = true,
            "--api-base" => parsed.api_base = Some(next_value(&mut iter, "--api-base")?),
            "--api-key-env" => parsed.api_key_env = next_value(&mut iter, "--api-key-env")?,
            "--env-file" => {
                parsed.env_file = Some(PathBuf::from(next_value(&mut iter, "--env-file")?))
            }
            "--mock" => parsed.mock = Some(next_value(&mut iter, "--mock")?),
            "--warning" => {
                parsed.warning_threshold =
                    parse_percent(&next_value(&mut iter, "--warning")?, "--warning")?;
            }
            "--danger" => {
                parsed.danger_threshold =
                    parse_percent(&next_value(&mut iter, "--danger")?, "--danger")?;
            }
            "--watch" => {
                let value = next_value(&mut iter, "--watch")?;
                parsed.watch = value.parse::<u64>().map_err(|_| {
                    "--watch must be a non-negative integer number of seconds".to_string()
                })?;
            }
            "--fail-on" => {
                parsed.fail_on = match next_value(&mut iter, "--fail-on")?.as_str() {
                    "never" => FailOn::Never,
                    "warning" => FailOn::Warning,
                    "danger" => FailOn::Danger,
                    other => {
                        return Err(format!(
                            "--fail-on must be one of: never, warning, danger; got {other}"
                        ));
                    }
                };
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    if parsed.demo && parsed.mock.is_some() {
        return Err("--demo and --mock are mutually exclusive".to_string());
    }
    if parsed.monitor && parsed.output != OutputMode::Human {
        return Err("--monitor cannot be combined with --json or --compact".to_string());
    }
    if parsed.warning_threshold >= parsed.danger_threshold {
        return Err("--warning must be lower than --danger".to_string());
    }
    Ok(parsed)
}

fn set_output_mode(current: OutputMode, next: OutputMode) -> Result<OutputMode, String> {
    if current != OutputMode::Human && current != next {
        return Err("--json and --compact are mutually exclusive".to_string());
    }
    Ok(next)
}

fn parse_percent(value: &str, option: &str) -> Result<f64, String> {
    let percent = value
        .trim_end_matches('%')
        .parse::<f64>()
        .map_err(|_| format!("{option} must be a percentage number"))?;
    if !(0.0..=100.0).contains(&percent) {
        return Err(format!("{option} must be between 0 and 100"));
    }
    Ok(percent)
}

fn next_value<I>(iter: &mut I, option: &str) -> Result<String, String>
where
    I: Iterator<Item = String>,
{
    iter.next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn print_help() {
    println!(
        "\
nglimit {VERSION}

Safe NeuroGate quota monitor for Codex/Droid workflows.

USAGE:
  nglimit [OPTIONS]

OPTIONS:
      --demo                 Use built-in demo data without a key or network
      --mock <PATH>          Read a saved /v1/me JSON payload instead of calling NeuroGate
      --json                 Print machine-readable JSON
      --compact              Print one-line output for widgets/status bars
      --monitor              Full-screen live dashboard, abtop-style
      --with-abtop           Merge local abtop --status-json output if available
      --watch <SECONDS>      Poll every N seconds; default is 5 in --monitor
      --fail-on <LEVEL>      Exit non-zero on threshold: never, warning, danger
      --warning <PCT>        Warning threshold percentage [default: 75]
      --danger <PCT>         Danger threshold percentage [default: 90]
      --env-file <PATH>      Load .env file explicitly
      --api-base <URL>       API base URL [env: NEUROGATE_API_BASE]
      --api-key-env <NAME>   API key environment variable [default: NEUROGATE_API_KEY]
  -V, --version              Print version
  -h, --help                 Print help

.env lookup:
  1. --env-file <PATH>
  2. .env in the current directory
  3. .env next to the nglimit executable
"
    );
}

fn run_once(args: &Args, config: &RuntimeConfig) -> Result<i32, String> {
    let snapshot = collect_status(args, config)?;
    let status = summary_to_json(&snapshot.windows, snapshot.abtop.as_ref());

    match args.output {
        OutputMode::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&status)
                    .map_err(|error| format!("cannot render JSON: {error}"))?
            );
        }
        OutputMode::Compact => print_compact(&snapshot.windows, snapshot.abtop.as_ref()),
        OutputMode::Human => print_human(&snapshot.windows, snapshot.abtop.as_ref()),
    }

    Ok(exit_code(&snapshot.windows, args.fail_on))
}

fn collect_status(args: &Args, config: &RuntimeConfig) -> Result<StatusSnapshot, String> {
    let payload = if args.demo {
        demo_payload()
    } else if let Some(path) = &args.mock {
        load_mock(path)?
    } else {
        fetch_me(&config.api_key, &config.api_base)?
    };

    let windows = summarize_me(&payload, args.warning_threshold, args.danger_threshold);
    let abtop = if args.with_abtop {
        read_abtop_status(&config.abtop_bin)
    } else {
        None
    };

    Ok(StatusSnapshot {
        windows,
        abtop,
        fetched_at: Utc::now(),
    })
}

fn run_monitor(args: &Args) -> Result<i32, String> {
    let interval_secs = monitor_interval(args);
    let interval = Duration::from_secs(interval_secs);
    let _terminal = TerminalGuard::enter()?;
    let mut last_snapshot = None::<StatusSnapshot>;
    let mut last_error = None::<String>;
    let mut force_refresh = true;
    let mut next_refresh = Instant::now();
    let mut frame = MonitorFrame::default();

    loop {
        let now = Instant::now();
        if force_refresh || now >= next_refresh {
            match load_dotenv_for_args(args)
                .map(|dotenv| runtime_config(args, &dotenv))
                .and_then(|config| collect_status(args, &config))
            {
                Ok(snapshot) => {
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
        frame.draw(
            last_snapshot.as_ref(),
            last_error.as_deref(),
            interval_secs,
            next_refresh_secs,
            args.with_abtop,
            args.warning_threshold,
        )?;

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
                _ => {}
            }
        }
    }
}

fn monitor_interval(args: &Args) -> u64 {
    if args.watch == 0 {
        5
    } else {
        args.watch.max(1)
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self, String> {
        terminal::enable_raw_mode()
            .map_err(|error| format!("cannot enable terminal raw mode: {error}"))?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, Hide, Clear(ClearType::All)) {
            let _ = terminal::disable_raw_mode();
            return Err(format!("cannot initialize terminal dashboard: {error}"));
        }
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
    }
}

#[derive(Default)]
struct MonitorFrame {
    lines: Vec<String>,
    size: Option<(u16, u16)>,
}

impl MonitorFrame {
    fn draw(
        &mut self,
        snapshot: Option<&StatusSnapshot>,
        error: Option<&str>,
        interval_secs: u64,
        next_refresh_secs: u64,
        with_abtop: bool,
        warning_threshold: f64,
    ) -> Result<(), String> {
        let (width, height) = terminal::size().unwrap_or((100, 30));
        let mut stdout = io::stdout();
        if self.size != Some((width, height)) {
            execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))
                .map_err(|error| format!("cannot resize monitor frame: {error}"))?;
            self.lines.clear();
            self.size = Some((width, height));
        }

        let next = render_monitor_lines(
            snapshot,
            error,
            width,
            height,
            interval_secs,
            next_refresh_secs,
            with_abtop,
            warning_threshold,
        );
        if next == self.lines {
            return Ok(());
        }

        let max_lines = next.len().max(self.lines.len());
        for index in 0..max_lines {
            if next.get(index) == self.lines.get(index) {
                continue;
            }
            let row = index as u16;
            execute!(stdout, MoveTo(0, row), Clear(ClearType::CurrentLine))
                .map_err(|error| format!("cannot redraw monitor line: {error}"))?;
            if let Some(line) = next.get(index) {
                stdout
                    .write_all(line.as_bytes())
                    .map_err(|error| format!("cannot write monitor output: {error}"))?;
            }
        }
        stdout
            .flush()
            .map_err(|error| format!("cannot flush monitor output: {error}"))?;
        self.lines = next;
        Ok(())
    }
}

#[cfg(test)]
fn render_monitor(
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

fn render_monitor_lines(
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
    let now = Utc::now().format("%H:%M:%S UTC");
    let title = match snapshot {
        Some(snapshot) => {
            let level = worst_level(&snapshot.windows);
            let peak = peak_percent_all(&snapshot.windows)
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
            let peak = peak_percent(window)
                .map(|value| format!("{value:.1}%"))
                .unwrap_or_else(|| "n/a".to_string());
            let bar = hbar(peak_percent(window).unwrap_or(0.0), bar_width(width));
            lines.push(panel_line(
                &format!(
                    "{:<4} {:<7} {:<30} peak {:>6} reset {}",
                    window.key,
                    window.level,
                    bar,
                    peak,
                    format_duration(window.reset_in_seconds)
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
                    .and_then(to_number)
                    .map(|value| format!("{value:.1}/min"))
                    .unwrap_or_else(|| "n/a".to_string());
                let sessions_total =
                    value_string(abtop.get("sessions_total")).unwrap_or_else(|| "?".to_string());
                let sessions_active =
                    value_string(abtop.get("sessions_active")).unwrap_or_else(|| "?".to_string());
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

fn panel_top(title: &str, width: usize) -> String {
    let label = format!(" {title} ");
    let inner = width.saturating_sub(2);
    format!(
        "+{}{}+",
        label,
        "-".repeat(inner.saturating_sub(label.len()))
    )
}

fn panel_bottom(width: usize) -> String {
    format!("+{}+", "-".repeat(width.saturating_sub(2)))
}

fn panel_line(text: &str, width: usize) -> String {
    let inner = width.saturating_sub(4);
    let fitted = fit_text(text, inner);
    let padding = inner.saturating_sub(fitted.chars().count());
    format!("| {}{} |", fitted, " ".repeat(padding))
}

fn fit_text(text: &str, width: usize) -> String {
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

fn bar_width(width: usize) -> usize {
    if width >= 120 {
        28
    } else if width >= 90 {
        22
    } else {
        16
    }
}

fn hbar(percent: f64, width: usize) -> String {
    let percent = percent.clamp(0.0, 100.0);
    let filled = ((percent / 100.0) * width as f64).round() as usize;
    format!(
        "[{}{}]",
        "#".repeat(filled.min(width)),
        "-".repeat(width.saturating_sub(filled))
    )
}

fn monitor_metric(label: &str, metric: Option<&MetricSummary>) -> String {
    match metric {
        Some(metric) => format!(
            "{label} {}/{} {:.1}% left {}",
            short_number(metric.used),
            short_number(metric.limit),
            metric.percent,
            short_number(metric.remaining)
        ),
        None => format!("{label} n/a"),
    }
}

fn monitor_alerts(windows: &[WindowSummary], warning_threshold: f64) -> Vec<String> {
    let mut alerts = Vec::new();
    for window in windows {
        for (label, metric) in [
            ("credits", window.credits.as_ref()),
            ("requests", window.requests.as_ref()),
        ] {
            let Some(metric) = metric else {
                continue;
            };
            if matches!(window.level, "warning" | "danger") && metric.percent >= warning_threshold {
                alerts.push(format!(
                    "{} {} at {:.1}%: {} left, reset {}",
                    window.level,
                    format!("{}/{}", window.key, label),
                    metric.percent,
                    short_number(metric.remaining),
                    format_duration(window.reset_in_seconds)
                ));
            }
        }
    }
    alerts
}

fn monitor_agent(agent: &Value) -> String {
    let agent_cli = agent
        .get("agent_cli")
        .and_then(Value::as_str)
        .unwrap_or("agent");
    let sessions = value_string(agent.get("sessions")).unwrap_or_else(|| "?".to_string());
    let active = value_string(agent.get("active")).unwrap_or_else(|| "?".to_string());
    let waiting = value_string(agent.get("waiting")).unwrap_or_else(|| "?".to_string());
    let total_tokens = agent
        .get("total_tokens")
        .and_then(to_number)
        .map(short_number)
        .unwrap_or_else(|| "?".to_string());
    let active_tokens = agent
        .get("active_tokens")
        .and_then(to_number)
        .map(short_number)
        .unwrap_or_else(|| "?".to_string());
    let context = agent
        .get("max_context_pct")
        .and_then(to_number)
        .map(|value| format!("{value:.0}%"))
        .unwrap_or_else(|| "n/a".to_string());
    let turns = value_string(agent.get("max_turn_count")).unwrap_or_else(|| "?".to_string());
    format!(
        "{agent_cli:<8} {sessions:>8} {active:>6} {waiting:>7} {context:>7} {total_tokens:>12} {active_tokens:>13} {turns:>5}"
    )
}

fn abtop_agent_summary(abtop: Option<&Value>) -> String {
    let Some(abtop) = abtop else {
        return "agents:n/a".to_string();
    };
    let sessions = value_string(abtop.get("sessions_total")).unwrap_or_else(|| "?".to_string());
    let active = value_string(abtop.get("sessions_active")).unwrap_or_else(|| "?".to_string());
    let ctx = abtop
        .get("agents")
        .and_then(Value::as_array)
        .and_then(|agents| {
            agents
                .iter()
                .filter_map(|agent| agent.get("max_context_pct").and_then(to_number))
                .fold(None, |peak: Option<f64>, value| {
                    Some(peak.map_or(value, |peak| peak.max(value)))
                })
        })
        .map(|value| format!("{value:.0}%"))
        .unwrap_or_else(|| "n/a".to_string());
    format!("sessions:{sessions} active:{active} ctx:{ctx}")
}

fn peak_percent_all(windows: &[WindowSummary]) -> Option<f64> {
    windows
        .iter()
        .filter_map(peak_percent)
        .fold(None, |peak: Option<f64>, value| {
            Some(peak.map_or(value, |peak| peak.max(value)))
        })
}

fn worst_level(windows: &[WindowSummary]) -> &'static str {
    windows
        .iter()
        .map(|window| window.level)
        .max_by_key(|level| level_rank(level))
        .unwrap_or("unknown")
}

fn level_rank(level: &str) -> u8 {
    match level {
        "danger" => 3,
        "warning" => 2,
        "ok" => 1,
        _ => 0,
    }
}

fn short_number(value: f64) -> String {
    let abs = value.abs();
    if abs >= 1_000_000_000.0 {
        format!("{:.1}B", value / 1_000_000_000.0)
    } else if abs >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{:.1}K", value / 1_000.0)
    } else {
        compact_number(value)
    }
}

fn runtime_config(args: &Args, dotenv: &HashMap<String, String>) -> RuntimeConfig {
    RuntimeConfig {
        api_base: args
            .api_base
            .clone()
            .or_else(|| config_value("NEUROGATE_API_BASE", dotenv))
            .unwrap_or_else(|| DEFAULT_API_BASE.to_string()),
        api_key: config_value(&args.api_key_env, dotenv).unwrap_or_default(),
        abtop_bin: config_value("ABTOP_BIN", dotenv).unwrap_or_else(|| "abtop".to_string()),
    }
}

fn config_value(key: &str, dotenv: &HashMap<String, String>) -> Option<String> {
    env::var(key)
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| dotenv.get(key).cloned().filter(|value| !value.is_empty()))
}

fn load_dotenv_for_args(args: &Args) -> Result<HashMap<String, String>, String> {
    let Some(path) = find_dotenv(args)? else {
        return Ok(HashMap::new());
    };
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read env file {}: {error}", path.display()))?;
    parse_dotenv(&raw).map_err(|error| format!("{}: {error}", path.display()))
}

fn find_dotenv(args: &Args) -> Result<Option<PathBuf>, String> {
    if let Some(path) = &args.env_file {
        if path.is_file() {
            return Ok(Some(path.clone()));
        }
        return Err(format!("env file not found: {}", path.display()));
    }

    let cwd_env = PathBuf::from(".env");
    if cwd_env.is_file() {
        return Ok(Some(cwd_env));
    }

    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            let exe_env = dir.join(".env");
            if exe_env.is_file() {
                return Ok(Some(exe_env));
            }
        }
    }
    Ok(None)
}

fn parse_dotenv(raw: &str) -> Result<HashMap<String, String>, String> {
    let mut values = HashMap::new();
    for (index, line) in raw.lines().enumerate() {
        let mut line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("export ") {
            line = rest.trim_start();
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("line {} is not KEY=VALUE", index + 1));
        };
        let key = key.trim();
        if !is_env_key(key) {
            return Err(format!("line {} has an invalid key", index + 1));
        }
        values.insert(key.to_string(), unquote_env_value(value.trim()).to_string());
    }
    Ok(values)
}

fn is_env_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(first) if first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn unquote_env_value(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return &value[1..value.len() - 1];
        }
    }
    value
}

fn fetch_me(api_key: &str, api_base: &str) -> Result<Value, String> {
    if api_key.is_empty() {
        return Err("NEUROGATE_API_KEY is required unless --demo or --mock is used".to_string());
    }

    let url = format!("{}/v1/me", api_base.trim_end_matches('/'));
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("cannot initialize HTTP client: {error}"))?;

    let response = client
        .get(url)
        .bearer_auth(api_key)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .map_err(|error| format!("cannot reach NeuroGate API: {error}"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "NeuroGate /v1/me returned HTTP {}",
            status.as_u16()
        ));
    }

    let value: Value = response
        .json()
        .map_err(|error| format!("NeuroGate /v1/me returned invalid JSON: {error}"))?;
    if !value.is_object() {
        return Err("NeuroGate /v1/me returned a non-object JSON payload".to_string());
    }
    Ok(value)
}

fn load_mock(path: &str) -> Result<Value, String> {
    let raw =
        fs::read_to_string(path).map_err(|error| format!("cannot read mock payload: {error}"))?;
    let value: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("mock payload is invalid JSON: {error}"))?;
    if !value.is_object() {
        return Err("mock payload must be a JSON object".to_string());
    }
    Ok(value)
}

fn demo_payload() -> Value {
    json!({
        "usage": {
            "rows": [
                {
                    "model": "demo-model",
                    "creditLimit5Hours": 50,
                    "creditLimit24Hours": 180,
                    "creditLimit7Days": 600,
                    "creditLimit30Days": 2000,
                    "requestLimit5Hours": 1000,
                    "requestLimit24Hours": 4000,
                    "requestLimit7Days": 20000,
                    "requestLimit30Days": 80000,
                    "credits5Hours": 39,
                    "credits24Hours": 91,
                    "credits7Days": 214,
                    "credits30Days": 819,
                    "requests5Hours": 610,
                    "requests24Hours": 1510,
                    "requests7Days": 8300,
                    "requests30Days": 26000
                }
            ]
        }
    })
}

fn summarize_me(
    payload: &Value,
    warning_threshold: f64,
    danger_threshold: f64,
) -> Vec<WindowSummary> {
    let rows = extract_usage_rows(payload);
    let now = Utc::now();
    let mut summaries = Vec::new();

    for (key, suffix, reset_field) in WINDOWS {
        let credits = summarize_metric(
            &rows,
            &format!("credits{suffix}"),
            &format!("creditLimit{suffix}"),
        );
        let requests = summarize_metric(
            &rows,
            &format!("requests{suffix}"),
            &format!("requestLimit{suffix}"),
        );
        let reset_value = first_value(&rows, reset_field);
        let (reset_at, reset_in_seconds) = parse_reset(reset_value, now);

        if credits.is_none() && requests.is_none() && reset_at.is_none() {
            continue;
        }

        summaries.push(WindowSummary {
            key,
            level: window_level(
                credits.as_ref(),
                requests.as_ref(),
                warning_threshold,
                danger_threshold,
            ),
            credits,
            requests,
            reset_at,
            reset_in_seconds,
        });
    }
    summaries
}

fn extract_usage_rows(payload: &Value) -> Vec<&Map<String, Value>> {
    if let Some(rows) = payload
        .get("usage")
        .and_then(Value::as_object)
        .and_then(|usage| usage.get("rows"))
        .and_then(Value::as_array)
    {
        return object_rows(rows);
    }

    if let Some(rows) = payload
        .get("data")
        .and_then(Value::as_object)
        .and_then(|data| data.get("usage"))
        .and_then(Value::as_object)
        .and_then(|usage| usage.get("rows"))
        .and_then(Value::as_array)
    {
        return object_rows(rows);
    }

    if let Some(rows) = payload
        .as_object()
        .and_then(|object| object.get("rows"))
        .and_then(Value::as_array)
    {
        return object_rows(rows);
    }

    Vec::new()
}

fn object_rows(rows: &[Value]) -> Vec<&Map<String, Value>> {
    rows.iter().filter_map(Value::as_object).collect()
}

fn summarize_metric(
    rows: &[&Map<String, Value>],
    used_field: &str,
    limit_field: &str,
) -> Option<MetricSummary> {
    let mut used_total = 0.0;
    let mut limits = Vec::<f64>::new();
    let mut seen = false;

    for row in rows {
        let used = row.get(used_field).and_then(to_number);
        let limit = row.get(limit_field).and_then(to_number);
        if used.is_none() && limit.is_none() {
            continue;
        }
        seen = true;
        used_total += used.unwrap_or(0.0);
        if let Some(limit) = limit {
            if limit > 0.0
                && !limits
                    .iter()
                    .any(|existing| (*existing - limit).abs() < f64::EPSILON)
            {
                limits.push(limit);
            }
        }
    }

    let limit_total: f64 = limits.iter().sum();
    if !seen || limit_total <= 0.0 {
        return None;
    }

    let remaining = (limit_total - used_total).max(0.0);
    let percent = ((used_total / limit_total) * 100.0).clamp(0.0, 999.0);
    Some(MetricSummary {
        used: used_total,
        limit: limit_total,
        remaining,
        percent,
    })
}

fn first_value<'a>(rows: &[&'a Map<String, Value>], field: &str) -> Option<&'a Value> {
    rows.iter().find_map(|row| match row.get(field) {
        Some(Value::Null) | None => None,
        Some(Value::String(text)) if text.is_empty() => None,
        value => value,
    })
}

fn to_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse::<f64>().ok(),
        _ => None,
    }
}

fn parse_reset(value: Option<&Value>, now: DateTime<Utc>) -> (Option<String>, Option<i64>) {
    let Some(value) = value else {
        return (None, None);
    };

    let datetime = match value {
        Value::Number(number) => number
            .as_i64()
            .and_then(|timestamp| Utc.timestamp_opt(timestamp, 0).single()),
        Value::String(text) => {
            let raw = text.trim();
            if raw.is_empty() {
                return (None, None);
            }
            if let Ok(timestamp) = raw.parse::<i64>() {
                Utc.timestamp_opt(timestamp, 0).single()
            } else {
                DateTime::parse_from_rfc3339(raw)
                    .map(|datetime| datetime.with_timezone(&Utc))
                    .ok()
            }
        }
        _ => None,
    };

    if let Some(datetime) = datetime {
        let seconds = (datetime - now).num_seconds().max(0);
        (
            Some(datetime.to_rfc3339_opts(SecondsFormat::Secs, true)),
            Some(seconds),
        )
    } else {
        (Some(value.to_string()), None)
    }
}

fn window_level(
    credits: Option<&MetricSummary>,
    requests: Option<&MetricSummary>,
    warning_threshold: f64,
    danger_threshold: f64,
) -> &'static str {
    let peak = [credits, requests]
        .into_iter()
        .flatten()
        .map(|metric| metric.percent)
        .fold(None, |peak: Option<f64>, percent| {
            Some(peak.map_or(percent, |peak| peak.max(percent)))
        });

    match peak {
        Some(peak) if peak >= danger_threshold => "danger",
        Some(peak) if peak >= warning_threshold => "warning",
        Some(_) => "ok",
        None => "unknown",
    }
}

fn summary_to_json(windows: &[WindowSummary], abtop: Option<&Value>) -> Value {
    json!({
        "source": "neurogate",
        "windows": windows.iter().map(window_to_json).collect::<Vec<_>>(),
        "abtop": abtop.cloned().unwrap_or(Value::Null),
    })
}

fn window_to_json(window: &WindowSummary) -> Value {
    json!({
        "window": window.key,
        "level": window.level,
        "reset_at": window.reset_at,
        "reset_in_seconds": window.reset_in_seconds,
        "credits": metric_to_json(window.credits.as_ref()),
        "requests": metric_to_json(window.requests.as_ref()),
    })
}

fn metric_to_json(metric: Option<&MetricSummary>) -> Value {
    match metric {
        Some(metric) => json!({
            "used": metric.used,
            "limit": metric.limit,
            "remaining": metric.remaining,
            "percent": metric.percent,
        }),
        None => Value::Null,
    }
}

fn read_abtop_status(binary: &str) -> Option<Value> {
    let output = Command::new(binary).arg("--status-json").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let parsed: Value = serde_json::from_slice(&output.stdout).ok()?;
    parsed.is_object().then_some(parsed)
}

fn print_human(windows: &[WindowSummary], abtop: Option<&Value>) {
    println!("NeuroGate limits");
    if windows.is_empty() {
        println!("  usage rows not found in /v1/me response");
    }
    for window in windows {
        println!(
            "  {:<4} {:<7} reset {}",
            window.key,
            window.level,
            format_duration(window.reset_in_seconds)
        );
        if let Some(metric) = &window.credits {
            println!("       credits  {}", format_metric(metric));
        }
        if let Some(metric) = &window.requests {
            println!("       requests {}", format_metric(metric));
        }
    }

    if let Some(abtop) = abtop {
        if let Some(agents) = abtop.get("agents").and_then(Value::as_array) {
            if !agents.is_empty() {
                println!("\nLocal agents from abtop");
                for agent in agents {
                    println!("  {}", format_agent(agent));
                }
            }
        }
    }
}

fn print_compact(windows: &[WindowSummary], abtop: Option<&Value>) {
    let mut parts = vec!["NG".to_string()];
    for window in windows {
        let peak = peak_percent(window)
            .map(|value| format!("{value:.0}%"))
            .unwrap_or_else(|| "n/a".to_string());
        parts.push(format!("{}:{}:{}", window.key, window.level, peak));
    }
    if let Some(abtop) = abtop {
        if let Some(agents) = abtop.get("agents").and_then(Value::as_array) {
            for agent in agents {
                let agent_cli = agent
                    .get("agent_cli")
                    .and_then(Value::as_str)
                    .unwrap_or("agent");
                if let Some(ctx) = agent.get("max_context_pct").and_then(to_number) {
                    parts.push(format!("{agent_cli}:ctx{ctx:.0}%"));
                }
            }
        }
    }
    println!("{}", parts.join(" "));
}

fn peak_percent(window: &WindowSummary) -> Option<f64> {
    [window.credits.as_ref(), window.requests.as_ref()]
        .into_iter()
        .flatten()
        .map(|metric| metric.percent)
        .fold(None, |peak: Option<f64>, percent| {
            Some(peak.map_or(percent, |peak| peak.max(percent)))
        })
}

fn format_metric(metric: &MetricSummary) -> String {
    format!(
        "{}/{} ({:.1}%, left {})",
        compact_number(metric.used),
        compact_number(metric.limit),
        metric.percent,
        compact_number(metric.remaining)
    )
}

fn compact_number(value: f64) -> String {
    if (value.fract()).abs() < f64::EPSILON {
        format!("{}", value as i64)
    } else {
        format!("{value:.2}")
    }
}

fn format_duration(seconds: Option<i64>) -> String {
    match seconds {
        None => "unknown".to_string(),
        Some(seconds) if seconds < 60 => format!("in {seconds}s"),
        Some(seconds) if seconds < 3600 => format!("in {}m", seconds / 60),
        Some(seconds) if seconds < 86_400 => {
            format!("in {}h {}m", seconds / 3600, (seconds % 3600) / 60)
        }
        Some(seconds) => format!("in {}d {}h", seconds / 86_400, (seconds % 86_400) / 3600),
    }
}

fn format_agent(agent: &Value) -> String {
    let agent_cli = agent
        .get("agent_cli")
        .and_then(Value::as_str)
        .unwrap_or("agent");
    let sessions = value_string(agent.get("sessions")).unwrap_or_else(|| "?".to_string());
    let active = value_string(agent.get("active")).unwrap_or_else(|| "?".to_string());
    let tokens = value_string(agent.get("active_tokens")).unwrap_or_else(|| "?".to_string());
    let context = agent
        .get("max_context_pct")
        .and_then(to_number)
        .map(|value| format!("{value:.0}%"))
        .unwrap_or_else(|| "n/a".to_string());
    format!("{agent_cli:<8} sessions {sessions:<2} active {active:<2} ctx max {context} tokens {tokens}")
}

fn value_string(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::Number(number) => Some(number.to_string()),
        Value::String(text) => Some(text.clone()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn exit_code(windows: &[WindowSummary], fail_on: FailOn) -> i32 {
    match fail_on {
        FailOn::Never => 0,
        FailOn::Danger => windows
            .iter()
            .any(|window| window.level == "danger")
            .then_some(3)
            .unwrap_or(0),
        FailOn::Warning => windows
            .iter()
            .any(|window| matches!(window.level, "warning" | "danger"))
            .then_some(2)
            .unwrap_or(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_credit_and_request_windows() {
        let windows = summarize_me(&demo_payload(), 75.0, 90.0);

        assert_eq!(
            windows.iter().map(|window| window.key).collect::<Vec<_>>(),
            ["5h", "24h", "7d", "30d"]
        );
        assert_eq!(windows[0].level, "warning");
        assert_eq!(windows[0].credits.as_ref().unwrap().percent, 78.0);
    }

    #[test]
    fn repeated_limit_rows_do_not_double_count_cap() {
        let payload = json!({
            "usage": {
                "rows": [
                    {"credits5Hours": 10, "creditLimit5Hours": 50},
                    {"credits5Hours": 20, "creditLimit5Hours": 50}
                ]
            }
        });

        let windows = summarize_me(&payload, 75.0, 90.0);

        let credits = windows[0].credits.as_ref().unwrap();
        assert_eq!(credits.used, 30.0);
        assert_eq!(credits.limit, 50.0);
        assert_eq!(credits.percent, 60.0);
    }

    #[test]
    fn json_summary_has_no_account_identity() {
        let payload = json!({
            "id": "usr_demo",
            "usage": {"rows": [{"credits5Hours": 39, "creditLimit5Hours": 50}]}
        });
        let encoded = summary_to_json(&summarize_me(&payload, 75.0, 90.0), None).to_string();

        assert!(encoded.contains("\"source\":\"neurogate\""));
        assert!(!encoded.contains("usr_demo"));
    }

    #[test]
    fn fail_on_warning_catches_warning_and_danger() {
        let windows = vec![
            WindowSummary {
                key: "5h",
                credits: Some(MetricSummary {
                    used: 10.0,
                    limit: 100.0,
                    remaining: 90.0,
                    percent: 10.0,
                }),
                requests: None,
                reset_at: None,
                reset_in_seconds: None,
                level: "ok",
            },
            WindowSummary {
                key: "7d",
                credits: Some(MetricSummary {
                    used: 95.0,
                    limit: 100.0,
                    remaining: 5.0,
                    percent: 95.0,
                }),
                requests: None,
                reset_at: None,
                reset_in_seconds: None,
                level: "danger",
            },
        ];

        assert_eq!(exit_code(&windows, FailOn::Warning), 2);
        assert_eq!(exit_code(&windows, FailOn::Danger), 3);
        assert_eq!(exit_code(&windows, FailOn::Never), 0);
    }

    #[test]
    fn custom_thresholds_change_window_level() {
        let windows = summarize_me(&demo_payload(), 80.0, 95.0);

        assert_eq!(windows[0].level, "ok");
    }

    #[test]
    fn parses_dotenv_without_leaking_comments() {
        let parsed = parse_dotenv(
            r#"
            # comment
            export NEUROGATE_API_KEY="demo"
            NEUROGATE_API_BASE=https://api.neurogate.space
            ABTOP_BIN='abtop'
            "#,
        )
        .unwrap();

        assert_eq!(parsed.get("NEUROGATE_API_KEY").unwrap(), "demo");
        assert_eq!(
            parsed.get("NEUROGATE_API_BASE").unwrap(),
            "https://api.neurogate.space"
        );
        assert_eq!(parsed.get("ABTOP_BIN").unwrap(), "abtop");
    }

    #[test]
    fn compact_output_uses_peak_percent() {
        let windows = summarize_me(&demo_payload(), 75.0, 90.0);

        assert_eq!(peak_percent(&windows[0]).unwrap(), 78.0);
    }

    #[test]
    fn monitor_output_has_dashboard_sections() {
        let snapshot = StatusSnapshot {
            windows: summarize_me(&demo_payload(), 75.0, 90.0),
            abtop: Some(json!({
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
            fetched_at: Utc.timestamp_opt(0, 0).single().unwrap(),
        };

        let rendered = render_monitor(Some(&snapshot), None, 100, 30, 5, 4, true, 75.0);

        assert!(rendered.contains("neurogate quota"));
        assert!(rendered.contains("alerts"));
        assert!(rendered.contains("local agents"));
        assert!(rendered.contains("codex"));
    }

    #[test]
    fn monitor_rejects_machine_output_modes() {
        let error = parse_args(["--monitor".to_string(), "--json".to_string()]).unwrap_err();

        assert!(error.contains("--monitor cannot be combined"));
    }
}
