//! UI overlay: minimal nameplates rendered as 2D text above wizards.
//!
//! This module implements a tiny CPU text atlas using `ab_glyph` and a simple
//! textured-quad pipeline to draw labels in screen space. It avoids pulling in
//! heavier text renderers to stay compatible with our wgpu version.

use std::collections::HashMap;

use ab_glyph::{Font, FontArc, Glyph, PxScale, ScaleFont};

use crate::gfx::pipeline;
use crate::gfx::types::TextVertex;

struct GlyphInfo {
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    bounds_min: [f32; 2], // px_bounds().min relative to baseline position (0, ascent)
    size: [f32; 2],       // width/height in pixels
    advance: f32,         // advance width in pixels
    id: ab_glyph::GlyphId,
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
            let g0 = Glyph { id: gid, scale, position: ab_glyph::point(0.0, ascent) };
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
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&atlas_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&atlas_sampler) },
            ],
        });
        let shader = crate::gfx::pipeline::create_shader(device);
        let text_pipeline = pipeline::create_text_pipeline(device, &shader, &text_bgl, color_format);

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
            vcap_bytes: vcap_bytes,
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
            "Ten", "Eleven", "Twelve", "Thirteen", "Fourteen", "Fifteen", "Sixteen", "Seventeen", "Eighteen", "Nineteen",
        ];
        let tens = ["", "", "Twenty", "Thirty", "Forty", "Fifty", "Sixty", "Seventy", "Eighty", "Ninety"];
        if n < 10 { return ones[n].to_string(); }
        if n < 20 { return teens[n - 10].to_string(); }
        if n < 100 {
            let t = n / 10; let o = n % 10;
            if o == 0 { tens[t].to_string() } else { format!("{}{}", tens[t], ones[o]) }
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
            if clip.w <= 0.0 { continue; }
            let ndc = clip.truncate() / clip.w;
            if ndc.x < -1.2 || ndc.x > 1.2 || ndc.y < -1.2 || ndc.y > 1.2 { continue; }
            let mut cx = (ndc.x * 0.5 + 0.5) * w;
            let cy = (1.0 - (ndc.y * 0.5 + 0.5)) * h;
            // baseline a bit above the head (slightly lower than before)
            let baseline_y = (cy - 26.0).max(0.0);

            // Measure label width
            let text = &labels[i];
            let mut prev: Option<ab_glyph::GlyphId> = None;
            let mut width = 0.0f32;
            for ch in text.chars() {
                let gi = match self.glyphs.get(&ch) { Some(g) => g, None => continue };
                if let Some(pg) = prev { width += scaled.kern(pg, gi.id); }
                width += gi.advance;
                prev = Some(gi.id);
            }
            cx -= width * 0.5; // center horizontally

            // Emit quads
            let mut pen_x = 0.0f32;
            prev = None;
            for ch in text.chars() {
                let gi = match self.glyphs.get(&ch) { Some(g) => g, None => continue };
                if let Some(pg) = prev { pen_x += scaled.kern(pg, gi.id); }

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

                verts.push(TextVertex { pos_ndc: p0, uv: uv0 });
                verts.push(TextVertex { pos_ndc: p1, uv: uv1 });
                verts.push(TextVertex { pos_ndc: p2, uv: uv2 });
                verts.push(TextVertex { pos_ndc: p0, uv: uv0 });
                verts.push(TextVertex { pos_ndc: p2, uv: uv2 });
                verts.push(TextVertex { pos_ndc: p3, uv: uv3 });

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
        if self.vcount == 0 { return; }
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("nameplate-pass"),
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
        rpass.set_pipeline(&self.text_pipeline);
        rpass.set_bind_group(0, &self.text_bg, &[]);
        rpass.set_vertex_buffer(0, self.vbuf.slice(..));
        rpass.draw(0..self.vcount, 0..1);
    }
}
impl Nameplates {
    pub fn upload_atlas(&self, queue: &wgpu::Queue) {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &self.atlas_tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &self.atlas_cpu,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(self.atlas_size.0), rows_per_image: Some(self.atlas_size.1) },
            wgpu::Extent3d { width: self.atlas_size.0, height: self.atlas_size.1, depth_or_array_layers: 1 },
        );
    }
}
