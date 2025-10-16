use crate::config::OpenAIConfig;
use base64::Engine as _;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
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
    max_retries: u32,
    backoff_millis: u64,
}

impl OpenAIClient {
    pub fn new(cfg: OpenAIConfig) -> Self {
        let http = reqwest::Client::builder()
            .gzip(true)
            .brotli(true)
            .timeout(std::time::Duration::from_secs(cfg.timeout_secs))
            .build()
            .expect("client");
        Self {
            cfg,
            http,
            max_retries: 0,
            backoff_millis: 100,
        }
    }

    pub fn with_max_retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }
    pub fn with_backoff_millis(mut self, ms: u64) -> Self {
        self.backoff_millis = ms;
        self
    }

    pub async fn chatgpt_codex_post(&self, body: Value) -> Result<Value, OpenAIError> {
        let (mut access_token, mut account_id, refresh_token_opt) =
            load_chatgpt_tokens(self.cfg.codex_home.clone())
                .map_err(|e| OpenAIError::Auth(format!("auth load: {e}")))?;
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", access_token)).unwrap(),
        );
        headers.insert(
            HeaderName::from_static("chatgpt-account-id"),
            HeaderValue::from_str(&account_id).unwrap(),
        );
        let url = format!("{}/codex", self.cfg.chatgpt_base_url.trim_end_matches('/'));
        let mut attempts = 0u32;
        let mut tried_refresh = false;
        loop {
            attempts += 1;
            let res = self
                .http
                .post(&url)
                .headers(headers.clone())
                .body(body.to_string())
                .send()
                .await
                .map_err(|e| OpenAIError::Network(e.to_string()))?;
            let status = res.status();
            let hdrs = res.headers().clone();
            let text = res
                .text()
                .await
                .map_err(|e| OpenAIError::Network(e.to_string()))?;
            if status.is_success() {
                let v: Value =
                    serde_json::from_str(&text).map_err(|e| OpenAIError::Decode(e.to_string()))?;
                return Ok(v);
            } else if status.as_u16() == 401 || status.as_u16() == 403 {
                if !tried_refresh {
                    if let Some(rt) = refresh_token_opt.clone() {
                        if let Err(e) =
                            refresh_chatgpt_tokens(self.cfg.codex_home.clone(), &self.http, &rt)
                                .await
                        {
                            return Err(OpenAIError::Auth(format!("refresh failed: {e}")));
                        }
                        if let Ok((new_access, new_acc, _)) =
                            load_chatgpt_tokens(self.cfg.codex_home.clone())
                        {
                            access_token = new_access;
                            account_id = new_acc;
                            headers.insert(
                                AUTHORIZATION,
                                HeaderValue::from_str(&format!("Bearer {}", access_token)).unwrap(),
                            );
                            headers.insert(
                                HeaderName::from_static("chatgpt-account-id"),
                                HeaderValue::from_str(&account_id).unwrap(),
                            );
                        }
                        tried_refresh = true;
                        continue;
                    }
                }
                return Err(OpenAIError::Auth(format!("{}", status)));
            } else if status.as_u16() == 429 && attempts <= self.max_retries {
                let mut delay_ms = self.backoff_millis;
                if let Some(v) = hdrs.get("retry-after") {
                    if let Ok(s) = v.to_str() {
                        if let Ok(secs) = s.parse::<u64>() {
                            delay_ms = secs * 1000;
                        }
                    }
                }
                if delay_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                continue;
            } else if attempts <= self.max_retries && status.is_server_error() {
                if self.backoff_millis > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(self.backoff_millis)).await;
                }
                continue;
            } else {
                return Err(OpenAIError::Http {
                    status: status.as_u16(),
                    body: text,
                });
            }
        }
    }
}

use reqwest::header::HeaderName;

fn load_chatgpt_tokens(codex_home: PathBuf) -> std::io::Result<(String, String, Option<String>)> {
    let auth_path = codex_home.join("auth.json");
    let raw = fs::read_to_string(&auth_path)?;
    let v: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| std::io::Error::other(format!("parse auth.json: {e}")))?;
    let tokens = v
        .get("tokens")
        .ok_or(std::io::Error::other("missing tokens"))?;
    let access_token = tokens
        .get("access_token")
        .and_then(|s| s.as_str())
        .ok_or(std::io::Error::other("missing access_token"))?
        .to_string();
    let account_id = tokens
        .get("account_id")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .or_else(|| decode_account_from_id_token(tokens.get("id_token").and_then(|s| s.as_str())))
        .ok_or(std::io::Error::other(
            "missing account_id in tokens or id_token",
        ))?;
    let refresh_token = tokens
        .get("refresh_token")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());
    Ok((access_token, account_id, refresh_token))
}

fn decode_account_from_id_token(id_token_opt: Option<&str>) -> Option<String> {
    let idt = id_token_opt?;
    let parts: Vec<&str> = idt.split('.').collect();
    if parts.len() < 2 {
        return None;
    }
    let payload_b64 = parts[1];
    let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_b64)
        .ok()?;
    let payload: serde_json::Value = serde_json::from_slice(&payload_bytes).ok()?;
    if let Some(auth) = payload.get("https://api.openai.com/auth") {
        if let Some(acc) = auth.get("chatgpt_account_id").and_then(|s| s.as_str()) {
            return Some(acc.to_string());
        }
    }
    None
}

const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

async fn refresh_chatgpt_tokens(
    codex_home: PathBuf,
    client: &reqwest::Client,
    refresh_token: &str,
) -> std::io::Result<()> {
    let url = "https://auth.openai.com/oauth/token";
    let body = serde_json::json!({
        "client_id": CLIENT_ID,
        "grant_type": "refresh_token",
        "refresh_token": refresh_token,
        "scope": "openid profile email"
    });
    let res = client
        .post(url)
        .header(CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .await
        .map_err(std::io::Error::other)?;
    if !res.status().is_success() {
        return Err(std::io::Error::other(format!(
            "refresh status {}",
            res.status()
        )));
    }
    let v: serde_json::Value = res.json().await.map_err(std::io::Error::other)?;
    let id_token = v
        .get("id_token")
        .and_then(|s| s.as_str())
        .ok_or(std::io::Error::other("missing id_token in refresh"))?
        .to_string();
    let access_token = v
        .get("access_token")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());
    let new_refresh = v
        .get("refresh_token")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());

    let path = codex_home.join("auth.json");
    let raw = fs::read_to_string(&path)?;
    let mut root: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| std::io::Error::other(format!("parse auth.json: {e}")))?;
    let tokens = root
        .get_mut("tokens")
        .ok_or(std::io::Error::other("missing tokens"))?;
    if let Some(obj) = tokens.as_object_mut() {
        obj.insert("id_token".into(), serde_json::Value::String(id_token));
        if let Some(acc) = access_token {
            obj.insert("access_token".into(), serde_json::Value::String(acc));
        }
        if let Some(rt) = new_refresh {
            obj.insert("refresh_token".into(), serde_json::Value::String(rt));
        }
    }
    // Update last_refresh to now (RFC3339)
    let now = chrono::Utc::now().to_rfc3339();
    root.as_object_mut()
        .unwrap()
        .insert("last_refresh".into(), serde_json::Value::String(now));
    fs::write(
        path,
        serde_json::to_string_pretty(&root).map_err(std::io::Error::other)?,
    )
}
