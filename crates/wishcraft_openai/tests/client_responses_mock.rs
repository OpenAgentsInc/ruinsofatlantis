use httpmock::prelude::*;
use serial_test::serial;
use wishcraft_openai::{client::OpenAIClient, config::OpenAIConfig};

#[tokio::test]
#[serial]
async fn responses_success_happy_path() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/responses")
            .header("authorization", "Bearer sk-test");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{
              "model": "gpt-5-pro",
              "usage": { "total_tokens": 1234 },
              "output_text": "1. Step A\n2. Step B"
            }"#,
            );
    });

    std::env::set_var("OPENAI_API_KEY", "sk-test");
    let cfg = OpenAIConfig {
        base_url: format!("{}/v1", &server.base_url()),
        api_key: "sk-test".into(),
        organization: None,
        project: None,
        model: "gpt-5-pro".into(),
        temperature: Some(0.2),
        timeout_secs: 10,
        azure: false,
        azure_api_version: None,
    };
    let client = OpenAIClient::new(cfg);

    let body = serde_json::json!({"model":"gpt-5-pro","input":[{"role":"user","content":"hello"}]});
    let resp = client.responses_create(body, false).await.expect("api");
    assert_eq!(resp.get("model").unwrap().as_str().unwrap(), "gpt-5-pro");
    assert_eq!(m.hits(), 1);
}
