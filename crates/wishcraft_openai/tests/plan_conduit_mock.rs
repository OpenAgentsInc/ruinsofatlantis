use httpmock::prelude::*;
use serial_test::serial;
use wishcraft::conduit::{ConduitExec, ExecMode};
use wishcraft_openai::{
    client::OpenAIClient,
    conduit::{OpenAIConduit, PlanInput},
    config::OpenAIConfig,
};

#[tokio::test]
#[serial]
async fn plan_conduit_returns_steps_and_audit_fields() {
    let server = MockServer::start();
    let _m = server.mock(|when, then| {
        when.method(POST).path("/backend-api/codex");
        then.status(200).body(r#"{
          "model":"gpt-5-pro",
          "usage":{"total_tokens": 321},
          "choices":[{"message":{"content":"1. Enable cache\n2. Prune symbols\n3. Parallelize shaders"}}]
        }"#);
    });

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
    let client = OpenAIClient::new(cfg);
    let conduit = OpenAIConduit::new(client);

    let input = PlanInput {
        repo: "ruinsofatlantis".into(),
        paths: vec!["crates/**".into()],
        objective: "Reduce build times by 20% in 7 days without API changes".into(),
        invariants: vec!["No public API change".into()],
        context_snippets: vec![],
    };

    let out = conduit
        .exec("openai.codex.v2025.plan", input.clone(), ExecMode::Commit)
        .await
        .expect("plan ok");

    assert!(
        out.plan_steps.len() >= 2,
        "should produce multiple plan steps"
    );
    assert_eq!(out.model.as_deref(), Some("gpt-5-pro"));
    assert_eq!(out.tokens_used, Some(321));
    assert_eq!(out.prompt_hash.len(), 64, "sha256 hex");
}
