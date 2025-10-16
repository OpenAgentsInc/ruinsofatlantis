use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Disposition {
    #[default]
    Literalist,
    Maximizer,
    Egalitarian,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Domain {
    Code,
    Content,
    Cartography,
    Logistics,
    WorldAuthoring,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum RiskClass {
    #[default]
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Determinism {
    #[default]
    Deterministic,
    Stochastic,
    Mockable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Latency {
    #[default]
    Instant,
    Short,
    Long,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Permission(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Limits {
    pub per_wish_calls: Option<u32>,
    pub per_day_calls: Option<u32>,
    pub per_wish_entities: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CostProfile {
    pub tokens_per_call: Option<String>,
    pub rate_per_min: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ScopeRule {
    pub repo: Option<String>,
    pub paths: Option<Vec<String>>,
    pub actions: Option<Vec<String>>,
    pub zone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConduitDescriptor {
    pub id: String,
    pub label: String,
    pub disposition: Disposition,
    #[serde(default)]
    pub domains: Vec<Domain>,
    #[serde(default)]
    pub scopes: Vec<ScopeRule>,
    #[serde(default)]
    pub cost_profile: CostProfile,
    pub risk_class: RiskClass,
    pub determinism: Determinism,
    pub latency_class: Latency,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub limits: Limits,
    #[serde(default)]
    pub audit_fields: Vec<String>,
}

impl Default for ConduitDescriptor {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            disposition: Disposition::Literalist,
            domains: vec![],
            scopes: vec![],
            cost_profile: CostProfile::default(),
            risk_class: RiskClass::Low,
            determinism: Determinism::Deterministic,
            latency_class: Latency::Instant,
            permissions: vec![],
            limits: Limits::default(),
            audit_fields: vec![],
        }
    }
}

pub trait ConduitRegistry {
    fn get(&self, id: &str) -> Option<ConduitDescriptor>;
    fn allow(&self, id: &str) -> bool {
        self.get(id).is_some()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ExecMode {
    ShadowRun,
    Commit,
}

#[async_trait::async_trait]
pub trait ConduitExec {
    type Input: Serialize + for<'de> Deserialize<'de>;
    type Output: Serialize + for<'de> Deserialize<'de>;
    async fn exec(
        &self,
        conduit_id: &str,
        input: Self::Input,
        mode: ExecMode,
    ) -> anyhow::Result<Self::Output>;
}
