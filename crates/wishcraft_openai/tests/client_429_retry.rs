use httpmock::prelude::*;
use serial_test::serial;
use wishcraft_openai::{client::OpenAIClient, config::OpenAIConfig};

#[tokio::test]
#[serial]
async fn retries_on_429_then_succeeds() {
    let server = MockServer::start();

    let _m1 = server.mock(|when, then| {
        when.method(POST).path("/v1/responses");
        then.status(429).header("retry-after", "0");
    });
    let m2 = server.mock(|when, then| {
        when.method(POST).path("/v1/responses");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"model":"gpt-5-pro","usage":{"total_tokens": 10},"output_text":"1. OK"}"#);
    });

    std::env::set_var("OPENAI_API_KEY", "sk-test");
    let cfg = OpenAIConfig {
        base_url: format!("{}/v1", server.base_url()),
        api_key: "sk-test".into(),
        organization: None,
        project: None,
        model: "gpt-5-pro".into(),
        temperature: Some(0.0),
        timeout_secs: 5,
        azure: false,
        azure_api_version: None,
    };
    let client = OpenAIClient::new(cfg)
        .with_max_retries(2)
        .with_backoff_millis(0);

    let body = serde_json::json!({"model":"gpt-5-pro","input":[{"role":"user","content":"plan"}]});
    let resp = client.responses_create(body, false).await.expect("api ok");
    assert_eq!(resp.get("model").unwrap().as_str().unwrap(), "gpt-5-pro");
    assert_eq!(m2.hits(), 1, "second call should succeed after one 429");
}
