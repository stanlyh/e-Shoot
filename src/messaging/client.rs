use crate::error::{AppError, Result};
use reqwest::StatusCode;
use serde::Serialize;
use std::time::Duration;
use tracing::{debug, warn};

pub struct HttpMessagingClient {
    inner: reqwest::Client,
    pub base_url: String,
    token: String,
    max_retries: u32,
    backoff: Duration,
}

impl HttpMessagingClient {
    pub fn new(
        base_url: String,
        token: String,
        connect_timeout_secs: u64,
        request_timeout_secs: u64,
        max_retries: u32,
        retry_backoff_secs: u64,
    ) -> anyhow::Result<Self> {
        let inner = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(connect_timeout_secs))
            .timeout(Duration::from_secs(request_timeout_secs))
            .build()?;
        Ok(Self {
            inner,
            base_url,
            token,
            max_retries,
            backoff: Duration::from_secs(retry_backoff_secs),
        })
    }

    pub async fn post_json<T: Serialize>(&self, path: &str, body: &T) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let mut attempt = 0u32;

        loop {
            debug!(attempt, url, "sending HTTP request");

            let response = self
                .inner
                .post(&url)
                .bearer_auth(&self.token)
                .json(body)
                .send()
                .await
                .map_err(|e| AppError::Http {
                    url: url.clone(),
                    source: e,
                })?;

            let status = response.status();

            if status.is_success() {
                debug!(url, status = status.as_u16(), "request succeeded");
                return Ok(());
            }

            let retryable = status == StatusCode::TOO_MANY_REQUESTS
                || status.is_server_error();

            if !retryable || attempt >= self.max_retries {
                let body_text = response.text().await.unwrap_or_default();
                return Err(AppError::JobFailed {
                    id: url.clone(),
                    attempts: attempt + 1,
                    reason: format!("HTTP {}: {}", status, body_text),
                });
            }

            attempt += 1;
            warn!(
                attempt,
                status = status.as_u16(),
                backoff_secs = self.backoff.as_secs(),
                "retryable error, backing off"
            );
            tokio::time::sleep(self.backoff).await;
        }
    }
}
