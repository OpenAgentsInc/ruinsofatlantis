use crate::{GenieRegistry, schema::Wish};

#[derive(Debug, Default, Clone)]
pub struct LintReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl LintReport {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn lint_wish(w: &Wish, registry: &dyn GenieRegistry) -> LintReport {
    let mut r = LintReport::default();

    if w.objective.trim().is_empty() {
        r.errors
            .push("objective must be non-empty and measurable".into());
    }
    if w.rollback.is_empty() {
        r.errors
            .push("rollback plan must include at least one action".into());
    }
    if w.invariants.is_empty() {
        r.warnings.push("no invariants provided".into());
    }
    for t in &w.tools {
        if !registry.allow_tool(t) {
            r.errors.push(format!("tool not allowed or unknown: {}", t));
        }
    }
    // heuristic: warn on ambiguous pronouns in the objective
    let obj_lc = w.objective.to_ascii_lowercase();
    for bad in ["this", "that", "it", "they", "them"] {
        if obj_lc.contains(&format!(" {} ", bad)) || obj_lc.starts_with(bad) {
            r.warnings.push(format!(
                "objective may be ambiguous: contains '{}'; prefer explicit nouns",
                bad
            ));
            break;
        }
    }
    r
}
