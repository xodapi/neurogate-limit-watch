use chrono::{DateTime, Duration as ChronoDuration, SecondsFormat, TimeZone, Utc};
use serde_json::{json, Map, Value};
use slint::{ComponentHandle, SharedString, Timer, TimerMode, Weak};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

slint::slint! {
    import { Button } from "std-widgets.slint";

    component MeterBar inherits Rectangle {
        in property <float> value;
        in property <brush> fill;
        private property <float> clipped: root.value < 0 ? 0 : root.value > 100 ? 100 : root.value;

        height: 12px;
        background: #252c36;
        border-radius: 6px;

        Rectangle {
            width: max(0px, parent.width * root.clipped / 100);
            height: parent.height;
            border-radius: 6px;
            background: root.fill;
        }
    }

    component LimitCard inherits Rectangle {
        in property <string> name;
        in property <string> level;
        in property <string> reset;
        in property <string> credits;
        in property <string> requests;
        in property <string> rate;
        in property <string> percent-text;
        in property <float> percent;
        in property <float> credit-percent;
        in property <float> request-percent;

        in property <bool> dimmed;

        private property <brush> accent: percent >= 90 ? #d66f73 : percent >= 75 ? #caa45f : #78ad84;

        opacity: root.dimmed ? 0.36 : 1.0;
        background: #171d25;
        border-color: root.dimmed ? #252c36 : #334052;
        border-width: 1px;
        border-radius: 8px;
        min-height: 170px;

        VerticalLayout {
            padding: 12px;
            spacing: 6px;

            HorizontalLayout {
                Text {
                    text: root.name;
                    color: #f2f4f8;
                    font-size: 25px;
                    font-weight: 700;
                }

                Rectangle { }

                Rectangle {
                width: 92px;
                    height: 28px;
                    border-radius: 14px;
                    background: root.accent;

                    Text {
                        text: root.level;
                        color: #11141a;
                        font-size: 13px;
                        font-weight: 700;
                        horizontal-alignment: center;
                        vertical-alignment: center;
                    }
                }
            }

            MeterBar {
                value: root.percent;
                fill: root.accent;
            }

            Text {
                text: root.percent-text;
                color: root.accent;
                font-size: 16px;
                font-weight: 700;
            }

            Rectangle {
                height: 34px;
            background: #111721;
                border-radius: 6px;

                VerticalLayout {
                    padding-left: 10px;
                    padding-right: 10px;
                    padding-top: 5px;
                    padding-bottom: 5px;
                    spacing: 4px;

                    HorizontalLayout {
                        spacing: 8px;

                        Text {
                            width: 58px;
                            text: "кред";
                            color: #8d96aa;
                            font-size: 11px;
                        }

                        MeterBar {
                            value: root.credit-percent;
                            fill: #78ad84;
                        }
                    }

                    HorizontalLayout {
                        spacing: 8px;

                        Text {
                            width: 58px;
                            text: "запросы";
                            color: #8d96aa;
                            font-size: 11px;
                        }

                        MeterBar {
                            value: root.request-percent;
                            fill: #7f9fc8;
                        }
                    }
                }
            }

            Text {
                text: root.credits;
                color: #c9d0de;
                font-size: 12px;
            }

            Text {
                text: root.requests;
                color: #c9d0de;
                font-size: 12px;
            }

            Text {
                text: root.rate;
                color: #b8c2d8;
                font-size: 11px;
            }

            Text {
                text: root.reset;
                color: #8d96aa;
                font-size: 11px;
            }
        }
    }

    component SpectrumColumn inherits Rectangle {
        in property <float> value;
        in property <brush> fill;
        in property <string> caption;
        private property <float> clipped: root.value < 0 ? 0 : root.value > 100 ? 100 : root.value;

        width: 74px;
        background: transparent;

        VerticalLayout {
            spacing: 8px;

            Rectangle {
                height: 92px;
            background: #202833;
                border-radius: 8px;

                Rectangle {
                y: parent.height - max(6px, parent.height * root.clipped / 100);
                height: max(6px, parent.height * root.clipped / 100);
                    width: parent.width;
                    border-radius: 8px;
                    background: root.fill;
                }

                Rectangle {
                    x: parent.width / 2 - 6px;
                y: parent.height - max(6px, parent.height * root.clipped / 100) - 8px;
                    width: 12px;
                    height: 12px;
                    border-radius: 6px;
                    background: #f8fbff;
                    border-color: root.fill;
                    border-width: 3px;
                }
            }

            Text {
                text: root.caption;
                color: #9fa8bb;
                font-size: 12px;
                horizontal-alignment: center;
            }
        }
    }

    export component AppWindow inherits Window {
        title: "nglimit-gui";
        preferred-width: 1120px;
        preferred-height: 680px;
        min-width: 860px;
        min-height: 620px;

        callback refresh-requested();
        callback demo-requested();

        in property <string> status-text: "Запуск...";
        in property <string> source-text: "источник: загрузка";
        in property <string> agent-text: "агенты: нет данных";
        in property <string> token-rate-text: "токены/мин: нет данных";
        in property <string> footer-text: ".env: текущая папка, затем рядом с exe";
        in-out property <bool> settings-open: false;
        in-out property <int> period-mode: 0;
        in-out property <bool> risk-only: false;

        in property <string> five-level: "загрузка";
        in property <string> five-reset: "сброс неизвестен";
        in property <string> five-credits: "кредиты: н/д";
        in property <string> five-requests: "запросы: н/д";
        in property <string> five-rate: "темп окна: н/д";
        in property <string> five-percent-text: "0,0% пик";
        in property <float> five-percent: 0;
        in property <float> five-credit-percent: 0;
        in property <float> five-request-percent: 0;

        in property <string> day-level: "загрузка";
        in property <string> day-reset: "сброс неизвестен";
        in property <string> day-credits: "кредиты: н/д";
        in property <string> day-requests: "запросы: н/д";
        in property <string> day-rate: "темп окна: н/д";
        in property <string> day-percent-text: "0,0% пик";
        in property <float> day-percent: 0;
        in property <float> day-credit-percent: 0;
        in property <float> day-request-percent: 0;

        in property <string> week-level: "загрузка";
        in property <string> week-reset: "сброс неизвестен";
        in property <string> week-credits: "кредиты: н/д";
        in property <string> week-requests: "запросы: н/д";
        in property <string> week-rate: "темп окна: н/д";
        in property <string> week-percent-text: "0,0% пик";
        in property <float> week-percent: 0;
        in property <float> week-credit-percent: 0;
        in property <float> week-request-percent: 0;

        in property <string> month-level: "загрузка";
        in property <string> month-reset: "сброс неизвестен";
        in property <string> month-credits: "кредиты: н/д";
        in property <string> month-requests: "запросы: н/д";
        in property <string> month-rate: "темп окна: н/д";
        in property <string> month-percent-text: "0,0% пик";
        in property <float> month-percent: 0;
        in property <float> month-credit-percent: 0;
        in property <float> month-request-percent: 0;

        Rectangle {
            background: #0f131a;

            VerticalLayout {
                padding: 18px;
                spacing: 12px;

                Rectangle {
                    height: 190px;
                    background: #151a22;
                    border-radius: 10px;
                    border-color: #2b3444;
                    border-width: 1px;

                    HorizontalLayout {
                        padding: 16px;
                        spacing: 24px;

                        VerticalLayout {
                            spacing: 10px;

                            Text {
                                text: "NeuroGate Control";
                                color: #f5f7fb;
                                font-size: 28px;
                                font-weight: 800;
                            }

                            Text {
                                text: root.status-text;
                                color: #9fa8bb;
                                font-size: 15px;
                            }

                            Rectangle {
                                height: 1px;
                                background: #2c3549;
                            }

                            Text {
                                text: root.source-text;
                                color: #7fb69c;
                                font-size: 14px;
                                font-weight: 600;
                            }

                            Text {
                                text: root.agent-text;
                                color: #8da6c8;
                                font-size: 14px;
                            }

                            Text {
                                text: root.token-rate-text;
                                color: #c8b56b;
                                font-size: 14px;
                                font-weight: 600;
                            }

                            HorizontalLayout {
                                spacing: 10px;
                                height: 34px;

                                Button {
                                    text: "Обновить";
                                    clicked => { root.refresh-requested(); }
                                }

                                Button {
                                    text: "Демо";
                                    clicked => { root.demo-requested(); }
                                }

                                Button {
                                    text: root.settings-open ? "Скрыть настройки" : "Настройки";
                                    clicked => { root.settings-open = !root.settings-open; }
                                }

                                Rectangle { }
                            }
                        }

                        Rectangle { }

                        SpectrumColumn {
                            caption: "5h";
                            value: root.five-percent;
                            fill: #9a84b8;
                        }
                        SpectrumColumn {
                            caption: "24h";
                            value: root.day-percent;
                            fill: #7fb4bd;
                        }
                        SpectrumColumn {
                            caption: "7d";
                            value: root.week-percent;
                            fill: #c8b56b;
                        }
                        SpectrumColumn {
                            caption: "30d";
                            value: root.month-percent;
                            fill: #7faf7a;
                        }
                    }
                }

                Rectangle {
                    visible: root.settings-open;
                    height: root.settings-open ? 74px : 0px;
                    background: #151a22;
                    border-radius: 8px;
                    border-color: #2b3444;
                    border-width: 1px;

                    HorizontalLayout {
                        padding: 10px;
                        spacing: 12px;

                        VerticalLayout {
                            width: 220px;
                            spacing: 4px;

                            Text {
                                text: "Настройки панели";
                                color: #e6edf5;
                                font-size: 14px;
                                font-weight: 800;
                            }

                            Text {
                                text: "фильтры без чтения приватных project/session данных";
                                color: #8f9aaa;
                                font-size: 11px;
                            }
                        }

                        Button {
                            text: "Все";
                            clicked => {
                                root.period-mode = 0;
                                root.risk-only = false;
                            }
                        }

                        Button {
                            text: "5h + 24h";
                            clicked => {
                                root.period-mode = 1;
                                root.risk-only = false;
                            }
                        }

                        Button {
                            text: "7d + 30d";
                            clicked => {
                                root.period-mode = 2;
                                root.risk-only = false;
                            }
                        }

                        Button {
                            text: "Только риск";
                            clicked => {
                                root.period-mode = 0;
                                root.risk-only = !root.risk-only;
                            }
                        }

                        Rectangle { }

                        Text {
                            text: root.risk-only ? "режим: риск" : root.period-mode == 1 ? "режим: короткие окна" : root.period-mode == 2 ? "режим: длинные окна" : "режим: все окна";
                            color: #aeb8c7;
                            font-size: 13px;
                            vertical-alignment: center;
                        }
                    }
                }

                GridLayout {
                    spacing: 14px;

                    LimitCard {
                        row: 0;
                        col: 0;
                        name: "5h";
                        level: root.five-level;
                    reset: root.five-reset;
                    credits: root.five-credits;
                    requests: root.five-requests;
                    rate: root.five-rate;
                    percent-text: root.five-percent-text;
                    percent: root.five-percent;
                    credit-percent: root.five-credit-percent;
                    request-percent: root.five-request-percent;
                    dimmed: root.period-mode == 2 || (root.risk-only && root.five-percent < 75);
                }

                LimitCard {
                        row: 0;
                        col: 1;
                        name: "24h";
                        level: root.day-level;
                    reset: root.day-reset;
                    credits: root.day-credits;
                    requests: root.day-requests;
                    rate: root.day-rate;
                    percent-text: root.day-percent-text;
                    percent: root.day-percent;
                    credit-percent: root.day-credit-percent;
                    request-percent: root.day-request-percent;
                    dimmed: root.period-mode == 2 || (root.risk-only && root.day-percent < 75);
                }

                LimitCard {
                        row: 1;
                        col: 0;
                        name: "7d";
                        level: root.week-level;
                    reset: root.week-reset;
                    credits: root.week-credits;
                    requests: root.week-requests;
                    rate: root.week-rate;
                    percent-text: root.week-percent-text;
                    percent: root.week-percent;
                    credit-percent: root.week-credit-percent;
                    request-percent: root.week-request-percent;
                    dimmed: root.period-mode == 1 || (root.risk-only && root.week-percent < 75);
                }

                LimitCard {
                        row: 1;
                        col: 1;
                        name: "30d";
                        level: root.month-level;
                    reset: root.month-reset;
                    credits: root.month-credits;
                    requests: root.month-requests;
                    rate: root.month-rate;
                    percent-text: root.month-percent-text;
                    percent: root.month-percent;
                    credit-percent: root.month-credit-percent;
                    request-percent: root.month-request-percent;
                    dimmed: root.period-mode == 1 || (root.risk-only && root.month-percent < 75);
                }
                }

                HorizontalLayout {
                    visible: false;
                    spacing: 12px;
                    height: 0px;

                Button {
                    text: "Обновить";
                    clicked => { root.refresh-requested(); }
                }

                Button {
                    text: "Демо";
                    clicked => { root.demo-requested(); }
                }

                Button {
                    text: root.settings-open ? "Скрыть настройки" : "Настройки";
                    clicked => { root.settings-open = !root.settings-open; }
                }

                    Rectangle { }

                    Text {
                        text: root.footer-text;
                        color: #808aa0;
                        font-size: 13px;
                        vertical-alignment: center;
                    }
                }
            }
        }
    }
}

const DEFAULT_API_BASE: &str = "https://api.neurogate.space";
const USER_AGENT: &str = concat!("neurogate-limit-watch-gui/", env!("CARGO_PKG_VERSION"));

const WINDOWS: [(&str, &str, &str, &str); 4] = [
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

#[derive(Clone, Debug)]
struct Metric {
    used: f64,
    limit: f64,
    remaining: f64,
    percent: f64,
}

#[derive(Clone, Debug)]
struct WindowState {
    key: &'static str,
    level: &'static str,
    reset: String,
    credits: Option<Metric>,
    requests: Option<Metric>,
    percent: f64,
    rate: String,
}

#[derive(Clone, Debug)]
struct Dashboard {
    source: String,
    status: String,
    agent: String,
    token_rate: String,
    windows: Vec<WindowState>,
}

#[derive(Clone, Debug)]
struct AgentStatus {
    summary: String,
    token_rate: String,
}

#[derive(Debug)]
struct RuntimeConfig {
    api_base: String,
    api_key: String,
    abtop_bin: String,
}

fn main() {
    let app = AppWindow::new().expect("cannot initialize Slint window");

    let weak = app.as_weak();
    app.on_refresh_requested(move || {
        start_refresh(weak.clone(), false);
    });

    let weak = app.as_weak();
    app.on_demo_requested(move || {
        start_refresh(weak.clone(), true);
    });

    let timer = Timer::default();
    let weak = app.as_weak();
    timer.start(TimerMode::Repeated, Duration::from_secs(10), move || {
        start_refresh(weak.clone(), false);
    });

    start_refresh(app.as_weak(), false);
    app.run().expect("Slint event loop failed");
}

fn start_refresh(app: Weak<AppWindow>, demo: bool) {
    thread::spawn(move || {
        let result = load_dashboard(demo);
        let _ = app.upgrade_in_event_loop(move |app| {
            apply_dashboard(&app, result);
        });
    });
}

fn apply_dashboard(app: &AppWindow, result: Result<Dashboard, String>) {
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
            app.set_status_text(format!("Ошибка: {error}").into());
            app.set_source_text("источник: NeuroGate недоступен".into());
        }
    }
}

fn apply_window(app: &AppWindow, key: &str, window: Option<&WindowState>) {
    let fallback = WindowState {
        key: "n/a",
        level: "н/д",
        reset: "сброс неизвестен".to_string(),
        credits: None,
        requests: None,
        percent: 0.0,
        rate: "темп окна: н/д".to_string(),
    };
    let window = window.unwrap_or(&fallback);
    let level: SharedString = window.level.into();
    let reset: SharedString = window.reset.clone().into();
    let credits: SharedString = metric_text("кредиты", window.credits.as_ref()).into();
    let requests: SharedString = metric_text("запросы", window.requests.as_ref()).into();
    let rate: SharedString = window.rate.clone().into();
    let percent_text: SharedString = format!("{} пик", format_percent(window.percent)).into();
    let percent = window.percent as f32;
    let credit_percent = metric_percent(window.credits.as_ref()) as f32;
    let request_percent = metric_percent(window.requests.as_ref()) as f32;

    match key {
        "5h" => {
            app.set_five_level(level);
            app.set_five_reset(reset);
            app.set_five_credits(credits);
            app.set_five_requests(requests);
            app.set_five_rate(rate);
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
            app.set_day_rate(rate);
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
            app.set_week_rate(rate);
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
            app.set_month_rate(rate);
            app.set_month_percent_text(percent_text);
            app.set_month_percent(percent);
            app.set_month_credit_percent(credit_percent);
            app.set_month_request_percent(request_percent);
        }
        _ => {}
    }
}

fn load_dashboard(force_demo: bool) -> Result<Dashboard, String> {
    let dotenv = load_dotenv()?;
    let config = runtime_config(&dotenv);
    let (payload, source) = if force_demo {
        (
            demo_payload(),
            "источник: встроенные демо-данные".to_string(),
        )
    } else if config.api_key.is_empty() {
        (
            demo_payload(),
            "источник: демо; добавьте NEUROGATE_API_KEY в .env для live-лимитов".to_string(),
        )
    } else {
        (
            fetch_me(&config.api_key, &config.api_base)?,
            format!("источник: live NeuroGate /v1/me на {}", config.api_base),
        )
    };

    let windows = summarize_me(&payload);
    let status = dashboard_status(&windows);
    let agent = read_agent_status(&config.abtop_bin);

    Ok(Dashboard {
        source,
        status,
        agent: agent.summary,
        token_rate: agent.token_rate,
        windows,
    })
}

fn runtime_config(dotenv: &HashMap<String, String>) -> RuntimeConfig {
    RuntimeConfig {
        api_base: config_value("NEUROGATE_API_BASE", dotenv)
            .unwrap_or_else(|| DEFAULT_API_BASE.to_string()),
        api_key: config_value("NEUROGATE_API_KEY", dotenv).unwrap_or_default(),
        abtop_bin: config_value("ABTOP_BIN", dotenv).unwrap_or_else(|| "abtop".to_string()),
    }
}

fn config_value(key: &str, dotenv: &HashMap<String, String>) -> Option<String> {
    env::var(key)
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| dotenv.get(key).cloned().filter(|value| !value.is_empty()))
}

fn load_dotenv() -> Result<HashMap<String, String>, String> {
    let Some(path) = find_dotenv() else {
        return Ok(HashMap::new());
    };
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read env file {}: {error}", path.display()))?;
    parse_dotenv(&raw).map_err(|error| format!("{}: {error}", path.display()))
}

fn find_dotenv() -> Option<PathBuf> {
    let cwd_env = PathBuf::from(".env");
    if cwd_env.is_file() {
        return Some(cwd_env);
    }

    let exe = env::current_exe().ok()?;
    let dir = exe.parent()?;
    let exe_env = dir.join(".env");
    exe_env.is_file().then_some(exe_env)
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
        values.insert(
            key.trim().to_string(),
            unquote_env_value(value.trim()).to_string(),
        );
    }
    Ok(values)
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

fn demo_payload() -> Value {
    let now = Utc::now();
    let five_start = now - ChronoDuration::minutes(28);
    let day_start = now - ChronoDuration::hours(5);
    let week_start = now - ChronoDuration::days(1);
    let month_start = now - ChronoDuration::days(9);
    json!({
        "usage": {
            "rows": [
                {
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
                    "window5HoursEndsAt": (now + ChronoDuration::hours(4)).to_rfc3339_opts(SecondsFormat::Secs, true),
                    "window24HoursStartedAt": day_start.to_rfc3339_opts(SecondsFormat::Secs, true),
                    "window24HoursEndsAt": (now + ChronoDuration::hours(19)).to_rfc3339_opts(SecondsFormat::Secs, true),
                    "window7DaysStartedAt": week_start.to_rfc3339_opts(SecondsFormat::Secs, true),
                    "window7DaysEndsAt": (now + ChronoDuration::days(6)).to_rfc3339_opts(SecondsFormat::Secs, true),
                    "window30DaysStartedAt": month_start.to_rfc3339_opts(SecondsFormat::Secs, true),
                    "window30DaysEndsAt": (now + ChronoDuration::days(21)).to_rfc3339_opts(SecondsFormat::Secs, true)
                }
            ]
        }
    })
}

fn summarize_me(payload: &Value) -> Vec<WindowState> {
    let rows = extract_usage_rows(payload);
    let now = Utc::now();
    let mut summaries = Vec::new();

    for (key, suffix, start_field, reset_field) in WINDOWS {
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
        let reset = parse_reset(first_value(&rows, reset_field), now);
        let rate = rate_text(
            credits.as_ref(),
            requests.as_ref(),
            first_value(&rows, start_field),
            now,
        );
        let percent = peak_percent(credits.as_ref(), requests.as_ref()).unwrap_or(0.0);
        summaries.push(WindowState {
            key,
            level: window_level(percent),
            reset,
            credits,
            requests,
            percent,
            rate,
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

fn to_number(value: &Value) -> Option<f64> {
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

fn parse_reset(value: Option<&Value>, now: DateTime<Utc>) -> String {
    let Some(value) = value else {
        return "сброс неизвестен".to_string();
    };

    if let Some(datetime) = parse_datetime_value(value) {
        let seconds = (datetime - now).num_seconds().max(0);
        format!("сброс {}", format_duration(seconds))
    } else {
        format!(
            "сброс в {}",
            value
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| value.to_string())
        )
    }
}

fn window_level(percent: f64) -> &'static str {
    if percent >= 90.0 {
        "лимит"
    } else if percent >= 75.0 {
        "внимание"
    } else {
        "норма"
    }
}

fn peak_percent(credits: Option<&Metric>, requests: Option<&Metric>) -> Option<f64> {
    [credits, requests]
        .into_iter()
        .flatten()
        .map(|metric| metric.percent)
        .fold(None, |peak: Option<f64>, percent| {
            Some(peak.map_or(percent, |peak| peak.max(percent)))
        })
}

fn metric_percent(metric: Option<&Metric>) -> f64 {
    metric.map(|metric| metric.percent).unwrap_or(0.0)
}

fn dashboard_status(windows: &[WindowState]) -> String {
    let peak = windows
        .iter()
        .map(|window| window.percent)
        .fold(0.0, f64::max);
    let level = window_level(peak);
    format!(
        "квота: {level} | пик {} | обновлено {}",
        format_percent(peak),
        Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
    )
}

fn metric_text(label: &str, metric: Option<&Metric>) -> String {
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

fn format_duration(seconds: i64) -> String {
    match seconds {
        seconds if seconds < 60 => format!("через {seconds}с"),
        seconds if seconds < 3600 => format!("через {}м", seconds / 60),
        seconds if seconds < 86_400 => {
            format!("через {}ч {}м", seconds / 3600, (seconds % 3600) / 60)
        }
        seconds => format!("через {}д {}ч", seconds / 86_400, (seconds % 86_400) / 3600),
    }
}

fn rate_text(
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

fn format_percent(value: f64) -> String {
    format!("{}%", one_decimal(value))
}

fn one_decimal(value: f64) -> String {
    format!("{value:.1}").replace('.', ",")
}

fn short_rate(value: f64) -> String {
    if value >= 1000.0 {
        short_number(value)
    } else {
        one_decimal(value)
    }
}

fn read_agent_status(binary: &str) -> AgentStatus {
    let Ok(output) = Command::new(binary).arg("--status-json").output() else {
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

fn short_number(value: f64) -> String {
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
