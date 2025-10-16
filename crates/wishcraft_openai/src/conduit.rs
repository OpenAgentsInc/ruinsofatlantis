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

fn codex_base_instructions_for_model(model: &str) -> &'static str {
    // Mirror codex-rs: gpt-5-codex and codex-* use the GPT_5_CODEX prompt; others use prompt.md
    const PROMPT_BASE: &str =
        include_str!("../../../third_party/openai-codex/codex-rs/core/prompt.md");
    const PROMPT_G5_CODEX: &str =
        include_str!("../../../third_party/openai-codex/codex-rs/core/gpt_5_codex_prompt.md");
    if model.starts_with("gpt-5-codex") || model.starts_with("codex-") {
        PROMPT_G5_CODEX
    } else {
        PROMPT_BASE
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

        // Commit mode: Use Responses API payload shape exactly like codex-rs
        let instructions = codex_base_instructions_for_model(&self.client.cfg.model);
        let input_items = vec![serde_json::json!({
            "type": "message",
            "role": "user",
            "content": [ {"type": "input_text", "text": prompt } ]
        })];
        let tools_json: Vec<serde_json::Value> = vec![]; // no tools for planning text output
        let body = serde_json::json!({
            "model": self.client.cfg.model,
            "instructions": instructions,
            "input": input_items,
            "tools": tools_json,
            "tool_choice": "auto",
            "parallel_tool_calls": false,
            "store": false,
            "stream": true,
            "include": [],
            // omit prompt_cache_key/text fields for now
        });
        let resp = match self.client.chatgpt_codex_post(body.clone()).await {
            Ok(v) => v,
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("Instructions are required") || msg.contains("Not Found") {
                    // Fallback to Chat Completions wire
                    let tools = serde_json::json!([
                      {"type":"function","function":{
                        "name":"plan",
                        "description":"Propose a step-by-step plan and acceptance checks for the requested objective.",
                        "parameters": {"type":"object","properties":{},"additionalProperties": true}
                      }}
                    ]);
                    let chat_body = serde_json::json!({
                        "model": self.client.cfg.model,
                        "messages": [
                            {"role":"system","content":"You are a cautious code planner."},
                            {"role":"user","content": prompt}
                        ],
                        "tools": tools,
                        "stream": true
                    });
                    self.client.chatgpt_codex_post_chat(chat_body).await?
                } else {
                    return Err(anyhow::anyhow!(e));
                }
            }
        };
        let (steps, notes, model, tokens) =
            parse_responses_plan(&resp).or_else(|_| parse_chat_plan(&resp))?;
        Ok(PlanOutput {
            plan_steps: steps,
            notes,
            tokens_used: tokens,
            model,
            prompt_hash,
        })
    }
}

fn parse_responses_plan(
    resp: &serde_json::Value,
) -> anyhow::Result<(Vec<String>, Vec<String>, Option<String>, Option<u64>)> {
    let model = resp
        .get("model")
        .and_then(|m| m.as_str())
        .map(|s| s.to_string());
    let tokens = resp
        .get("usage")
        .and_then(|u| u.get("total_tokens"))
        .and_then(|t| t.as_u64());
    let text = resp
        .get("output_text")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            resp.pointer("/output/0/content/0/text")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_default();
    let steps = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    Ok((steps, vec![], model, tokens))
}

fn parse_chat_plan(
    resp: &serde_json::Value,
) -> anyhow::Result<(Vec<String>, Vec<String>, Option<String>, Option<u64>)> {
    let model = resp
        .get("model")
        .and_then(|m| m.as_str())
        .map(|s| s.to_string());
    let tokens = resp
        .get("usage")
        .and_then(|u| u.get("total_tokens"))
        .and_then(|t| t.as_u64());
    let content = resp
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let steps = content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    Ok((steps, vec![], model, tokens))
}
