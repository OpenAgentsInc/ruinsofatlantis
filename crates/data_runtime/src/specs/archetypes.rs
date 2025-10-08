//! Archetype spawn specifications for server-side defaults.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct ArchetypeSpec {
    pub radius_m: f32,
    pub move_speed_mps: f32,
    pub aggro_radius_m: f32,
    pub attack_radius_m: f32,
    pub melee_damage: i32,
    pub melee_cooldown_s: f32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ArchetypeSpecDb {
    pub entries: HashMap<String, ArchetypeSpec>,
}

fn data_root() -> std::path::PathBuf {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = here.join("../../data");
    if ws.is_dir() { ws } else { here.join("data") }
}

impl ArchetypeSpecDb {
    pub fn load_default() -> Result<Self> {
        let path = data_root().join("config/archetypes.toml");
        if path.is_file() {
            let txt = std::fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?;
            let db: Self = toml::from_str(&txt).context("parse archetypes TOML")?;
            Ok(db)
        } else {
            // Defaults for Undead, WizardNPC (caster), DeathKnight
            let mut db = Self::default();
            db.entries.insert(
                "Undead".into(),
                ArchetypeSpec {
                    radius_m: 0.9,
                    move_speed_mps: 2.0,
                    aggro_radius_m: 25.0,
                    attack_radius_m: 0.35,
                    melee_damage: 5,
                    melee_cooldown_s: 0.6,
                },
            );
            db.entries.insert(
                "WizardNPC".into(),
                ArchetypeSpec {
                    radius_m: 0.7,
                    move_speed_mps: 0.0,
                    aggro_radius_m: 0.0,
                    attack_radius_m: 0.0,
                    melee_damage: 0,
                    melee_cooldown_s: 0.0,
                },
            );
            db.entries.insert(
                "DeathKnight".into(),
                ArchetypeSpec {
                    radius_m: 1.0,
                    move_speed_mps: 2.2,
                    aggro_radius_m: 40.0,
                    attack_radius_m: 0.45,
                    melee_damage: 18,
                    melee_cooldown_s: 0.9,
                },
            );
            Ok(db)
        }
    }
}

