#![allow(clippy::collapsible_if)]

pub mod cli;

use chrono::{DateTime, SecondsFormat, TimeZone, Utc};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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

pub struct Router {
    endpoints: Vec<String>,
    failures: Vec<u32>,
    degraded_until: Vec<Option<Instant>>,
    current: usize,
    threshold: u32,
    cooldown: Duration,
}

impl Router {
    pub fn new(initial_base: String, fallbacks: Vec<String>) -> Self {
        let mut endpoints = vec![initial_base];
        let first = endpoints[0].clone();
        for fb in fallbacks {
            if fb != first {
                endpoints.push(fb);
            }
        }
        let count = endpoints.len();
        Self {
            endpoints,
            failures: vec![0; count],
            degraded_until: vec![None; count],
            current: 0,
            threshold: 3,
            cooldown: Duration::from_secs(60),
        }
    }

    pub fn default_fallbacks() -> Vec<String> {
        vec![DEFAULT_API_BASE.to_string(), VPN_API_BASE.to_string()]
    }

    pub fn active_endpoint(&self) -> &str {
        &self.endpoints[self.current]
    }

    pub fn active_label(&self) -> &str {
        let url = &self.endpoints[self.current];
        if url.contains("r-api") {
            "r-api"
        } else if url.contains("api.vibe") {
            "api"
        } else {
            "custom"
        }
    }

    pub fn record_success(&mut self) {
        self.failures[self.current] = 0;
        self.degraded_until[self.current] = None;
    }

    pub fn record_failure(&mut self) -> bool {
        self.failures[self.current] += 1;
        if self.failures[self.current] >= self.threshold {
            self.degraded_until[self.current] = Some(Instant::now() + self.cooldown);
            self.failover();
            true
        } else {
            false
        }
    }

    fn failover(&mut self) {
        let len = self.endpoints.len();
        for offset in 1..len {
            let idx = (self.current + offset) % len;
            let is_degraded =
                matches!(self.degraded_until[idx], Some(until) if Instant::now() < until);
            if !is_degraded {
                self.current = idx;
                return;
            }
        }
    }
}

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
}

#[derive(Debug, Clone)]
pub struct AgentStatus {
    pub summary: String,
    pub token_rate: String,
}

// ── API ─────────────────────────────────────────────────────────────────────

pub struct HttpClient {
    client: reqwest::blocking::Client,
}

impl HttpClient {
    pub fn new(user_agent: &str) -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent(user_agent)
            .build()
            .map_err(|error| format!("cannot initialize HTTP client: {error}"))?;
        Ok(Self { client })
    }

    pub fn fetch_me(&self, api_key: &str, api_base: &str) -> Result<Value, String> {
        if api_key.is_empty() {
            return Err("VIBEMODE_API_KEY is required unless --demo or --mock is used".to_string());
        }

        let url = format!("{}/v1/me", api_base.trim_end_matches('/'));
        let response = self
            .client
            .get(url)
            .bearer_auth(api_key)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .map_err(|error| format!("cannot reach VibeMode API: {error}"))?;

        let status = response.status();
        if !status.is_success() {
            let code = status.as_u16();
            let hint = match code {
                401 => "check your VIBEMODE_API_KEY".to_string(),
                403 => "your API key does not have access".to_string(),
                404 => "check VIBEMODE_API_BASE".to_string(),
                429 => "rate limited — try --watch with a longer interval".to_string(),
                _ if code >= 500 => {
                    "server error — try again later or check status.vibemod.pro".to_string()
                }
                _ => String::new(),
            };
            let hint = if hint.is_empty() {
                String::new()
            } else {
                format!("\n  hint: {hint}")
            };
            return Err(format!("VibeMode /v1/me returned HTTP {code}{hint}"));
        }

        let value: Value = response
            .json()
            .map_err(|error| format!("VibeMode /v1/me returned invalid JSON: {error}"))?;
        if !value.is_object() {
            return Err("VibeMode /v1/me returned a non-object JSON payload".to_string());
        }
        Ok(value)
    }

    pub fn fetch_me_with_retry(
        &self,
        api_key: &str,
        router: &mut Router,
        api_base: &str,
    ) -> Result<(Value, String), String> {
        let max_retries: u32 = 3;
        let base_delay_ms: u64 = 1000;

        for attempt in 0..=max_retries {
            let endpoint = if attempt == 0 {
                api_base
            } else {
                router.active_endpoint()
            };
            match self.fetch_me(api_key, endpoint) {
                Ok(value) => {
                    router.record_success();
                    return Ok((value, router.active_label().to_string()));
                }
                Err(error) => {
                    let is_retryable = is_retryable_error(&error);
                    if !is_retryable {
                        return Err(error);
                    }
                    let failed_over = router.record_failure();
                    if failed_over && attempt < max_retries {
                        continue;
                    }
                    if attempt == max_retries {
                        return Err(format!(
                            "{error} (after {} retr{})",
                            attempt + 1,
                            if attempt == 0 { "y" } else { "ies" }
                        ));
                    }
                    let delay_ms = base_delay_ms * (1u64 << attempt);
                    let jitter = (rand::random::<u64>() % 500).min(delay_ms / 2);
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms + jitter));
                }
            }
        }

        Err("max retries exceeded".to_string())
    }
}

fn is_retryable_error(error: &str) -> bool {
    error.contains("HTTP 429")
        || error.contains("HTTP 5")
        || error.contains("cannot reach")
        || error.contains("timed out")
        || error.contains("connection")
}

pub fn fetch_me(api_key: &str, api_base: &str, user_agent: &str) -> Result<Value, String> {
    let http = HttpClient::new(user_agent)?;
    http.fetch_me(api_key, api_base)
}

pub fn load_mock(path: &str) -> Result<Value, String> {
    let raw =
        fs::read_to_string(path).map_err(|error| format!("cannot read mock payload: {error}"))?;
    let value: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("mock payload is invalid JSON: {error}"))?;
    if !value.is_object() {
        return Err("mock payload must be a JSON object".to_string());
    }
    Ok(value)
}

// ── Payloads ────────────────────────────────────────────────────────────────

pub fn demo_payload() -> Value {
    let now = Utc::now();
    let five_start = now - chrono::Duration::minutes(28);
    let day_start = now - chrono::Duration::hours(5);
    let week_start = now - chrono::Duration::days(1);
    let month_start = now - chrono::Duration::days(9);
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
                    "requests30Days": 26000,
                    "window5HoursStartedAt": five_start.to_rfc3339_opts(SecondsFormat::Secs, true),
                    "window5HoursEndsAt": (now + chrono::Duration::hours(4)).to_rfc3339_opts(SecondsFormat::Secs, true),
                    "window24HoursStartedAt": day_start.to_rfc3339_opts(SecondsFormat::Secs, true),
                    "window24HoursEndsAt": (now + chrono::Duration::hours(19)).to_rfc3339_opts(SecondsFormat::Secs, true),
                    "window7DaysStartedAt": week_start.to_rfc3339_opts(SecondsFormat::Secs, true),
                    "window7DaysEndsAt": (now + chrono::Duration::days(6)).to_rfc3339_opts(SecondsFormat::Secs, true),
                    "window30DaysStartedAt": month_start.to_rfc3339_opts(SecondsFormat::Secs, true),
                    "window30DaysEndsAt": (now + chrono::Duration::days(21)).to_rfc3339_opts(SecondsFormat::Secs, true)
                }
            ]
        }
    })
}

// ── Parsing / Summarize ─────────────────────────────────────────────────────

pub fn summarize_me(
    payload: &Value,
    warning_threshold: f64,
    danger_threshold: f64,
) -> Vec<WindowState> {
    summarize_me_with_thresholds(
        payload,
        warning_threshold,
        danger_threshold,
        &std::collections::HashMap::new(),
    )
}

pub fn summarize_me_with_thresholds(
    payload: &Value,
    warning_threshold: f64,
    danger_threshold: f64,
    window_thresholds: &std::collections::HashMap<String, (f64, f64)>,
) -> Vec<WindowState> {
    let rows = extract_usage_rows(payload);
    let now = Utc::now();
    let mut summaries = Vec::new();

    for (key, suffix, _start_field, reset_field) in WINDOWS {
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
        let (reset, reset_in_seconds) = parse_reset(reset_value, now);
        let percent = peak_percent(credits.as_ref(), requests.as_ref()).unwrap_or(0.0);

        if credits.is_none() && requests.is_none() && reset_in_seconds.is_none() {
            continue;
        }

        let (w, d) = window_thresholds
            .get(key)
            .copied()
            .unwrap_or((warning_threshold, danger_threshold));

        let level = window_level(credits.as_ref(), requests.as_ref(), w, d);

        summaries.push(WindowState {
            key,
            level: level.to_string(),
            reset,
            reset_in_seconds,
            credits,
            requests,
            percent,
        });
    }
    summaries
}

fn extract_usage_rows(payload: &Value) -> Vec<&Map<String, Value>> {
    // Try usage.rows at various nesting levels
    if let Some(rows) = payload
        .get("usage")
        .and_then(Value::as_object)
        .and_then(|u| u.get("rows"))
        .and_then(Value::as_array)
    {
        let parsed = object_rows(rows);
        if !parsed.is_empty() {
            return parsed;
        }
    }

    if let Some(rows) = payload
        .get("data")
        .and_then(Value::as_object)
        .and_then(|d| d.get("usage"))
        .and_then(Value::as_object)
        .and_then(|u| u.get("rows"))
        .and_then(Value::as_array)
    {
        let parsed = object_rows(rows);
        if !parsed.is_empty() {
            return parsed;
        }
    }

    if let Some(rows) = payload.get("rows").and_then(Value::as_array) {
        let parsed = object_rows(rows);
        if !parsed.is_empty() {
            return parsed;
        }
    }

    if let Some(rows) = payload
        .get("data")
        .and_then(Value::as_object)
        .and_then(|d| d.get("rows"))
        .and_then(Value::as_array)
    {
        let parsed = object_rows(rows);
        if !parsed.is_empty() {
            return parsed;
        }
    }

    if let Some(rows) = payload.get("usage").and_then(Value::as_array) {
        let parsed = object_rows(rows);
        if !parsed.is_empty() {
            return parsed;
        }
    }

    // Last resort: scan all arrays for objects with credit/request fields
    if let Some(object) = payload.as_object() {
        for (_key, value) in object {
            if let Some(rows) = value.as_array() {
                let parsed = object_rows(rows);
                if parsed.iter().any(|row| has_usage_fields(row)) {
                    return parsed;
                }
            }
        }
        if let Some(data) = object.get("data").and_then(Value::as_object) {
            for (_key, value) in data {
                if let Some(rows) = value.as_array() {
                    let parsed = object_rows(rows);
                    if parsed.iter().any(|row| has_usage_fields(row)) {
                        return parsed;
                    }
                }
            }
        }
    }

    Vec::new()
}

fn has_usage_fields(row: &Map<String, Value>) -> bool {
    for key in row.keys() {
        let lower = key.to_lowercase();
        if (lower.contains("credit") || lower.contains("request") || lower.contains("limit"))
            && row.get(key.as_str()).and_then(to_number).is_some()
        {
            return true;
        }
    }
    false
}

fn object_rows(rows: &[Value]) -> Vec<&Map<String, Value>> {
    rows.iter().filter_map(Value::as_object).collect()
}

fn summarize_metric(
    rows: &[&Map<String, Value>],
    used_field: &str,
    limit_field: &str,
) -> Option<Metric> {
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
        if let Some(limit) = limit.filter(|&limit| {
            limit > 0.0
                && !limits
                    .iter()
                    .any(|existing| (*existing - limit).abs() < f64::EPSILON)
        }) {
            limits.push(limit);
        }
    }

    let limit_total: f64 = limits.iter().sum();
    if !seen || limit_total <= 0.0 {
        return None;
    }

    let remaining = (limit_total - used_total).max(0.0);
    let percent = ((used_total / limit_total) * 100.0).clamp(0.0, 999.0);
    Some(Metric {
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

pub fn to_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse::<f64>().ok(),
        _ => None,
    }
}

fn parse_datetime_value(value: &Value) -> Option<DateTime<Utc>> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .and_then(|timestamp| Utc.timestamp_opt(timestamp, 0).single()),
        Value::String(text) => {
            let raw = text.trim();
            if raw.is_empty() {
                None
            } else if let Ok(timestamp) = raw.parse::<i64>() {
                Utc.timestamp_opt(timestamp, 0).single()
            } else {
                DateTime::parse_from_rfc3339(raw)
                    .map(|datetime| datetime.with_timezone(&Utc))
                    .ok()
            }
        }
        _ => None,
    }
}

fn parse_reset(value: Option<&Value>, now: DateTime<Utc>) -> (String, Option<i64>) {
    let Some(value) = value else {
        return ("сброс неизвестен".to_string(), None);
    };

    if let Some(datetime) = parse_datetime_value(value) {
        let seconds = (datetime - now).num_seconds().max(0);
        (
            format!("сброс {}", format_duration_secs(seconds)),
            Some(seconds),
        )
    } else {
        let text = value
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| value.to_string());
        (format!("сброс в {text}"), None)
    }
}

pub fn window_level(
    credits: Option<&Metric>,
    requests: Option<&Metric>,
    warning_threshold: f64,
    danger_threshold: f64,
) -> &'static str {
    let peak = peak_percent(credits, requests);
    match peak {
        Some(peak) if peak >= danger_threshold => "danger",
        Some(peak) if peak >= warning_threshold => "warning",
        Some(_) => "ok",
        None => "unknown",
    }
}

pub fn peak_percent(credits: Option<&Metric>, requests: Option<&Metric>) -> Option<f64> {
    [credits, requests]
        .into_iter()
        .flatten()
        .map(|metric| metric.percent)
        .fold(None, |peak: Option<f64>, percent| {
            Some(peak.map_or(percent, |peak| peak.max(percent)))
        })
}

pub fn peak_percent_all(windows: &[WindowState]) -> Option<f64> {
    windows
        .iter()
        .map(|w| w.percent)
        .fold(None, |peak: Option<f64>, value| {
            Some(peak.map_or(value, |peak| peak.max(value)))
        })
}

pub fn worst_level(windows: &[WindowState]) -> &str {
    windows
        .iter()
        .map(|window| window.level.as_str())
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

pub fn rate_text(
    credits: Option<&Metric>,
    requests: Option<&Metric>,
    start: Option<&Value>,
    now: DateTime<Utc>,
) -> String {
    let Some(start) = start.and_then(parse_datetime_value) else {
        return "темп окна: нет window*StartedAt".to_string();
    };
    let elapsed_minutes = ((now - start).num_seconds().max(60) as f64) / 60.0;
    let credit_rate = credits
        .map(|metric| format!("{}/мин", short_rate(metric.used / elapsed_minutes)))
        .unwrap_or_else(|| "н/д".to_string());
    let request_rate = requests
        .map(|metric| format!("{}/мин", short_rate(metric.used / elapsed_minutes)))
        .unwrap_or_else(|| "н/д".to_string());

    format!("темп окна: кред {credit_rate} | запр {request_rate}")
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
        "квота: {level} | пик {} | обновлено {}",
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

pub fn summary_to_json(windows: &[WindowState], abtop: Option<&Value>) -> Value {
    json!({
        "source": "vibemode",
        "windows": windows.iter().map(window_to_json).collect::<Vec<_>>(),
        "abtop": abtop.cloned().unwrap_or(Value::Null),
    })
}

pub fn summary_to_json_with_stale(
    windows: &[WindowState],
    abtop: Option<&Value>,
    stale: bool,
    latency_ms: u64,
    active_endpoint: &str,
) -> Value {
    let mut obj = json!({
        "source": "vibemode",
        "windows": windows.iter().map(window_to_json).collect::<Vec<_>>(),
        "abtop": abtop.cloned().unwrap_or(Value::Null),
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

    for k in keys {
        if let Some(val) = env::var(k).ok().filter(|v| !v.is_empty()) {
            return Some(val);
        }
        if let Some(val) = dotenv.get(k).cloned().filter(|v| !v.is_empty()) {
            return Some(val);
        }
    }
    None
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

        assert!(encoded.contains("\"source\":\"vibemode\""));
        assert!(!encoded.contains("usr_demo"));
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
    fn peak_percent_returns_max() {
        let windows = summarize_me(&demo_payload(), 75.0, 90.0);
        assert_eq!(
            peak_percent(windows[0].credits.as_ref(), windows[0].requests.as_ref()).unwrap(),
            78.0
        );
    }

    #[test]
    fn extract_usage_rows_handles_nested_data_usage() {
        let payload = json!({
            "data": {
                "usage": {
                    "rows": [{"credits5Hours": 10, "creditLimit5Hours": 100}]
                }
            }
        });
        let windows = summarize_me(&payload, 75.0, 90.0);
        assert!(!windows.is_empty());
        assert_eq!(windows[0].credits.as_ref().unwrap().used, 10.0);
    }

    #[test]
    fn extract_usage_rows_handles_flat_rows() {
        let payload = json!({
            "rows": [{"credits5Hours": 20, "creditLimit5Hours": 100}]
        });
        let windows = summarize_me(&payload, 75.0, 90.0);
        assert!(!windows.is_empty());
    }

    #[test]
    fn extract_usage_rows_handles_string_numbers() {
        let payload = json!({
            "usage": {"rows": [{"credits5Hours": "30", "creditLimit5Hours": "100"}]}
        });
        let windows = summarize_me(&payload, 75.0, 90.0);
        assert!(!windows.is_empty());
        assert_eq!(windows[0].credits.as_ref().unwrap().used, 30.0);
    }

    #[test]
    fn summarize_me_returns_empty_for_unknown_schema() {
        let payload = json!({"something": "unrelated"});
        let windows = summarize_me(&payload, 75.0, 90.0);
        assert!(windows.is_empty());
    }
}
