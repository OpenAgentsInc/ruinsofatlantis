use serial_test::serial;
use wishcraft_openai::config::OpenAIConfig;

#[test]
#[serial]
fn config_reads_env_and_sets_defaults() {
    std::env::set_var("OPENAI_API_KEY", "sk-test");
    std::env::remove_var("OPENAI_ORG");
    std::env::remove_var("OPENAI_PROJECT");
    std::env::remove_var("OPENAI_BASE_URL");

    let cfg = OpenAIConfig::from_env_defaults().expect("config");
    assert!(cfg.api_key.starts_with("sk-"));
    assert_eq!(cfg.base_url, "https://api.openai.com/v1");
    assert!(!cfg.model.is_empty());
}
