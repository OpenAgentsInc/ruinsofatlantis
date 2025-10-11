//! Worldsmithing: in-world authoring primitives (V1: Place Tree only).
//! This crate is UI- and renderer-agnostic; it holds pure logic for
//! placement state, caps/rate-limiting, and authoring import/export.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Caps and pacing limits for authoring actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    pub max_trees_per_zone: u32,
    pub max_place_per_second: u32,
}

impl Default for Caps {
    fn default() -> Self {
        Self {
            max_trees_per_zone: 5000,
            max_place_per_second: 5,
        }
    }
}

/// A single placed tree instance (authoring record)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlacedTreeV1 {
    pub id: String,
    pub kind: String, // e.g., "tree.default"
    pub pos: [f32; 3],
    pub yaw_deg: f32,
}

/// Authoring file (export/import)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlantingFileV1 {
    pub schema: String,           // "rua.plantings.v1"
    pub map_id: String,           // current zone slug
    pub coordinate_space: String, // "world"
    pub unit: String,             // "meters"
    pub objects: Vec<PlacedTreeV1>,
    pub created_at: String, // ISO-8601 (opaque here)
    pub engine_version: String,
}

/// In-memory placement/session state
#[derive(Debug)]
pub struct WorldsmithingState {
    pub active: bool,
    pub current_yaw_deg: f32,
    pub placed: Vec<PlacedTreeV1>,
    caps: Caps,
    // simple rate limiter: store timestamps (ms since epoch or session start) of recent placements
    recent_ms: Vec<u64>,
}

impl WorldsmithingState {
    pub fn new() -> Self {
        Self {
            active: false,
            current_yaw_deg: 0.0,
            placed: Vec::new(),
            caps: Caps::default(),
            recent_ms: Vec::new(),
        }
    }

    pub fn with_caps(caps: Caps) -> Self {
        Self {
            caps,
            ..Self::new()
        }
    }

    pub fn caps(&self) -> Caps {
        self.caps
    }

    pub fn set_active(&mut self, v: bool) {
        self.active = v;
    }

    pub fn rotate_step(&mut self, delta_deg: f32) {
        let mut d = self.current_yaw_deg + delta_deg;
        while d < 0.0 {
            d += 360.0;
        }
        while d >= 360.0 {
            d -= 360.0;
        }
        self.current_yaw_deg = d;
    }

    /// true if zone cap is near (>= 80%)
    pub fn nearing_cap(&self) -> bool {
        let cap = self.caps.max_trees_per_zone as usize;
        if cap == 0 {
            return false;
        }
        self.placed.len() * 100 / cap >= 80
    }

    fn cleanup_rate_window(&mut self, now_ms: u64) {
        let window_start = now_ms.saturating_sub(1000);
        self.recent_ms.retain(|&t| t >= window_start);
    }

    pub fn can_place(&mut self, now_ms: u64) -> Result<(), PlaceError> {
        if self.placed.len() as u32 >= self.caps.max_trees_per_zone {
            return Err(PlaceError::ZoneCapReached(self.caps.max_trees_per_zone));
        }
        self.cleanup_rate_window(now_ms);
        if self.recent_ms.len() as u32 >= self.caps.max_place_per_second {
            return Err(PlaceError::RateLimited(self.caps.max_place_per_second));
        }
        Ok(())
    }

    pub fn place(
        &mut self,
        kind: &str,
        pos: [f32; 3],
        yaw_deg: f32,
        now_ms: u64,
    ) -> Result<&PlacedTreeV1, PlaceError> {
        self.can_place(now_ms)?;
        let id = Uuid::new_v4().to_string();
        let rec = PlacedTreeV1 {
            id,
            kind: kind.to_string(),
            pos,
            yaw_deg,
        };
        self.placed.push(rec);
        self.recent_ms.push(now_ms);
        Ok(self.placed.last().unwrap())
    }

    pub fn export_json(
        &self,
        map_id: &str,
        engine_version: &str,
        now_iso8601: &str,
    ) -> Result<String, serde_json::Error> {
        let file = PlantingFileV1 {
            schema: "rua.plantings.v1".into(),
            map_id: map_id.into(),
            coordinate_space: "world".into(),
            unit: "meters".into(),
            objects: self.placed.clone(),
            created_at: now_iso8601.into(),
            engine_version: engine_version.into(),
        };
        serde_json::to_string_pretty(&file)
    }

    pub fn import_json(
        &mut self,
        s: &str,
        current_map_id: &str,
    ) -> Result<ImportResult, ImportError> {
        let file: PlantingFileV1 = serde_json::from_str(s).map_err(ImportError::Serde)?;
        if file.schema != "rua.plantings.v1" {
            return Err(ImportError::Schema(file.schema));
        }
        let mismatch = file.map_id != current_map_id;
        let mut added = 0usize;
        for obj in file.objects.into_iter() {
            // For V1, allow any kind that starts with "tree."; append duplicates.
            if !obj.kind.starts_with("tree.") {
                continue;
            }
            self.placed.push(obj);
            added += 1;
        }
        Ok(ImportResult {
            placed: added,
            map_id_mismatch: mismatch,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlaceError {
    #[error("zone cap reached ({0})")]
    ZoneCapReached(u32),
    #[error("rate limited ({0}/s)")]
    RateLimited(u32),
}

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("bad schema: {0}")]
    Schema(String),
    #[error("parse error")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportResult {
    pub placed: usize,
    pub map_id_mismatch: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_wraps() {
        let mut s = WorldsmithingState::new();
        s.rotate_step(370.0);
        assert!((s.current_yaw_deg - 10.0).abs() < 1e-6);
        s.rotate_step(-20.0);
        assert!((s.current_yaw_deg - 350.0).abs() < 1e-6);
    }

    #[test]
    fn caps_and_rate_limit_enforced() {
        let mut s = WorldsmithingState::with_caps(Caps {
            max_trees_per_zone: 2,
            max_place_per_second: 2,
        });
        // place two quickly ok
        assert!(s.place("tree.default", [0.0, 0.0, 0.0], 0.0, 1000).is_ok());
        assert!(s.place("tree.default", [1.0, 0.0, 0.0], 0.0, 1100).is_ok());
        // third hits zone cap first
        let e = s
            .place("tree.default", [2.0, 0.0, 0.0], 0.0, 1200)
            .unwrap_err();
        assert_eq!(e, PlaceError::ZoneCapReached(2));

        // reset caps to test rate limit
        let mut s = WorldsmithingState::with_caps(Caps {
            max_trees_per_zone: 10,
            max_place_per_second: 2,
        });
        assert!(s.place("tree.default", [0.0, 0.0, 0.0], 0.0, 2000).is_ok());
        assert!(s.place("tree.default", [0.5, 0.0, 0.0], 0.0, 2050).is_ok());
        let e = s
            .place("tree.default", [1.0, 0.0, 0.0], 0.0, 2100)
            .unwrap_err();
        assert_eq!(e, PlaceError::RateLimited(2));
        // after 1s window, should succeed
        assert!(s.place("tree.default", [1.0, 0.0, 0.0], 0.0, 3051).is_ok());
    }

    #[test]
    fn export_and_import_round_trip() {
        let mut s = WorldsmithingState::new();
        s.place("tree.default", [1.0, 0.0, -2.0], 270.0, 10)
            .unwrap();
        let json = s
            .export_json("campaign_builder", "0.1.0", "2025-10-11T00:00:00Z")
            .unwrap();
        let mut t = WorldsmithingState::new();
        let res = t.import_json(&json, "campaign_builder").unwrap();
        assert_eq!(res.placed, 1);
        assert!(!res.map_id_mismatch);
        assert_eq!(t.placed.len(), 1);
        let m = &t.placed[0];
        assert_eq!(m.kind, "tree.default");
        assert_eq!(m.pos, [1.0, 0.0, -2.0]);
        assert!((m.yaw_deg - 270.0).abs() < 1e-6);
    }

    #[test]
    fn import_map_mismatch_warn_only() {
        let mut s = WorldsmithingState::new();
        s.place("tree.default", [0.0, 0.0, 0.0], 0.0, 0).unwrap();
        let json = s
            .export_json("wizard_woods", "0.1.0", "2025-10-11T00:00:00Z")
            .unwrap();
        let mut t = WorldsmithingState::new();
        let res = t.import_json(&json, "campaign_builder").unwrap();
        assert_eq!(res.placed, 1);
        assert!(res.map_id_mismatch);
        assert_eq!(t.placed.len(), 1);
    }
}
