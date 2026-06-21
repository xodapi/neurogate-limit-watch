use chrono::{DateTime, SecondsFormat, TimeZone, Utc};
use serde_json::{json, Map, Value};
use std::env;
use std::fs;
use std::process::Command;
use std::thread;
use std::time::Duration;

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

#[derive(Debug)]
struct Args {
    api_base: String,
    api_key_env: String,
    demo: bool,
    mock: Option<String>,
    json: bool,
    with_abtop: bool,
    watch: u64,
    fail_on: FailOn,
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

fn main() {
    std::process::exit(match real_main() {
        Ok(code) => code,
        Err(message) => {
            eprintln!("nglimit: {message}");
            2
        }
    });
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

    loop {
        let code = run_once(&args)?;
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
        api_base: env::var("NEUROGATE_API_BASE").unwrap_or_else(|_| DEFAULT_API_BASE.to_string()),
        api_key_env: "NEUROGATE_API_KEY".to_string(),
        demo: false,
        mock: None,
        json: false,
        with_abtop: false,
        watch: 0,
        fail_on: FailOn::Never,
        help: false,
        version: false,
    };

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => parsed.help = true,
            "-V" | "--version" => parsed.version = true,
            "--demo" => parsed.demo = true,
            "--json" => parsed.json = true,
            "--with-abtop" => parsed.with_abtop = true,
            "--api-base" => parsed.api_base = next_value(&mut iter, "--api-base")?,
            "--api-key-env" => parsed.api_key_env = next_value(&mut iter, "--api-key-env")?,
            "--mock" => parsed.mock = Some(next_value(&mut iter, "--mock")?),
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
    Ok(parsed)
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
      --with-abtop           Merge local abtop --status-json output if available
      --watch <SECONDS>      Poll every N seconds
      --fail-on <LEVEL>      Exit non-zero on threshold: never, warning, danger
      --api-base <URL>       API base URL [env: NEUROGATE_API_BASE]
      --api-key-env <NAME>   API key environment variable [default: NEUROGATE_API_KEY]
  -V, --version              Print version
  -h, --help                 Print help
"
    );
}

fn run_once(args: &Args) -> Result<i32, String> {
    let payload = if args.demo {
        demo_payload()
    } else if let Some(path) = &args.mock {
        load_mock(path)?
    } else {
        let api_key = env::var(&args.api_key_env).unwrap_or_default();
        fetch_me(&api_key, &args.api_base)?
    };

    let windows = summarize_me(&payload);
    let abtop = if args.with_abtop {
        read_abtop_status()
    } else {
        None
    };
    let status = summary_to_json(&windows, abtop.as_ref());

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&status)
                .map_err(|error| format!("cannot render JSON: {error}"))?
        );
    } else {
        print_human(&windows, abtop.as_ref());
    }

    Ok(exit_code(&windows, args.fail_on))
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

fn summarize_me(payload: &Value) -> Vec<WindowSummary> {
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
            level: window_level(credits.as_ref(), requests.as_ref()),
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

fn window_level(credits: Option<&MetricSummary>, requests: Option<&MetricSummary>) -> &'static str {
    let peak = [credits, requests]
        .into_iter()
        .flatten()
        .map(|metric| metric.percent)
        .fold(None, |peak: Option<f64>, percent| {
            Some(peak.map_or(percent, |peak| peak.max(percent)))
        });

    match peak {
        Some(peak) if peak >= 90.0 => "danger",
        Some(peak) if peak >= 75.0 => "warning",
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

fn read_abtop_status() -> Option<Value> {
    let binary = env::var("ABTOP_BIN").unwrap_or_else(|_| "abtop".to_string());
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
        let windows = summarize_me(&demo_payload());

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

        let windows = summarize_me(&payload);

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
        let encoded = summary_to_json(&summarize_me(&payload), None).to_string();

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
}
