use crate::config::OpenAIConfig;
use base64::Engine as _;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::Value;
use sha2::Digest as _;
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
        // Add default headers similar to codex-rs default_client
        use reqwest::header::HeaderMap as Hm;
        let mut default_headers = Hm::new();
        default_headers.insert("originator", HeaderValue::from_static("codex_cli_rs"));
        let http = reqwest::Client::builder()
            .gzip(true)
            .brotli(true)
            .user_agent("codex_cli_rs/0.0.0 (roa)")
            .default_headers(default_headers)
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
        // Choose Accept header based on requested stream mode
        let want_stream = body.get("stream").and_then(|v| v.as_bool()).unwrap_or(true);
        if want_stream {
            headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        } else {
            headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        }
        headers.insert(
            HeaderName::from_static("chatgpt-account-id"),
            HeaderValue::from_str(&account_id).unwrap(),
        );
        // Extra headers observed in codex-rs provider
        let convo_id = format!(
            "codex-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        headers.insert(
            HeaderName::from_static("conversation_id"),
            HeaderValue::from_str(&convo_id).unwrap(),
        );
        headers.insert(
            HeaderName::from_static("session_id"),
            HeaderValue::from_str(&convo_id).unwrap(),
        );
        headers.insert(
            HeaderName::from_static("codex-task-type"),
            HeaderValue::from_static("standard"),
        );
        headers.insert(
            HeaderName::from_static("openai-beta"),
            HeaderValue::from_static("responses=experimental"),
        );
        // Optional niceties that reduce CF interstitials in some environments
        headers.insert(
            HeaderName::from_static("referer"),
            HeaderValue::from_static("https://chatgpt.com/"),
        );
        // Mirror codex-rs provider: base points at /backend-api/codex. Prefer Responses wire at `/responses`.
        let base = self.cfg.chatgpt_base_url.trim_end_matches('/');
        let url_primary = if base.ends_with("/backend-api") {
            format!("{}/codex/responses", base)
        } else if base.ends_with("/backend-api/codex") {
            format!("{}/responses", base)
        } else if base.ends_with("/responses") {
            base.to_string()
        } else {
            format!("{}/responses", base)
        };
        let url_fallback = if base.ends_with("/backend-api") {
            format!("{}/responses", base)
        } else {
            format!("https://chatgpt.com/backend-api/responses")
        };
        let mut attempts = 0u32;
        let mut tried_refresh = false;
        loop {
            attempts += 1;
            eprintln!(
                "[wishcraft_openai] POST {}",
                if attempts == 1 {
                    &url_primary
                } else {
                    &url_fallback
                }
            );
            // Log request shape for visibility (without dumping full prompts)
            if let Some(model) = body.get("model").and_then(|m| m.as_str()) {
                let instr_len = body
                    .get("instructions")
                    .and_then(|v| v.as_str())
                    .map(|s| s.len())
                    .unwrap_or(0);
                let user_len = body
                    .get("input")
                    .and_then(|i| i.as_array())
                    .and_then(|arr| arr.get(0))
                    .and_then(|v| v.get("content"))
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.get(0))
                    .and_then(|v| v.get("text"))
                    .and_then(|t| t.as_str())
                    .map(|s| s.len())
                    .unwrap_or(0);
                // Best-effort prompt hash: hash of instructions + first user text
                let mut hasher = sha2::Sha256::new();
                if let Some(ins) = body.get("instructions").and_then(|v| v.as_str()) {
                    use sha2::Digest;
                    hasher.update(ins.as_bytes());
                }
                if let Some(txt) = body
                    .get("input")
                    .and_then(|i| i.as_array())
                    .and_then(|arr| arr.get(0))
                    .and_then(|v| v.get("content"))
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.get(0))
                    .and_then(|v| v.get("text"))
                    .and_then(|t| t.as_str())
                {
                    use sha2::Digest;
                    hasher.update(txt.as_bytes());
                }
                let prompt_hash = format!("{:x}", hasher.finalize());
                eprintln!(
                    "[wishcraft_openai] model={} instr_len={} user_len={} prompt_hash={}",
                    model, instr_len, user_len, prompt_hash
                );
            }
            let res = self
                .http
                .post(if attempts == 1 {
                    &url_primary
                } else {
                    &url_fallback
                })
                .headers(headers.clone())
                .body(body.to_string())
                .send()
                .await
                .map_err(|e| OpenAIError::Network(e.to_string()))?;
            let status = res.status();
            let hdrs = res.headers().clone();
            // Read raw bytes to avoid decode issues; fall back to lossy string
            let bytes = res
                .bytes()
                .await
                .map_err(|e| OpenAIError::Network(format!("bytes: {}", e)))?;
            let text = String::from_utf8_lossy(&bytes).to_string();
            if status.is_success() {
                // Non-streaming: plain JSON body expected
                if !want_stream {
                    if let Ok(v) = serde_json::from_str::<Value>(&text) {
                        return Ok(v);
                    } else {
                        return Err(OpenAIError::Decode("json body".into()));
                    }
                }
                // Streaming (SSE): parse transcript. Accumulate output_text and usage/model from events.
                let mut out_text = String::new();
                let mut model: Option<String> = None;
                let mut usage: Option<Value> = None;
                for line in text.lines() {
                    let l = line.trim_start();
                    if !l.starts_with("data: ") {
                        continue;
                    }
                    let payload = &l[6..];
                    if payload == "[DONE]" {
                        continue;
                    }
                    let Ok(ev): Result<Value, _> = serde_json::from_str(payload) else {
                        continue;
                    };
                    if let Some(kind) = ev.get("type").and_then(|v| v.as_str()) {
                        match kind {
                            "response.delta" => {
                                if let Some(delta) = ev.get("delta").and_then(|d| d.as_str()) {
                                    out_text.push_str(delta);
                                }
                            }
                            "response.output_item.done" => {
                                if let Some(item) = ev.get("item") {
                                    if item.get("type").and_then(|t| t.as_str()) == Some("message")
                                    {
                                        if let Some(contents) =
                                            item.get("content").and_then(|c| c.as_array())
                                        {
                                            for c in contents {
                                                if c.get("type").and_then(|t| t.as_str())
                                                    == Some("output_text")
                                                {
                                                    if let Some(t) =
                                                        c.get("text").and_then(|s| s.as_str())
                                                    {
                                                        out_text.push_str(t);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            "response.completed" => {
                                if let Some(resp) = ev.get("response") {
                                    model = resp
                                        .get("model")
                                        .and_then(|m| m.as_str())
                                        .map(|s| s.to_string());
                                    usage = resp.get("usage").cloned();
                                }
                            }
                            _ => {}
                        }
                    }
                }
                let mut root = serde_json::json!({ "output_text": out_text });
                if let Some(m) = model {
                    root["model"] = Value::String(m);
                }
                if let Some(u) = usage {
                    root["usage"] = u;
                }
                return Ok(root);
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
                let detail = extract_error_detail(&text);
                return Err(OpenAIError::Auth(format!("{}: {}", status, detail)));
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
            } else if attempts <= self.max_retries
                && (status.is_server_error() || status.as_u16() == 404)
            {
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

    pub async fn chatgpt_codex_post_chat(&self, body: Value) -> Result<Value, OpenAIError> {
        let (mut access_token, mut account_id, refresh_token_opt) =
            load_chatgpt_tokens(self.cfg.codex_home.clone())
                .map_err(|e| OpenAIError::Auth(format!("auth load: {e}")))?;
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", access_token)).unwrap(),
        );
        headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        headers.insert(
            HeaderName::from_static("chatgpt-account-id"),
            HeaderValue::from_str(&account_id).unwrap(),
        );
        let convo_id = format!(
            "codex-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        headers.insert(
            HeaderName::from_static("conversation_id"),
            HeaderValue::from_str(&convo_id).unwrap(),
        );
        headers.insert(
            HeaderName::from_static("session_id"),
            HeaderValue::from_str(&convo_id).unwrap(),
        );
        headers.insert(
            HeaderName::from_static("codex-task-type"),
            HeaderValue::from_static("standard"),
        );
        headers.insert(
            HeaderName::from_static("referer"),
            HeaderValue::from_static("https://chatgpt.com/"),
        );

        // Chat Completions wire
        let base = self.cfg.chatgpt_base_url.trim_end_matches('/');
        let url = if base.ends_with("/backend-api") {
            format!("{}/codex/chat/completions", base)
        } else if base.ends_with("/backend-api/codex") {
            format!("{}/chat/completions", base)
        } else if base.ends_with("/chat/completions") {
            base.to_string()
        } else {
            format!("{}/chat/completions", base)
        };

        let mut attempts = 0u32;
        let mut tried_refresh = false;
        loop {
            attempts += 1;
            eprintln!("[wishcraft_openai] POST {} (chat)", &url);
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
                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                    return Ok(v);
                }
                let last_json = text
                    .lines()
                    .filter_map(|line| {
                        let l = line.trim_start();
                        if let Some(rest) = l.strip_prefix("data: ") {
                            Some(rest)
                        } else {
                            None
                        }
                    })
                    .filter(|s| !s.eq(&"[DONE]"))
                    .last()
                    .ok_or_else(|| OpenAIError::Decode("empty SSE".into()))?;
                let v: Value = serde_json::from_str(last_json)
                    .map_err(|e| OpenAIError::Decode(format!("sse parse: {e}")))?;
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
                let detail = extract_error_detail(&text);
                return Err(OpenAIError::Auth(format!("{}: {}", status, detail)));
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

fn extract_error_detail(text: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(m) = v
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|s| s.as_str())
        {
            return m.to_string();
        }
        if let Some(m) = v.get("message").and_then(|s| s.as_str()) {
            return m.to_string();
        }
    }
    let s = text.trim();
    if s.len() > 512 {
        s[..512].to_string()
    } else {
        s.to_string()
    }
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
