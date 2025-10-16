use anyhow::Result;

#[derive(Clone, Debug)]
pub struct OpenAIConfig {
    pub chatgpt_base_url: String,
    pub codex_home: std::path::PathBuf,
    pub model: String,
    pub temperature: Option<f32>,
    pub timeout_secs: u64,
}

impl OpenAIConfig {
    pub fn from_env_defaults() -> Result<Self> {
        // Default to Codex Chat wire base used by codex-rs
        let chatgpt_base_url = std::env::var("CHATGPT_BASE_URL")
            .unwrap_or_else(|_| "https://chatgpt.com/backend-api/codex".to_string());
        // Default to a ChatGPT-accepted slug used by Codex presets
        let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-5".to_string());
        let temperature = std::env::var("OPENAI_TEMPERATURE")
            .ok()
            .and_then(|s| s.parse::<f32>().ok());
        let timeout_secs = std::env::var("OPENAI_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(30);
        let codex_home = std::env::var("CODEX_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| dirs::home_dir().unwrap_or_default().join(".codex"));
        Ok(Self {
            chatgpt_base_url,
            codex_home,
            model,
            temperature,
            timeout_secs,
        })
    }
}
