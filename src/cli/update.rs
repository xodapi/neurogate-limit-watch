use self_update::cargo_crate_version;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use super::config::dirs_or_default;

const STATE_FILE_NAME: &str = "state.json";

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct State {
    last_update_check: Option<u64>, // timestamp in seconds
    latest_available_version: Option<String>,
    #[serde(default = "default_true")]
    auto_update_check: bool,
    #[serde(default = "default_true")]
    auto_api_failover: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            last_update_check: None,
            latest_available_version: None,
            auto_update_check: true,
            auto_api_failover: true,
        }
    }
}

fn state_path() -> Option<PathBuf> {
    let home = dirs_or_default()?;
    let config_dir = if cfg!(windows) {
        home.join("vimit")
    } else {
        home.join(".config").join("vimit")
    };
    Some(config_dir.join(STATE_FILE_NAME))
}

fn load_state() -> State {
    let Some(path) = state_path() else {
        return State::default();
    };
    if !path.is_file() {
        return State::default();
    }
    let raw = fs::read_to_string(&path).unwrap_or_default();
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_state(state: &State) {
    let Some(path) = state_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(raw) = serde_json::to_string_pretty(state) {
        let _ = fs::write(path, raw);
    }
}

pub fn check_and_update(check_only: bool) -> Result<(), String> {
    let current_version = cargo_crate_version!();

    let mut builder = self_update::backends::github::Update::configure();
    builder
        .repo_owner("xodapi")
        .repo_name("vimit")
        .bin_name("vimit")
        .current_version(current_version);

    if check_only {
        println!(
            "Проверка обновлений для vimit (текущая версия: v{})...",
            current_version
        );
        let latest = builder
            .build()
            .map_err(|e| format!("Ошибка конфигурации обновления: {e}"))?
            .get_latest_release()
            .map_err(|e| format!("Не удалось получить последний релиз: {e}"))?;

        if self_update::version::bump_is_greater(current_version, &latest.version).unwrap_or(false)
        {
            println!("Доступна новая версия: v{}!", latest.version);
            println!("Запустите `vimit update` для установки.");

            // Update state cache as well since we manually checked
            let mut state = load_state();
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            state.last_update_check = Some(now);
            state.latest_available_version = Some(latest.version);
            save_state(&state);
        } else {
            println!(
                "У вас уже установлена последняя версия: v{}.",
                current_version
            );
        }
        Ok(())
    } else {
        println!("Запуск обновления vimit v{}...", current_version);
        let status = builder
            .build()
            .map_err(|e| format!("Ошибка конфигурации обновления: {e}"))?
            .update()
            .map_err(|e| format!("Ошибка при установке обновления: {e}"))?;

        if status.updated() {
            println!("Успешно обновлено до версии v{}!", status.version());
            // Clear state cache to avoid showing notification on next run
            let mut state = load_state();
            state.latest_available_version = None;
            save_state(&state);
        } else {
            println!(
                "У вас уже установлена актуальная версия v{}.",
                status.version()
            );
        }
        Ok(())
    }
}

pub fn start_background_check() {
    std::thread::spawn(move || {
        let current_version = cargo_crate_version!();
        let mut state = load_state();
        if !state.auto_update_check {
            return;
        }
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // 24 hours = 86400 seconds
        if state
            .last_update_check
            .filter(|&last_check| now >= last_check && now - last_check < 86400)
            .is_some()
        {
            return;
        }

        // Query github release
        let mut builder = self_update::backends::github::Update::configure();
        builder
            .repo_owner("xodapi")
            .repo_name("vimit")
            .bin_name("vimit")
            .current_version(current_version);

        if let Ok(latest) = builder
            .build()
            .and_then(|updater| updater.get_latest_release())
        {
            state.last_update_check = Some(now);
            if self_update::version::bump_is_greater(current_version, &latest.version)
                .unwrap_or(false)
            {
                state.latest_available_version = Some(latest.version);
            } else {
                state.latest_available_version = None;
            }
            save_state(&state);
        }
    });
}

pub fn latest_checked_version() -> Option<String> {
    let current_version = cargo_crate_version!();
    let state = load_state();
    if let Some(ver) = state
        .latest_available_version
        .filter(|ver| self_update::version::bump_is_greater(current_version, ver).unwrap_or(false))
    {
        return Some(ver);
    }
    None
}

pub fn is_auto_check_enabled() -> bool {
    load_state().auto_update_check
}

pub fn set_auto_check_enabled(enabled: bool) {
    let mut state = load_state();
    state.auto_update_check = enabled;
    save_state(&state);
}

pub fn is_auto_api_failover_enabled() -> bool {
    load_state().auto_api_failover
}

pub fn set_auto_api_failover_enabled(enabled: bool) {
    let mut state = load_state();
    state.auto_api_failover = enabled;
    save_state(&state);
}
