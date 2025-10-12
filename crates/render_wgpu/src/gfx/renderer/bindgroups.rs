//! BindGroup cache (skeleton) with simple LRU eviction.
//!
//! Purpose: avoid rebuilding identical bind groups across frames by using
//! a stable key of resource ids. This skeleton provides a small API surface
//! and counters; adoption will follow in later PRs.

use std::collections::{HashMap, VecDeque};

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
#[allow(dead_code)]
pub struct BgKey {
    pub layout_hash: u64,
    pub ids: Vec<u64>,
}

#[allow(dead_code)]
impl BgKey {
    pub fn new(layout: &wgpu::BindGroupLayout, ids: &[u64]) -> Self {
        let layout_hash = (layout as *const _ as usize) as u64;
        Self {
            layout_hash,
            ids: ids.to_vec(),
        }
    }
}

#[allow(dead_code)]
pub struct BgCache {
    map: HashMap<BgKey, wgpu::BindGroup>,
    order: VecDeque<BgKey>,
    cap: usize,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

#[allow(dead_code)]
impl BgCache {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            cap,
            hits: 0,
            misses: 0,
            evictions: 0,
        }
    }

    /// Return a cached bind group or create+insert it via `make`.
    pub fn get_or_create<F>(&mut self, key: BgKey, make: F) -> wgpu::BindGroup
    where
        F: FnOnce() -> wgpu::BindGroup,
    {
        if let Some(bg) = self.map.get(&key) {
            self.hits += 1;
            return bg.clone();
        }
        self.misses += 1;
        // Evict oldest if over capacity (simple FIFO/LRU-ish)
        if self.map.len() >= self.cap
            && let Some(old) = self.order.pop_front()
            && self.map.remove(&old).is_some()
        {
            self.evictions += 1;
        }
        let bg = make();
        self.order.push_back(key.clone());
        let inserted = self.map.insert(key, bg);
        debug_assert!(inserted.is_none());
        self.map.values().last().unwrap().clone()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }
}
