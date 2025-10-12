//! CPU-only draw list builder and batch grouper.
//!
//! Pure data structures for deterministic grouping of draws by a key
//! (e.g., pipeline/material/mesh). Rendering integration can consume
//! the produced batches to minimize state changes.

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct DrawKey {
    pub pipeline_id: u32,
    pub material_id: u32,
    pub mesh_id: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DrawItem {
    pub key: DrawKey,
    pub index_count: u32,
    pub instance_count: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DrawBatch {
    pub key: DrawKey,
    pub index_count: u32,
    pub instance_count: u32,
}

#[derive(Default)]
pub struct DrawList {
    items: Vec<DrawItem>,
}

impl DrawList {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }
    pub fn add(&mut self, item: DrawItem) {
        self.items.push(item);
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Group contiguous items with the same key into batches.
    pub fn to_batches(&self) -> Vec<DrawBatch> {
        let mut out: Vec<DrawBatch> = Vec::new();
        for it in &self.items {
            if let Some(last) = out.last_mut() {
                if last.key == it.key {
                    last.index_count += it.index_count;
                    last.instance_count += it.instance_count;
                    continue;
                }
            }
            out.push(DrawBatch {
                key: it.key,
                index_count: it.index_count,
                instance_count: it.instance_count,
            });
        }
        out
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
}
