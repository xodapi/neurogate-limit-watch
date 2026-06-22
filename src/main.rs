use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use neurogate_limit_watch::{self as ng, VERSION};

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
    notify: bool,
    watch: u64,
    fail_on: FailOn,
    warning_threshold: f64,
    danger_threshold: f64,
    help: bool,
    version: bool,
}

#[derive(Debug)]
struct RuntimeConfigCli {
    api_base: String,
    api_key: String,
    abtop_bin: String,
}

#[derive(Debug)]
struct StatusSnapshot {
    windows: Vec<ng::WindowState>,
    abtop: Option<Value>,
    fetched_at: chrono::DateTime<chrono::Utc>,
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
    let mut notifier = Notifier::new(args.notify);
    if args.monitor {
        return run_monitor(&args, &mut notifier);
    }

    loop {
        let dotenv = load_dotenv_for_args(&args)?;
        let config = runtime_config(&args, &dotenv);
        let code = run_once(&args, &config, &mut notifier)?;
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
        notify: false,
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
            "--notify" => parsed.notify = true,
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
      --notify               Desktop alert when a window enters warning/danger
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

fn run_once(args: &Args, config: &RuntimeConfigCli, notifier: &mut Notifier) -> Result<i32, String> {
    let snapshot = collect_status(args, config)?;
    let status = ng::summary_to_json(&snapshot.windows, snapshot.abtop.as_ref());

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

    notifier.check_windows(&snapshot.windows);

    Ok(exit_code(&snapshot.windows, args.fail_on))
}

fn collect_status(args: &Args, config: &RuntimeConfigCli) -> Result<StatusSnapshot, String> {
    let payload = if args.demo {
        ng::demo_payload()
    } else if let Some(path) = &args.mock {
        ng::load_mock(path)?
    } else {
        ng::fetch_me(&config.api_key, &config.api_base, ng::USER_AGENT)?
    };

    let windows =
        ng::summarize_me(&payload, args.warning_threshold, args.danger_threshold);
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

fn run_monitor(args: &Args, notifier: &mut Notifier) -> Result<i32, String> {
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
        None => format!(
            "nglimit v{VERSION}  NeuroGate monitor  waiting for first refresh  {now}"
        ),
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
                ng::peak_percent(window.credits.as_ref(), window.requests.as_ref())
                    .unwrap_or(0.0),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlertLevel {
    Ok,
    Warning,
    Danger,
}

impl AlertLevel {
    fn from_summary(level: &str) -> Self {
        match level {
            "danger" => Self::Danger,
            "warning" => Self::Warning,
            _ => Self::Ok,
        }
    }

    fn severity(self) -> u8 {
        match self {
            Self::Ok => 1,
            Self::Warning => 2,
            Self::Danger => 3,
        }
    }

    fn is_escalation_from(self, previous: Self) -> bool {
        self.severity() > previous.severity() && self != Self::Ok
    }

    fn label(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warning => "warning",
            Self::Danger => "danger",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NotificationMessage {
    window: String,
    level: AlertLevel,
    title: String,
    body: String,
}

#[derive(Debug)]
struct Notifier {
    enabled: bool,
    last_levels: HashMap<String, AlertLevel>,
    failure_reported: bool,
}

impl Notifier {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            last_levels: HashMap::new(),
            failure_reported: false,
        }
    }

    fn check_windows(&mut self, windows: &[ng::WindowState]) {
        if !self.enabled {
            return;
        }
        for window in windows {
            if let Some(message) = next_notification(&mut self.last_levels, window) {
                if let Err(error) = fire_desktop_notification(&message) {
                    if !self.failure_reported {
                        eprintln!("nglimit: notification failed (non-fatal): {error}");
                        self.failure_reported = true;
                    }
                }
            }
        }
    }
}

fn next_notification(
    last_levels: &mut HashMap<String, AlertLevel>,
    window: &ng::WindowState,
) -> Option<NotificationMessage> {
    let level = AlertLevel::from_summary(&window.level);
    let previous = last_levels
        .get(window.key)
        .copied()
        .unwrap_or(AlertLevel::Ok);
    last_levels.insert(window.key.to_string(), level);

    if !level.is_escalation_from(previous) {
        return None;
    }

    let title = match level {
        AlertLevel::Danger => format!("NeuroGate: {} window critical", window.key),
        AlertLevel::Warning => format!("NeuroGate: {} window high usage", window.key),
        AlertLevel::Ok => return None,
    };
    Some(NotificationMessage {
        window: window.key.to_string(),
        level,
        title,
        body: notification_body(window),
    })
}

fn notification_body(window: &ng::WindowState) -> String {
    let peak = ng::peak_percent(window.credits.as_ref(), window.requests.as_ref())
        .map(|value| format!("{value:.1}%"))
        .unwrap_or_else(|| "n/a".to_string());
    let credits = ng::metric_text_en("credits", window.credits.as_ref());
    let requests = ng::metric_text_en("requests", window.requests.as_ref());
    let reset = ng::format_duration_opt(window.reset_in_seconds);
    format!(
        "{} | peak {peak} | {credits} | {requests} | reset {reset}",
        window.level
    )
}

#[cfg(windows)]
fn fire_desktop_notification(message: &NotificationMessage) -> Result<(), String> {
    let title = powershell_quote(&message.title);
    let body = powershell_quote(&message.body);
    let script = format!(
        r#"
$title = '{title}'
$body = '{body}'
try {{
  [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
  [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null
  $xmlTitle = [System.Security.SecurityElement]::Escape($title)
  $xmlBody = [System.Security.SecurityElement]::Escape($body)
  $xml = New-Object Windows.Data.Xml.Dom.XmlDocument
  $xml.LoadXml("<toast><visual><binding template='ToastGeneric'><text>$xmlTitle</text><text>$xmlBody</text></binding></visual></toast>")
  $toast = [Windows.UI.Notifications.ToastNotification]::new($xml)
  [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier("nglimit").Show($toast)
}} catch {{
  try {{
    $icon = if ('{level}' -eq 'danger') {{ 48 }} else {{ 64 }}
    (New-Object -ComObject WScript.Shell).Popup($body, 8, $title, $icon) | Out-Null
  }} catch {{}}
}}
"#,
        level = message.level.label()
    );
    std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &script,
        ])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("cannot start PowerShell notification helper: {error}"))
}

#[cfg(windows)]
fn powershell_quote(text: &str) -> String {
    text.replace('\'', "''")
        .replace('\r', " ")
        .replace('\n', " ")
}

#[cfg(target_os = "macos")]
fn fire_desktop_notification(message: &NotificationMessage) -> Result<(), String> {
    let script = format!(
        "display notification {} with title {}",
        applescript_quote(&message.body),
        applescript_quote(&message.title)
    );
    std::process::Command::new("osascript")
        .args(["-e", &script])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("cannot start osascript notification helper: {error}"))
}

#[cfg(target_os = "macos")]
fn applescript_quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn fire_desktop_notification(message: &NotificationMessage) -> Result<(), String> {
    let urgency = match message.level {
        AlertLevel::Danger => "critical",
        AlertLevel::Warning => "normal",
        AlertLevel::Ok => "low",
    };
    std::process::Command::new("notify-send")
        .args([
            "-a",
            "nglimit",
            "-u",
            urgency,
            &message.title,
            &message.body,
        ])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("cannot start notify-send: {error}"))
}

#[cfg(not(any(windows, unix)))]
fn fire_desktop_notification(_message: &NotificationMessage) -> Result<(), String> {
    Err("desktop notifications are not supported on this platform".to_string())
}

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
                    "{} {} at {:.1}%: {} left, reset {}",
                    window.level,
                    format!("{}/{}", window.key, label),
                    metric.percent,
                    ng::short_number(metric.remaining),
                    ng::format_duration_opt(window.reset_in_seconds)
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
    let turns =
        ng::value_string(agent.get("max_turn_count")).unwrap_or_else(|| "?".to_string());
    format!(
        "{agent_cli:<8} {sessions:>8} {active:>6} {waiting:>7} {context:>7} {total_tokens:>12} {active_tokens:>13} {turns:>5}"
    )
}

fn abtop_agent_summary(abtop: Option<&Value>) -> String {
    let Some(abtop) = abtop else {
        return "agents:n/a".to_string();
    };
    let sessions =
        ng::value_string(abtop.get("sessions_total")).unwrap_or_else(|| "?".to_string());
    let active =
        ng::value_string(abtop.get("sessions_active")).unwrap_or_else(|| "?".to_string());
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

fn runtime_config(args: &Args, dotenv: &HashMap<String, String>) -> RuntimeConfigCli {
    RuntimeConfigCli {
        api_base: args
            .api_base
            .clone()
            .or_else(|| ng::config_value("NEUROGATE_API_BASE", dotenv))
            .unwrap_or_else(|| ng::DEFAULT_API_BASE.to_string()),
        api_key: ng::config_value(&args.api_key_env, dotenv).unwrap_or_default(),
        abtop_bin: ng::config_value("ABTOP_BIN", dotenv)
            .unwrap_or_else(|| "abtop".to_string()),
    }
}

fn load_dotenv_for_args(args: &Args) -> Result<HashMap<String, String>, String> {
    if let Some(path) = &args.env_file {
        if !path.is_file() {
            return Err(format!("env file not found: {}", path.display()));
        }
    }
    ng::load_dotenv_custom(args.env_file.as_ref())
}

fn print_human(windows: &[ng::WindowState], abtop: Option<&Value>) {
    println!("NeuroGate limits");
    if windows.is_empty() {
        println!("  usage rows not found in /v1/me response");
    }
    for window in windows {
        println!(
            "  {:<4} {:<7} reset {}",
            window.key,
            window.level,
            ng::format_duration_opt(window.reset_in_seconds)
        );
        if let Some(metric) = &window.credits {
            println!("       credits  {}", ng::format_metric(metric));
        }
        if let Some(metric) = &window.requests {
            println!("       requests {}", ng::format_metric(metric));
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

fn print_compact(windows: &[ng::WindowState], abtop: Option<&Value>) {
    let mut parts = vec!["NG".to_string()];
    for window in windows {
        let peak = ng::peak_percent(window.credits.as_ref(), window.requests.as_ref())
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
                if let Some(ctx) = agent.get("max_context_pct").and_then(ng::to_number) {
                    parts.push(format!("{agent_cli}:ctx{ctx:.0}%"));
                }
            }
        }
    }
    println!("{}", parts.join(" "));
}

fn format_agent(agent: &Value) -> String {
    let agent_cli = agent
        .get("agent_cli")
        .and_then(Value::as_str)
        .unwrap_or("agent");
    let sessions =
        ng::value_string(agent.get("sessions")).unwrap_or_else(|| "?".to_string());
    let active =
        ng::value_string(agent.get("active")).unwrap_or_else(|| "?".to_string());
    let tokens =
        ng::value_string(agent.get("active_tokens")).unwrap_or_else(|| "?".to_string());
    let context = agent
        .get("max_context_pct")
        .and_then(ng::to_number)
        .map(|value| format!("{value:.0}%"))
        .unwrap_or_else(|| "n/a".to_string());
    format!("{agent_cli:<8} sessions {sessions:<2} active {active:<2} ctx max {context} tokens {tokens}")
}

fn exit_code(windows: &[ng::WindowState], fail_on: FailOn) -> i32 {
    match fail_on {
        FailOn::Never => 0,
        FailOn::Danger => windows
            .iter()
            .any(|window| window.level == "danger")
            .then_some(3)
            .unwrap_or(0),
        FailOn::Warning => windows
            .iter()
            .any(|window| matches!(window.level.as_str(), "warning" | "danger"))
            .then_some(2)
            .unwrap_or(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    #[test]
    fn summarized_windows_match_demo() {
        let windows =
            ng::summarize_me(&ng::demo_payload(), 75.0, 90.0);

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

        let windows = ng::summarize_me(&payload, 75.0, 90.0);

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
        let encoded =
            ng::summary_to_json(&ng::summarize_me(&payload, 75.0, 90.0), None).to_string();

        assert!(encoded.contains("\"source\":\"neurogate\""));
        assert!(!encoded.contains("usr_demo"));
    }

    #[test]
    fn fail_on_warning_catches_warning_and_danger() {
        let windows = vec![
            ng::WindowState {
                key: "5h",
                credits: Some(ng::Metric {
                    used: 10.0,
                    limit: 100.0,
                    remaining: 90.0,
                    percent: 10.0,
                }),
                requests: None,
                reset: "unknown".to_string(),
                reset_in_seconds: None,
                level: "ok".to_string(),
                percent: 10.0,
            },
            ng::WindowState {
                key: "7d",
                credits: Some(ng::Metric {
                    used: 95.0,
                    limit: 100.0,
                    remaining: 5.0,
                    percent: 95.0,
                }),
                requests: None,
                reset: "unknown".to_string(),
                reset_in_seconds: None,
                level: "danger".to_string(),
                percent: 95.0,
            },
        ];

        assert_eq!(exit_code(&windows, FailOn::Warning), 2);
        assert_eq!(exit_code(&windows, FailOn::Danger), 3);
        assert_eq!(exit_code(&windows, FailOn::Never), 0);
    }

    #[test]
    fn custom_thresholds_change_window_level() {
        let windows = ng::summarize_me(&ng::demo_payload(), 80.0, 95.0);

        assert_eq!(windows[0].level, "ok");
    }

    #[test]
    fn parses_dotenv_without_leaking_comments() {
        let parsed = ng::parse_dotenv(
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
        let windows = ng::summarize_me(&ng::demo_payload(), 75.0, 90.0);

        assert_eq!(
            ng::peak_percent(windows[0].credits.as_ref(), windows[0].requests.as_ref()).unwrap(),
            78.0
        );
    }

    #[test]
    fn monitor_output_has_dashboard_sections() {
        let snapshot = StatusSnapshot {
            windows: ng::summarize_me(&ng::demo_payload(), 75.0, 90.0),
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
            fetched_at: chrono::Utc.timestamp_opt(0, 0).single().unwrap(),
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

    #[test]
    fn notify_flag_is_parsed() {
        let args = parse_args([
            "--notify".to_string(),
            "--watch".to_string(),
            "60".to_string(),
        ])
        .unwrap();

        assert!(args.notify);
        assert_eq!(args.watch, 60);
    }

    #[test]
    fn notifications_only_fire_on_escalation() {
        let mut last_levels = HashMap::new();
        let warning = test_window("5h", "warning", 78.0);
        let danger = test_window("5h", "danger", 96.0);
        let ok = test_window("5h", "ok", 12.0);

        assert_eq!(
            next_notification(&mut last_levels, &warning)
                .unwrap()
                .level,
            AlertLevel::Warning
        );
        assert!(next_notification(&mut last_levels, &warning).is_none());
        assert_eq!(
            next_notification(&mut last_levels, &danger)
                .unwrap()
                .level,
            AlertLevel::Danger
        );
        assert!(next_notification(&mut last_levels, &ok).is_none());
        assert_eq!(
            next_notification(&mut last_levels, &warning)
                .unwrap()
                .level,
            AlertLevel::Warning
        );
    }

    fn test_window(key: &'static str, level: &'static str, percent: f64) -> ng::WindowState {
        ng::WindowState {
            key,
            credits: Some(ng::Metric {
                used: percent,
                limit: 100.0,
                remaining: 100.0 - percent,
                percent,
            }),
            requests: None,
            reset: "unknown".to_string(),
            reset_in_seconds: Some(3600),
            level: level.to_string(),
            percent,
        }
    }
}
