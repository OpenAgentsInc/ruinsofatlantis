use sha2::{Digest, Sha256};
use wishcraft::conduit::{ConduitExec, ExecMode};

pub struct OpenAIConduit {
    pub client: crate::client::OpenAIClient,
}

impl OpenAIConduit {
    pub fn new(client: crate::client::OpenAIClient) -> Self {
        Self { client }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PlanInput {
    pub repo: String,
    pub paths: Vec<String>,
    pub objective: String,
    pub invariants: Vec<String>,
    pub context_snippets: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PlanOutput {
    pub plan_steps: Vec<String>,
    pub notes: Vec<String>,
    pub tokens_used: Option<u64>,
    pub model: Option<String>,
    pub prompt_hash: String,
}

fn build_planning_prompt(input: &PlanInput) -> String {
    format!(
        r#"You are a cautious code planner.
Repository: {repo}
Paths: {paths}
Objective: {obj}
Invariants:
- {inv}

Return a numbered plan with concrete steps and acceptance checks."#,
        repo = input.repo,
        paths = input.paths.join(", "),
        obj = input.objective,
        inv = input.invariants.join("\n- "),
    )
}

// (formerly parse_responses_plan) — removed after pivot to ChatGPT backend

#[async_trait::async_trait]
impl ConduitExec for OpenAIConduit {
    type Input = PlanInput;
    type Output = PlanOutput;

    async fn exec(
        &self,
        _conduit_id: &str,
        input: Self::Input,
        mode: ExecMode,
    ) -> anyhow::Result<Self::Output> {
        let prompt = build_planning_prompt(&input);
        let prompt_hash = {
            let mut h = Sha256::new();
            h.update(prompt.as_bytes());
            format!("{:x}", h.finalize())
        };

        if matches!(mode, ExecMode::ShadowRun) {
            // Offline stub: turn prompt into a few simple steps
            let mut steps = vec![
                "Gather code context and constraints".to_string(),
                "Draft numbered plan with acceptance checks".to_string(),
                "Review invariants and adjust plan".to_string(),
            ];
            if !input.context_snippets.is_empty() {
                steps.push("Incorporate provided context snippets".to_string());
            }
            return Ok(PlanOutput {
                plan_steps: steps,
                notes: vec!["shadow-run stub".to_string()],
                tokens_used: None,
                model: Some(self.client.cfg.model.clone()),
                prompt_hash,
            });
        }

        // Commit mode: call ChatGPT backend /codex using Chat Completions-like payload
        let body = serde_json::json!({
            "model": self.client.cfg.model,
            "messages": [
                {"role":"system","content":"You are a cautious code planner."},
                {"role":"user","content": prompt}
            ],
            "stream": false
        });
        let resp = self.client.chatgpt_codex_post(body).await?;
        let (steps, notes, model, tokens) = parse_chatgpt_plan(&resp)?;
        Ok(PlanOutput {
            plan_steps: steps,
            notes,
            tokens_used: tokens,
            model,
            prompt_hash,
        })
    }
}

fn parse_chatgpt_plan(
    resp: &serde_json::Value,
) -> anyhow::Result<(Vec<String>, Vec<String>, Option<String>, Option<u64>)> {
    let model = resp
        .get("model")
        .and_then(|m| m.as_str())
        .map(|s| s.to_string());
    // Some backends include usage; optional
    let tokens = resp
        .get("usage")
        .and_then(|u| u.get("total_tokens"))
        .and_then(|t| t.as_u64());
    let content = resp
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let steps = content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    Ok((steps, vec![], model, tokens))
}
