use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Persona {
    Literalist,
    Maximizer,
    Egalitarian,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenieCapability {
    pub id: String,
    pub persona: Persona,
    pub allowed: bool,
    // placeholder for caps/costs
}

pub trait GenieRegistry {
    fn get(&self, id: &str) -> Option<GenieCapability>;
    fn allow_tool(&self, id: &str) -> bool {
        self.get(id).map(|c| c.allowed).unwrap_or(false)
    }
}
