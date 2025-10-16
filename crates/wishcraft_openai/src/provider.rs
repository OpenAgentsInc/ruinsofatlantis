use crate::{
    conduit::OpenAIConduit,
    conduit::{PlanInput, PlanOutput},
};
use wishcraft::conduit::ExecMode;

pub trait PlanProvider {
    fn plan(&self, input: PlanInput, mode: ExecMode) -> anyhow::Result<PlanOutput>;
}

/// Thin provider using our direct HTTP client via OpenAIConduit
pub struct OpenAIThinClient(pub OpenAIConduit);

impl PlanProvider for OpenAIThinClient {
    fn plan(&self, input: PlanInput, mode: ExecMode) -> anyhow::Result<PlanOutput> {
        self.0.exec("openai.codex.v2025.plan", input, mode)
    }
}

/// Feature-gated stub for a future codex-rs backed adapter
#[cfg(feature = "codex-adapter")]
pub struct OpenAICodexAdapter;

#[cfg(feature = "codex-adapter")]
impl PlanProvider for OpenAICodexAdapter {
    fn plan(&self, _input: PlanInput, _mode: ExecMode) -> anyhow::Result<PlanOutput> {
        anyhow::bail!("codex adapter not wired yet; disable feature or use OpenAIThinClient")
    }
}
