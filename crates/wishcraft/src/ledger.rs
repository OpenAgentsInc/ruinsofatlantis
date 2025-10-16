use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct LedgerEntry {
    pub wish_hash: String,
    pub diff_hash: Option<String>,
    pub author_id: String,
    pub timestamp: String, // iso8601
    pub anchor_id: String,
    pub summary: Option<String>,
}

pub trait WishLedger {
    fn append(&mut self, entry: &LedgerEntry) -> anyhow::Result<()>;
    fn last_anchor(&self) -> Option<String>;
}
