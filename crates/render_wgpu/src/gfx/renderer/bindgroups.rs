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
            // Refresh recency: move this key to the back
            if let Some(pos) = self.order.iter().position(|k| *k == key) {
                self.order.remove(pos);
            }
            self.order.push_back(key.clone());
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
        let inserted = self.map.insert(key.clone(), bg);
        debug_assert!(inserted.is_none());
        // Return the freshly inserted one
        self.map.get(&key).unwrap().clone()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::util::DeviceExt;

    fn make_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
        }));
        let adapter = match adapter {
            Ok(a) => a,
            Err(_) => return None,
        };
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("bgcache-test-device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        }))
        .expect("device");
        Some((device, queue))
    }

    fn make_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bgcache-test-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(std::num::NonZeroU64::new(16).unwrap()),
                },
                count: None,
            }],
        })
    }

    fn make_uniform(device: &wgpu::Device) -> wgpu::Buffer {
        let data = [0u8; 16];
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("bgcache-test-ubo"),
            contents: &data,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        })
    }

    #[test]
    fn cache_counts_and_len_behave() {
        let Some((device, _queue)) = make_device() else {
            return;
        }; // skip if no adapter in CI
        let bgl = make_layout(&device);
        let buf_a = make_uniform(&device);
        let buf_b = make_uniform(&device);
        let mut cache = BgCache::with_capacity(8);

        let key_a = BgKey::new(&bgl, &[&buf_a as *const _ as u64]);
        let key_b = BgKey::new(&bgl, &[&buf_b as *const _ as u64]);

        let _a0 = cache.get_or_create(key_a.clone(), || {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("A0"),
                layout: &bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf_a.as_entire_binding(),
                }],
            })
        });
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.misses, 1);
        assert_eq!(cache.hits, 0);

        // Hit on A again
        let _a1 = cache.get_or_create(key_a.clone(), || unreachable!("should hit"));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.misses, 1);
        assert_eq!(cache.hits, 1);

        // Miss on B
        let _b0 = cache.get_or_create(key_b.clone(), || {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("B0"),
                layout: &bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf_b.as_entire_binding(),
                }],
            })
        });
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.misses, 2);
        assert_eq!(cache.hits, 1);

        // Hit on B
        let _b1 = cache.get_or_create(key_b.clone(), || unreachable!("should hit"));
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.misses, 2);
        assert_eq!(cache.hits, 2);
        assert_eq!(cache.evictions, 0);
    }

    #[test]
    fn cache_evicts_oldest_when_at_capacity() {
        let Some((device, _queue)) = make_device() else {
            return;
        }; // skip if no adapter in CI
        let bgl = make_layout(&device);
        let buf_a = make_uniform(&device);
        let buf_b = make_uniform(&device);
        let buf_c = make_uniform(&device);
        let mut cache = BgCache::with_capacity(2);

        let key_a = BgKey::new(&bgl, &[&buf_a as *const _ as u64]);
        let key_b = BgKey::new(&bgl, &[&buf_b as *const _ as u64]);
        let key_c = BgKey::new(&bgl, &[&buf_c as *const _ as u64]);

        let _ = cache.get_or_create(key_a.clone(), || {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("A"),
                layout: &bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf_a.as_entire_binding(),
                }],
            })
        });
        let _ = cache.get_or_create(key_b.clone(), || {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("B"),
                layout: &bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf_b.as_entire_binding(),
                }],
            })
        });
        // Refresh A so B becomes the oldest
        let _ = cache.get_or_create(key_a.clone(), || unreachable!());

        // Insert C, should evict B
        let _ = cache.get_or_create(key_c.clone(), || {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("C"),
                layout: &bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf_c.as_entire_binding(),
                }],
            })
        });
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.evictions, 1);
    }
}
