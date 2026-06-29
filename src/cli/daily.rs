use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::config::dirs_or_default;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyConfig {
    #[serde(default)]
    pub limit: f64,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub spent_today: f64,
    #[serde(default)]
    pub last_credits_7d: f64,
}

impl Default for DailyConfig {
    fn default() -> Self {
        Self {
            limit: 0.0,
            date: current_date_str(),
            spent_today: 0.0,
            last_credits_7d: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailyFile {
    #[serde(default)]
    pub daily: DailyConfig,
}

impl DailyFile {
    pub fn load() -> Self {
        let Some(path) = daily_config_path() else {
            return Self::default();
        };
        if !path.is_file() {
            return Self::default();
        }
        let raw = fs::read_to_string(&path).unwrap_or_default();
        let mut file: Self = toml::from_str(&raw).unwrap_or_default();

        let today = current_date_str();
        if file.daily.date != today {
            file.daily.date = today;
            file.daily.spent_today = 0.0;
        }

        file
    }

    pub fn save(&self) -> Result<(), String> {
        let Some(path) = daily_config_path() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let raw = toml::to_string_pretty(self)
            .map_err(|e| format!("cannot serialize daily config: {e}"))?;
        let tmp_path = path.with_extension("toml.tmp");
        fs::write(&tmp_path, &raw)
            .map_err(|e| format!("cannot write daily config {}: {e}", tmp_path.display()))?;
        fs::rename(&tmp_path, &path)
            .map_err(|e| format!("cannot rename daily config {}: {e}", path.display()))?;
        Ok(())
    }

    pub fn update(&mut self, current_remaining_7d: f64) {
        let today = current_date_str();
        let config = &mut self.daily;
        if config.date != today {
            config.date = today;
            config.spent_today = 0.0;
            config.last_credits_7d = current_remaining_7d;
        } else {
            let delta = config.last_credits_7d - current_remaining_7d;
            if delta > 0.0 {
                config.spent_today += delta;
            }
            config.last_credits_7d = current_remaining_7d;
        }
    }

    pub fn atomic_update(current_remaining_7d: f64) -> Result<Self, String> {
        let Some(path) = daily_config_path() else {
            return Ok(Self::default());
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        use fs3::FileExt;
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| format!("cannot open daily config: {e}"))?;

        let _ = file.lock_exclusive();

        let mut raw = String::new();
        use std::io::{Read, Seek, SeekFrom, Write};
        let _ = file.read_to_string(&mut raw);

        let mut df: Self = toml::from_str(&raw).unwrap_or_default();
        df.update(current_remaining_7d);

        let new_raw = toml::to_string_pretty(&df).unwrap_or_default();
        let _ = file.seek(SeekFrom::Start(0));
        let _ = file.set_len(0);
        let _ = file.write_all(new_raw.as_bytes());
        let _ = file.unlock();

        Ok(df)
    }

    pub fn get_state(&self, limit_7d: f64, override_limit: Option<f64>) -> crate::DailyState {
        let limit = override_limit.unwrap_or_else(|| {
            if self.daily.limit > 0.0 {
                self.daily.limit
            } else {
                limit_7d / 7.0
            }
        });

        let mut percent = 0.0;
        if limit > 0.0 {
            percent = (self.daily.spent_today / limit) * 100.0;
        }

        let level = if percent >= 90.0 {
            "danger"
        } else if percent >= 75.0 {
            "warning"
        } else {
            "normal"
        };

        crate::DailyState {
            spent_today: self.daily.spent_today,
            daily_limit: limit,
            percent,
            level: level.to_string(),
        }
    }
}

fn current_date_str() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

fn daily_config_path() -> Option<PathBuf> {
    let config_dir = dirs_or_default()?;
    Some(config_dir.join("daily.toml"))
}
