use httpmock::prelude::*;
use serial_test::serial;
use wishcraft_openai::{client::OpenAIClient, config::OpenAIConfig};

#[tokio::test]
#[serial]
async fn retries_on_429_then_succeeds() {
    let server = MockServer::start();

    let _m1 = server.mock(|when, then| {
        when.method(POST).path("/backend-api/codex");
        then.status(429).header("retry-after", "0");
    });
    let m2 = server.mock(|when, then| {
        when.method(POST).path("/backend-api/codex");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"model":"gpt-5-pro","usage":{"total_tokens": 10},"choices":[{"message":{"content":"1. OK"}}]}"#);
    });
    // Prepare CODEX_HOME auth.json
    let dir = tempfile::tempdir().unwrap();
    let auth = serde_json::json!({
        "tokens": {"id_token":"a.b.c","access_token":"Access Token","refresh_token":"r","account_id":"acc-1"}
    });
    std::fs::write(
        dir.path().join("auth.json"),
        serde_json::to_string(&auth).unwrap(),
    )
    .unwrap();
    std::env::set_var("CODEX_HOME", dir.path());
    std::env::set_var(
        "CHATGPT_BASE_URL",
        format!("{}/backend-api", server.base_url()),
    );
    let cfg = OpenAIConfig::from_env_defaults().unwrap();
    let client = OpenAIClient::new(cfg)
        .with_max_retries(2)
        .with_backoff_millis(0);

    let body =
        serde_json::json!({"model":"gpt-5-pro","messages":[{"role":"user","content":"plan"}]});
    let resp = client.chatgpt_codex_post(body).await.expect("api ok");
    assert_eq!(resp.get("model").unwrap().as_str().unwrap(), "gpt-5-pro");
    assert_eq!(m2.hits(), 1, "second call should succeed after one 429");
}
