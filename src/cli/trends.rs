use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{NaiveDate, Utc};
use redb::{Database, TableDefinition};
use serde_json::Value;

use vimit::WindowState;

use super::config::dirs_or_default;

const TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("snapshots");

pub struct TrendStore {
    db: Database,
}

#[derive(Debug, Clone)]
pub struct TrendDay {
    pub date: NaiveDate,
    pub windows: Vec<TrendWindow>,
}

#[derive(Debug, Clone)]
pub struct TrendWindow {
    pub key: String,
    pub samples: usize,
    pub peak_max: f64,
    pub peak_avg: f64,
    pub credits_avg_used: f64,
    pub credits_avg_limit: f64,
    pub requests_avg_used: f64,
    pub requests_avg_limit: f64,
}

impl TrendStore {
    pub fn open() -> Result<Option<Self>, String> {
        let path = match default_trends_path() {
            Some(p) => p,
            None => return Ok(None),
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create trends directory: {e}"))?;
        }
        let db =
            Database::create(&path).map_err(|e| format!("cannot create trends database: {e}"))?;
        let tx = db
            .begin_write()
            .map_err(|e| format!("cannot initialize trends: {e}"))?;
        let _ = tx.open_table(TABLE);
        tx.commit()
            .map_err(|e| format!("trends init commit: {e}"))?;
        Ok(Some(Self { db }))
    }

    pub fn open_readonly() -> Result<Option<Self>, String> {
        let path = match default_trends_path() {
            Some(p) if p.is_file() => p,
            _ => return Ok(None),
        };
        let db = Database::open(&path).map_err(|e| format!("cannot open trends database: {e}"))?;
        Ok(Some(Self { db }))
    }

    pub fn save_snapshot(
        &self,
        windows: &[WindowState],
        fetched_at: chrono::DateTime<Utc>,
    ) -> Result<(), String> {
        let ts = fetched_at.timestamp() as u64;
        let value = serialize_windows(windows, &fetched_at);
        let bytes =
            serde_json::to_vec(&value).map_err(|e| format!("cannot serialize trends: {e}"))?;
        let tx = self
            .db
            .begin_write()
            .map_err(|e| format!("cannot begin write trends: {e}"))?;
        {
            let mut table = tx
                .open_table(TABLE)
                .map_err(|e| format!("trends open table: {e}"))?;
            table
                .insert(ts, bytes.as_slice())
                .map_err(|e| format!("trends insert: {e}"))?;
        }
        tx.commit().map_err(|e| format!("trends commit: {e}"))?;
        Ok(())
    }

    pub fn query_trends(&self, days: u64) -> Result<Vec<TrendDay>, String> {
        let now = Utc::now();
        let start_ts = (now - chrono::Duration::days(days as i64)).timestamp() as u64;
        let end_ts = now.timestamp() as u64;

        let tx = self
            .db
            .begin_read()
            .map_err(|e| format!("cannot begin read trends: {e}"))?;
        let table = tx
            .open_table(TABLE)
            .map_err(|e| format!("trends open table: {e}"))?;

        let mut by_day: BTreeMap<NaiveDate, Vec<Value>> = BTreeMap::new();

        let range = table
            .range(start_ts..=end_ts)
            .map_err(|e| format!("trends range: {e}"))?;

        for result in range {
            let (_ts, bytes) = result.map_err(|e| format!("trends read: {e}"))?;
            if let Ok(value) = serde_json::from_slice::<Value>(bytes.value()) {
                if let Some(ts_str) = value.get("fetched_at").and_then(Value::as_str) {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts_str) {
                        let date = dt.date_naive();
                        by_day.entry(date).or_default().push(value);
                    }
                }
            }
        }

        let mut days: Vec<TrendDay> = Vec::new();
        for (date, snapshots) in by_day {
            let mut window_data: BTreeMap<String, Vec<Value>> = BTreeMap::new();
            for snap in &snapshots {
                if let Some(windows) = snap.get("windows").and_then(Value::as_array) {
                    for w in windows {
                        if let Some(key) = w.get("key").and_then(Value::as_str) {
                            window_data
                                .entry(key.to_string())
                                .or_default()
                                .push(w.clone());
                        }
                    }
                }
            }

            let mut windows: Vec<TrendWindow> = Vec::new();
            for (key, entries) in window_data {
                let samples = entries.len();
                let mut peaks: Vec<f64> = Vec::new();
                let mut credits_used: Vec<f64> = Vec::new();
                let mut credits_limit: Vec<f64> = Vec::new();
                let mut requests_used: Vec<f64> = Vec::new();
                let mut requests_limit: Vec<f64> = Vec::new();
                for entry in &entries {
                    if let Some(p) = entry.get("percent").and_then(Value::as_f64) {
                        peaks.push(p);
                    }
                    if let Some(c) = entry.get("credits_used").and_then(Value::as_f64) {
                        credits_used.push(c);
                    }
                    if let Some(c) = entry.get("credits_limit").and_then(Value::as_f64) {
                        credits_limit.push(c);
                    }
                    if let Some(r) = entry.get("requests_used").and_then(Value::as_f64) {
                        requests_used.push(r);
                    }
                    if let Some(r) = entry.get("requests_limit").and_then(Value::as_f64) {
                        requests_limit.push(r);
                    }
                }
                let peak_max = peaks.iter().cloned().fold(0.0_f64, f64::max);
                let peak_avg = if peaks.is_empty() {
                    0.0
                } else {
                    peaks.iter().sum::<f64>() / peaks.len() as f64
                };
                let credits_avg_used = if credits_used.is_empty() {
                    0.0
                } else {
                    credits_used.iter().sum::<f64>() / credits_used.len() as f64
                };
                let credits_avg_limit = if credits_limit.is_empty() {
                    0.0
                } else {
                    credits_limit.iter().sum::<f64>() / credits_limit.len() as f64
                };
                let requests_avg_used = if requests_used.is_empty() {
                    0.0
                } else {
                    requests_used.iter().sum::<f64>() / requests_used.len() as f64
                };
                let requests_avg_limit = if requests_limit.is_empty() {
                    0.0
                } else {
                    requests_limit.iter().sum::<f64>() / requests_limit.len() as f64
                };

                windows.push(TrendWindow {
                    key,
                    samples,
                    peak_max,
                    peak_avg,
                    credits_avg_used,
                    credits_avg_limit,
                    requests_avg_used,
                    requests_avg_limit,
                });
            }
            windows.sort_by(|a, b| a.key.cmp(&b.key));
            days.push(TrendDay { date, windows });
        }

        Ok(days)
    }
}

fn serialize_windows(windows: &[WindowState], fetched_at: &chrono::DateTime<Utc>) -> Value {
    let win_list: Vec<Value> = windows
        .iter()
        .map(|w| {
            serde_json::json!({
                "key": w.key,
                "level": w.level,
                "percent": w.percent,
                "credits_used": w.credits.as_ref().map(|m| m.used),
                "credits_limit": w.credits.as_ref().map(|m| m.limit),
                "requests_used": w.requests.as_ref().map(|m| m.used),
                "requests_limit": w.requests.as_ref().map(|m| m.limit),
            })
        })
        .collect();
    serde_json::json!({
        "fetched_at": fetched_at.to_rfc3339(),
        "windows": win_list,
    })
}

fn default_trends_path() -> Option<PathBuf> {
    let home = dirs_or_default()?;
    let config_dir = if cfg!(windows) {
        home.join("vimit")
    } else {
        home.join(".config").join("vimit")
    };
    Some(config_dir.join("trends.redb"))
}

pub fn print_trends_human(days: &[TrendDay]) {
    if days.is_empty() {
        println!("No trend data found. Run vimit a few times to collect snapshots.");
        return;
    }
    println!("Trends ({} day(s)):", days.len());
    println!();
    for day in days {
        println!("  {}:", day.date);
        for w in &day.windows {
            println!(
                "    {:4}  peak max {:5.1}%  avg {:5.1}%  ({} samples)",
                w.key, w.peak_max, w.peak_avg, w.samples
            );
            if w.credits_avg_limit > 0.0 {
                println!(
                    "         credits avg {:7.1}/{:7.1}",
                    w.credits_avg_used, w.credits_avg_limit
                );
            }
            if w.requests_avg_limit > 0.0 {
                println!(
                    "         requests avg {:7.1}/{:7.1}",
                    w.requests_avg_used, w.requests_avg_limit
                );
            }
        }
    }
}

pub fn print_trends_json(days: &[TrendDay]) {
    let list: Vec<Value> = days
        .iter()
        .map(|d| {
            serde_json::json!({
                "date": d.date.to_string(),
                "windows": d.windows.iter().map(|w| serde_json::json!({
                    "key": w.key,
                    "samples": w.samples,
                    "peak_max": w.peak_max,
                    "peak_avg": w.peak_avg,
                    "credits_avg_used": w.credits_avg_used,
                    "credits_avg_limit": w.credits_avg_limit,
                    "requests_avg_used": w.requests_avg_used,
                    "requests_avg_limit": w.requests_avg_limit,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let output = serde_json::json!({ "source": "vibemode", "trends": list });
    println!(
        "{}",
        serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
    );
}
