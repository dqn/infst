use crate::error::{Error, Result};
use reqwest::{Client, Response, StatusCode};
use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;
use tracing::warn;

const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 100;
const MAX_BACKOFF_MS: u64 = 5000;

#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    base_url: String,
    api_key: String,
}

/// Retry configuration for HTTP requests
struct RetryState {
    attempt: u32,
    backoff_ms: u64,
}

impl RetryState {
    fn new() -> Self {
        Self {
            attempt: 0,
            backoff_ms: INITIAL_BACKOFF_MS,
        }
    }

    fn can_retry(&self) -> bool {
        self.attempt < MAX_RETRIES - 1
    }

    fn increment(&mut self) {
        self.attempt += 1;
        self.backoff_ms = (self.backoff_ms * 2).min(MAX_BACKOFF_MS);
    }

    /// Get delay from Retry-After header or use backoff
    fn get_delay(&self, response: &Response) -> u64 {
        response
            .headers()
            .get("Retry-After")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .map(|secs| secs * 1000)
            .unwrap_or(self.backoff_ms)
    }

    async fn wait(&self, delay_ms: u64) {
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    }
}

/// Check if response requires retry and handle logging/waiting
async fn should_retry_response(response: &Response, state: &mut RetryState) -> bool {
    if !state.can_retry() {
        return false;
    }

    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        let delay = state.get_delay(response);
        warn!(
            "Rate limited (attempt {}/{}), retrying in {}ms",
            state.attempt + 1,
            MAX_RETRIES,
            delay
        );
        state.wait(delay).await;
        state.increment();
        return true;
    }

    if response.status().is_server_error() {
        warn!(
            "Server error {} (attempt {}/{}), retrying in {}ms",
            response.status(),
            state.attempt + 1,
            MAX_RETRIES,
            state.backoff_ms
        );
        state.wait(state.backoff_ms).await;
        state.increment();
        return true;
    }

    false
}

impl HttpClient {
    pub fn new(base_url: String, api_key: String) -> Result<Self> {
        let user_agent = format!(
            "Reflux-RS/{} ({})",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS
        );
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(user_agent)
            .build()
            .map_err(|e| Error::NetworkError(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            client,
            base_url,
            api_key,
        })
    }

    /// Execute a request with retry logic
    async fn with_retry<F, Fut>(&self, request_fn: F) -> Result<String>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = reqwest::Result<Response>>,
    {
        let mut state = RetryState::new();

        loop {
            let result = request_fn().await;

            match result {
                Ok(response) => {
                    if should_retry_response(&response, &mut state).await {
                        continue;
                    }
                    let response = response.error_for_status()?;
                    return Ok(response.text().await?);
                }
                Err(e) if (e.is_timeout() || e.is_connect()) && state.can_retry() => {
                    warn!(
                        "Connection error (attempt {}/{}): {}, retrying in {}ms",
                        state.attempt + 1,
                        MAX_RETRIES,
                        e,
                        state.backoff_ms
                    );
                    state.wait(state.backoff_ms).await;
                    state.increment();
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    pub async fn post_form(&self, endpoint: &str, form: HashMap<String, String>) -> Result<String> {
        let url = format!("{}/{}", self.base_url, endpoint);

        let mut form = form;
        form.insert("apikey".to_string(), self.api_key.clone());

        self.with_retry(|| self.client.post(&url).form(&form).send())
            .await
    }

    pub async fn get(&self, url: &str) -> Result<String> {
        self.with_retry(|| self.client.get(url).send()).await
    }
}
