#![allow(clippy::collapsible_if)]

pub mod api;
pub mod cli;
pub mod parse;

pub use api::*;
pub use parse::*;

use chrono::{SecondsFormat, Utc};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

pub static OFFLINE_SINCE: Mutex<Option<Instant>> = Mutex::new(None);
static DEPRECATED_ENV_WARNINGS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

pub fn update_offline_state(is_error: bool) -> Option<u64> {
    let mut state = OFFLINE_SINCE.lock().unwrap();
    if is_error {
        if state.is_none() {
            *state = Some(Instant::now());
        }
        state.map(|since| since.elapsed().as_secs() / 60)
    } else {
        *state = None;
        None
    }
}

pub fn get_offline_duration_min() -> Option<u64> {
    OFFLINE_SINCE
        .lock()
        .unwrap()
        .map(|since| since.elapsed().as_secs() / 60)
}

pub const DEFAULT_API_BASE: &str = "https://r-api.vibemod.pro";
pub const VPN_API_BASE: &str = "https://api.vibemod.pro";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const USER_AGENT: &str = concat!("vimit/", env!("CARGO_PKG_VERSION"));
pub const USER_AGENT_GUI: &str = concat!("vimit-gui/", env!("CARGO_PKG_VERSION"));
pub const DEFAULT_WARNING_THRESHOLD: f64 = 75.0;
pub const DEFAULT_DANGER_THRESHOLD: f64 = 90.0;
pub const DEFAULT_ABTOP_BIN: &str = "abtop";

pub const WINDOWS: [(&str, &str, &str, &str); 4] = [
    (
        "5h",
        "5Hours",
        "window5HoursStartedAt",
        "window5HoursEndsAt",
    ),
    (
        "24h",
        "24Hours",
        "window24HoursStartedAt",
        "window24HoursEndsAt",
    ),
    ("7d", "7Days", "window7DaysStartedAt", "window7DaysEndsAt"),
    (
        "30d",
        "30Days",
        "window30DaysStartedAt",
        "window30DaysEndsAt",
    ),
];

#[derive(Debug, Clone)]
pub struct Metric {
    pub used: f64,
    pub limit: f64,
    pub remaining: f64,
    pub percent: f64,
}

#[derive(Debug, Clone)]
pub struct WindowState {
    pub key: &'static str,
    pub level: String,
    pub reset: String,
    pub reset_in_seconds: Option<i64>,
    pub credits: Option<Metric>,
    pub requests: Option<Metric>,
    pub percent: f64,
}

#[derive(Debug)]
pub struct RuntimeConfig {
    pub api_base: String,
    pub api_key: String,
    pub abtop_bin: String,
}

impl RuntimeConfig {
    pub fn from_dotenv(
        api_base_override: Option<String>,
        api_key_env: &str,
        env_file: Option<&PathBuf>,
    ) -> Result<Self, String> {
        if let Some(path) = env_file.filter(|p| !p.is_file()) {
            return Err(format!("env file not found: {}", path.display()));
        }
        let dotenv = load_dotenv_custom(env_file)?;
        Ok(Self {
            api_base: api_base_override
                .or_else(|| config_value("VIBEMODE_API_BASE", &dotenv))
                .unwrap_or_else(|| DEFAULT_API_BASE.to_string()),
            api_key: config_value(api_key_env, &dotenv).unwrap_or_default(),
            abtop_bin: config_value("ABTOP_BIN", &dotenv)
                .unwrap_or_else(|| DEFAULT_ABTOP_BIN.to_string()),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Dashboard {
    pub source: String,
    pub status: String,
    pub agent: String,
    pub token_rate: String,
    pub windows: Vec<WindowState>,
    pub daily: Option<DailyState>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DailyState {
    pub spent_today: f64,
    pub daily_limit: f64,
    pub percent: f64,
    pub level: String,
}

#[derive(Debug, Clone)]
pub struct AgentStatus {
    pub summary: String,
    pub token_rate: String,
}

// ── API (now in api.rs) ─────────────────────────────────────────────────────
// HttpClient, Router, fetch_me, load_mock moved to api.rs

// ── Parsing / Summarize (now in parse.rs) ────────────────────────────────────
// demo_payload, summarize_me, etc. moved to parse.rs

// ── Utility ─────────────────────────────────────────────────────────────────

pub fn to_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse::<f64>().ok(),
        _ => None,
    }
}

// ── Formatting ──────────────────────────────────────────────────────────────

pub fn format_duration_secs(seconds: i64) -> String {
    match seconds {
        seconds if seconds < 60 => format!("через {seconds}с"),
        seconds if seconds < 3600 => format!("через {}м", seconds / 60),
        seconds if seconds < 86_400 => {
            format!("через {}ч {}м", seconds / 3600, (seconds % 3600) / 60)
        }
        seconds => format!("через {}д {}ч", seconds / 86_400, (seconds % 86_400) / 3600),
    }
}

pub fn format_duration_opt(seconds: Option<i64>) -> String {
    match seconds {
        None => "unknown".to_string(),
        Some(seconds) => format_duration_secs(seconds),
    }
}

pub fn format_percent(value: f64) -> String {
    format!("{}%", one_decimal(value))
}

pub fn one_decimal(value: f64) -> String {
    format!("{value:.1}").replace('.', ",")
}

pub fn short_number(value: f64) -> String {
    let abs = value.abs();
    if abs >= 1_000_000_000.0 {
        format!("{}B", one_decimal(value / 1_000_000_000.0))
    } else if abs >= 1_000_000.0 {
        format!("{}M", one_decimal(value / 1_000_000.0))
    } else if abs >= 1_000.0 {
        format!("{}K", one_decimal(value / 1_000.0))
    } else if value.fract().abs() < f64::EPSILON {
        format!("{}", value as i64)
    } else {
        one_decimal(value)
    }
}

pub fn short_rate(value: f64) -> String {
    if value >= 1000.0 {
        short_number(value)
    } else {
        one_decimal(value)
    }
}

pub fn compact_number(value: f64) -> String {
    if (value.fract()).abs() < f64::EPSILON {
        format!("{}", value as i64)
    } else {
        format!("{value:.2}")
    }
}

pub fn metric_text(label: &str, metric: Option<&Metric>) -> String {
    match metric {
        Some(metric) => format!(
            "{label}: {}/{} ({}%, осталось {})",
            short_number(metric.used),
            short_number(metric.limit),
            one_decimal(metric.percent),
            short_number(metric.remaining)
        ),
        None => format!("{label}: н/д"),
    }
}

pub fn metric_text_en(label: &str, metric: Option<&Metric>) -> String {
    match metric {
        Some(metric) => format!(
            "{label} {:.1}% left {}",
            metric.percent,
            short_number(metric.remaining)
        ),
        None => format!("{label} n/a"),
    }
}

pub fn format_metric(metric: &Metric) -> String {
    format!(
        "{}/{} ({:.1}%, left {})",
        compact_number(metric.used),
        compact_number(metric.limit),
        metric.percent,
        compact_number(metric.remaining)
    )
}

pub fn value_string(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::Number(number) => Some(number.to_string()),
        Value::String(text) => Some(text.clone()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

// ── Dashboard helpers ───────────────────────────────────────────────────────

pub fn dashboard_status(windows: &[WindowState]) -> String {
    let peak = windows
        .iter()
        .map(|window| window.percent)
        .fold(0.0, f64::max);
    let level = window_level_from_peak(peak);
    format!(
        "квота: {level} | макс {} | обновлено {}",
        format_percent(peak),
        Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
    )
}

fn window_level_from_peak(peak: f64) -> &'static str {
    if peak >= DEFAULT_DANGER_THRESHOLD {
        "лимит"
    } else if peak >= DEFAULT_WARNING_THRESHOLD {
        "внимание"
    } else {
        "норма"
    }
}

pub fn summary_to_json(
    windows: &[WindowState],
    abtop: Option<&Value>,
    daily: Option<&DailyState>,
) -> Value {
    json!({
        "source": "vibemode",
        "windows": windows.iter().map(window_to_json).collect::<Vec<_>>(),
        "abtop": abtop.cloned().unwrap_or(Value::Null),
        "daily": daily.map(|d| json!(d)).unwrap_or_else(|| json!({
            "spent_today": 0.0,
            "daily_limit": 0.0,
            "percent": 0.0,
            "level": "normal"
        })),
    })
}

pub fn summary_to_json_with_stale(
    windows: &[WindowState],
    abtop: Option<&Value>,
    daily: Option<&DailyState>,
    stale: bool,
    latency_ms: u64,
    active_endpoint: &str,
) -> Value {
    let mut obj = json!({
        "source": "vibemode",
        "windows": windows.iter().map(window_to_json).collect::<Vec<_>>(),
        "abtop": abtop.cloned().unwrap_or(Value::Null),
        "daily": daily.map(|d| json!(d)).unwrap_or_else(|| json!({
            "spent_today": 0.0,
            "daily_limit": 0.0,
            "percent": 0.0,
            "level": "normal"
        })),
    });
    if let Some(map) = obj.as_object_mut() {
        if stale {
            map.insert("stale".to_string(), Value::Bool(true));
        }
        map.insert(
            "latency_ms".to_string(),
            Value::Number(serde_json::Number::from(latency_ms)),
        );
        map.insert(
            "active_endpoint".to_string(),
            Value::String(active_endpoint.to_string()),
        );
        if let Some(offline_mins) = get_offline_duration_min() {
            map.insert(
                "api_status".to_string(),
                Value::String("offline".to_string()),
            );
            map.insert(
                "offline_duration_min".to_string(),
                Value::Number(serde_json::Number::from(offline_mins)),
            );
        } else {
            map.insert(
                "api_status".to_string(),
                Value::String("online".to_string()),
            );
        }
    }
    obj
}

fn window_to_json(window: &WindowState) -> Value {
    json!({
        "window": window.key,
        "level": window.level,
        "reset": window.reset,
        "reset_in_seconds": window.reset_in_seconds,
        "credits": metric_to_json(window.credits.as_ref()),
        "requests": metric_to_json(window.requests.as_ref()),
    })
}

fn metric_to_json(metric: Option<&Metric>) -> Value {
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

// ── Agent status ────────────────────────────────────────────────────────────

pub fn read_agent_status(binary: &str) -> AgentStatus {
    let Ok(output) = std::process::Command::new(binary)
        .arg("--status-json")
        .output()
    else {
        return AgentStatus {
            summary: "агенты: abtop не найден; задайте ABTOP_BIN".to_string(),
            token_rate: "токены/мин: нет данных abtop".to_string(),
        };
    };
    if !output.status.success() {
        return AgentStatus {
            summary: "агенты: статус abtop недоступен".to_string(),
            token_rate: "токены/мин: нет данных abtop".to_string(),
        };
    }
    let Ok(parsed) = serde_json::from_slice::<Value>(&output.stdout) else {
        return AgentStatus {
            summary: "агенты: abtop вернул невалидный JSON".to_string(),
            token_rate: "токены/мин: нет данных abtop".to_string(),
        };
    };
    let sessions = parsed
        .get("sessions_total")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let active = parsed
        .get("sessions_active")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let ctx = parsed
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
        .unwrap_or_else(|| "н/д".to_string());
    let token_rate = parsed
        .get("token_rate")
        .and_then(to_number)
        .or_else(|| summed_agent_token_rate(&parsed));

    AgentStatus {
        summary: format!("агенты: сессий {sessions}, активных {active}, контекст макс. {ctx}"),
        token_rate: token_rate
            .map(|value| format!("токены/мин: {}", short_rate(value)))
            .unwrap_or_else(|| "токены/мин: нет данных abtop".to_string()),
    }
}

pub fn read_abtop_status(binary: &str) -> Option<Value> {
    let output = std::process::Command::new(binary)
        .arg("--status-json")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let parsed: Value = serde_json::from_slice(&output.stdout).ok()?;
    parsed.is_object().then_some(parsed)
}

fn summed_agent_token_rate(parsed: &Value) -> Option<f64> {
    parsed
        .get("agents")
        .and_then(Value::as_array)
        .and_then(|agents| {
            let mut total = 0.0;
            let mut seen = false;
            for agent in agents {
                if let Some(rate) = agent.get("token_rate").and_then(to_number) {
                    total += rate;
                    seen = true;
                }
            }
            seen.then_some(total)
        })
}

// ── .env ────────────────────────────────────────────────────────────────────

pub fn config_value(key: &str, dotenv: &HashMap<String, String>) -> Option<String> {
    let keys = if key == "NEUROGATE_API_KEY"
        || key == "VIBEMODE_API_KEY"
        || key == "VIBEMOD_API_KEY"
    {
        vec!["VIBEMODE_API_KEY", "VIBEMOD_API_KEY", "NEUROGATE_API_KEY"]
    } else if key == "NEUROGATE_API_BASE" || key == "VIBEMODE_API_BASE" || key == "VIBEMOD_API_BASE"
    {
        vec![
            "VIBEMODE_API_BASE",
            "VIBEMOD_API_BASE",
            "NEUROGATE_API_BASE",
        ]
    } else {
        vec![key]
    };

    for k in &keys {
        if let Some(val) = env::var(k).ok().filter(|v| !v.is_empty()) {
            if let Some(suffix) = k.strip_prefix("NEUROGATE_") {
                warn_deprecated_env_once(k, suffix);
            }
            return Some(val);
        }
        if let Some(val) = dotenv.get(*k).cloned().filter(|v| !v.is_empty()) {
            if let Some(suffix) = k.strip_prefix("NEUROGATE_") {
                warn_deprecated_env_once(k, suffix);
            }
            return Some(val);
        }
    }
    None
}

fn warn_deprecated_env_once(key: &str, suffix: &str) {
    if remember_deprecated_env_warning(key) {
        eprintln!("warning: {key} is deprecated, rename to VIBEMODE_{suffix}");
    }
}

fn remember_deprecated_env_warning(key: &str) -> bool {
    let warnings = DEPRECATED_ENV_WARNINGS.get_or_init(|| Mutex::new(HashSet::new()));
    warnings.lock().unwrap().insert(key.to_string())
}

pub fn find_dotenv_custom(explicit: Option<&PathBuf>) -> Option<PathBuf> {
    if let Some(path) = explicit {
        if path.is_file() {
            return Some(path.clone());
        }
        return None;
    }

    let cwd_env = PathBuf::from(".env");
    if cwd_env.is_file() {
        return Some(cwd_env);
    }

    if let Some(exe_env) = env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(".env")))
        .filter(|exe_env| exe_env.is_file())
    {
        return Some(exe_env);
    }

    // Fallback to ~/.vimit/.env
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok();
    if let Some(home_path) = home {
        let home_env = PathBuf::from(home_path).join(".vimit").join(".env");
        if home_env.is_file() {
            return Some(home_env);
        }
    }

    None
}

pub fn load_dotenv_custom(explicit: Option<&PathBuf>) -> Result<HashMap<String, String>, String> {
    let Some(path) = find_dotenv_custom(explicit) else {
        return Ok(HashMap::new());
    };
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read env file {}: {error}", path.display()))?;
    parse_dotenv(&raw).map_err(|error| format!("{}: {error}", path.display()))
}

pub fn parse_dotenv(raw: &str) -> Result<HashMap<String, String>, String> {
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
        values.insert(
            key.trim().to_string(),
            unquote_env_value(value.trim()).to_string(),
        );
    }
    Ok(values)
}

pub fn unquote_env_value(value: &str) -> &str {
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

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_summary_has_no_account_identity() {
        let payload = json!({
            "id": "usr_demo",
            "usage": {"rows": [{"credits5Hours": 39, "creditLimit5Hours": 50}]}
        });
        let encoded = summary_to_json(&summarize_me(&payload, 75.0, 90.0), None, None).to_string();

        assert!(encoded.contains("\"source\":\"vibemode\""));
        assert!(!encoded.contains("usr_demo"));
    }

    #[test]
    fn parses_dotenv_without_leaking_comments() {
        let parsed = parse_dotenv(
            r#"
            # comment
            export NEUROGATE_API_KEY="demo"
            NEUROGATE_API_BASE=https://r-api.vibemod.pro
            ABTOP_BIN='abtop'
            "#,
        )
        .unwrap();

        assert_eq!(parsed.get("NEUROGATE_API_KEY").unwrap(), "demo");
        assert_eq!(
            parsed.get("NEUROGATE_API_BASE").unwrap(),
            "https://r-api.vibemod.pro"
        );
        assert_eq!(parsed.get("ABTOP_BIN").unwrap(), "abtop");
    }

    #[test]
    fn deprecated_env_warning_is_recorded_once_per_key() {
        let key = format!("NEUROGATE_TEST_{}", std::process::id());

        assert!(remember_deprecated_env_warning(&key));
        assert!(!remember_deprecated_env_warning(&key));
    }
}
