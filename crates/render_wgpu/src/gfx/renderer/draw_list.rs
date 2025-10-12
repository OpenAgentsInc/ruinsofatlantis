//! CPU-only draw list builder and batch grouper.
//!
//! Pure data structures for deterministic grouping of draws by a key
//! (e.g., pipeline/material/mesh). Rendering integration can consume
//! the produced batches to minimize state changes.

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code)] // Adopted by Main in a later PR
pub struct DrawKey {
    pub pipeline_id: u32,
    pub material_id: u32,
    pub mesh_id: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub struct DrawItem {
    pub key: DrawKey,
    pub index_count: u32,
    pub instance_count: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub struct DrawBatch {
    pub key: DrawKey,
    pub index_count: u32,
    pub instance_count: u32,
}

#[derive(Default)]
#[allow(dead_code)]
pub struct DrawList {
    items: Vec<DrawItem>,
}

impl DrawList {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }
    #[allow(dead_code)]
    pub fn add(&mut self, item: DrawItem) {
        self.items.push(item);
    }
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.items.len()
    }
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Group contiguous items with the same key into batches.
    #[allow(dead_code)]
    pub fn to_batches(&self) -> Vec<DrawBatch> {
        let mut out: Vec<DrawBatch> = Vec::new();
        for it in &self.items {
            if let Some(last) = out.last_mut()
                && last.key == it.key
            {
                last.index_count += it.index_count;
                last.instance_count += it.instance_count;
                continue;
            }
            out.push(DrawBatch {
                key: it.key,
                index_count: it.index_count,
                instance_count: it.instance_count,
            });
        }
        out
    }

    /// Compute simple state-change counters for this list assuming
    /// render order equals item order.
    ///
    /// - `pipeline_binds`: increments when `pipeline_id` changes
    /// - `bg_binds`: increments when `(pipeline_id, material_id)` changes
    /// - `vb_ib_sets`: number of items (assumes per-draw VB/IB set)
    #[allow(dead_code)]
    pub fn state_counters(&self) -> (u32, u32, u32) {
        let mut pipeline_binds = 0u32;
        let mut bg_binds = 0u32;
        let mut prev_pipe: Option<u32> = None;
        let mut prev_pair: Option<(u32, u32)> = None;
        for it in &self.items {
            if prev_pipe.map_or(true, |p| p != it.key.pipeline_id) {
                pipeline_binds += 1;
                prev_pipe = Some(it.key.pipeline_id);
            }
            let pair = (it.key.pipeline_id, it.key.material_id);
            if prev_pair.map_or(true, |pp| pp != pair) {
                bg_binds += 1;
                prev_pair = Some(pair);
            }
        }
        let vb_ib_sets = self.items.len() as u32;
        (pipeline_binds, bg_binds, vb_ib_sets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_contiguous_same_key() {
        let k = DrawKey {
            pipeline_id: 1,
            material_id: 2,
            mesh_id: 3,
        };
        let mut dl = DrawList::new();
        dl.add(DrawItem {
            key: k,
            index_count: 10,
            instance_count: 1,
        });
        dl.add(DrawItem {
            key: k,
            index_count: 6,
            instance_count: 2,
        });
        let b = dl.to_batches();
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].index_count, 16);
        assert_eq!(b[0].instance_count, 3);
    }

    #[test]
    fn does_not_merge_across_different_keys() {
        let k0 = DrawKey {
            pipeline_id: 1,
            material_id: 2,
            mesh_id: 3,
        };
        let k1 = DrawKey {
            pipeline_id: 1,
            material_id: 2,
            mesh_id: 4,
        };
        let mut dl = DrawList::new();
        dl.add(DrawItem {
            key: k0,
            index_count: 4,
            instance_count: 1,
        });
        dl.add(DrawItem {
            key: k1,
            index_count: 8,
            instance_count: 1,
        });
        dl.add(DrawItem {
            key: k0,
            index_count: 2,
            instance_count: 1,
        });
        let b = dl.to_batches();
        assert_eq!(b.len(), 3);
        assert_eq!(b[0].key.mesh_id, 3);
        assert_eq!(b[1].key.mesh_id, 4);
        assert_eq!(b[2].key.mesh_id, 3);
    }

    #[test]
    fn stable_ordering() {
        let keys = [
            DrawKey {
                pipeline_id: 1,
                material_id: 2,
                mesh_id: 1,
            },
            DrawKey {
                pipeline_id: 1,
                material_id: 2,
                mesh_id: 1,
            },
            DrawKey {
                pipeline_id: 2,
                material_id: 2,
                mesh_id: 3,
            },
        ];
        let mut dl = DrawList::new();
        for k in keys {
            dl.add(DrawItem {
                key: k,
                index_count: 5,
                instance_count: 1,
            });
        }
        let b = dl.to_batches();
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].key.pipeline_id, 1);
        assert_eq!(b[1].key.pipeline_id, 2);
    }

    #[test]
    fn state_counters_pipeline_and_bg_changes() {
        let mut dl = DrawList::new();
        // Two draws with same (pipeline, material) → 1 bg bind; then pipeline changes
        dl.add(DrawItem {
            key: DrawKey {
                pipeline_id: 1,
                material_id: 10,
                mesh_id: 100,
            },
            index_count: 3,
            instance_count: 1,
        });
        dl.add(DrawItem {
            key: DrawKey {
                pipeline_id: 1,
                material_id: 10,
                mesh_id: 101,
            },
            index_count: 3,
            instance_count: 1,
        });
        // material changes (same pipeline) → bg bind increments, pipeline doesn't
        dl.add(DrawItem {
            key: DrawKey {
                pipeline_id: 1,
                material_id: 11,
                mesh_id: 102,
            },
            index_count: 3,
            instance_count: 1,
        });
        // pipeline changes → both pipeline and bg bind
        dl.add(DrawItem {
            key: DrawKey {
                pipeline_id: 2,
                material_id: 20,
                mesh_id: 200,
            },
            index_count: 3,
            instance_count: 1,
        });
        let (pipe_binds, bg_binds, vb_ib_sets) = dl.state_counters();
        assert_eq!(pipe_binds, 2); // pipeline 1 then 2
        assert_eq!(bg_binds, 3); // (1,10) then (1,11) then (2,20)
        assert_eq!(vb_ib_sets, 4); // 4 draws
    }
}
