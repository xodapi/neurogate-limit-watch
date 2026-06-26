use slint::{ComponentHandle, SharedString, Timer, TimerMode, Weak};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use vimit as ng;

slint::include_modules!();

#[derive(Debug, Clone, Default)]
struct GuiAccount {
    api_key_env: Option<String>,
    api_base: Option<String>,
}

fn load_gui_accounts() -> (Vec<String>, Vec<GuiAccount>) {
    use std::fs;
    use std::path::PathBuf;
    let home = if cfg!(windows) {
        std::env::var("APPDATA")
            .ok()
            .map(PathBuf::from)
            .or_else(|| std::env::var("USERPROFILE").ok().map(PathBuf::from))
    } else {
        std::env::var("HOME").ok().map(PathBuf::from)
    };
    let Some(home) = home else {
        return (vec![], vec![]);
    };
    let config_dir = if cfg!(windows) {
        home.join("vimit")
    } else {
        home.join(".config").join("vimit")
    };
    let path = config_dir.join("accounts.toml");
    if !path.is_file() {
        return (vec![], vec![]);
    }
    let raw = match fs::read_to_string(&path) {
        Ok(r) => r,
        Err(_) => return (vec![], vec![]),
    };
    #[derive(serde::Deserialize)]
    struct RawAcct {
        api_key_env: Option<String>,
        api_base: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct RawRoot {
        accounts: HashMap<String, RawAcct>,
    }
    let parsed: RawRoot = match toml::from_str(&raw) {
        Ok(p) => p,
        Err(_) => return (vec![], vec![]),
    };
    let mut names: Vec<String> = parsed.accounts.keys().cloned().collect();
    names.sort();
    let configs: Vec<GuiAccount> = names
        .iter()
        .filter_map(|n| parsed.accounts.get(n))
        .map(|r| GuiAccount {
            api_key_env: r.api_key_env.clone(),
            api_base: r.api_base.clone(),
        })
        .collect();
    (names, configs)
}

fn main() {
    let app = AppWindow::new().expect("cannot initialize Slint window");
    let http = std::sync::Arc::new(
        ng::HttpClient::new(ng::USER_AGENT_GUI).expect("cannot initialize HTTP client"),
    );

    let (account_names, account_configs) = load_gui_accounts();
    let current_acct = Arc::new(Mutex::new(None::<GuiAccount>));

    if !account_names.is_empty() {
        let slist: Vec<SharedString> = account_names.iter().map(|n| n.as_str().into()).collect();
        let arr = slint::ModelRc::new(std::rc::Rc::new(slint::VecModel::from(slist)));
        app.set_account_names(arr);
        app.set_current_account(account_names[0].as_str().into());
        {
            let mut cur = current_acct.lock().unwrap();
            *cur = Some(account_configs[0].clone());
        }

        let state = current_acct.clone();
        let configs = account_configs.clone();
        app.on_account_changed(move |name| {
            if let Some(idx) = account_names.iter().position(|n| n == name.as_str()) {
                let mut cur = state.lock().unwrap();
                *cur = Some(configs[idx].clone());
            }
        });
    }

    let weak = app.as_weak();
    let http_clone = http.clone();
    let acct = current_acct.clone();
    app.on_refresh_requested(move || {
        start_refresh(
            weak.clone(),
            false,
            http_clone.clone(),
            acct.clone(),
            ng::DEFAULT_WARNING_THRESHOLD,
            ng::DEFAULT_DANGER_THRESHOLD,
        );
    });

    let weak = app.as_weak();
    let http_clone = http.clone();
    let acct = current_acct.clone();
    app.on_demo_requested(move || {
        start_refresh(
            weak.clone(),
            true,
            http_clone.clone(),
            acct.clone(),
            ng::DEFAULT_WARNING_THRESHOLD,
            ng::DEFAULT_DANGER_THRESHOLD,
        );
    });

    let weak = app.as_weak();
    let http_clone = http.clone();
    let acct = current_acct.clone();
    app.on_settings_changed(move |warning, danger| {
        start_refresh(
            weak.clone(),
            false,
            http_clone.clone(),
            acct.clone(),
            warning as f64,
            danger as f64,
        );
    });

    let timer = Timer::default();
    let weak = app.as_weak();
    let http_clone = http.clone();
    let acct = current_acct.clone();
    timer.start(TimerMode::Repeated, Duration::from_secs(10), move || {
        start_refresh(
            weak.clone(),
            false,
            http_clone.clone(),
            acct.clone(),
            ng::DEFAULT_WARNING_THRESHOLD,
            ng::DEFAULT_DANGER_THRESHOLD,
        );
    });

    start_refresh(
        app.as_weak(),
        false,
        http,
        current_acct.clone(),
        ng::DEFAULT_WARNING_THRESHOLD,
        ng::DEFAULT_DANGER_THRESHOLD,
    );
    app.run().expect("Slint event loop failed");
}

fn start_refresh(
    app: Weak<AppWindow>,
    demo: bool,
    http: Arc<ng::HttpClient>,
    account: Arc<Mutex<Option<GuiAccount>>>,
    warning: f64,
    danger: f64,
) {
    thread::spawn(move || {
        let result = load_dashboard(demo, &http, &account, warning, danger);
        let _ = app.upgrade_in_event_loop(move |app| {
            apply_dashboard(&app, result);
        });
    });
}

fn apply_dashboard(app: &AppWindow, result: Result<ng::Dashboard, String>) {
    match result {
        Ok(dashboard) => {
            app.set_status_text(dashboard.status.into());
            app.set_source_text(dashboard.source.into());
            app.set_agent_text(dashboard.agent.into());
            app.set_token_rate_text(dashboard.token_rate.into());
            apply_window(
                app,
                "5h",
                dashboard.windows.iter().find(|window| window.key == "5h"),
            );
            apply_window(
                app,
                "24h",
                dashboard.windows.iter().find(|window| window.key == "24h"),
            );
            apply_window(
                app,
                "7d",
                dashboard.windows.iter().find(|window| window.key == "7d"),
            );
            apply_window(
                app,
                "30d",
                dashboard.windows.iter().find(|window| window.key == "30d"),
            );
        }
        Err(error) => {
            let msg = if error.contains("VibeMode /v1/me returned HTTP 401") {
                "Check your VIBEMODE_API_KEY in .env"
            } else if error.contains("cannot reach VibeMode API") {
                "Check network / VIBEMODE_API_BASE"
            } else if error.contains("VIBEMODE_API_KEY is required")
                || error.contains("NEUROGATE_API_KEY is required")
            {
                "Set VIBEMODE_API_KEY or use Demo"
            } else {
                &error
            };
            app.set_status_text(msg.into());
            app.set_source_text(format!("error: {error}").into());
        }
    }
}

fn apply_window(app: &AppWindow, key: &str, window: Option<&ng::WindowState>) {
    let fallback = ng::WindowState {
        key: "n/a",
        level: "н/д".to_string(),
        reset: "сброс неизвестен".to_string(),
        reset_in_seconds: None,
        credits: None,
        requests: None,
        percent: 0.0,
    };
    let window = window.unwrap_or(&fallback);
    let level: SharedString = window.level.clone().into();
    let reset: SharedString = window.reset.clone().into();
    let credits: SharedString = ng::metric_text("кредиты", window.credits.as_ref()).into();
    let requests: SharedString = ng::metric_text("запросы", window.requests.as_ref()).into();
    let percent_text: SharedString = format!("{} пик", ng::format_percent(window.percent)).into();
    let percent = window.percent as f32;
    let credit_percent =
        ng::peak_percent(window.credits.as_ref(), window.requests.as_ref()).unwrap_or(0.0) as f32;
    let request_percent =
        ng::peak_percent(window.credits.as_ref(), window.requests.as_ref()).unwrap_or(0.0) as f32;

    match key {
        "5h" => {
            app.set_five_level(level);
            app.set_five_reset(reset);
            app.set_five_credits(credits);
            app.set_five_requests(requests);
            app.set_five_rate("".into());
            app.set_five_percent_text(percent_text);
            app.set_five_percent(percent);
            app.set_five_credit_percent(credit_percent);
            app.set_five_request_percent(request_percent);
        }
        "24h" => {
            app.set_day_level(level);
            app.set_day_reset(reset);
            app.set_day_credits(credits);
            app.set_day_requests(requests);
            app.set_day_rate("".into());
            app.set_day_percent_text(percent_text);
            app.set_day_percent(percent);
            app.set_day_credit_percent(credit_percent);
            app.set_day_request_percent(request_percent);
        }
        "7d" => {
            app.set_week_level(level);
            app.set_week_reset(reset);
            app.set_week_credits(credits);
            app.set_week_requests(requests);
            app.set_week_rate("".into());
            app.set_week_percent_text(percent_text);
            app.set_week_percent(percent);
            app.set_week_credit_percent(credit_percent);
            app.set_week_request_percent(request_percent);
        }
        "30d" => {
            app.set_month_level(level);
            app.set_month_reset(reset);
            app.set_month_credits(credits);
            app.set_month_requests(requests);
            app.set_month_rate("".into());
            app.set_month_percent_text(percent_text);
            app.set_month_percent(percent);
            app.set_month_credit_percent(credit_percent);
            app.set_month_request_percent(request_percent);
        }
        _ => {}
    }
}

fn gui_load_dotenv() -> Result<HashMap<String, String>, String> {
    ng::load_dotenv_custom(None).or_else(|error| {
        eprintln!("vimit-gui: {error}");
        Ok(HashMap::new())
    })
}

fn load_dashboard(
    force_demo: bool,
    http: &ng::HttpClient,
    account: &Arc<Mutex<Option<GuiAccount>>>,
    warning: f64,
    danger: f64,
) -> Result<ng::Dashboard, String> {
    let dotenv = gui_load_dotenv()?;
    let config = runtime_config(&dotenv, account);
    let (payload, source) = if force_demo {
        (
            ng::demo_payload(),
            "источник: встроенные демо-данные".to_string(),
        )
    } else if config.api_key.is_empty() {
        (
            ng::demo_payload(),
            "источник: демо; добавьте VIBEMODE_API_KEY в .env для live-лимитов".to_string(),
        )
    } else {
        (
            http.fetch_me(&config.api_key, &config.api_base)?,
            format!("источник: live VibeMode /v1/me на {}", config.api_base),
        )
    };

    let windows = ng::summarize_me(&payload, warning, danger);
    let status = ng::dashboard_status(&windows);
    let agent = ng::read_agent_status(&config.abtop_bin);

    Ok(ng::Dashboard {
        source,
        status,
        agent: agent.summary,
        token_rate: agent.token_rate,
        windows,
    })
}

fn runtime_config(
    dotenv: &HashMap<String, String>,
    account: &Arc<Mutex<Option<GuiAccount>>>,
) -> ng::RuntimeConfig {
    let acct = account.lock().unwrap();
    let (api_key_env, api_base_override) = match acct.as_ref() {
        Some(a) => (
            a.api_key_env.as_deref().unwrap_or("VIBEMODE_API_KEY"),
            a.api_base.clone(),
        ),
        None => ("VIBEMODE_API_KEY", None),
    };
    ng::RuntimeConfig {
        api_base: api_base_override
            .or_else(|| ng::config_value("VIBEMODE_API_BASE", dotenv))
            .unwrap_or_else(|| ng::DEFAULT_API_BASE.to_string()),
        api_key: ng::config_value(api_key_env, dotenv).unwrap_or_default(),
        abtop_bin: ng::config_value("ABTOP_BIN", dotenv)
            .unwrap_or_else(|| ng::DEFAULT_ABTOP_BIN.to_string()),
    }
}
