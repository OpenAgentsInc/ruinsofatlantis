use crate::config::OpenAIConfig;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::Value;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OpenAIError {
    #[error("auth error: {0}")]
    Auth(String),
    #[error("rate limited (retry after: {0:?})")]
    RateLimited(Option<u64>),
    #[error("http {status}: {body}")]
    Http { status: u16, body: String },
    #[error("decode: {0}")]
    Decode(String),
    #[error("network: {0}")]
    Network(String),
}

#[derive(Clone)]
pub struct OpenAIClient {
    pub cfg: OpenAIConfig,
    http: reqwest::Client,
}

impl OpenAIClient {
    pub fn new(cfg: OpenAIConfig) -> Self {
        let http = reqwest::Client::builder()
            .gzip(true)
            .brotli(true)
            .timeout(std::time::Duration::from_secs(cfg.timeout_secs))
            .build()
            .expect("client");
        Self { cfg, http }
    }

    #[cfg(feature = "responses")]
    pub async fn responses_create(&self, body: Value, _stream: bool) -> Result<Value, OpenAIError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.cfg.api_key)).unwrap(),
        );
        if let Some(org) = &self.cfg.organization {
            headers.insert("OpenAI-Organization", HeaderValue::from_str(org).unwrap());
        }
        if let Some(project) = &self.cfg.project {
            headers.insert("OpenAI-Project", HeaderValue::from_str(project).unwrap());
        }

        // Endpoint
        let mut url = format!("{}/responses", self.cfg.base_url.trim_end_matches('/'));
        if self.cfg.azure {
            // Azure uses deployment route and api-version; expect base_url to already include deployment
            if let Some(v) = &self.cfg.azure_api_version {
                let sep = if url.contains('?') { '&' } else { '?' };
                url = format!("{}{}api-version={}", url, sep, v);
            }
        }

        let res = self
            .http
            .post(url)
            .headers(headers)
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| OpenAIError::Network(e.to_string()))?;
        let status = res.status();
        let text = res
            .text()
            .await
            .map_err(|e| OpenAIError::Network(e.to_string()))?;
        if status.is_success() {
            let v: Value =
                serde_json::from_str(&text).map_err(|e| OpenAIError::Decode(e.to_string()))?;
            Ok(v)
        } else if status.as_u16() == 401 || status.as_u16() == 403 {
            Err(OpenAIError::Auth(format!("{}", status)))
        } else if status.as_u16() == 429 {
            Err(OpenAIError::RateLimited(None))
        } else {
            Err(OpenAIError::Http {
                status: status.as_u16(),
                body: text,
            })
        }
    }
}
