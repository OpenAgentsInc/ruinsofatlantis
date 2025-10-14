//! Upload ring (skeleton): CPU→GPU staging allocator for per‑frame updates.
//!
//! Minimal, behavior‑neutral scaffold with a simple bump allocator per frame.
//! If the current frame’s buffer is too small, it grows that buffer and
//! resets the cursor. Data is written via `Queue::write_buffer`.

#[allow(dead_code)]
pub struct UploadSlice {
    pub buffer: wgpu::Buffer,
    pub offset: u64,
    pub size: u64,
}

#[allow(dead_code)]
pub struct UploadRing {
    device: wgpu::Device,
    buffers: Vec<wgpu::Buffer>,
    sizes: Vec<u64>,
    usage: wgpu::BufferUsages,
    label: Option<String>,
    frame: usize,
    cursor: u64,
}

#[allow(dead_code)]
impl UploadRing {
    pub fn new(
        device: &wgpu::Device,
        frames: usize,
        initial_size: u64,
        usage: wgpu::BufferUsages,
        label: Option<&str>,
    ) -> Self {
        let frames = frames.max(1);
        let mut buffers = Vec::with_capacity(frames);
        let mut sizes = Vec::with_capacity(frames);
        for i in 0..frames {
            let lab = label
                .map(|s| format!("{}[{}]", s, i))
                .unwrap_or_else(|| format!("upload[{}]", i));
            let b = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&lab),
                size: initial_size,
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            buffers.push(b);
            sizes.push(initial_size);
        }
        Self {
            device: device.clone(),
            buffers,
            sizes,
            usage,
            label: label.map(|s| s.to_string()),
            frame: 0,
            cursor: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn _cursor_for_test(&self) -> u64 {
        self.cursor
    }
    #[cfg(test)]
    pub(crate) fn _frame_ix_for_test(&self) -> usize {
        self.frame
    }

    #[inline]
    pub fn next_frame(&mut self) {
        self.frame = (self.frame + 1) % self.buffers.len();
        self.cursor = 0;
    }

    #[inline]
    fn align_up(addr: u64, align: u64) -> u64 {
        if align <= 1 {
            return addr;
        }
        addr.div_ceil(align) * align
    }

    /// Allocates space and writes `data` into the current frame buffer.
    /// Grows the current frame's buffer if necessary (cursor resets on grow).
    pub fn allocate(&mut self, queue: &wgpu::Queue, data: &[u8], align: u64) -> UploadSlice {
        let idx = self.frame;
        let need = data.len() as u64;
        let off = Self::align_up(self.cursor, align.max(1));
        let cap = self.sizes[idx];
        // Grow buffer if necessary; reset cursor to 0 on growth.
        let (off, _cap) = if off + need > cap {
            // Double until it fits at offset 0.
            let mut new_cap = cap.max(256);
            while need > new_cap {
                new_cap *= 2;
            }
            let lab = self
                .label
                .as_deref()
                .map(|s| format!("{}[{}]", s, idx))
                .unwrap_or_else(|| format!("upload[{}]", idx));
            self.buffers[idx] = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&lab),
                size: new_cap,
                usage: self.usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.sizes[idx] = new_cap;
            self.cursor = 0;
            (0, new_cap)
        } else {
            (off, cap)
        };
        let buf = &self.buffers[idx];
        queue.write_buffer(buf, off, data);
        self.cursor = off + need;
        UploadSlice {
            buffer: buf.clone(),
            offset: off,
            size: need,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UploadRing;

    #[test]
    fn align_up_basics() {
        assert_eq!(UploadRing::align_up(0, 1), 0);
        assert_eq!(UploadRing::align_up(1, 1), 1);
        assert_eq!(UploadRing::align_up(1, 4), 4);
        assert_eq!(UploadRing::align_up(4, 4), 4);
        assert_eq!(UploadRing::align_up(5, 4), 8);
    }

    #[test]
    fn next_frame_resets_cursor() {
        // Create a minimal device for buffer creation
        let instance = wgpu::Instance::default();
        let adapter =
            match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: None,
            })) {
                Ok(a) => a,
                Err(_) => return, // skip test if no adapter (CI-safe)
            };
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("upload-test"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        }))
        .expect("device");
        let mut ring =
            UploadRing::new(&device, 3, 1024, wgpu::BufferUsages::COPY_SRC, Some("ring"));
        let before_frame = ring._frame_ix_for_test();
        let data = [1u8; 128];
        let _s = ring.allocate(&queue, &data, 256);
        assert!(ring._cursor_for_test() >= 128);
        ring.next_frame();
        assert_eq!(ring._cursor_for_test(), 0);
        assert_eq!(ring._frame_ix_for_test(), (before_frame + 1) % 3);
    }
}
