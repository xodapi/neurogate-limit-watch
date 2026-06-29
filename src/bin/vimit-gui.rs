#![allow(clippy::collapsible_if)]

use slint::{ComponentHandle, SharedString, Timer, TimerMode, Weak};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tray_icon::{
    Icon, MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};

use vimit as ng;

slint::include_modules!();

thread_local! {
    static TRAY_ICON: RefCell<Option<TrayIcon>> = const { RefCell::new(None) };
}

fn create_status_icon(color: (u8, u8, u8)) -> Icon {
    let width = 32;
    let height = 32;
    let mut rgba = vec![0u8; width * height * 4];

    let center_x = 16.0;
    let center_y = 16.0;
    let radius = 10.0;

    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - center_x;
            let dy = y as f32 - center_y;
            let dist = (dx * dx + dy * dy).sqrt();

            let idx = (y * width + x) * 4;
            if dist <= radius {
                rgba[idx] = color.0;
                rgba[idx + 1] = color.1;
                rgba[idx + 2] = color.2;
                rgba[idx + 3] = 255;
            } else if dist <= radius + 1.0 {
                let alpha = ((1.0 - (dist - radius)) * 255.0) as u8;
                rgba[idx] = color.0;
                rgba[idx + 1] = color.1;
                rgba[idx + 2] = color.2;
                rgba[idx + 3] = alpha;
            }
        }
    }

    Icon::from_rgba(rgba, width as u32, height as u32).expect("failed to create tray icon")
}

#[derive(Debug, Clone, Default)]
struct GuiAccount {
    api_key_env: Option<String>,
    api_base: Option<String>,
}

fn load_gui_accounts() -> (Vec<String>, Vec<GuiAccount>) {
    if let Ok(config) = ng::cli::accounts::AccountsConfig::load() {
        let names = config.list_names();
        let configs: Vec<GuiAccount> = names
            .iter()
            .filter_map(|n| config.resolve(n).ok())
            .map(|r| GuiAccount {
                api_key_env: r.api_key_env.clone(),
                api_base: r.api_base.clone(),
            })
            .collect();
        (names, configs)
    } else {
        (vec![], vec![])
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let is_overlay = args.iter().any(|arg| arg == "--overlay");
    let is_compact = args.iter().any(|arg| arg == "--compact");

    ng::cli::update::start_background_check();
    let app = AppWindow::new().expect("cannot initialize Slint window");

    if is_overlay {
        app.set_is_overlay(true);
        if is_compact {
            app.set_is_compact(true);
        }
    }

    // Initialize System Tray
    let tray_menu = Menu::new();
    let show_item = MenuItem::new("Показать окно", true, None);
    let quit_item = MenuItem::new("Выход", true, None);
    tray_menu.append(&show_item).unwrap();
    tray_menu.append(&quit_item).unwrap();

    let show_id = show_item.id().clone();
    let quit_id = quit_item.id().clone();

    let tray_icon_instance = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("VibeMode Control")
        .with_icon(create_status_icon((141, 150, 170))) // grey by default
        .build()
        .unwrap();

    TRAY_ICON.with(|cell| {
        *cell.borrow_mut() = Some(tray_icon_instance);
    });

    // Minimize to tray on close request
    let weak = app.as_weak();
    app.window().on_close_requested(move || {
        if let Some(app) = weak.upgrade() {
            let _ = app.hide();
        }
        slint::CloseRequestResponse::KeepWindowShown
    });

    app.on_close_overlay(move || {
        let _ = slint::quit_event_loop();
    });

    let countdown_timer = Timer::default();
    let weak = app.as_weak();
    countdown_timer.start(TimerMode::Repeated, Duration::from_secs(1), move || {
        if let Some(app) = weak.upgrade() {
            let seconds = app.get_overlay_reset_seconds();
            if seconds > 0 {
                let next = seconds - 1;
                app.set_overlay_reset_seconds(next);
                app.set_overlay_reset_text(format_overlay_countdown(next).into());
            }
        }
    });

    // Listen for Tray Events
    let tray_timer = Timer::default();
    let weak = app.as_weak();
    let show_id_clone = show_id.clone();
    let quit_id_clone = quit_id.clone();
    tray_timer.start(TimerMode::Repeated, Duration::from_millis(100), move || {
        if let (Ok(event), Some(app)) = (MenuEvent::receiver().try_recv(), weak.upgrade()) {
            if event.id == show_id_clone {
                let _ = app.show();
            } else if event.id == quit_id_clone {
                let _ = slint::quit_event_loop();
            }
        }
        if let (Ok(event), Some(app)) = (TrayIconEvent::receiver().try_recv(), weak.upgrade()) {
            match event {
                TrayIconEvent::Click { button, .. } => {
                    if button == MouseButton::Left {
                        let _ = app.show();
                    }
                }
                TrayIconEvent::DoubleClick { .. } => {
                    let _ = app.show();
                }
                _ => {}
            }
        }
    });

    let http = std::sync::Arc::new(
        ng::HttpClient::new(ng::USER_AGENT_GUI).expect("cannot initialize HTTP client"),
    );

    let router = std::sync::Arc::new(std::sync::Mutex::new(ng::Router::new(
        ng::DEFAULT_API_BASE.to_string(),
        ng::Router::default_fallbacks(),
    )));

    let refresh_gen = Arc::new(AtomicU64::new(0));
    let is_refreshing = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let overlay_history = Arc::new(Mutex::new(OverlayHistory::default()));

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
        let weak_app = app.as_weak();
        app.on_account_changed(move |name| {
            if let Some(idx) = account_names.iter().position(|n| n == name.as_str()) {
                {
                    let mut cur = state.lock().unwrap();
                    *cur = Some(configs[idx].clone());
                }
                if let Some(app) = weak_app.upgrade() {
                    app.invoke_refresh_requested();
                }
            }
        });
    }

    let weak = app.as_weak();
    let http_clone = http.clone();
    let acct = current_acct.clone();
    let router_clone = router.clone();
    let gen1 = refresh_gen.clone();
    let is_ref1 = is_refreshing.clone();
    let history1 = overlay_history.clone();
    app.on_refresh_requested(move || {
        start_refresh(
            weak.clone(),
            false,
            http_clone.clone(),
            acct.clone(),
            router_clone.clone(),
            ng::DEFAULT_WARNING_THRESHOLD,
            ng::DEFAULT_DANGER_THRESHOLD,
            gen1.clone(),
            is_ref1.clone(),
            history1.clone(),
        );
    });

    let weak = app.as_weak();
    let http_clone = http.clone();
    let acct = current_acct.clone();
    let router_clone = router.clone();
    let gen2 = refresh_gen.clone();
    let is_ref2 = is_refreshing.clone();
    let history2 = overlay_history.clone();
    app.on_demo_requested(move || {
        start_refresh(
            weak.clone(),
            true,
            http_clone.clone(),
            acct.clone(),
            router_clone.clone(),
            ng::DEFAULT_WARNING_THRESHOLD,
            ng::DEFAULT_DANGER_THRESHOLD,
            gen2.clone(),
            is_ref2.clone(),
            history2.clone(),
        );
    });

    let weak = app.as_weak();
    let http_clone = http.clone();
    let acct = current_acct.clone();
    let router_clone = router.clone();
    let gen3 = refresh_gen.clone();
    let is_ref3 = is_refreshing.clone();
    let history3 = overlay_history.clone();
    app.on_settings_changed(move |warning, danger| {
        start_refresh(
            weak.clone(),
            false,
            http_clone.clone(),
            acct.clone(),
            router_clone.clone(),
            warning as f64,
            danger as f64,
            gen3.clone(),
            is_ref3.clone(),
            history3.clone(),
        );
    });

    let timer = Timer::default();
    let weak = app.as_weak();
    let http_clone = http.clone();
    let acct = current_acct.clone();
    let router_clone = router.clone();
    let gen4 = refresh_gen.clone();
    let is_ref4 = is_refreshing.clone();
    let history4 = overlay_history.clone();
    timer.start(TimerMode::Repeated, Duration::from_secs(10), move || {
        start_refresh(
            weak.clone(),
            false,
            http_clone.clone(),
            acct.clone(),
            router_clone.clone(),
            ng::DEFAULT_WARNING_THRESHOLD,
            ng::DEFAULT_DANGER_THRESHOLD,
            gen4.clone(),
            is_ref4.clone(),
            history4.clone(),
        );
    });

    start_refresh(
        app.as_weak(),
        false,
        http,
        current_acct.clone(),
        router.clone(),
        ng::DEFAULT_WARNING_THRESHOLD,
        ng::DEFAULT_DANGER_THRESHOLD,
        refresh_gen.clone(),
        is_refreshing.clone(),
        overlay_history.clone(),
    );

    if let Ok(dotenv) = gui_load_dotenv() {
        let config = runtime_config(&dotenv, &current_acct);
        if config.api_key.is_empty() {
            app.set_needs_setup(true);
        }
    }

    app.set_auto_update_check(ng::cli::update::is_auto_check_enabled());

    app.on_auto_update_changed(|enabled| {
        ng::cli::update::set_auto_check_enabled(enabled);
    });

    let weak = app.as_weak();
    app.on_check_updates(move || {
        let weak_clone = weak.clone();
        thread::spawn(move || {
            let _ = weak_clone.upgrade_in_event_loop(|app| {
                app.set_status_text("Проверка обновлений...".into());
            });
            let current_version = ng::VERSION;
            let mut builder = self_update::backends::github::Update::configure();
            builder
                .repo_owner("xodapi")
                .repo_name("vimit")
                .bin_name("vimit")
                .current_version(current_version);

            if let Ok(updater) = builder.build() {
                match updater.get_latest_release() {
                    Ok(latest) => {
                        let is_greater =
                            self_update::version::bump_is_greater(current_version, &latest.version)
                                .unwrap_or(false);
                        let _ = weak_clone.upgrade_in_event_loop(move |app| {
                            if is_greater {
                                app.set_new_version_label(latest.version.clone().into());
                                app.set_status_text(
                                    format!("Доступно обновление: v{}", latest.version).into(),
                                );
                            } else {
                                app.set_new_version_label("".into());
                                app.set_status_text(
                                    format!("У вас последняя версия v{}", current_version).into(),
                                );
                            }
                        });
                    }
                    Err(e) => {
                        let _ = weak_clone.upgrade_in_event_loop(move |app| {
                            app.set_status_text(format!("Ошибка проверки: {}", e).into());
                        });
                    }
                }
            }
        });
    });

    let weak = app.as_weak();
    app.on_update_now(move || {
        let weak_clone = weak.clone();
        thread::spawn(move || {
            let _ = weak_clone.upgrade_in_event_loop(|app| {
                app.set_status_text("Скачивание обновления...".into());
            });
            match ng::cli::update::check_and_update(false) {
                Ok(_) => {
                    let _ = weak_clone.upgrade_in_event_loop(|app| {
                        app.set_status_text(
                            "Обновление установлено! Перезапустите приложение.".into(),
                        );
                        app.set_new_version_label("".into());
                    });
                }
                Err(e) => {
                    let _ = weak_clone.upgrade_in_event_loop(move |app| {
                        app.set_status_text(format!("Ошибка обновления: {}", e).into());
                    });
                }
            }
        });
    });

    app.on_open_config_dir(move || {
        let home = if cfg!(windows) {
            std::env::var("APPDATA")
                .ok()
                .map(std::path::PathBuf::from)
                .or_else(|| {
                    std::env::var("USERPROFILE")
                        .ok()
                        .map(std::path::PathBuf::from)
                })
        } else {
            std::env::var("HOME").ok().map(std::path::PathBuf::from)
        };
        if let Some(home) = home {
            let config_dir = if cfg!(windows) {
                home.join("vimit")
            } else {
                home.join(".config").join("vimit")
            };
            let _ = std::fs::create_dir_all(&config_dir);
            #[cfg(windows)]
            let _ = std::process::Command::new("explorer")
                .arg(&config_dir)
                .spawn();
            #[cfg(target_os = "macos")]
            let _ = std::process::Command::new("open").arg(&config_dir).spawn();
            #[cfg(target_os = "linux")]
            let _ = std::process::Command::new("xdg-open")
                .arg(&config_dir)
                .spawn();
        }
    });

    app.on_open_accounts_config(move || {
        let home = if cfg!(windows) {
            std::env::var("APPDATA")
                .ok()
                .map(std::path::PathBuf::from)
                .or_else(|| {
                    std::env::var("USERPROFILE")
                        .ok()
                        .map(std::path::PathBuf::from)
                })
        } else {
            std::env::var("HOME").ok().map(std::path::PathBuf::from)
        };
        if let Some(home) = home {
            let config_dir = if cfg!(windows) {
                home.join("vimit")
            } else {
                home.join(".config").join("vimit")
            };
            let accounts_toml = config_dir.join("accounts.toml");
            if accounts_toml.exists() {
                #[cfg(windows)]
                let _ = std::process::Command::new("explorer")
                    .arg(&accounts_toml)
                    .spawn();
                #[cfg(target_os = "macos")]
                let _ = std::process::Command::new("open")
                    .arg(&accounts_toml)
                    .spawn();
                #[cfg(target_os = "linux")]
                let _ = std::process::Command::new("xdg-open")
                    .arg(&accounts_toml)
                    .spawn();
            }
        }
    });

    app.run().expect("Slint event loop failed");
}

struct GuiDashboardResult {
    dashboard: ng::Dashboard,
    active_endpoint_label: String,
    five_trend_data: Vec<f32>,
    day_trend_data: Vec<f32>,
    week_trend_data: Vec<f32>,
    month_trend_data: Vec<f32>,
    overlay: OverlayState,
}

#[derive(Default)]
struct OverlayHistory {
    last_used: Option<f64>,
    last_at: Option<Instant>,
    samples: VecDeque<f32>,
}

struct OverlayState {
    credit_rate_text: String,
    token_rate_text: String,
    percent_hour_text: String,
    spark_data: Vec<f32>,
    delta_text: String,
    delta_level: String,
    reset_label: String,
    reset_seconds: i32,
}

#[allow(clippy::too_many_arguments)]
fn start_refresh(
    app: Weak<AppWindow>,
    demo: bool,
    http: Arc<ng::HttpClient>,
    account: Arc<Mutex<Option<GuiAccount>>>,
    router: Arc<Mutex<ng::Router>>,
    warning: f64,
    danger: f64,
    generation: Arc<AtomicU64>,
    is_refreshing: Arc<std::sync::atomic::AtomicBool>,
    overlay_history: Arc<Mutex<OverlayHistory>>,
) {
    if is_refreshing.swap(true, Ordering::SeqCst) {
        return;
    }

    let my_gen = generation.fetch_add(1, Ordering::Relaxed) + 1;
    thread::spawn(move || {
        let result = load_dashboard(
            demo,
            &http,
            &account,
            warning,
            danger,
            &router,
            &overlay_history,
        );
        if generation.load(Ordering::Relaxed) != my_gen {
            is_refreshing.store(false, Ordering::SeqCst);
            return;
        }
        let _ = app.upgrade_in_event_loop(move |app| {
            apply_dashboard(&app, result);
        });
        is_refreshing.store(false, Ordering::SeqCst);
    });
}
fn apply_dashboard(app: &AppWindow, result: Result<GuiDashboardResult, String>) {
    if let Ok(res) = result.as_ref() {
        let max_percent = res
            .dashboard
            .windows
            .iter()
            .map(|w| w.percent)
            .fold(0.0f64, f64::max);

        let color = if max_percent >= 90.0 {
            (214, 111, 115) // red
        } else if max_percent >= 75.0 {
            (202, 164, 95) // yellow
        } else if max_percent > 0.0 {
            (120, 173, 132) // green
        } else {
            (141, 150, 170) // grey
        };

        let mut tooltip_parts = Vec::new();
        for w in &res.dashboard.windows {
            let level_emoji = match w.level.as_str() {
                "danger" => "🚨",
                "warning" => "⚠️",
                "ok" => "✅",
                _ => "⚪",
            };
            tooltip_parts.push(format!(
                "{}: {} {} ({:.1}%)",
                w.key, level_emoji, w.level, w.percent
            ));
        }
        let tooltip_text = if tooltip_parts.is_empty() {
            "VibeMode Control".to_string()
        } else {
            format!("VibeMode Control\n{}", tooltip_parts.join("\n"))
        };

        TRAY_ICON.with(|cell| {
            if let Some(ref mut tray) = *cell.borrow_mut() {
                let _ = tray.set_icon(Some(create_status_icon(color)));
                let _ = tray.set_tooltip(Some(tooltip_text));
            }
        });
    }

    let offline_min = ng::get_offline_duration_min()
        .map(|m| m as i32)
        .unwrap_or(-1);
    app.set_api_offline_min(offline_min);

    match result {
        Ok(res) => {
            let dashboard = res.dashboard;
            app.set_status_text(dashboard.status.into());
            app.set_source_text(dashboard.source.into());
            app.set_agent_text(dashboard.agent.into());
            app.set_token_rate_text(dashboard.token_rate.clone().into());
            let raw_rate = dashboard
                .token_rate
                .split_whitespace()
                .next()
                .and_then(|s| s.replace(',', ".").parse::<f32>().ok())
                .unwrap_or(0.0);
            app.set_token_rate_raw(raw_rate);
            app.set_active_endpoint_label(res.active_endpoint_label.into());
            app.set_overlay_credit_rate_text(res.overlay.credit_rate_text.into());
            app.set_overlay_token_rate_text(res.overlay.token_rate_text.into());
            app.set_overlay_percent_hour_text(res.overlay.percent_hour_text.into());
            app.set_overlay_delta_text(res.overlay.delta_text.into());
            app.set_overlay_delta_level(res.overlay.delta_level.into());
            app.set_overlay_reset_label(res.overlay.reset_label.into());
            app.set_overlay_reset_seconds(res.overlay.reset_seconds);
            app.set_overlay_reset_text(format_overlay_countdown(res.overlay.reset_seconds).into());

            if let Some(latest) = ng::cli::update::latest_checked_version() {
                app.set_new_version_label(latest.into());
            } else {
                app.set_new_version_label("".into());
            }

            let create_model =
                |v: Vec<f32>| slint::ModelRc::new(std::rc::Rc::new(slint::VecModel::from(v)));
            app.set_five_trend_data(create_model(res.five_trend_data));
            app.set_day_trend_data(create_model(res.day_trend_data));
            app.set_week_trend_data(create_model(res.week_trend_data));
            app.set_month_trend_data(create_model(res.month_trend_data));
            app.set_overlay_spark_data(create_model(res.overlay.spark_data));

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
            app.set_error_text(msg.into());
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
    let percent_text: SharedString = ng::format_percent(window.percent).into();
    let percent = window.percent as f32;
    let credit_percent =
        ng::peak_percent(window.credits.as_ref(), window.requests.as_ref()).unwrap_or(0.0) as f32;
    let request_percent =
        ng::peak_percent(window.credits.as_ref(), window.requests.as_ref()).unwrap_or(0.0) as f32;

    let donut_remaining = match window.credits.as_ref() {
        Some(m) => ng::short_number(m.remaining),
        None => "н/д".to_string(),
    };
    let donut_limit = match window.credits.as_ref() {
        Some(m) => ng::short_number(m.limit),
        None => "н/д".to_string(),
    };

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
            app.set_donut_remaining(donut_remaining.into());
            app.set_donut_limit(donut_limit.into());
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
    router: &Arc<Mutex<ng::Router>>,
    overlay_history: &Arc<Mutex<OverlayHistory>>,
) -> Result<GuiDashboardResult, String> {
    let dotenv = gui_load_dotenv()?;
    let config = runtime_config(&dotenv, account);
    let ((payload, source), active_endpoint_label) = if force_demo {
        (
            (
                ng::demo_payload(),
                "источник: встроенные демо-данные".to_string(),
            ),
            "demo".to_string(),
        )
    } else if config.api_key.is_empty() {
        (
            (
                ng::demo_payload(),
                "источник: демо; добавьте VIBEMODE_API_KEY в .env для live-лимитов".to_string(),
            ),
            "demo".to_string(),
        )
    } else {
        let mut r = router.lock().unwrap();
        let (val, label) = http.fetch_me_with_retry(&config.api_key, &mut r, &config.api_base)?;
        (
            (
                val,
                format!("источник: live VibeMode /v1/me на {}", r.active_endpoint()),
            ),
            label,
        )
    };

    let windows = ng::summarize_me(&payload, warning, danger);
    let status = ng::dashboard_status(&windows);
    let agent = ng::read_agent_status(&config.abtop_bin);
    let overlay = build_overlay_state(&windows, &agent.token_rate, overlay_history);

    // Save snapshot to TrendStore and load trend data
    let mut five_trend_data = Vec::new();
    let mut day_trend_data = Vec::new();
    let mut week_trend_data = Vec::new();
    let mut month_trend_data = Vec::new();

    if let Ok(Some(store)) = ng::cli::trends::TrendStore::open() {
        if !force_demo && !config.api_key.is_empty() {
            let _ = store.save_snapshot(&windows, chrono::Utc::now());
        }
        if let Ok(days) = store.query_trends(15) {
            for day in &days {
                if let Some(w) = day.windows.iter().find(|w| w.key == "5h") {
                    five_trend_data.push(w.peak_max as f32);
                }
                if let Some(w) = day.windows.iter().find(|w| w.key == "24h") {
                    day_trend_data.push(w.peak_max as f32);
                }
                if let Some(w) = day.windows.iter().find(|w| w.key == "7d") {
                    week_trend_data.push(w.peak_max as f32);
                }
                if let Some(w) = day.windows.iter().find(|w| w.key == "30d") {
                    month_trend_data.push(w.peak_max as f32);
                }
            }
        }
    }

    // Generate nice mock sparkline values if we have no historical snapshots yet
    if five_trend_data.is_empty() {
        five_trend_data = vec![
            10.0, 15.0, 20.0, 35.0, 40.0, 30.0, 45.0, 50.0, 40.0, 55.0, 60.0, 65.0, 78.0, 70.0,
            78.0,
        ];
        day_trend_data = vec![
            20.0, 25.0, 30.0, 42.0, 35.0, 40.0, 48.0, 55.0, 50.0, 60.0, 58.0, 62.0, 70.0, 65.0,
            75.0,
        ];
        week_trend_data = vec![
            15.0, 18.0, 22.0, 30.0, 28.0, 32.0, 38.0, 45.0, 42.0, 50.0, 48.0, 52.0, 58.0, 55.0,
            62.0,
        ];
        month_trend_data = vec![
            5.0, 8.0, 12.0, 18.0, 15.0, 20.0, 25.0, 32.0, 28.0, 35.0, 32.0, 38.0, 42.0, 40.0, 45.0,
        ];
    }

    Ok(GuiDashboardResult {
        dashboard: ng::Dashboard {
            source,
            status,
            agent: agent.summary,
            token_rate: agent.token_rate,
            windows,
            daily: None,
        },
        active_endpoint_label,
        five_trend_data,
        day_trend_data,
        week_trend_data,
        month_trend_data,
        overlay,
    })
}

fn build_overlay_state(
    windows: &[ng::WindowState],
    token_rate_text: &str,
    history: &Arc<Mutex<OverlayHistory>>,
) -> OverlayState {
    let now = Instant::now();
    let five = windows.iter().find(|window| window.key == "5h");
    let current_used = five.and_then(|window| window.credits.as_ref().map(|metric| metric.used));
    let credit_limit = five
        .and_then(|window| window.credits.as_ref().map(|metric| metric.limit))
        .unwrap_or(0.0);

    let mut history = history.lock().unwrap();
    let mut credit_rate = 0.0f32;
    if let (Some(used), Some(last_used), Some(last_at)) =
        (current_used, history.last_used, history.last_at)
    {
        let elapsed_min = now.duration_since(last_at).as_secs_f64() / 60.0;
        if elapsed_min > 0.0 {
            credit_rate = ((used - last_used).max(0.0) / elapsed_min) as f32;
        }
    }

    if let Some(used) = current_used {
        history.last_used = Some(used);
        history.last_at = Some(now);
    }

    if history.samples.len() == 15 {
        history.samples.pop_front();
    }
    history.samples.push_back(credit_rate.max(0.0));

    let samples: Vec<f32> = history.samples.iter().copied().collect();
    let spark_data = scale_samples(&samples);
    let token_rate = parse_rate_value(token_rate_text).unwrap_or(credit_rate * 750.0);
    let percent_hour = if credit_limit > 0.0 {
        (credit_rate as f64 / credit_limit * 60.0 * 100.0) as f32
    } else {
        0.0
    };
    let (delta_text, delta_level) = overlay_delta(&samples);
    let (reset_label, reset_seconds) = nearest_reset(windows);

    OverlayState {
        credit_rate_text: format!("{} кред/мин", one_decimal_local(credit_rate)),
        token_rate_text: format!("{} токены/мин", one_decimal_local(token_rate)),
        percent_hour_text: format!("{}%/час", one_decimal_local(percent_hour)),
        spark_data,
        delta_text,
        delta_level,
        reset_label,
        reset_seconds,
    }
}

fn scale_samples(samples: &[f32]) -> Vec<f32> {
    let max = samples.iter().copied().fold(0.0f32, f32::max).max(1.0);
    samples
        .iter()
        .map(|value| ((*value / max) * 100.0).clamp(2.0, 100.0))
        .collect()
}

fn overlay_delta(samples: &[f32]) -> (String, String) {
    let Some((&latest, previous)) = samples.split_last() else {
        return ("стабильно".to_string(), "ok".to_string());
    };
    if previous.is_empty() {
        return ("старт".to_string(), "ok".to_string());
    }
    let avg = previous.iter().sum::<f32>() / previous.len() as f32;
    if avg <= 0.05 {
        if latest <= 0.05 {
            return ("стабильно".to_string(), "ok".to_string());
        }
        return ("новый расход".to_string(), "warning".to_string());
    }
    let delta = ((latest - avg) / avg) * 100.0;
    let level = if delta >= 50.0 {
        "danger"
    } else if delta >= 15.0 {
        "warning"
    } else {
        "ok"
    };
    let sign = if delta >= 0.0 { "+" } else { "" };
    (
        format!("{sign}{}% к норме", one_decimal_local(delta)),
        level.to_string(),
    )
}

fn nearest_reset(windows: &[ng::WindowState]) -> (String, i32) {
    let nearest = windows
        .iter()
        .filter_map(|window| {
            window
                .reset_in_seconds
                .map(|seconds| (window.key, seconds.max(0)))
        })
        .min_by_key(|(_, seconds)| *seconds);

    match nearest {
        Some((key, seconds)) => (
            format!("сброс {key} окна"),
            seconds.min(i32::MAX as i64) as i32,
        ),
        None => ("сброс окна".to_string(), -1),
    }
}

fn format_overlay_countdown(seconds: i32) -> String {
    if seconds < 0 {
        "unknown".to_string()
    } else {
        ng::format_duration_secs(seconds as i64)
    }
}

fn parse_rate_value(text: &str) -> Option<f32> {
    text.split_whitespace()
        .next()
        .and_then(|value| value.replace(',', ".").parse::<f32>().ok())
}

fn one_decimal_local(value: f32) -> String {
    format!("{value:.1}").replace('.', ",")
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
