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
        when.method(POST).path("/v1/responses");
        then.status(200).body(
            r#"{
          "model":"gpt-5-pro",
          "usage":{"total_tokens": 321},
          "output_text":"1. Enable cache\n2. Prune symbols\n3. Parallelize shaders"
        }"#,
        );
    });

    std::env::set_var("OPENAI_API_KEY", "sk-test");
    let cfg = OpenAIConfig {
        base_url: format!("{}/v1", server.base_url()),
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
    let conduit = OpenAIConduit::new(client);

    let input = PlanInput {
        repo: "ruinsofatlantis".into(),
        paths: vec!["crates/**".into()],
        objective: "Reduce build times by 20% in 7 days without API changes".into(),
        invariants: vec!["No public API change".into()],
        context_snippets: vec![],
    };

    let out = conduit
        .exec(
            "openai.codex.v2025.plan",
            input.clone(),
            ExecMode::ShadowRun,
        )
        .await
        .expect("plan ok");

    assert!(
        out.plan_steps.len() >= 2,
        "should produce multiple plan steps"
    );
    // in ShadowRun we set model to cfg.model; live mode would reflect API
    assert_eq!(out.model.as_deref(), Some("gpt-5-pro"));
    // ShadowRun has None tokens, but Commit would set it; accept None or Some(_)
    assert!(out.tokens_used.is_none() || out.tokens_used == Some(321));
    assert_eq!(out.prompt_hash.len(), 64, "sha256 hex");
}
