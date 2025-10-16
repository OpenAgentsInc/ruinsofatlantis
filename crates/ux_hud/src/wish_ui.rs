//! Wishcraft HUD panels: Wisp Dock, Motes (checklist), Terminal.
//! This module holds CPU-side state and event application; rendering is handled by the HUD layer.

use serde_json::Value;

#[derive(Clone, Debug)]
pub enum WishStatus {
    Draft,
    ShadowRunning,
    CourtPending,
    CommitQueued,
    Committing,
    EchoMonitoring,
    Completed,
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct WishUiEvent {
    pub ts: String,
    pub kind: String,
    pub data: Value,
}

#[derive(Default, Clone, Debug)]
pub struct WishUiState {
    pub active_id: Option<String>,
    pub status: WishStatus,
    pub progress: f32,
    pub motes: Vec<String>,
    pub terminal_lines: Vec<String>,
    pub tokens_used: Option<u64>,
    pub model: Option<String>,
}

impl WishUiState {
    pub fn apply_event(&mut self, e: WishUiEvent) {
        match e.kind.as_str() {
            "iteration.start" => {
                // synthetic progress bump
                self.progress = (self.progress + 0.05).min(0.95);
            }
            "plan.completed" => {
                if let Some(t) = e.data.get("tokens").and_then(|v| v.as_u64()) {
                    self.tokens_used = Some(t);
                }
                if let Some(m) = e.data.get("model").and_then(|v| v.as_str()) {
                    self.model = Some(m.to_string());
                }
                self.motes.push("Plan completed".into());
            }
            "patch.applied" => {
                let line = format!("{}: patch applied", e.ts);
                self.terminal_lines.push(line);
            }
            "wish.success" => {
                self.status = WishStatus::Completed;
                self.progress = 1.0;
            }
            _ => {}
        }
        // keep last N lines
        if self.terminal_lines.len() > 400 { let trim = self.terminal_lines.len() - 400; self.terminal_lines.drain(0..trim); }
    }
}

