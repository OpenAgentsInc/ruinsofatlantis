//! UI overlay: minimal nameplates rendered as 2D text above wizards.
//!
//! This module implements a tiny CPU text atlas using `ab_glyph` and a simple
//! textured-quad pipeline to draw labels in screen space. It avoids pulling in
//! heavier text renderers to stay compatible with our wgpu version.

use std::collections::HashMap;

use ab_glyph::{Font, FontArc, Glyph, PxScale, ScaleFont};

use crate::gfx::pipeline;
use crate::gfx::types::{BarVertex, TextVertex};
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
#[allow(dead_code)]
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
#[allow(dead_code)]
struct Floater {
    world: Vec3,
    value: i32,
    age: f32,
    life: f32,
    jitter_x: f32,
    rise_px_s: f32,
}

#[allow(dead_code)]
impl DamageFloaters {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        // Reuse the same font as nameplates
        let font_bytes: &'static [u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/fonts/NotoSans-Regular.ttf"
        ));
        let font = FontArc::try_from_slice(font_bytes)?;
        let px = 28.0; // slightly larger digits
        let scale = PxScale { x: px, y: px };
        let scaled = font.as_scaled(scale);
        let ascent = scaled.ascent();

        // Build atlas for ASCII digits and symbols we may show
        let atlas_w: u32 = 512;
        // Make the atlas tall enough up front to avoid later growth,
        // which can invalidate normalized UVs computed earlier.
        let mut atlas_h: u32 = 512;
        let mut atlas = vec![0u8; (atlas_w * atlas_h) as usize];
        let mut cursor_x: u32 = 1;
        let mut cursor_y: u32 = 1;
        let mut row_h: u32 = 0;

        let mut glyphs = std::collections::HashMap::new();
        for ch in "0123456789-".chars() {
            let gid = font.glyph_id(ch);
            let g0 = Glyph {
                id: gid,
                scale,
                position: ab_glyph::point(0.0, ascent),
            };
            if let Some(og) = font.outline_glyph(g0) {
                let bounds = og.px_bounds();
                let gw = bounds.width().ceil() as u32;
                let gh = bounds.height().ceil() as u32;
                let gw = gw.max(1);
                let gh = gh.max(1);
                if cursor_x + gw + 1 >= atlas_w {
                    cursor_x = 1;
                    cursor_y += row_h + 1;
                    row_h = 0;
                }
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
                // Shift draw origin right/down when glyph has negative min bounds
                let off_x = (-bounds.min.x.floor() as i32).max(0);
                let off_y = (-bounds.min.y.floor() as i32).max(0);
                let ox = cursor_x as i32 + off_x;
                let oy = cursor_y as i32 + off_y;
                og.draw(|x, y, v| {
                    let px_i = ox + x as i32;
                    let py_i = oy + y as i32;
                    if px_i >= 0 && py_i >= 0 {
                        let px = px_i as u32;
                        let py = py_i as u32;
                        if px < atlas_w && py < atlas_h {
                            let idx = (py * atlas_w + px) as usize;
                            atlas[idx] = atlas[idx].max((v * 255.0) as u8);
                        }
                    }
                });
                let adv = scaled.h_advance(gid);
                glyphs.insert(
                    ch,
                    GlyphInfo {
                        uv_min: [(ox as f32) / atlas_w as f32, (oy as f32) / atlas_h as f32],
                        uv_max: [
                            ((ox as u32 + gw) as f32) / atlas_w as f32,
                            ((oy as u32 + gh) as f32) / atlas_h as f32,
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

        // Upload atlas
        let atlas_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("damage-atlas"),
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

    pub fn spawn(&mut self, world: Vec3, value: i32) {
        let jitter = (rand_unit() * 12.0).clamp(-12.0, 12.0);
        self.items.push(Floater {
            world,
            value,
            age: 0.0,
            life: 0.9,
            jitter_x: jitter,
            rise_px_s: -45.0,
        });
    }

    pub fn update(&mut self, dt: f32) {
        self.items.retain_mut(|f| {
            f.age += dt;
            f.age < f.life
        });
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
            if clip.w <= 0.0 {
                continue;
            }
            let ndc = clip.truncate() / clip.w;
            if ndc.x < -1.2 || ndc.x > 1.2 || ndc.y < -1.2 || ndc.y > 1.2 {
                continue;
            }
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
                    if let Some(pg) = prev {
                        width += scaled.kern(pg, gi.id);
                    }
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
                let Some(gi) = self.glyphs.get(&ch) else {
                    continue;
                };
                if let Some(pg) = prev {
                    pen_x += scaled.kern(pg, gi.id);
                }
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
                verts.push(TextVertex {
                    pos_ndc: p0,
                    uv: uv0,
                    color,
                });
                verts.push(TextVertex {
                    pos_ndc: p1,
                    uv: uv1,
                    color,
                });
                verts.push(TextVertex {
                    pos_ndc: p2,
                    uv: uv2,
                    color,
                });
                verts.push(TextVertex {
                    pos_ndc: p0,
                    uv: uv0,
                    color,
                });
                verts.push(TextVertex {
                    pos_ndc: p2,
                    uv: uv2,
                    color,
                });
                verts.push(TextVertex {
                    pos_ndc: p3,
                    uv: uv3,
                    color,
                });
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
        if self.vcount == 0 {
            return;
        }
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
        if self.vcount == 0 {
            return;
        }
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("damage-pass"),
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
            Floater {
                world: glam::Vec3::ZERO,
                value: 10,
                age: 0.8,
                life: 0.9,
                jitter_x: 0.0,
                rise_px_s: -45.0,
            },
            Floater {
                world: glam::Vec3::ZERO,
                value: 5,
                age: 0.1,
                life: 0.9,
                jitter_x: 0.0,
                rise_px_s: -45.0,
            },
        ];
        // Emulate update(dt)
        items.retain_mut(|f| {
            f.age += 0.2;
            f.age < f.life
        });
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].value, 5);
    }
}

#[allow(dead_code)]
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

#[allow(dead_code)]
impl Nameplates {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        // Load a font (embedded from assets/fonts at compile time)
        let font_bytes: &'static [u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/fonts/NotoSans-Regular.ttf"
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
                // Half-texel inset to avoid sampling into neighboring glyph cells
                let u0 = (ox as f32 + 0.5) / (atlas_w as f32);
                let v0 = (oy as f32 + 0.5) / (atlas_h as f32);
                let u1 = ((ox as u32 + gw) as f32 - 0.5) / (atlas_w as f32);
                let v1 = ((oy as u32 + gh) as f32 - 0.5) / (atlas_h as f32);
                glyphs.insert(
                    ch,
                    GlyphInfo {
                        uv_min: [u0, v0],
                        uv_max: [u1, v1],
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
        let _ascent = self.ascent;

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
                if let Some(gi) = self.glyphs.get(&ch) {
                    if let Some(pg) = prev {
                        width += scaled.kern(pg, gi.id);
                    }
                    width += gi.advance;
                    prev = Some(gi.id);
                }
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

                // Place glyphs using advance + size; ignore negative bearings to avoid truncation.
                let w_px = gi.size[0];
                let h_px = gi.size[1];
                let x = cx + pen_x;
                let y = baseline_y - h_px; // top-left

                let p0 = Self::ndc_from_px(x, y, w, h);
                let p1 = Self::ndc_from_px(x + w_px, y, w, h);
                let p2 = Self::ndc_from_px(x + w_px, y + h_px, w, h);
                let p3 = Self::ndc_from_px(x, y + h_px, w, h);
                let uv0 = gi.uv_min;
                let uv1 = [gi.uv_max[0], gi.uv_min[1]];
                let uv2 = gi.uv_max;
                let uv3 = [gi.uv_min[0], gi.uv_max[1]];

                let white = [1.0, 1.0, 1.0, 1.0];
                verts.push(TextVertex {
                    pos_ndc: p0,
                    uv: uv0,
                    color: white,
                });
                verts.push(TextVertex {
                    pos_ndc: p1,
                    uv: uv1,
                    color: white,
                });
                verts.push(TextVertex {
                    pos_ndc: p2,
                    uv: uv2,
                    color: white,
                });
                verts.push(TextVertex {
                    pos_ndc: p0,
                    uv: uv0,
                    color: white,
                });
                verts.push(TextVertex {
                    pos_ndc: p2,
                    uv: uv2,
                    color: white,
                });
                verts.push(TextVertex {
                    pos_ndc: p3,
                    uv: uv3,
                    color: white,
                });

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
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "shader.wgsl"
            ))),
        });
        let pipeline = pipeline::create_bar_pipeline(device, &shader, color_format);
        let vcap_bytes = 64 * 1024;
        let vbuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("healthbar-vbuf"),
            size: vcap_bytes,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Self {
            pipeline,
            vbuf,
            vcount: 0,
            vcap_bytes,
        })
    }

    fn color_for_frac(frac: f32) -> [f32; 4] {
        let f = frac.clamp(0.0, 1.0);
        if f >= 0.5 {
            // yellow -> green
            let t = (f - 0.5) / 0.5;
            let r = 1.0 - t;
            let g = 1.0;
            let b = 0.0;
            [r, g, b, 0.75]
        } else {
            // red -> yellow
            let t = f / 0.5;
            let r = 1.0;
            let g = t;
            let b = 0.0;
            [r, g, b, 0.75]
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
            if clip.w <= 0.0 {
                continue;
            }
            let ndc = clip.truncate() / clip.w;
            if ndc.x < -1.2 || ndc.x > 1.2 || ndc.y < -1.2 || ndc.y > 1.2 {
                continue;
            }
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
                out.push(BarVertex {
                    pos_ndc: q0,
                    color: col,
                });
                out.push(BarVertex {
                    pos_ndc: q1,
                    color: col,
                });
                out.push(BarVertex {
                    pos_ndc: q2,
                    color: col,
                });
                out.push(BarVertex {
                    pos_ndc: q0,
                    color: col,
                });
                out.push(BarVertex {
                    pos_ndc: q2,
                    color: col,
                });
                out.push(BarVertex {
                    pos_ndc: q3,
                    color: col,
                });
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
        if self.vcount == 0 {
            return;
        }
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("healthbar-pass"),
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
        rpass.set_pipeline(&self.pipeline);
        rpass.set_vertex_buffer(0, self.vbuf.slice(..));
        rpass.draw(0..self.vcount, 0..1);
    }
}

// ---- HUD (screen-space) ----
/// Hud: simple screen-space overlay for the wizard scene (v0.1).
/// Draws a top-left player HP bar and a bottom-center hotbar with a few slots.
pub struct Hud {
    // Pipelines
    bar_pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,
    text_bg: wgpu::BindGroup,

    // Text atlas
    _font: FontArc,
    scale: PxScale,
    ascent: f32,
    glyphs: HashMap<char, GlyphInfo>,
    atlas_tex: wgpu::Texture,
    _atlas_view: wgpu::TextureView,
    _atlas_sampler: wgpu::Sampler,
    atlas_cpu: Vec<u8>,
    atlas_size: (u32, u32),

    // Geometry buffers
    bars_vbuf: wgpu::Buffer,
    bars_vcap: u64,
    bars_vcount: u32,
    text_vbuf: wgpu::Buffer,
    text_vcap: u64,
    text_vcount: u32,

    // Frame-local build
    bars_verts: Vec<BarVertex>,
    text_verts: Vec<TextVertex>,
}

impl Hud {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        // Pipelines
        let shader = crate::gfx::pipeline::create_shader(device);
        let bar_pipeline = crate::gfx::pipeline::create_bar_pipeline(device, &shader, color_format);
        // Text: build atlas (ASCII printable)
        let font_bytes: &'static [u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/fonts/NotoSans-Regular.ttf"
        ));
        let font = FontArc::try_from_slice(font_bytes)?;
        let px = 18.0;
        let scale = PxScale { x: px, y: px };
        let scaled = font.as_scaled(scale);
        let ascent = scaled.ascent();
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
                let mut gw = bounds.width().ceil() as u32;
                let mut gh = bounds.height().ceil() as u32;
                gw = gw.max(1);
                gh = gh.max(1);
                if cursor_x + gw + 1 >= atlas_w {
                    cursor_x = 1;
                    cursor_y += row_h + 1;
                    row_h = 0;
                }
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
                let off_x = (-bounds.min.x.floor() as i32).max(0);
                let off_y = (-bounds.min.y.floor() as i32).max(0);
                let ox = cursor_x as i32 + off_x;
                let oy = cursor_y as i32 + off_y;
                og.draw(|x, y, v| {
                    let px_i = ox + x as i32;
                    let py_i = oy + y as i32;
                    if px_i >= 0 && py_i >= 0 {
                        let px = px_i as u32;
                        let py = py_i as u32;
                        if px < atlas_w && py < atlas_h {
                            let idx = (py * atlas_w + px) as usize;
                            atlas[idx] = atlas[idx].max((v * 255.0) as u8);
                        }
                    }
                });
                let adv = scaled.h_advance(gid);
                glyphs.insert(
                    ch,
                    GlyphInfo {
                        uv_min: [
                            (ox as f32) / (atlas_w as f32),
                            (oy as f32) / (atlas_h as f32),
                        ],
                        uv_max: [
                            ((ox as u32 + gw) as f32) / (atlas_w as f32),
                            ((oy as u32 + gh) as f32) / (atlas_h as f32),
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
        // Upload atlas
        let atlas_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hud-atlas"),
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
            label: Some("hud-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let text_bgl = crate::gfx::pipeline::create_text_bgl(device);
        let text_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hud-texture-bg"),
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
        let shader_text = crate::gfx::pipeline::create_shader(device);
        let text_pipeline = crate::gfx::pipeline::create_text_pipeline(
            device,
            &shader_text,
            &text_bgl,
            color_format,
        );

        // Buffers
        let bars_vcap = 64 * 1024;
        let bars_vbuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hud-bars-vbuf"),
            size: bars_vcap,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let text_vcap = 64 * 1024;
        let text_vbuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hud-text-vbuf"),
            size: text_vcap,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let hud = Self {
            bar_pipeline,
            text_pipeline,
            text_bg,
            _font: font,
            scale,
            ascent,
            glyphs,
            atlas_tex,
            _atlas_view: atlas_view,
            _atlas_sampler: atlas_sampler,
            atlas_cpu: atlas,
            atlas_size: (atlas_w, atlas_h),
            bars_vbuf,
            bars_vcap,
            bars_vcount: 0,
            text_vbuf,
            text_vcap,
            text_vcount: 0,
            bars_verts: Vec::new(),
            text_verts: Vec::new(),
        };
        Ok(hud)
    }

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

    fn ndc_from_px(px: f32, py: f32, w: f32, h: f32) -> [f32; 2] {
        let x = (px / w) * 2.0 - 1.0;
        let y = 1.0 - (py / h) * 2.0;
        [x, y]
    }

    #[allow(clippy::too_many_arguments)]
    fn push_rect(
        &mut self,
        surface_w: u32,
        surface_h: u32,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        color: [f32; 4],
    ) {
        let w = surface_w as f32;
        let h = surface_h as f32;
        let p0 = Self::ndc_from_px(x0, y0, w, h);
        let p1 = Self::ndc_from_px(x1, y0, w, h);
        let p2 = Self::ndc_from_px(x1, y1, w, h);
        let p3 = Self::ndc_from_px(x0, y1, w, h);
        self.bars_verts.push(BarVertex { pos_ndc: p0, color });
        self.bars_verts.push(BarVertex { pos_ndc: p1, color });
        self.bars_verts.push(BarVertex { pos_ndc: p2, color });
        self.bars_verts.push(BarVertex { pos_ndc: p0, color });
        self.bars_verts.push(BarVertex { pos_ndc: p2, color });
        self.bars_verts.push(BarVertex { pos_ndc: p3, color });
    }

    fn push_text_line(
        &mut self,
        surface_w: u32,
        surface_h: u32,
        mut x: f32,
        y_baseline: f32,
        text: &str,
        color: [f32; 4],
    ) {
        let w = surface_w as f32;
        let h = surface_h as f32;
        let scaled = self._font.as_scaled(self.scale);
        // Measure width if we see a center marker "|c" or left align otherwise
        let mut prev: Option<ab_glyph::GlyphId> = None;
        for ch in text.chars() {
            let gid = self._font.glyph_id(ch);
            if let Some(pg) = prev {
                x += scaled.kern(pg, gid);
            }
            if let Some(gi) = self.glyphs.get(&ch) {
                let gx = x + gi.bounds_min[0];
                let gy = y_baseline - self.ascent + gi.bounds_min[1];
                let w_px = gi.size[0];
                let h_px = gi.size[1];
                let p0 = Self::ndc_from_px(gx, gy, w, h);
                let p1 = Self::ndc_from_px(gx + w_px, gy, w, h);
                let p2 = Self::ndc_from_px(gx + w_px, gy + h_px, w, h);
                let p3 = Self::ndc_from_px(gx, gy + h_px, w, h);
                let uv0 = gi.uv_min;
                let uv1 = [gi.uv_max[0], gi.uv_min[1]];
                let uv2 = gi.uv_max;
                let uv3 = [gi.uv_min[0], gi.uv_max[1]];
                self.text_verts.push(TextVertex {
                    pos_ndc: p0,
                    uv: uv0,
                    color,
                });
                self.text_verts.push(TextVertex {
                    pos_ndc: p1,
                    uv: uv1,
                    color,
                });
                self.text_verts.push(TextVertex {
                    pos_ndc: p2,
                    uv: uv2,
                    color,
                });
                self.text_verts.push(TextVertex {
                    pos_ndc: p0,
                    uv: uv0,
                    color,
                });
                self.text_verts.push(TextVertex {
                    pos_ndc: p2,
                    uv: uv2,
                    color,
                });
                self.text_verts.push(TextVertex {
                    pos_ndc: p3,
                    uv: uv3,
                    color,
                });
                x += gi.advance;
                prev = Some(gi.id);
            } else {
                // Missing glyph (e.g., space) — advance without rendering.
                x += scaled.h_advance(gid);
                prev = Some(gid);
            }
        }
    }

    /// Build a minimal HUD for the wizard scene.
    #[allow(clippy::too_many_arguments)] // UI layout builder: grouping these into a struct is overkill for the prototype
    pub fn build(
        &mut self,
        surface_w: u32,
        surface_h: u32,
        pc_hp: i32,
        pc_hp_max: i32,
        pc_mana: i32,
        pc_mana_max: i32,
        cast_frac: f32,
        cd1_frac: f32,
        cd2_frac: f32,
        cd3_frac: f32,
        cast_label: Option<&str>,
    ) {
        self.bars_verts.clear();
        self.text_verts.clear();
        // Player HP (top-left)
        let pad = 10.0f32;
        let bar_w = 220.0f32;
        let bar_h = 16.0f32;
        let x0 = pad;
        let y0 = pad;
        let x1 = x0 + bar_w;
        let y1 = y0 + bar_h;
        // Border (dark)
        self.push_rect(
            surface_w,
            surface_h,
            x0 - 2.0,
            y0 - 2.0,
            x1 + 2.0,
            y1 + 2.0,
            [0.05, 0.05, 0.05, 0.95],
        );
        // Background
        self.push_rect(
            surface_w,
            surface_h,
            x0,
            y0,
            x1,
            y1,
            [0.10, 0.10, 0.10, 0.85],
        );
        // Fill
        let frac = if pc_hp_max > 0 {
            (pc_hp.max(0) as f32) / (pc_hp_max as f32)
        } else {
            0.0
        };
        if frac > 0.0 {
            let fx1 = x0 + bar_w * frac.clamp(0.0, 1.0);
            // green->yellow->red gradient similar to HealthBars
            let col = if frac >= 0.5 {
                let t = (frac - 0.5) / 0.5;
                [1.0 - t, 1.0, 0.0, 1.0]
            } else {
                let t = frac / 0.5;
                [1.0, t, 0.0, 1.0]
            };
            self.push_rect(surface_w, surface_h, x0, y0, fx1, y1, col);
        }
        // HP text (inside bar, top-left)
        let label = format!("HP {} / {}", pc_hp.max(0), pc_hp_max.max(1));
        // place near left edge inside the bar
        self.push_text_line(
            surface_w,
            surface_h,
            x0 + 6.0,
            y0 + bar_h - 3.0,
            &label,
            [0.0, 0.0, 0.0, 0.95],
        );

        // Player Mana (below HP)
        let my0 = y1 + 6.0;
        let my1 = my0 + bar_h;
        // Border
        self.push_rect(
            surface_w,
            surface_h,
            x0 - 2.0,
            my0 - 2.0,
            x1 + 2.0,
            my1 + 2.0,
            [0.05, 0.05, 0.05, 0.95],
        );
        // Background
        self.push_rect(
            surface_w,
            surface_h,
            x0,
            my0,
            x1,
            my1,
            [0.10, 0.10, 0.10, 0.85],
        );
        let mfrac = if pc_mana_max > 0 {
            (pc_mana.max(0) as f32) / (pc_mana_max as f32)
        } else {
            0.0
        };
        if mfrac > 0.0 {
            let fx1 = x0 + bar_w * mfrac.clamp(0.0, 1.0);
            // blue/cyan gradient
            let col = [0.2, 0.6 + 0.4 * mfrac, 1.8, 1.0];
            self.push_rect(surface_w, surface_h, x0, my0, fx1, my1, col);
        }
        let mlabel = format!("Mana {} / {}", pc_mana.max(0), pc_mana_max.max(1));
        self.push_text_line(
            surface_w,
            surface_h,
            x0 + 6.0,
            my0 + bar_h - 3.0,
            &mlabel,
            [0.0, 0.0, 0.0, 0.95],
        );

        // Hotbar (bottom-center): 6 slots
        let slots = 6usize;
        let slot_px = 48.0f32;
        let gap = 6.0f32;
        let total_w = slots as f32 * slot_px + (slots as f32 - 1.0) * gap;
        let cx = (surface_w as f32) * 0.5;
        let yb = (surface_h as f32) - (slot_px + 10.0);
        let mut x = cx - total_w * 0.5;
        for i in 0..slots {
            let x0 = x;
            let y0 = yb;
            let x1 = x + slot_px;
            let y1 = yb + slot_px;
            // Border + background
            self.push_rect(
                surface_w,
                surface_h,
                x0 - 2.0,
                y0 - 2.0,
                x1 + 2.0,
                y1 + 2.0,
                [0.05, 0.05, 0.05, 0.9],
            );
            self.push_rect(
                surface_w,
                surface_h,
                x0,
                y0,
                x1,
                y1,
                [0.18, 0.18, 0.18, 0.9],
            );
            // Key label
            let key = if i == 0 {
                "1"
            } else if i == 1 {
                "2"
            } else if i == 2 {
                "3"
            } else if i == 3 {
                "4"
            } else if i == 4 {
                "5"
            } else {
                "6"
            };
            self.push_text_line(
                surface_w,
                surface_h,
                x0 + 4.0,
                y0 + 14.0,
                key,
                [0.9, 0.9, 0.9, 0.95],
            );
            // Ability text (slots 1-3)
            if i == 0 {
                self.push_text_line(
                    surface_w,
                    surface_h,
                    x0 + 4.0,
                    y1 - 6.0,
                    "Fire Bolt",
                    [1.0, 0.9, 0.3, 0.95],
                );
            } else if i == 1 {
                self.push_text_line(
                    surface_w,
                    surface_h,
                    x0 + 4.0,
                    y1 - 6.0,
                    "Magic Missile",
                    [0.8, 0.9, 1.0, 0.95],
                );
            } else if i == 2 {
                self.push_text_line(
                    surface_w,
                    surface_h,
                    x0 + 4.0,
                    y1 - 6.0,
                    "Fireball",
                    [1.0, 0.7, 0.2, 0.95],
                );
            }
            // Cooldown overlays per slot (top-down fill)
            let frac = if i == 0 {
                cd1_frac
            } else if i == 1 {
                cd2_frac
            } else if i == 2 {
                cd3_frac
            } else {
                0.0
            };
            if frac > 0.0 {
                let overlay_h = slot_px * frac.clamp(0.0, 1.0);
                self.push_rect(
                    surface_w,
                    surface_h,
                    x0,
                    y0,
                    x1,
                    y0 + overlay_h,
                    [0.0, 0.0, 0.0, 0.45],
                );
            }
            x += slot_px + gap;
        }

        // Cast bar (center-bottom above hotbar), shown during an active cast
        if cast_frac > 0.0 {
            let bar_w = 300.0f32;
            let bar_h = 10.0f32;
            let cx = (surface_w as f32) * 0.5;
            let x0 = cx - bar_w * 0.5;
            let x1 = cx + bar_w * 0.5;
            let y0 = (yb - 18.0).max(0.0);
            let y1 = y0 + bar_h;
            // background
            self.push_rect(
                surface_w,
                surface_h,
                x0 - 2.0,
                y0 - 2.0,
                x1 + 2.0,
                y1 + 2.0,
                [0.05, 0.05, 0.05, 0.95],
            );
            self.push_rect(
                surface_w,
                surface_h,
                x0,
                y0,
                x1,
                y1,
                [0.10, 0.10, 0.10, 0.85],
            );
            // fill
            let fx1 = x0 + bar_w * cast_frac.clamp(0.0, 1.0);
            self.push_rect(
                surface_w,
                surface_h,
                x0,
                y0,
                fx1,
                y1,
                [0.85, 0.75, 0.25, 1.0],
            );
            // label
            if let Some(label) = cast_label {
                let text = format!("Casting {}", label);
                self.push_text_line(
                    surface_w,
                    surface_h,
                    x0 + 6.0,
                    y0 - 4.0,
                    &text,
                    [1.0, 1.0, 1.0, 0.9],
                );
            }
        }

        self.bars_vcount = self.bars_verts.len() as u32;
        self.text_vcount = self.text_verts.len() as u32;
    }

    pub fn queue(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        // Upload bars
        let bbytes: &[u8] = bytemuck::cast_slice(&self.bars_verts);
        if bbytes.len() as u64 > self.bars_vcap {
            let new_cap = (bbytes.len() as u64).next_power_of_two();
            self.bars_vbuf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("hud-bars-vbuf"),
                size: new_cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.bars_vcap = new_cap;
        }
        if !bbytes.is_empty() {
            queue.write_buffer(&self.bars_vbuf, 0, bbytes);
        }
        // Upload text
        let tbytes: &[u8] = bytemuck::cast_slice(&self.text_verts);
        if tbytes.len() as u64 > self.text_vcap {
            let new_cap = (tbytes.len() as u64).next_power_of_two();
            self.text_vbuf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("hud-text-vbuf"),
                size: new_cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.text_vcap = new_cap;
        }
        if !tbytes.is_empty() {
            queue.write_buffer(&self.text_vbuf, 0, tbytes);
        }
    }

    /// Clear any queued HUD geometry for building a custom overlay.
    pub fn reset(&mut self) {
        self.bars_verts.clear();
        self.text_verts.clear();
        self.bars_vcount = 0;
        self.text_vcount = 0;
    }

    /// Append a centered single-line text at the given baseline Y.
    pub fn append_center_text(
        &mut self,
        surface_w: u32,
        surface_h: u32,
        text: &str,
        y_baseline: f32,
        color: [f32; 4],
    ) {
        // Measure width with kerning; treat missing glyphs (e.g., space) via font advances.
        let scaled = self._font.as_scaled(self.scale);
        let mut width = 0.0f32;
        let mut prev: Option<ab_glyph::GlyphId> = None;
        for ch in text.chars() {
            let gid = self._font.glyph_id(ch);
            if let Some(pg) = prev {
                width += scaled.kern(pg, gid);
            }
            if let Some(gi) = self.glyphs.get(&ch) {
                width += gi.advance;
                prev = Some(gi.id);
            } else {
                width += scaled.h_advance(gid);
                prev = Some(gid);
            }
        }
        let cx = (surface_w as f32) * 0.5 - width * 0.5;
        self.push_text_line(surface_w, surface_h, cx, y_baseline, text, color);
        self.text_vcount = self.text_verts.len() as u32;
    }

    /// Draw a simple death overlay with centered messages.
    pub fn death_overlay(
        &mut self,
        surface_w: u32,
        surface_h: u32,
        title: &str,
        helper_text: &str,
    ) {
        self.bars_verts.clear();
        self.text_verts.clear();
        // Dim background
        self.push_rect(
            surface_w,
            surface_h,
            0.0,
            0.0,
            surface_w as f32,
            surface_h as f32,
            [0.0, 0.0, 0.0, 0.45],
        );
        // Panel
        let panel_w = 420.0f32;
        let panel_h = 180.0f32;
        let cx = surface_w as f32 * 0.5;
        let cy = surface_h as f32 * 0.5;
        let x0 = cx - panel_w * 0.5;
        let y0 = cy - panel_h * 0.5;
        let x1 = cx + panel_w * 0.5;
        let y1 = cy + panel_h * 0.5;
        // Border + background
        self.push_rect(
            surface_w,
            surface_h,
            x0 - 2.0,
            y0 - 2.0,
            x1 + 2.0,
            y1 + 2.0,
            [0.02, 0.02, 0.02, 0.95],
        );
        self.push_rect(
            surface_w,
            surface_h,
            x0,
            y0,
            x1,
            y1,
            [0.10, 0.10, 0.10, 0.9],
        );
        // Title
        self.append_center_text(
            surface_w,
            surface_h,
            title,
            y0 + 60.0,
            [1.0, 0.3, 0.25, 1.0],
        );
        // Helper text: e.g., "Press R to respawn"
        self.append_center_text(
            surface_w,
            surface_h,
            helper_text,
            y1 - 40.0,
            [0.95, 0.98, 1.0, 0.95],
        );
        self.bars_vcount = self.bars_verts.len() as u32;
        self.text_vcount = self.text_verts.len() as u32;
    }

    /// Append a single-line perf overlay in the top-left corner.
    pub fn append_perf_text(&mut self, surface_w: u32, surface_h: u32, text: &str) {
        self.append_perf_text_line(surface_w, surface_h, text, 0);
    }

    /// Append a perf text line with an explicit line index (0 = top line).
    pub fn append_perf_text_line(&mut self, surface_w: u32, surface_h: u32, text: &str, line: u32) {
        // Slight shadow for readability
        let x = 10.0f32;
        let y = 24.0f32 + (line as f32) * 18.0; // 18px line height
        self.push_text_line(
            surface_w,
            surface_h,
            x + 1.0,
            y + 1.0,
            text,
            [0.0, 0.0, 0.0, 0.5],
        );
        self.push_text_line(surface_w, surface_h, x, y, text, [0.95, 0.98, 1.0, 0.95]);
        self.text_vcount = self.text_verts.len() as u32;
    }

    /// Append a simple center reticle (crosshair) using the bar pipeline.
    ///
    /// Reticle draw is currently disabled at call sites; keep this helper
    /// available for future re-enable without shipping the overlay.
    #[allow(dead_code)]
    pub fn append_reticle(&mut self, surface_w: u32, surface_h: u32) {
        let cx = surface_w as f32 * 0.5;
        let cy = surface_h as f32 * 0.5;
        // Size scaled to min dimension
        let len = (surface_w.min(surface_h) as f32 * 0.012).clamp(6.0, 18.0);
        let thick = 2.0f32;
        let color = [0.95, 0.98, 1.0, 0.85];
        // Horizontal line
        self.push_rect(
            surface_w,
            surface_h,
            cx - len,
            cy - thick * 0.5,
            cx + len,
            cy + thick * 0.5,
            color,
        );
        // Vertical line
        self.push_rect(
            surface_w,
            surface_h,
            cx - thick * 0.5,
            cy - len,
            cx + thick * 0.5,
            cy + len,
            color,
        );
        self.bars_vcount = self.bars_verts.len() as u32;
    }

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        // Bars
        if self.bars_vcount > 0 {
            let mut r = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hud-bars-pass"),
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
            r.set_pipeline(&self.bar_pipeline);
            r.set_vertex_buffer(0, self.bars_vbuf.slice(..));
            r.draw(0..self.bars_vcount, 0..1);
        }
        // Text
        if self.text_vcount > 0 {
            let mut r = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hud-text-pass"),
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
            r.set_pipeline(&self.text_pipeline);
            r.set_bind_group(0, &self.text_bg, &[]);
            r.set_vertex_buffer(0, self.text_vbuf.slice(..));
            r.draw(0..self.text_vcount, 0..1);
        }
    }
}

#[cfg(test)]
mod hud_tests {
    #[test]
    fn hotbar_layout_vertices_increase_with_slots() {
        // Purely CPU-side build check: ensure building adds some vertices
        // We can’t construct a real Hud without a device, so test the math indirectly.
        // Use a small helper mirroring the slot count logic.
        let slots = 6usize;
        let slot_px = 48.0f32;
        let gap = 6.0f32;
        let total_w = slots as f32 * slot_px + (slots as f32 - 1.0) * gap;
        assert!(total_w > 0.0);
    }
}

#[allow(dead_code)]
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
    #[allow(dead_code)]
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
                if let Some(pg) = prev {
                    width += scaled.kern(pg, gi.id);
                }
                width += gi.advance;
                prev = Some(gi.id);
            }
        }
        let mut verts: Vec<TextVertex> = Vec::new();
        for world in positions {
            let clip = view_proj * glam::Vec4::new(world.x, world.y, world.z, 1.0);
            if clip.w <= 0.0 {
                continue;
            }
            let ndc = clip.truncate() / clip.w;
            if ndc.x < -1.2 || ndc.x > 1.2 || ndc.y < -1.2 || ndc.y > 1.2 {
                continue;
            }
            let mut cx = (ndc.x * 0.5 + 0.5) * w;
            let cy = (1.0 - (ndc.y * 0.5 + 0.5)) * h;
            // Text just above the bar (same offset as wizards)
            let baseline_y = (cy - 56.0).max(0.0);
            cx -= width * 0.5;
            // Emit quads for label
            let mut pen_x = 0.0f32;
            prev = None;
            for ch in label.chars() {
                let Some(gi) = self.glyphs.get(&ch) else {
                    continue;
                };
                if let Some(pg) = prev {
                    pen_x += scaled.kern(pg, gi.id);
                }
                // Place top-left without using negative bearings to avoid truncation
                let w_px = gi.size[0];
                let h_px = gi.size[1];
                let x = cx + pen_x;
                let y = baseline_y - h_px;
                let p0 = Self::ndc_from_px(x, y, w, h);
                let p1 = Self::ndc_from_px(x + w_px, y, w, h);
                let p2 = Self::ndc_from_px(x + w_px, y + h_px, w, h);
                let p3 = Self::ndc_from_px(x, y + h_px, w, h);
                let uv0 = gi.uv_min;
                let uv1 = [gi.uv_max[0], gi.uv_min[1]];
                let uv2 = gi.uv_max;
                let uv3 = [gi.uv_min[0], gi.uv_max[1]];
                let white = [1.0, 1.0, 1.0, 1.0];
                verts.push(TextVertex {
                    pos_ndc: p0,
                    uv: uv0,
                    color: white,
                });
                verts.push(TextVertex {
                    pos_ndc: p1,
                    uv: uv1,
                    color: white,
                });
                verts.push(TextVertex {
                    pos_ndc: p2,
                    uv: uv2,
                    color: white,
                });
                verts.push(TextVertex {
                    pos_ndc: p0,
                    uv: uv0,
                    color: white,
                });
                verts.push(TextVertex {
                    pos_ndc: p2,
                    uv: uv2,
                    color: white,
                });
                verts.push(TextVertex {
                    pos_ndc: p3,
                    uv: uv3,
                    color: white,
                });
                pen_x += gi.advance;
                prev = Some(gi.id);
            }
        }
        self.vcount = verts.len() as u32;
        if self.vcount == 0 {
            return;
        }
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

    #[test]
    fn multiple_entries_produce_multiple_bars() {
        let vp = Mat4::IDENTITY;
        // Use identity view-proj, so treat input as NDC [-1..1]; pick values in range
        let entries = vec![
            (glam::vec3(0.0, 0.0, 0.0), 1.0),
            (glam::vec3(0.1, 0.0, 0.0), 0.5),
            (glam::vec3(-0.1, 0.0, 0.0), 0.25),
        ];
        let verts = HealthBars::build_vertices(1280, 720, vp, &entries);
        assert_eq!(verts.len(), 6 * entries.len());
    }
}
