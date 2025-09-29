//! Attachments: depth + offscreen color targets managed together.
//!
//! Centralizes creation and resize of depth and offscreen scene textures
//! used by the renderer. This helps keep resize paths idempotent and
//! consolidates texture/view lifetimes.

use wgpu::TextureFormat;

#[derive(Debug)]
pub(crate) struct Attachments {
    pub depth_view: wgpu::TextureView,
    pub scene_color: wgpu::Texture,
    pub scene_view: wgpu::TextureView,
    pub scene_read: wgpu::Texture,
    pub scene_read_view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    pub swapchain_format: TextureFormat,
    pub offscreen_format: TextureFormat,
}

impl Attachments {
    pub fn create(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        swapchain_format: TextureFormat,
        offscreen_format: TextureFormat,
    ) -> Self {
        let depth_view =
            crate::gfx::util::create_depth_view(device, width, height, swapchain_format);
        // Offscreen SceneColor (HDR)
        let scene_color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene-color"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: offscreen_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let scene_view = scene_color.create_view(&wgpu::TextureViewDescriptor::default());
        let scene_read = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene-read"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: offscreen_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let scene_read_view = scene_read.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            depth_view,
            scene_color,
            scene_view,
            scene_read,
            scene_read_view,
            width,
            height,
            swapchain_format,
            offscreen_format,
        }
    }

    pub fn rebuild(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            // Idempotent for equal sizes
            return;
        }
        *self = Self::create(
            device,
            width,
            height,
            self.swapchain_format,
            self.offscreen_format,
        );
    }
}

#[cfg(test)]
mod tests {
    use winit::dpi::PhysicalSize;

    fn compute_extents(max_dim: u32, requested: PhysicalSize<u32>) -> (u32, u32) {
        let (w, h) = crate::gfx::util::scale_to_max((requested.width, requested.height), max_dim);
        (w, h)
    }

    #[test]
    fn extent_clamp_is_idempotent() {
        // 4K request on a 2048 max should clamp and then remain stable
        let max_dim = 2048u32;
        let first = compute_extents(max_dim, PhysicalSize::new(3840, 2160));
        let second = compute_extents(max_dim, PhysicalSize::new(first.0, first.1));
        assert_eq!(first, second);
    }

    #[test]
    fn extent_small_sizes_unchanged() {
        let max_dim = 4096u32;
        let first = compute_extents(max_dim, PhysicalSize::new(1280, 720));
        assert_eq!(first, (1280, 720));
        let second = compute_extents(max_dim, PhysicalSize::new(1920, 1080));
        assert_eq!(second, (1920, 1080));
    }
}
