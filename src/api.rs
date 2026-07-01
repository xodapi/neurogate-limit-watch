use serde_json::Value;
use std::fs;
use std::time::{Duration, Instant};

use crate::{DEFAULT_API_BASE, FALLBACK_API_BASE, update_offline_state};

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
        for fb in fallbacks {
            if !endpoints.iter().any(|endpoint| endpoint == &fb) {
                endpoints.push(fb);
            }
        }
        let count = endpoints.len();
        Self {
            endpoints,
            failures: vec![0; count],
            degraded_until: vec![None; count],
            current: 0,
            threshold: 1,
            cooldown: Duration::from_secs(60),
        }
    }

    pub fn default_fallbacks() -> Vec<String> {
        api_fallbacks_for(DEFAULT_API_BASE, true)
    }

    pub fn active_endpoint(&self) -> &str {
        &self.endpoints[self.current]
    }

    pub fn active_label(&self) -> &str {
        Self::label_for_endpoint(&self.endpoints[self.current])
    }

    pub fn label_for_endpoint(url: &str) -> &str {
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
        self.current = 0;
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

pub fn api_fallbacks_for(primary: &str, auto_failover: bool) -> Vec<String> {
    if !auto_failover {
        return Vec::new();
    }
    let normalized = primary.trim_end_matches('/');
    if normalized == DEFAULT_API_BASE {
        vec![FALLBACK_API_BASE.to_string()]
    } else if normalized == FALLBACK_API_BASE {
        vec![DEFAULT_API_BASE.to_string()]
    } else {
        Vec::new()
    }
}

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
        _api_base: &str,
    ) -> Result<(Value, String), String> {
        let max_retries: u32 = 3;
        let base_delay_ms: u64 = 1000;

        for attempt in 0..=max_retries {
            let endpoint = router.active_endpoint().to_string();
            let label = Router::label_for_endpoint(&endpoint).to_string();
            match self.fetch_me(api_key, &endpoint) {
                Ok(value) => {
                    router.record_success();
                    update_offline_state(false);
                    return Ok((value, label));
                }
                Err(error) => {
                    let is_retryable = is_retryable_error(&error);
                    if !is_retryable {
                        update_offline_state(true);
                        return Err(error);
                    }
                    let failed_over = router.record_failure();
                    if failed_over && attempt < max_retries {
                        continue;
                    }
                    if attempt == max_retries {
                        update_offline_state(true);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_fallbacks_prefer_api_then_r_api() {
        let router = Router::new(DEFAULT_API_BASE.to_string(), Router::default_fallbacks());
        assert_eq!(router.active_endpoint(), DEFAULT_API_BASE);
        assert_eq!(router.active_label(), "api");
    }

    #[test]
    fn router_fails_over_to_r_api_after_primary_failure() {
        let mut router = Router::new(DEFAULT_API_BASE.to_string(), Router::default_fallbacks());
        assert!(router.record_failure());
        assert_eq!(router.active_endpoint(), FALLBACK_API_BASE);
        assert_eq!(router.active_label(), "r-api");
    }

    #[test]
    fn fallback_success_returns_to_primary_for_next_poll() {
        let mut router = Router::new(DEFAULT_API_BASE.to_string(), Router::default_fallbacks());
        router.record_failure();
        assert_eq!(router.active_endpoint(), FALLBACK_API_BASE);
        router.record_success();
        assert_eq!(router.active_endpoint(), DEFAULT_API_BASE);
    }

    #[test]
    fn auto_failover_can_be_disabled() {
        assert!(api_fallbacks_for(DEFAULT_API_BASE, false).is_empty());
        let mut router = Router::new(DEFAULT_API_BASE.to_string(), Vec::new());
        assert!(router.record_failure());
        assert_eq!(router.active_endpoint(), DEFAULT_API_BASE);
    }
}
