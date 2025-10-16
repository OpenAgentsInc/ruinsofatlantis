use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    Micro,
    Meso,
    Macro,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Scope {
    pub region: String,
    pub duration_days: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Budget {
    pub chrono_sand: u16,
    pub genie_slots: u8,
    pub gold_cap: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Meta {
    pub author_id: Option<String>,
    pub petition_id: Option<String>,
    pub created_at: Option<String>, // iso8601
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Wish {
    pub title: String,
    pub objective: String,
    pub scope: Scope,
    pub invariants: Vec<String>,
    pub budget: Budget,
    pub tools: Vec<String>,
    pub plan: Vec<String>,
    pub safety_tests: Vec<String>,
    pub rollback: Vec<String>,
    #[serde(default)]
    pub tier: Option<Tier>,
    #[serde(default)]
    pub meta: Meta,
}

#[cfg(feature = "schemars")]
impl schemars::JsonSchema for Wish {}
