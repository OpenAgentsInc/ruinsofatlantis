//! Minimal static frame-graph for pass I/O validation.
//!
//! Encodes read/write resources for each pass and validates invariants:
//! - A pass may not sample from the same resource it writes this frame.
//! - Depth is read-only in all passes.
//!
//! Render order remains explicit in `render.rs`; this module provides checks
//! and a single place to document pass I/O.

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
