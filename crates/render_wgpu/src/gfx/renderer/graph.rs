//! Minimal frame‑graph for pass I/O validation and execution.
//!
//! Encodes read/write resources for each pass and validates invariants:
//! - A pass may not sample from the same resource it writes this frame.
//! - Depth is read-only in all passes.
//!
//! This module also hosts a builder that records execution closures
//! for passes; `Graph::execute` runs them in declared order.

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Res {
    SceneColor,
    SceneRead,
    Depth,
}

#[derive(Clone, Debug)]
pub struct PassSpec {
    pub label: &'static str,
    pub reads: &'static [Res],
    pub writes: &'static [Res],
}

#[derive(Default)]
pub struct FrameGraph {
    passes: Vec<PassSpec>,
}

impl FrameGraph {
    pub fn new() -> Self {
        Self { passes: Vec::new() }
    }
    pub fn add(mut self, p: PassSpec) -> Self {
        self.passes.push(p);
        self
    }
    pub fn validate(&self) {
        for p in &self.passes {
            for w in p.writes {
                if *w == Res::Depth {
                    // Depth is write-only in main pass in this prototype; no pass should write it here
                    // Keep permissive: just forbid read+write collisions
                }
                if p.reads.iter().any(|r| r == w) {
                    panic!(
                        "frame-graph violation in {}: reads and writes {:?}",
                        p.label, w
                    );
                }
            }
        }
    }

    /// Forwarder skeleton: execute the current render function unchanged.
    /// The encoder/view parameters are accepted for future pass routing.
    #[allow(unused_variables, dead_code)]
    pub fn run(
        renderer: &mut crate::gfx::Renderer,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    ) {
        // Forward to existing render path (no reordering yet)
        let _ = super::render::render_impl(renderer, None);
    }
}

// Static pass specs for the prototype
pub fn graph_for(
    enabled_ssgi: bool,
    enabled_ssr: bool,
    enabled_bloom: bool,
    direct_present: bool,
) -> FrameGraph {
    let mut g = FrameGraph::new()
        // Sky: writes SceneColor
        .add(PassSpec {
            label: "sky",
            reads: &[],
            writes: &[Res::SceneColor],
        })
        // Main: reads Depth, writes SceneColor
        .add(PassSpec {
            label: "main",
            reads: &[Res::Depth],
            writes: &[Res::SceneColor],
        });
    if !direct_present {
        // Blit SceneColor -> SceneRead for post passes that sample color
        g = g.add(PassSpec {
            label: "blit_scene_to_read",
            reads: &[Res::SceneColor],
            writes: &[Res::SceneRead],
        });
    }
    if enabled_ssr {
        // SSR: reads linear depth + SceneRead, writes SceneColor
        g = g.add(PassSpec {
            label: "ssr",
            reads: &[Res::Depth, Res::SceneRead],
            writes: &[Res::SceneColor],
        });
    }
    if enabled_ssgi {
        // SSGI: reads depth + SceneRead, writes SceneColor (additive)
        g = g.add(PassSpec {
            label: "ssgi",
            reads: &[Res::Depth, Res::SceneRead],
            writes: &[Res::SceneColor],
        });
    }
    // Post AO: reads depth, writes SceneColor
    g = g.add(PassSpec {
        label: "post_ao",
        reads: &[Res::Depth],
        writes: &[Res::SceneColor],
    });
    if enabled_bloom {
        // Bloom: reads SceneRead, writes SceneColor
        g = g.add(PassSpec {
            label: "bloom",
            reads: &[Res::SceneRead],
            writes: &[Res::SceneColor],
        });
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn graph_invariants_hold() {
        let g = graph_for(true, true, true, false);
        g.validate();
    }
}

// ---------------------------------------------------------------------------
// Phase-2 Framegraph core (builder, resources, validation)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub type Size2D = glam::UVec2;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum ImageKind {
    Color {
        format: wgpu::TextureFormat,
        size: Size2D,
        msaa: u32,
    },
    Depth {
        format: wgpu::TextureFormat,
        size: Size2D,
        msaa: u32,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Handle<T>(u32, std::marker::PhantomData<T>);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum Access {
    Read,
    Write,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub struct Img;

#[allow(dead_code)]
pub struct GraphBuilder {
    images: Vec<ImageKind>,
    passes: Vec<PassDecl>,
}

#[allow(dead_code)]
pub struct PassDecl {
    name: &'static str,
    reads: Vec<Handle<Img>>,
    writes: Vec<Handle<Img>>,
    exec: Box<dyn Fn(&mut ExecCtx) + Send + Sync>,
}

#[allow(dead_code)]
pub struct ExecCtx<'a> {
    pub renderer: &'a mut crate::gfx::Renderer,
    pub encoder: &'a mut wgpu::CommandEncoder,
    // Graph-provided views for declared image handles. The index matches Handle<Img>.0
    views: &'a [wgpu::TextureView],
}

impl<'a> ExecCtx<'a> {
    #[allow(dead_code)]
    #[inline]
    pub fn attachments(&mut self) -> &mut super::attachments::Attachments {
        &mut self.renderer.attachments
    }
    #[inline]
    pub fn view_color(&self, h: Handle<Img>) -> &wgpu::TextureView {
        &self.views[h.0 as usize]
    }
    #[inline]
    #[allow(dead_code)]
    pub fn view_depth(&self, h: Handle<Img>) -> &wgpu::TextureView {
        &self.views[h.0 as usize]
    }
    // Minimal accessors to avoid reaching into renderer directly from passes
    #[allow(dead_code)]
    #[inline]
    pub fn device(&self) -> &wgpu::Device {
        &self.renderer.device
    }
    #[allow(dead_code)]
    #[inline]
    pub fn queue(&self) -> &wgpu::Queue {
        &self.renderer.queue
    }
    #[allow(dead_code)]
    #[inline]
    pub fn surface_config(&self) -> &wgpu::SurfaceConfiguration {
        &self.renderer.config
    }
    #[inline]
    #[allow(dead_code)]
    pub fn pipelines(&self) -> &super::pipelines::Pipelines {
        // Typed pipelines accessor (backed by Renderer field). No behavior change yet.
        &self.renderer.pipelines
    }
    // Placeholder for future pipelines() accessor when adopted in ExecCtx
}

#[allow(dead_code)]
pub struct Graph {
    pub names: Vec<&'static str>,
    // Declared images (by handle id). Used to derive per-frame views mapping.
    images: Vec<ImageKind>,
    // Keep created textures alive for the duration of execute (allocation path)
    keep_textures: Vec<wgpu::Texture>,
    passes: Vec<PassDecl>,
}

impl GraphBuilder {
    pub fn new() -> Self {
        Self {
            images: Vec::new(),
            passes: Vec::new(),
        }
    }

    pub fn image(&mut self, kind: ImageKind) -> Handle<Img> {
        let id = self.images.len() as u32;
        self.images.push(kind);
        Handle(id, std::marker::PhantomData)
    }

    pub fn pass<F>(&mut self, name: &'static str, f: F) -> &mut PassDecl
    where
        F: Fn(&mut ExecCtx) + Send + Sync + 'static,
    {
        self.passes.push(PassDecl {
            name,
            reads: Vec::new(),
            writes: Vec::new(),
            exec: Box::new(f),
        });
        self.passes.last_mut().unwrap()
    }
}

#[allow(dead_code)]
impl PassDecl {
    pub fn reads(&mut self, h: Handle<Img>) -> &mut Self {
        self.reads.push(h);
        self
    }
    pub fn writes(&mut self, h: Handle<Img>) -> &mut Self {
        self.writes.push(h);
        self
    }
}

impl Graph {
    pub fn compile(mut b: GraphBuilder) -> Self {
        // Validate per-pass hazards: a pass may not both read and write the same image.
        for p in &b.passes {
            for w in &p.writes {
                if p.reads.iter().any(|r| r == w) {
                    panic!("framegraph hazard in '{}': read+write same image", p.name);
                }
            }
        }
        // Cross-pass monotonic rule: no write after any read of the same image.
        #[cfg(debug_assertions)]
        {
            use std::collections::HashSet;
            let mut seen_read: HashSet<u32> = HashSet::new();
            for p in &b.passes {
                for &w in &p.writes {
                    if seen_read.contains(&w.0) {
                        panic!(
                            "framegraph hazard: pass '{}' writes an image after it was read earlier",
                            p.name
                        );
                    }
                }
                for &r in &p.reads {
                    seen_read.insert(r.0);
                }
            }
        }
        // Preserve declaration order for now (topology is trivial without cross-pass deps).
        let names = b.passes.iter().map(|p| p.name).collect();
        Graph {
            names,
            images: b.images,
            keep_textures: Vec::new(),
            passes: b.passes.drain(..).collect(),
        }
    }

    pub fn execute(
        mut self,
        renderer: &mut crate::gfx::Renderer,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        // Behavior-neutral default: alias declared image handles to the current attachments.
        // Optionally allocate real textures per handle if RA_GRAPH_ALLOC=1 is set.
        let do_alloc = std::env::var("RA_GRAPH_ALLOC")
            .map(|v| v == "1")
            .unwrap_or(false);
        let mut views: Vec<wgpu::TextureView> = Vec::with_capacity(self.images.len());
        views.resize_with(self.images.len(), || {
            renderer.attachments.scene_view.clone()
        });

        if do_alloc {
            // Compute simple liveness and usages per image, then instantiate textures.
            #[derive(Clone, Copy, Default)]
            struct Live {
                first: usize,
                last: usize,
                init: bool,
            }
            let mut lives: Vec<Live> = vec![Live::default(); self.images.len()];
            let mut usages: Vec<wgpu::TextureUsages> =
                vec![wgpu::TextureUsages::empty(); self.images.len()];
            for (pi, p) in self.passes.iter().enumerate() {
                for &r in &p.reads {
                    let i = r.0 as usize;
                    let l = &mut lives[i];
                    if !l.init {
                        l.first = pi;
                        l.init = true;
                    }
                    l.last = pi.max(l.last);
                    usages[i] |= wgpu::TextureUsages::TEXTURE_BINDING;
                }
                for &w in &p.writes {
                    let i = w.0 as usize;
                    let l = &mut lives[i];
                    if !l.init {
                        l.first = pi;
                        l.init = true;
                    }
                    l.last = pi.max(l.last);
                    usages[i] |= wgpu::TextureUsages::RENDER_ATTACHMENT;
                }
            }
            // Optional aliasing of non-overlapping images behind RA_GRAPH_ALIASING=1
            let do_alias = std::env::var("RA_GRAPH_ALIASING")
                .map(|v| v == "1")
                .unwrap_or(false);
            if do_alias {
                struct PoolEntry {
                    kind: ImageKind,
                    usage: wgpu::TextureUsages,
                    live: Live,
                    _tex: wgpu::Texture,
                    view: wgpu::TextureView,
                }
                let mut pool: Vec<PoolEntry> = Vec::new();
                for (ix, kind) in self.images.iter().enumerate() {
                    // Try to find a reusable texture in the pool
                    let mut reused = false;
                    for e in &mut pool {
                        // Only alias exactly matching descriptors (format/size/msaa/usage)
                        if std::mem::discriminant(&e.kind) == std::mem::discriminant(kind) {
                            let same = match (&e.kind, kind) {
                                (
                                    ImageKind::Color {
                                        format: f0,
                                        size: s0,
                                        msaa: m0,
                                    },
                                    ImageKind::Color {
                                        format: f1,
                                        size: s1,
                                        msaa: m1,
                                    },
                                ) => f0 == f1 && s0 == s1 && m0 == m1,
                                (
                                    ImageKind::Depth {
                                        format: f0,
                                        size: s0,
                                        msaa: m0,
                                    },
                                    ImageKind::Depth {
                                        format: f1,
                                        size: s1,
                                        msaa: m1,
                                    },
                                ) => f0 == f1 && s0 == s1 && m0 == m1,
                                _ => false,
                            } && e.usage.contains(usages[ix]);
                            if same && e.live.last < lives[ix].first {
                                // Disjoint lifetimes; reuse
                                e.live = lives[ix];
                                self.keep_textures.push(e._tex.clone());
                                views[ix] = e.view.clone();
                                reused = true;
                                break;
                            }
                        }
                    }
                    if !reused {
                        // Create a new texture
                        match kind {
                            ImageKind::Color { format, size, msaa } => {
                                let tex =
                                    renderer.device.create_texture(&wgpu::TextureDescriptor {
                                        label: Some("graph-color"),
                                        size: wgpu::Extent3d {
                                            width: size.x,
                                            height: size.y,
                                            depth_or_array_layers: 1,
                                        },
                                        mip_level_count: 1,
                                        sample_count: *msaa,
                                        dimension: wgpu::TextureDimension::D2,
                                        format: *format,
                                        usage: usages[ix],
                                        view_formats: &[],
                                    });
                                let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                                self.keep_textures.push(tex.clone());
                                views[ix] = view.clone();
                                pool.push(PoolEntry {
                                    kind: kind.clone(),
                                    usage: usages[ix],
                                    live: lives[ix],
                                    _tex: tex,
                                    view,
                                });
                            }
                            ImageKind::Depth { format, size, msaa } => {
                                let tex =
                                    renderer.device.create_texture(&wgpu::TextureDescriptor {
                                        label: Some("graph-depth"),
                                        size: wgpu::Extent3d {
                                            width: size.x,
                                            height: size.y,
                                            depth_or_array_layers: 1,
                                        },
                                        mip_level_count: 1,
                                        sample_count: *msaa,
                                        dimension: wgpu::TextureDimension::D2,
                                        format: *format,
                                        usage: usages[ix],
                                        view_formats: &[],
                                    });
                                let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                                self.keep_textures.push(tex.clone());
                                views[ix] = view.clone();
                                pool.push(PoolEntry {
                                    kind: kind.clone(),
                                    usage: usages[ix],
                                    live: lives[ix],
                                    _tex: tex,
                                    view,
                                });
                            }
                        }
                    }
                }
            } else {
                // Instantiate textures per descriptor (no aliasing)
                for (ix, kind) in self.images.iter().enumerate() {
                    match kind {
                        ImageKind::Color { format, size, msaa } => {
                            let tex = renderer.device.create_texture(&wgpu::TextureDescriptor {
                                label: Some("graph-color"),
                                size: wgpu::Extent3d {
                                    width: size.x,
                                    height: size.y,
                                    depth_or_array_layers: 1,
                                },
                                mip_level_count: 1,
                                sample_count: *msaa,
                                dimension: wgpu::TextureDimension::D2,
                                format: *format,
                                usage: usages[ix],
                                view_formats: &[],
                            });
                            self.keep_textures.push(tex.clone());
                            views[ix] = tex.create_view(&wgpu::TextureViewDescriptor::default());
                        }
                        ImageKind::Depth { format, size, msaa } => {
                            let tex = renderer.device.create_texture(&wgpu::TextureDescriptor {
                                label: Some("graph-depth"),
                                size: wgpu::Extent3d {
                                    width: size.x,
                                    height: size.y,
                                    depth_or_array_layers: 1,
                                },
                                mip_level_count: 1,
                                sample_count: *msaa,
                                dimension: wgpu::TextureDimension::D2,
                                format: *format,
                                usage: usages[ix],
                                view_formats: &[],
                            });
                            self.keep_textures.push(tex.clone());
                            views[ix] = tex.create_view(&wgpu::TextureViewDescriptor::default());
                        }
                    }
                }
            }
        } else {
            // Aliasing path (default): route all images to current attachments
            for (ix, kind) in self.images.iter().enumerate() {
                match kind {
                    ImageKind::Color { .. } => {
                        views[ix] = renderer.attachments.scene_view.clone();
                    }
                    ImageKind::Depth { .. } => {
                        views[ix] = renderer.attachments.depth_view.clone();
                    }
                }
            }
        }
        for p in self.passes.drain(..) {
            let mut ctx = ExecCtx {
                renderer,
                encoder,
                views: &views,
            };
            (p.exec)(&mut ctx);
        }
    }
}

impl FrameGraph {
    /// Build and run a single Monolith pass that forwards to the existing render path.
    #[allow(unused_variables, dead_code)]
    pub fn run_forwarder(
        renderer: &mut crate::gfx::Renderer,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    ) {
        let mut builder = GraphBuilder::new();
        // Declare one dummy image to exercise the API; values unused by Monolith.
        let _color = builder.image(ImageKind::Color {
            format: wgpu::TextureFormat::Rgba16Float,
            size: glam::uvec2(1, 1),
            msaa: 1,
        });
        // Monolith pass: call the legacy render implementation
        builder.pass("Monolith", |_ctx| { /* forwarder; see execute() */ });
        let g = Graph::compile(builder);
        // Forward to legacy path to keep behavior parity
        let _ = super::render::render_impl(renderer, None);
    }

    // helper removed; Particles/UI now execute via pass closures
}

#[cfg(test)]
mod builder_tests {
    use super::*;

    #[test]
    #[should_panic]
    fn detects_read_write_hazard_in_pass() {
        let mut b = GraphBuilder::new();
        let img = b.image(ImageKind::Color {
            format: wgpu::TextureFormat::Rgba8Unorm,
            size: glam::uvec2(16, 16),
            msaa: 1,
        });
        b.pass("bad", |_ctx| {}).reads(img).writes(img);
        let _ = Graph::compile(b);
    }

    #[test]
    fn preserves_pass_declaration_order() {
        let mut b = GraphBuilder::new();
        b.pass("A", |_ctx| {});
        b.pass("B", |_ctx| {});
        let g = Graph::compile(b);
        assert_eq!(g.names, vec!["A", "B"]);
    }

    #[test]
    #[should_panic]
    fn panics_on_write_after_read() {
        let mut b = GraphBuilder::new();
        let img = b.image(ImageKind::Color {
            format: wgpu::TextureFormat::Rgba8Unorm,
            size: glam::uvec2(16, 16),
            msaa: 1,
        });
        b.pass("writer", |_ctx| {}).writes(img);
        b.pass("reader", |_ctx| {}).reads(img);
        // Illegal: write again after a read
        b.pass("writer2", |_ctx| {}).writes(img);
        let _ = Graph::compile(b);
    }
}
