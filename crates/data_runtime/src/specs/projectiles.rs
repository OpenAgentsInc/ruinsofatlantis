//! Projectile specifications used to parameterize server-side projectile spawns.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectileSpec {
    pub speed_mps: f32,
    pub radius_m: f32,
    pub damage: i32,
    pub life_s: f32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectileSpecDb {
    /// Map from action name (e.g., "AtWillLMB", "EncounterQ") to spec
    pub actions: HashMap<String, ProjectileSpec>,
}

fn data_root() -> std::path::PathBuf {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = here.join("../../data");
    if ws.is_dir() { ws } else { here.join("data") }
}

impl ProjectileSpecDb {
    pub fn load_default() -> Result<Self> {
        let path = data_root().join("config/projectiles.toml");
        if path.is_file() {
            let txt = std::fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?;
            let db: Self = toml::from_str(&txt).context("parse projectiles TOML")?;
            Ok(db)
        } else {
            // Reasonable defaults
            let mut db = Self::default();
            db.actions.insert(
                "AtWillLMB".to_string(),
                ProjectileSpec {
                    speed_mps: 40.0,
                    radius_m: 0.2,
                    damage: 10,
                    life_s: 1.5,
                },
            );
            db.actions.insert(
                "AtWillRMB".to_string(),
                ProjectileSpec {
                    speed_mps: 35.0,
                    radius_m: 0.25,
                    damage: 8,
                    life_s: 1.5,
                },
            );
            db.actions.insert(
                "EncounterQ".to_string(),
                ProjectileSpec {
                    speed_mps: 30.0,
                    radius_m: 6.0,   // Fireball AoE ~6 meters default
                    damage: 28,      // avg 8d6
                    life_s: 1.5,
                },
            );
            db.actions.insert(
                "EncounterE".to_string(),
                ProjectileSpec {
                    speed_mps: 28.0,
                    radius_m: 0.5,
                    damage: 18,
                    life_s: 1.5,
                },
            );
            db.actions.insert(
                "EncounterR".to_string(),
                ProjectileSpec {
                    speed_mps: 26.0,
                    radius_m: 0.45,
                    damage: 16,
                    life_s: 1.5,
                },
            );
            Ok(db)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_present() {
        let db = ProjectileSpecDb::load_default().expect("load");
        assert!(db.actions.contains_key("AtWillLMB"));
        assert!(db.actions.contains_key("EncounterQ"));
    }
}
