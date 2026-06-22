use slint::{ComponentHandle, SharedString, Timer, TimerMode, Weak};
use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use neurogate_limit_watch as ng;

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
            }
        }
    }
}

fn main() {
    let app = AppWindow::new().expect("cannot initialize Slint window");
    let http = std::sync::Arc::new(ng::HttpClient::new(ng::USER_AGENT_GUI).expect("cannot initialize HTTP client"));

    let weak = app.as_weak();
    let http_clone = http.clone();
    app.on_refresh_requested(move || {
        start_refresh(weak.clone(), false, http_clone.clone());
    });

    let weak = app.as_weak();
    let http_clone = http.clone();
    app.on_demo_requested(move || {
        start_refresh(weak.clone(), true, http_clone.clone());
    });

    let timer = Timer::default();
    let weak = app.as_weak();
    let http_clone = http.clone();
    timer.start(TimerMode::Repeated, Duration::from_secs(10), move || {
        start_refresh(weak.clone(), false, http_clone.clone());
    });

    start_refresh(app.as_weak(), false, http);
    app.run().expect("Slint event loop failed");
}

fn start_refresh(app: Weak<AppWindow>, demo: bool, http: std::sync::Arc<ng::HttpClient>) {
    thread::spawn(move || {
        let result = load_dashboard(demo, &http);
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
            app.set_status_text(format!("Ошибка: {error}").into());
            app.set_source_text("источник: NeuroGate недоступен".into());
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
    let credit_percent = ng::peak_percent(window.credits.as_ref(), window.requests.as_ref())
        .unwrap_or(0.0) as f32;
    let request_percent = ng::peak_percent(window.credits.as_ref(), window.requests.as_ref())
        .unwrap_or(0.0) as f32;

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

fn load_dashboard(force_demo: bool, http: &ng::HttpClient) -> Result<ng::Dashboard, String> {
    let dotenv = ng::load_dotenv_custom(None)?;
    let config = runtime_config(&dotenv);
    let (payload, source) = if force_demo {
        (
            ng::demo_payload(),
            "источник: встроенные демо-данные".to_string(),
        )
    } else if config.api_key.is_empty() {
        (
            ng::demo_payload(),
            "источник: демо; добавьте NEUROGATE_API_KEY в .env для live-лимитов".to_string(),
        )
    } else {
        (
            http.fetch_me(&config.api_key, &config.api_base)?,
            format!("источник: live NeuroGate /v1/me на {}", config.api_base),
        )
    };

    let windows = ng::summarize_me(&payload, 75.0, 90.0);
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

fn runtime_config(dotenv: &HashMap<String, String>) -> ng::RuntimeConfig {
    ng::RuntimeConfig {
        api_base: ng::config_value("NEUROGATE_API_BASE", dotenv)
            .unwrap_or_else(|| ng::DEFAULT_API_BASE.to_string()),
        api_key: ng::config_value("NEUROGATE_API_KEY", dotenv).unwrap_or_default(),
        abtop_bin: ng::config_value("ABTOP_BIN", dotenv)
            .unwrap_or_else(|| "abtop".to_string()),
    }
}
