//! SpecDb: canonical facade for content specs (spells/classes/monsters).
//!
//! Provides in-memory indexes and simple normalization so callers don't need
//! to guess file names or embed heuristics.

use crate::{loader, spell::SpellSpec};
use std::collections::HashMap;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // Prefer workspace root (two levels up) if it contains data/
    let ws = here.join("..").join("..");
    if ws.join("data").is_dir() { ws } else { here }
}

#[derive(Default)]
pub struct SpecDb {
    spells: HashMap<String, SpellSpec>,
}

impl SpecDb {
    pub fn load_default() -> Self {
        let mut db = SpecDb::default();
        let root = workspace_root();
        let dir = root.join("data/spells");
        if dir.is_dir() {
            for entry in std::fs::read_dir(&dir).unwrap_or_else(|_| std::fs::ReadDir::from(std::fs::read_dir(".").unwrap())) {
                if let Ok(ent) = entry {
                    let path = ent.path();
                    if path.extension().and_then(|s| s.to_str()) != Some("json") { continue; }
                    let rel = format!("spells/{}", path.file_name().unwrap().to_string_lossy());
                    if let Ok(spec) = loader::load_spell_spec(&rel) {
                        Self::index_spell(&mut db.spells, spec);
                    }
                }
            }
        }
        db
    }

    fn index_spell(map: &mut HashMap<String, SpellSpec>, spec: SpellSpec) {
        let canon = spec.id.clone();
        let name_key = spec.name.to_ascii_lowercase().replace(' ', "_");
        let last = canon.rsplit('.').next().unwrap_or(&canon).to_string();
        map.insert(canon, spec.clone());
        map.insert(last, spec.clone());
        map.insert(name_key, spec);
    }

    pub fn get_spell(&self, id: &str) -> Option<&SpellSpec> {
        if let Some(s) = self.spells.get(id) { return Some(s); }
        let last = id.rsplit('.').next().unwrap_or(id);
        if let Some(s) = self.spells.get(last) { return Some(s); }
        let name_key = id.to_ascii_lowercase().replace(' ', "_");
        self.spells.get(&name_key)
    }
}

