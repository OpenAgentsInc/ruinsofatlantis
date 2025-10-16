use serial_test::serial;
use wishcraft_openai::config::OpenAIConfig;

#[test]
#[serial]
fn config_reads_env_and_sets_defaults() {
    std::env::remove_var("CHATGPT_BASE_URL");
    let cfg = OpenAIConfig::from_env_defaults().expect("config");
    assert!(cfg.chatgpt_base_url.contains("chatgpt.com"));
    assert!(!cfg.model.is_empty());
    assert!(cfg.codex_home.ends_with(".codex") || cfg.codex_home.exists());
}
