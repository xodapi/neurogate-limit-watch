use std::collections::HashMap;

use vimit as ng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertLevel {
    Ok,
    Warning,
    Danger,
}

impl AlertLevel {
    fn from_summary(level: &str) -> Self {
        match level {
            "danger" => Self::Danger,
            "warning" => Self::Warning,
            _ => Self::Ok,
        }
    }

    fn severity(self) -> u8 {
        match self {
            Self::Ok => 1,
            Self::Warning => 2,
            Self::Danger => 3,
        }
    }

    fn is_escalation_from(self, previous: Self) -> bool {
        self.severity() > previous.severity() && self != Self::Ok
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warning => "warning",
            Self::Danger => "danger",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationMessage {
    pub window: String,
    pub level: AlertLevel,
    pub title: String,
    pub body: String,
}

#[derive(Debug)]
pub struct Notifier {
    enabled: bool,
    last_levels: HashMap<String, AlertLevel>,
    failure_reported: bool,
}

impl Notifier {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            last_levels: HashMap::new(),
            failure_reported: false,
        }
    }

    pub fn check_windows(&mut self, windows: &[ng::WindowState]) {
        if !self.enabled {
            return;
        }
        for window in windows {
            // Check for escalation (warning/danger)
            if let Some(message) = next_notification(&mut self.last_levels, window) {
                if let Err(error) = fire_desktop_notification(&message) {
                    if !self.failure_reported {
                        eprintln!("vimit: notification failed (non-fatal): {error}");
                        self.failure_reported = true;
                    }
                }
            }
            // Check for recovery (back to ok from warning/danger)
            if let Some(message) = recovery_notification(&mut self.last_levels, window) {
                let _ = fire_desktop_notification(&message);
            }
        }
    }
}

fn next_notification(
    last_levels: &mut HashMap<String, AlertLevel>,
    window: &ng::WindowState,
) -> Option<NotificationMessage> {
    let level = AlertLevel::from_summary(&window.level);
    let previous = last_levels
        .get(window.key)
        .copied()
        .unwrap_or(AlertLevel::Ok);
    last_levels.insert(window.key.to_string(), level);

    if !level.is_escalation_from(previous) {
        return None;
    }

    let title = match level {
        AlertLevel::Danger => format!("VibeMode: {} window critical", window.key),
        AlertLevel::Warning => format!("VibeMode: {} window high usage", window.key),
        AlertLevel::Ok => return None,
    };
    Some(NotificationMessage {
        window: window.key.to_string(),
        level,
        title,
        body: notification_body(window),
    })
}

fn recovery_notification(
    last_levels: &mut HashMap<String, AlertLevel>,
    window: &ng::WindowState,
) -> Option<NotificationMessage> {
    let level = AlertLevel::from_summary(&window.level);
    let previous = last_levels
        .get(window.key)
        .copied()
        .unwrap_or(AlertLevel::Ok);

    // Only notify on recovery from warning/danger to ok
    if level != AlertLevel::Ok || previous == AlertLevel::Ok {
        return None;
    }

    last_levels.insert(window.key.to_string(), level);

    Some(NotificationMessage {
        window: window.key.to_string(),
        level: AlertLevel::Ok,
        title: format!("VibeMode: {} window recovered", window.key),
        body: recovery_body(window, &previous),
    })
}

fn recovery_body(window: &ng::WindowState, previous: &AlertLevel) -> String {
    let reset = ng::format_duration_opt(window.reset_in_seconds);
    let recovered_from = previous.label();
    format!("Quota recovered from {} | reset {}", recovered_from, reset)
}

fn notification_body(window: &ng::WindowState) -> String {
    let peak = ng::peak_percent(window.credits.as_ref(), window.requests.as_ref())
        .map(|value| format!("{value:.1}%"))
        .unwrap_or_else(|| "n/a".to_string());
    let credits = ng::metric_text_en("credits", window.credits.as_ref());
    let requests = ng::metric_text_en("requests", window.requests.as_ref());
    let reset = ng::format_duration_opt(window.reset_in_seconds);
    format!(
        "{} | peak {peak} | {credits} | {requests} | reset {reset}",
        window.level
    )
}

#[cfg(windows)]
fn fire_desktop_notification(message: &NotificationMessage) -> Result<(), String> {
    let title = powershell_quote(&message.title);
    let body = powershell_quote(&message.body);
    let script = format!(
        r#"
$title = '{title}'
$body = '{body}'
try {{
  [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
  [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null
  $xmlTitle = [System.Security.SecurityElement]::Escape($title)
  $xmlBody = [System.Security.SecurityElement]::Escape($body)
  $xml = New-Object Windows.Data.Xml.Dom.XmlDocument
  $xml.LoadXml("<toast><visual><binding template='ToastGeneric'><text>$xmlTitle</text><text>$xmlBody</text></binding></visual></toast>")
  $toast = [Windows.UI.Notifications.ToastNotification]::new($xml)
  [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier("vimit").Show($toast)
}} catch {{
  try {{
    $icon = if ('{level}' -eq 'danger') {{ 48 }} else {{ 64 }}
    (New-Object -ComObject WScript.Shell).Popup($body, 8, $title, $icon) | Out-Null
  }} catch {{}}
}}
"#,
        level = message.level.label()
    );
    std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &script,
        ])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("cannot start PowerShell notification helper: {error}"))
}

#[cfg(windows)]
fn powershell_quote(text: &str) -> String {
    text.replace('\'', "''").replace(['\r', '\n'], " ")
}

#[cfg(target_os = "macos")]
fn fire_desktop_notification(message: &NotificationMessage) -> Result<(), String> {
    let script = format!(
        "display notification {} with title {}",
        applescript_quote(&message.body),
        applescript_quote(&message.title)
    );
    std::process::Command::new("osascript")
        .args(["-e", &script])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("cannot start osascript notification helper: {error}"))
}

#[cfg(target_os = "macos")]
fn applescript_quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn fire_desktop_notification(message: &NotificationMessage) -> Result<(), String> {
    let urgency = match message.level {
        AlertLevel::Danger => "critical",
        AlertLevel::Warning => "normal",
        AlertLevel::Ok => "low",
    };
    std::process::Command::new("notify-send")
        .args(["-a", "vimit", "-u", urgency, &message.title, &message.body])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("cannot start notify-send: {error}"))
}

#[cfg(not(any(windows, unix)))]
fn fire_desktop_notification(_message: &NotificationMessage) -> Result<(), String> {
    Err("desktop notifications are not supported on this platform".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_window(key: &'static str, level: &'static str, percent: f64) -> ng::WindowState {
        ng::WindowState {
            key,
            credits: Some(ng::Metric {
                used: percent,
                limit: 100.0,
                remaining: 100.0 - percent,
                percent,
            }),
            requests: None,
            reset: "unknown".to_string(),
            reset_in_seconds: Some(3600),
            level: level.to_string(),
            percent,
        }
    }

    #[test]
    fn notifications_only_fire_on_escalation() {
        let mut last_levels = HashMap::new();
        let warning = test_window("5h", "warning", 78.0);
        let danger = test_window("5h", "danger", 96.0);
        let ok = test_window("5h", "ok", 12.0);

        assert_eq!(
            next_notification(&mut last_levels, &warning).unwrap().level,
            AlertLevel::Warning
        );
        assert!(next_notification(&mut last_levels, &warning).is_none());
        assert_eq!(
            next_notification(&mut last_levels, &danger).unwrap().level,
            AlertLevel::Danger
        );
        assert!(next_notification(&mut last_levels, &ok).is_none());
        assert_eq!(
            next_notification(&mut last_levels, &warning).unwrap().level,
            AlertLevel::Warning
        );
    }

    #[test]
    fn alert_level_severity_ordering() {
        assert!(AlertLevel::Warning.severity() > AlertLevel::Ok.severity());
        assert!(AlertLevel::Danger.severity() > AlertLevel::Warning.severity());
        assert!(!AlertLevel::Ok.is_escalation_from(AlertLevel::Ok));
        assert!(AlertLevel::Warning.is_escalation_from(AlertLevel::Ok));
        assert!(AlertLevel::Danger.is_escalation_from(AlertLevel::Warning));
        assert!(!AlertLevel::Ok.is_escalation_from(AlertLevel::Danger));
    }

    #[test]
    fn notification_body_contains_peak() {
        let window = test_window("5h", "warning", 78.0);
        let body = notification_body(&window);
        assert!(body.contains("78.0%"));
        assert!(body.contains("credits"));
    }

    #[test]
    fn recovery_notification_fires_on_recovery() {
        let mut last_levels = HashMap::new();
        let warning = test_window("5h", "warning", 78.0);
        let ok = test_window("5h", "ok", 12.0);

        // First escalation
        assert_eq!(
            next_notification(&mut last_levels, &warning).unwrap().level,
            AlertLevel::Warning
        );

        // Recovery to ok should fire recovery notification
        assert!(recovery_notification(&mut last_levels, &ok).is_some());

        // No duplicate recovery notification
        assert!(recovery_notification(&mut last_levels, &ok).is_none());
    }

    #[test]
    fn recovery_notification_does_not_fire_without_prior_warning() {
        let mut last_levels = HashMap::new();
        let ok = test_window("5h", "ok", 12.0);

        // No prior warning/danger, so no recovery notification
        assert!(recovery_notification(&mut last_levels, &ok).is_none());
    }
}
