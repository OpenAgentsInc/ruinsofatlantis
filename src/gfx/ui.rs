//! UI overlay: minimal nameplates rendered as 2D text above wizards.
//!
//! This module implements a tiny CPU text atlas using `ab_glyph` and a simple
//! textured-quad pipeline to draw labels in screen space. It avoids pulling in
//! heavier text renderers to stay compatible with our wgpu version.

use std::collections::HashMap;

use ab_glyph::{Font, FontArc, Glyph, PxScale, ScaleFont};

use crate::gfx::pipeline;
use crate::gfx::types::{TextVertex, BarVertex};
use glam::Vec3;

struct GlyphInfo {
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    bounds_min: [f32; 2], // px_bounds().min relative to baseline position (0, ascent)
    size: [f32; 2],       // width/height in pixels
    advance: f32,         // advance width in pixels
    id: ab_glyph::GlyphId,
}

// ---- Damage Floaters (red numbers on hit) ----
pub struct DamageFloaters {
    // Font + metrics + atlas
    font: FontArc,
    scale: PxScale,
    ascent: f32,
    glyphs: std::collections::HashMap<char, GlyphInfo>,
    atlas_tex: wgpu::Texture,
    _atlas_view: wgpu::TextureView,
    _atlas_sampler: wgpu::Sampler,
    atlas_cpu: Vec<u8>,
    atlas_size: (u32, u32),

    // Pipeline
    _text_bgl: wgpu::BindGroupLayout,
    text_bg: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,

    // Geometry
    vbuf: wgpu::Buffer,
    vcount: u32,
    vcap_bytes: u64,

    // Live floaters
    items: Vec<Floater>,
}

#[derive(Clone, Debug)]
struct Floater {
    world: Vec3,
    value: i32,
    age: f32,
    life: f32,
    jitter_x: f32,
    rise_px_s: f32,
}

impl DamageFloaters {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        // Reuse the same font as nameplates
        let font_bytes: &'static [u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/fonts/NotoSans-Regular.ttf"
        ));
        let font = FontArc::try_from_slice(font_bytes)?;
        let px = 28.0; // slightly larger digits
        let scale = PxScale { x: px, y: px };
        let scaled = font.as_scaled(scale);
        let ascent = scaled.ascent();

        // Build atlas for ASCII digits and symbols we may show
        let atlas_w: u32 = 512;
        let mut atlas_h: u32 = 128;
        let mut atlas = vec![0u8; (atlas_w * atlas_h) as usize];
        let mut cursor_x: u32 = 1;
        let mut cursor_y: u32 = 1;
        let mut row_h: u32 = 0;

        let mut glyphs = std::collections::HashMap::new();
        for ch in "0123456789-".chars() {
            let gid = font.glyph_id(ch);
            let g0 = Glyph { id: gid, scale, position: ab_glyph::point(0.0, ascent) };
            if let Some(og) = font.outline_glyph(g0) {
                let bounds = og.px_bounds();
                let gw = bounds.width().ceil() as u32;
                let gh = bounds.height().ceil() as u32;
                let gw = gw.max(1);
                let gh = gh.max(1);
                if cursor_x + gw + 1 >= atlas_w { cursor_x = 1; cursor_y += row_h + 1; row_h = 0; }
                if cursor_y + gh + 1 >= atlas_h {
                    let new_h = (atlas_h * 2).max(cursor_y + gh + 2);
                    let mut new_buf = vec![0u8; (atlas_w * new_h) as usize];
                    for y in 0..atlas_h {
                        let off = (y * atlas_w) as usize;
                        new_buf[off..off + atlas_w as usize]
                            .copy_from_slice(&atlas[off..off + atlas_w as usize]);
                    }
                    atlas = new_buf;
                    atlas_h = new_h;
                }
                let ox = cursor_x as i32 + bounds.min.x.floor() as i32;
                let oy = cursor_y as i32 + bounds.min.y.floor() as i32;
                og.draw(|x, y, v| {
                    let px = (ox + x as i32) as u32;
                    let py = (oy + y as i32) as u32;
                    if px < atlas_w && py < atlas_h {
                        let idx = (py * atlas_w + px) as usize;
                        atlas[idx] = atlas[idx].max((v * 255.0) as u8);
                    }
                });
                let adv = scaled.h_advance(gid);
                glyphs.insert(
                    ch,
                    GlyphInfo {
                        uv_min: [(ox.max(0) as f32) / atlas_w as f32, (oy.max(0) as f32) / atlas_h as f32],
                        uv_max: [((ox.max(0) as u32 + gw) as f32) / atlas_w as f32, ((oy.max(0) as u32 + gh) as f32) / atlas_h as f32],
                        bounds_min: [bounds.min.x, bounds.min.y],
                        size: [gw as f32, gh as f32],
                        advance: adv,
                        id: gid,
                    },
                );
                cursor_x += gw + 1;
                row_h = row_h.max(gh);
            }
        }

        // Upload atlas
        let atlas_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("damage-atlas"),
            size: wgpu::Extent3d { width: atlas_w, height: atlas_h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            view_formats: &[],
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        });
        let atlas_view = atlas_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("damage-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let text_bgl = pipeline::create_text_bgl(device);
        let text_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("damage-texture-bg"),
            layout: &text_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&atlas_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&atlas_sampler) },
            ],
        });
        let shader = crate::gfx::pipeline::create_shader(device);
        let pipeline = pipeline::create_text_pipeline(device, &shader, &text_bgl, color_format);

        let vcap_bytes = 32 * 1024;
        let vbuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("damage-vbuf"),
            size: vcap_bytes,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            font,
            scale,
            ascent,
            glyphs,
            atlas_tex,
            _atlas_view: atlas_view,
            _atlas_sampler: atlas_sampler,
            atlas_cpu: atlas,
            atlas_size: (atlas_w, atlas_h),
            _text_bgl: text_bgl,
            text_bg,
            pipeline,
            vbuf,
            vcount: 0,
            vcap_bytes,
            items: Vec::new(),
        })
    }

    pub fn upload_atlas(&self, queue: &wgpu::Queue) {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &self.atlas_tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &self.atlas_cpu,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(self.atlas_size.0), rows_per_image: Some(self.atlas_size.1) },
            wgpu::Extent3d { width: self.atlas_size.0, height: self.atlas_size.1, depth_or_array_layers: 1 },
        );
    }

    pub fn spawn(&mut self, world: Vec3, value: i32) {
        let jitter = (rand_unit() * 12.0).clamp(-12.0, 12.0);
        self.items.push(Floater { world, value, age: 0.0, life: 0.9, jitter_x: jitter, rise_px_s: -45.0 });
    }

    pub fn update(&mut self, dt: f32) {
        self.items.retain_mut(|f| { f.age += dt; f.age < f.life });
    }

    fn build_vertices(
        &self,
        surface_w: u32,
        surface_h: u32,
        view_proj: glam::Mat4,
    ) -> Vec<TextVertex> {
        let w = surface_w as f32;
        let h = surface_h as f32;
        let scaled = self.font.as_scaled(self.scale);
        let mut verts: Vec<TextVertex> = Vec::new();
        for f in &self.items {
            let clip = view_proj * glam::Vec4::new(f.world.x, f.world.y, f.world.z, 1.0);
            if clip.w <= 0.0 { continue; }
            let ndc = clip.truncate() / clip.w;
            if ndc.x < -1.2 || ndc.x > 1.2 || ndc.y < -1.2 || ndc.y > 1.2 { continue; }
            let mut cx = (ndc.x * 0.5 + 0.5) * w;
            let mut cy = (1.0 - (ndc.y * 0.5 + 0.5)) * h;
            // Position baseline slightly above bars and rising over time
            cy = (cy - 72.0 + f.age * f.rise_px_s).max(0.0);
            // Measure text width to center it
            let s = f.value.to_string();
            let mut width = 0.0f32;
            let mut prev: Option<ab_glyph::GlyphId> = None;
            for ch in s.chars() {
                if let Some(gi) = self.glyphs.get(&ch) {
                    if let Some(pg) = prev { width += scaled.kern(pg, gi.id); }
                    width += gi.advance;
                    prev = Some(gi.id);
                }
            }
            // left align with jitter, then shift by -width/2 to center
            cx += f.jitter_x - width * 0.5;
            // Red color with slight fade over life
            let alpha = (1.0 - (f.age / f.life)).clamp(0.0, 1.0);
            let color = [1.0, 0.25, 0.2, alpha];
            // Emit glyph quads
            let mut pen_x = 0.0f32;
            prev = None;
            for ch in s.chars() {
                let Some(gi) = self.glyphs.get(&ch) else { continue; };
                if let Some(pg) = prev { pen_x += scaled.kern(pg, gi.id); }
                let x = cx + pen_x + gi.bounds_min[0];
                let y = cy - self.ascent + gi.bounds_min[1];
                let w_px = gi.size[0];
                let h_px = gi.size[1];
                let p0 = Nameplates::ndc_from_px(x, y, w, h);
                let p1 = Nameplates::ndc_from_px(x + w_px, y, w, h);
                let p2 = Nameplates::ndc_from_px(x + w_px, y + h_px, w, h);
                let p3 = Nameplates::ndc_from_px(x, y + h_px, w, h);
                let uv0 = gi.uv_min;
                let uv1 = [gi.uv_max[0], gi.uv_min[1]];
                let uv2 = gi.uv_max;
                let uv3 = [gi.uv_min[0], gi.uv_max[1]];
                verts.push(TextVertex { pos_ndc: p0, uv: uv0, color });
                verts.push(TextVertex { pos_ndc: p1, uv: uv1, color });
                verts.push(TextVertex { pos_ndc: p2, uv: uv2, color });
                verts.push(TextVertex { pos_ndc: p0, uv: uv0, color });
                verts.push(TextVertex { pos_ndc: p2, uv: uv2, color });
                verts.push(TextVertex { pos_ndc: p3, uv: uv3, color });
                pen_x += gi.advance;
                prev = Some(gi.id);
            }
        }
        verts
    }

    pub fn queue(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_w: u32,
        surface_h: u32,
        view_proj: glam::Mat4,
    ) {
        let verts = self.build_vertices(surface_w, surface_h, view_proj);
        self.vcount = verts.len() as u32;
        if self.vcount == 0 { return; }
        let bytes: &[u8] = bytemuck::cast_slice(&verts);
        if bytes.len() as u64 > self.vcap_bytes {
            let new_cap = (bytes.len() as u64).next_power_of_two();
            self.vbuf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("damage-vbuf"),
                size: new_cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.vcap_bytes = new_cap;
        }
        queue.write_buffer(&self.vbuf, 0, bytes);
    }

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        if self.vcount == 0 { return; }
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("damage-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.text_bg, &[]);
        rpass.set_vertex_buffer(0, self.vbuf.slice(..));
        rpass.draw(0..self.vcount, 0..1);
    }
}

fn rand_unit() -> f32 {
    use rand::Rng as _;
    let mut r = rand::rng();
    r.random::<f32>() * 2.0 - 1.0
}

#[cfg(test)]
mod floater_tests {
    use super::Floater;

    #[test]
    fn prune_expired() {
        let mut items = vec![
            Floater { world: glam::Vec3::ZERO, value: 10, age: 0.8, life: 0.9, jitter_x: 0.0, rise_px_s: -45.0 },
            Floater { world: glam::Vec3::ZERO, value: 5, age: 0.1, life: 0.9, jitter_x: 0.0, rise_px_s: -45.0 },
        ];
        // Emulate update(dt)
        items.retain_mut(|f| { f.age += 0.2; f.age < f.life });
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].value, 5);
    }
}

pub struct Nameplates {
    // Font + metrics
    font: FontArc,
    scale: PxScale,
    ascent: f32,
    // Atlas
    atlas_tex: wgpu::Texture,
    _atlas_view: wgpu::TextureView,
    _atlas_sampler: wgpu::Sampler,
    atlas_cpu: Vec<u8>,
    glyphs: HashMap<char, GlyphInfo>,
    atlas_size: (u32, u32),

    // Pipeline
    _text_bgl: wgpu::BindGroupLayout,
    text_bg: wgpu::BindGroup,
    text_pipeline: wgpu::RenderPipeline,

    // Geometry
    vbuf: wgpu::Buffer,
    vcount: u32,
    vcap_bytes: u64,
}

impl Nameplates {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        // Load a font (embedded from assets/fonts at compile time)
        let font_bytes: &'static [u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/fonts/NotoSans-Regular.ttf"
        ));
        let font = FontArc::try_from_slice(font_bytes)?;
        let px = 24.0; // logical pixel height (slightly smaller)
        let scale = PxScale { x: px, y: px };
        let scaled = font.as_scaled(scale);
        let ascent = scaled.ascent();

        // Pre-bake a basic ASCII atlas (printable range)
        let atlas_w: u32 = 1024;
        let mut atlas_h: u32 = 128;
        let mut atlas = vec![0u8; (atlas_w * atlas_h) as usize];
        let mut cursor_x: u32 = 1;
        let mut cursor_y: u32 = 1;
        let mut row_h: u32 = 0;

        let mut glyphs = HashMap::new();
        for ch_u in 32u8..=126u8 {
            let ch = ch_u as char;
            let gid = font.glyph_id(ch);
            let g0 = Glyph {
                id: gid,
                scale,
                position: ab_glyph::point(0.0, ascent),
            };
            if let Some(og) = font.outline_glyph(g0) {
                let bounds = og.px_bounds();
                // Round up width/height
                let gw = bounds.width().ceil() as u32;
                let gh = bounds.height().ceil() as u32;
                let gw = gw.max(1);
                let gh = gh.max(1);
                // Wrap to next row if needed
                if cursor_x + gw + 1 >= atlas_w {
                    cursor_x = 1;
                    cursor_y += row_h + 1;
                    row_h = 0;
                }
                // Grow atlas if needed (simple single reallocation)
                if cursor_y + gh + 1 >= atlas_h {
                    // double the height
                    let new_h = (atlas_h * 2).max(cursor_y + gh + 2);
                    let mut new_buf = vec![0u8; (atlas_w * new_h) as usize];
                    for y in 0..atlas_h {
                        let src = (y * atlas_w) as usize;
                        let dst = (y * atlas_w) as usize;
                        new_buf[dst..dst + atlas_w as usize]
                            .copy_from_slice(&atlas[src..src + atlas_w as usize]);
                    }
                    atlas = new_buf;
                    atlas_h = new_h;
                }
                // Draw glyph into atlas buffer at (cursor_x, cursor_y)
                let ox = cursor_x as i32 + bounds.min.x.floor() as i32;
                let oy = cursor_y as i32 + bounds.min.y.floor() as i32;
                og.draw(|x, y, v| {
                    let px = (ox + x as i32) as u32;
                    let py = (oy + y as i32) as u32;
                    if px < atlas_w && py < atlas_h {
                        let idx = (py * atlas_w + px) as usize;
                        let a = (v * 255.0) as u8;
                        // Max blend (keep strongest coverage)
                        atlas[idx] = atlas[idx].max(a);
                    }
                });

                let adv = scaled.h_advance(gid);
                glyphs.insert(
                    ch,
                    GlyphInfo {
                        uv_min: [
                            (ox.max(0) as f32) / (atlas_w as f32),
                            (oy.max(0) as f32) / (atlas_h as f32),
                        ],
                        uv_max: [
                            ((ox.max(0) as u32 + gw) as f32) / (atlas_w as f32),
                            ((oy.max(0) as u32 + gh) as f32) / (atlas_h as f32),
                        ],
                        bounds_min: [bounds.min.x, bounds.min.y],
                        size: [gw as f32, gh as f32],
                        advance: adv,
                        id: gid,
                    },
                );

                cursor_x += gw + 1;
                row_h = row_h.max(gh);
            }
        }

        // Upload atlas to GPU
        let atlas_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("nameplate-atlas"),
            size: wgpu::Extent3d {
                width: atlas_w,
                height: atlas_h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            view_formats: &[],
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        });
        let atlas_view = atlas_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("nameplate-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // BGL + pipeline
        let text_bgl = pipeline::create_text_bgl(device);
        let text_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("nameplate-texture-bg"),
            layout: &text_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });
        let shader = crate::gfx::pipeline::create_shader(device);
        let text_pipeline =
            pipeline::create_text_pipeline(device, &shader, &text_bgl, color_format);

        // Initial vertex buffer
        let vcap_bytes = 64 * 1024; // 64KB initial
        let vbuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("nameplate-vbuf"),
            size: vcap_bytes,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            font,
            scale,
            ascent,
            atlas_tex,
            _atlas_view: atlas_view,
            _atlas_sampler: atlas_sampler,
            atlas_cpu: atlas,
            glyphs,
            atlas_size: (atlas_w, atlas_h),
            _text_bgl: text_bgl,
            text_bg,
            text_pipeline,
            vbuf,
            vcount: 0,
            vcap_bytes,
        })
    }

    fn ndc_from_px(px: f32, py: f32, w: f32, h: f32) -> [f32; 2] {
        let x = (px / w) * 2.0 - 1.0;
        let y = 1.0 - (py / h) * 2.0;
        [x, y]
    }

    fn num_to_words(n: usize) -> String {
        // Supports 1..=99 for our demo needs
        let ones = [
            "Zero", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine",
        ];
        let teens = [
            "Ten",
            "Eleven",
            "Twelve",
            "Thirteen",
            "Fourteen",
            "Fifteen",
            "Sixteen",
            "Seventeen",
            "Eighteen",
            "Nineteen",
        ];
        let tens = [
            "", "", "Twenty", "Thirty", "Forty", "Fifty", "Sixty", "Seventy", "Eighty", "Ninety",
        ];
        if n < 10 {
            return ones[n].to_string();
        }
        if n < 20 {
            return teens[n - 10].to_string();
        }
        if n < 100 {
            let t = n / 10;
            let o = n % 10;
            if o == 0 {
                tens[t].to_string()
            } else {
                format!("{}{}", tens[t], ones[o])
            }
        } else {
            n.to_string()
        }
    }

    pub fn queue_labels(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_w: u32,
        surface_h: u32,
        view_proj: glam::Mat4,
        wizard_models: &[glam::Mat4],
    ) {
        let mut verts: Vec<TextVertex> = Vec::new();
        let w = surface_w as f32;
        let h = surface_h as f32;
        let ascent = self.ascent;

        // Precompute names
        let labels: Vec<String> = (0..wizard_models.len())
            .map(|i| format!("Wizard{}", Self::num_to_words(i + 1)))
            .collect();

        let scaled = self.font.as_scaled(self.scale);

        for (i, m) in wizard_models.iter().enumerate() {
            // World-space head position: model * (0, 1.7, 0)
            let p = *m * glam::Vec4::new(0.0, 1.7, 0.0, 1.0);
            let clip = view_proj * p;
            if clip.w <= 0.0 {
                continue;
            }
            let ndc = clip.truncate() / clip.w;
            if ndc.x < -1.2 || ndc.x > 1.2 || ndc.y < -1.2 || ndc.y > 1.2 {
                continue;
            }
            let mut cx = (ndc.x * 0.5 + 0.5) * w;
            let cy = (1.0 - (ndc.y * 0.5 + 0.5)) * h;
            // Place name ABOVE the health bar with small padding (~8px above bar)
            // Bars are anchored ~48px above the head center; text at ~56px.
            let baseline_y = (cy - 56.0).max(0.0);

            // Measure label width
            let text = &labels[i];
            let mut prev: Option<ab_glyph::GlyphId> = None;
            let mut width = 0.0f32;
            for ch in text.chars() {
                let gi = match self.glyphs.get(&ch) {
                    Some(g) => g,
                    None => continue,
                };
                if let Some(pg) = prev {
                    width += scaled.kern(pg, gi.id);
                }
                width += gi.advance;
                prev = Some(gi.id);
            }
            cx -= width * 0.5; // center horizontally

            // Emit quads
            let mut pen_x = 0.0f32;
            prev = None;
            for ch in text.chars() {
                let gi = match self.glyphs.get(&ch) {
                    Some(g) => g,
                    None => continue,
                };
                if let Some(pg) = prev {
                    pen_x += scaled.kern(pg, gi.id);
                }

                let x = cx + pen_x + gi.bounds_min[0];
                let y = baseline_y - ascent + gi.bounds_min[1];
                let w_px = gi.size[0];
                let h_px = gi.size[1];

                let p0 = Self::ndc_from_px(x, y, w, h);
                let p1 = Self::ndc_from_px(x + w_px, y, w, h);
                let p2 = Self::ndc_from_px(x + w_px, y + h_px, w, h);
                let p3 = Self::ndc_from_px(x, y + h_px, w, h);
                let uv0 = gi.uv_min;
                let uv1 = [gi.uv_max[0], gi.uv_min[1]];
                let uv2 = gi.uv_max;
                let uv3 = [gi.uv_min[0], gi.uv_max[1]];

                let white = [1.0, 1.0, 1.0, 1.0];
                verts.push(TextVertex { pos_ndc: p0, uv: uv0, color: white });
                verts.push(TextVertex { pos_ndc: p1, uv: uv1, color: white });
                verts.push(TextVertex { pos_ndc: p2, uv: uv2, color: white });
                verts.push(TextVertex { pos_ndc: p0, uv: uv0, color: white });
                verts.push(TextVertex { pos_ndc: p2, uv: uv2, color: white });
                verts.push(TextVertex { pos_ndc: p3, uv: uv3, color: white });

                pen_x += gi.advance;
                prev = Some(gi.id);
            }
        }

        self.vcount = verts.len() as u32;
        let bytes: &[u8] = bytemuck::cast_slice(&verts);
        if bytes.len() as u64 > self.vcap_bytes {
            let new_cap = (bytes.len() as u64).next_power_of_two();
            self.vbuf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("nameplate-vbuf"),
                size: new_cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.vcap_bytes = new_cap;
        }
        queue.write_buffer(&self.vbuf, 0, bytes);
    }

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        if self.vcount == 0 {
            return;
        }
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("nameplate-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        rpass.set_pipeline(&self.text_pipeline);
        rpass.set_bind_group(0, &self.text_bg, &[]);
        rpass.set_vertex_buffer(0, self.vbuf.slice(..));
        rpass.draw(0..self.vcount, 0..1);
    }
}

// ---- Health Bars Overlay ----
pub struct HealthBars {
    pipeline: wgpu::RenderPipeline,
    vbuf: wgpu::Buffer,
    vcount: u32,
    vcap_bytes: u64,
}

impl HealthBars {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bars-shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("shader.wgsl"))),
        });
        let pipeline = pipeline::create_bar_pipeline(device, &shader, color_format);
        let vcap_bytes = 64 * 1024;
        let vbuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("healthbar-vbuf"),
            size: vcap_bytes,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Self { pipeline, vbuf, vcount: 0, vcap_bytes })
    }

    fn color_for_frac(frac: f32) -> [f32; 4] {
        let f = frac.clamp(0.0, 1.0);
        if f >= 0.5 {
            // yellow -> green
            let t = (f - 0.5) / 0.5;
            let r = 1.0 - t;
            let g = 1.0;
            let b = 0.0;
            [r, g, b, 1.0]
        } else {
            // red -> yellow
            let t = f / 0.5;
            let r = 1.0;
            let g = t;
            let b = 0.0;
            [r, g, b, 1.0]
        }
    }

    fn ndc_from_px(px: f32, py: f32, w: f32, h: f32) -> [f32; 2] {
        let x = (px / w) * 2.0 - 1.0;
        let y = 1.0 - (py / h) * 2.0;
        [x, y]
    }

    fn build_vertices(
        surface_w: u32,
        surface_h: u32,
        view_proj: glam::Mat4,
        entries: &[(glam::Vec3, f32)],
    ) -> Vec<BarVertex> {
        let w = surface_w as f32;
        let h = surface_h as f32;
        let mut out: Vec<BarVertex> = Vec::new();
        let bar_w = 64.0f32;
        let bar_h = 6.0f32;
        for (world, frac) in entries {
            let clip = view_proj * glam::Vec4::new(world.x, world.y, world.z, 1.0);
            if clip.w <= 0.0 { continue; }
            let ndc = clip.truncate() / clip.w;
            if ndc.x < -1.2 || ndc.x > 1.2 || ndc.y < -1.2 || ndc.y > 1.2 { continue; }
            let cx = (ndc.x * 0.5 + 0.5) * w;
            let cy = (1.0 - (ndc.y * 0.5 + 0.5)) * h;
            // Anchor bar higher above the head to avoid overlapping the name text
            let by0 = (cy - 48.0).max(0.0);
            let by1 = by0 + bar_h;
            // Foreground (filled) quad based on frac
            let frac = frac.clamp(0.0, 1.0);
            if frac > 0.0 {
                let fw = bar_w * frac;
                let fx0 = cx - bar_w * 0.5;
                let fx1 = fx0 + fw;
                let fy0 = by0;
                let fy1 = by1;
                let col = Self::color_for_frac(frac);
                let q0 = Self::ndc_from_px(fx0, fy0, w, h);
                let q1 = Self::ndc_from_px(fx1, fy0, w, h);
                let q2 = Self::ndc_from_px(fx1, fy1, w, h);
                let q3 = Self::ndc_from_px(fx0, fy1, w, h);
                out.push(BarVertex { pos_ndc: q0, color: col });
                out.push(BarVertex { pos_ndc: q1, color: col });
                out.push(BarVertex { pos_ndc: q2, color: col });
                out.push(BarVertex { pos_ndc: q0, color: col });
                out.push(BarVertex { pos_ndc: q2, color: col });
                out.push(BarVertex { pos_ndc: q3, color: col });
            }
        }
        out
    }

    pub fn queue_entries(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_w: u32,
        surface_h: u32,
        view_proj: glam::Mat4,
        entries: &[(glam::Vec3, f32)],
    ) {
        let verts = Self::build_vertices(surface_w, surface_h, view_proj, entries);
        self.vcount = verts.len() as u32;
        let bytes: &[u8] = bytemuck::cast_slice(&verts);
        if bytes.len() as u64 > self.vcap_bytes {
            let new_cap = (bytes.len() as u64).next_power_of_two();
            self.vbuf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("healthbar-vbuf"),
                size: new_cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.vcap_bytes = new_cap;
        }
        queue.write_buffer(&self.vbuf, 0, bytes);
    }

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        if self.vcount == 0 { return; }
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("healthbar-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        rpass.set_pipeline(&self.pipeline);
        rpass.set_vertex_buffer(0, self.vbuf.slice(..));
        rpass.draw(0..self.vcount, 0..1);
    }
}

impl Nameplates {
    pub fn upload_atlas(&self, queue: &wgpu::Queue) {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.atlas_cpu,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.atlas_size.0),
                rows_per_image: Some(self.atlas_size.1),
            },
            wgpu::Extent3d {
                width: self.atlas_size.0,
                height: self.atlas_size.1,
                depth_or_array_layers: 1,
            },
        );
    }
}

impl Nameplates {
    #[allow(clippy::too_many_arguments)]
    pub fn queue_npc_labels(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_w: u32,
        surface_h: u32,
        view_proj: glam::Mat4,
        positions: &[glam::Vec3],
        label: &str,
    ) {
        let w = surface_w as f32;
        let h = surface_h as f32;
        let scaled = self.font.as_scaled(self.scale);
        // measure label once
        let mut width = 0.0f32;
        let mut prev: Option<ab_glyph::GlyphId> = None;
        for ch in label.chars() {
            if let Some(gi) = self.glyphs.get(&ch) {
                if let Some(pg) = prev { width += scaled.kern(pg, gi.id); }
                width += gi.advance;
                prev = Some(gi.id);
            }
        }
        let mut verts: Vec<TextVertex> = Vec::new();
        for world in positions {
            let clip = view_proj * glam::Vec4::new(world.x, world.y, world.z, 1.0);
            if clip.w <= 0.0 { continue; }
            let ndc = clip.truncate() / clip.w;
            if ndc.x < -1.2 || ndc.x > 1.2 || ndc.y < -1.2 || ndc.y > 1.2 { continue; }
            let mut cx = (ndc.x * 0.5 + 0.5) * w;
            let cy = (1.0 - (ndc.y * 0.5 + 0.5)) * h;
            // Text just above the bar (same offset as wizards)
            let baseline_y = (cy - 56.0).max(0.0);
            cx -= width * 0.5;
            // Emit quads for label
            let mut pen_x = 0.0f32;
            prev = None;
            for ch in label.chars() {
                let Some(gi) = self.glyphs.get(&ch) else { continue; };
                if let Some(pg) = prev { pen_x += scaled.kern(pg, gi.id); }
                let x = cx + pen_x + gi.bounds_min[0];
                let y = baseline_y - self.ascent + gi.bounds_min[1];
                let w_px = gi.size[0];
                let h_px = gi.size[1];
                let p0 = Self::ndc_from_px(x, y, w, h);
                let p1 = Self::ndc_from_px(x + w_px, y, w, h);
                let p2 = Self::ndc_from_px(x + w_px, y + h_px, w, h);
                let p3 = Self::ndc_from_px(x, y + h_px, w, h);
                let uv0 = gi.uv_min;
                let uv1 = [gi.uv_max[0], gi.uv_min[1]];
                let uv2 = gi.uv_max;
                let uv3 = [gi.uv_min[0], gi.uv_max[1]];
                let white = [1.0, 1.0, 1.0, 1.0];
                verts.push(TextVertex { pos_ndc: p0, uv: uv0, color: white });
                verts.push(TextVertex { pos_ndc: p1, uv: uv1, color: white });
                verts.push(TextVertex { pos_ndc: p2, uv: uv2, color: white });
                verts.push(TextVertex { pos_ndc: p0, uv: uv0, color: white });
                verts.push(TextVertex { pos_ndc: p2, uv: uv2, color: white });
                verts.push(TextVertex { pos_ndc: p3, uv: uv3, color: white });
                pen_x += gi.advance;
                prev = Some(gi.id);
            }
        }
        self.vcount = verts.len() as u32;
        if self.vcount == 0 { return; }
        let bytes: &[u8] = bytemuck::cast_slice(&verts);
        if bytes.len() as u64 > self.vcap_bytes {
            let new_cap = (bytes.len() as u64).next_power_of_two();
            self.vbuf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("nameplate-vbuf"),
                size: new_cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.vcap_bytes = new_cap;
        }
        queue.write_buffer(&self.vbuf, 0, bytes);
    }
}

#[cfg(test)]
mod bar_tests {
    use super::HealthBars;
    use glam::Mat4;
    #[test]
    fn color_mapping_is_monotonic() {
        let g = HealthBars::color_for_frac(1.0);
        let y = HealthBars::color_for_frac(0.5);
        let r = HealthBars::color_for_frac(0.0);
        assert!((g[1] - 1.0).abs() < 1e-6 && g[0] < 0.5);
        assert!((y[0] - 1.0).abs() < 1e-6 && (y[1] - 1.0).abs() < 1e-6);
        assert!((r[0] - 1.0).abs() < 1e-6 && r[1] < 0.1);
    }

    #[test]
    fn build_vertices_counts_match_fill_fraction() {
        let vp = Mat4::IDENTITY;
        // One entry at origin, full health -> filled (6)
        let v_full = HealthBars::build_vertices(1920, 1080, vp, &[(glam::Vec3::ZERO, 1.0)]);
        assert_eq!(v_full.len(), 6);
        // Zero health -> no vertices
        let v_zero = HealthBars::build_vertices(1920, 1080, vp, &[(glam::Vec3::ZERO, 0.0)]);
        assert_eq!(v_zero.len(), 0);
    }
}
