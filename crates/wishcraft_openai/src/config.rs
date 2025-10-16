use anyhow::{Result, anyhow};

#[derive(Clone, Debug)]
pub struct OpenAIConfig {
    pub base_url: String,
    pub api_key: String,
    pub organization: Option<String>,
    pub project: Option<String>,
    pub model: String,
    pub temperature: Option<f32>,
    pub timeout_secs: u64,
    pub azure: bool,
    pub azure_api_version: Option<String>,
}

impl OpenAIConfig {
    pub fn from_env_defaults() -> Result<Self> {
        let api_key =
            std::env::var("OPENAI_API_KEY").map_err(|_| anyhow!("OPENAI_API_KEY not set"))?;
        let base_url = std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
        let organization = std::env::var("OPENAI_ORG").ok();
        let project = std::env::var("OPENAI_PROJECT").ok();
        let temperature = std::env::var("OPENAI_TEMPERATURE")
            .ok()
            .and_then(|s| s.parse::<f32>().ok());
        let timeout_secs = std::env::var("OPENAI_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(30);
        let azure = std::env::var("OPENAI_PROVIDER")
            .map(|v| v.eq_ignore_ascii_case("azure"))
            .unwrap_or(false);
        let azure_api_version = std::env::var("OPENAI_API_VERSION").ok();
        Ok(Self {
            base_url,
            api_key,
            organization,
            project,
            model,
            temperature,
            timeout_secs,
            azure,
            azure_api_version,
        })
    }
}
