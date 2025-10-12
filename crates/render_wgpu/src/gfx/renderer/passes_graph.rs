//! Framegraph pass declarations (scaffold for phase two).
//!
//! These declare Sky, Main, and Present passes against the new GraphBuilder
//! API without changing behavior yet. Execution remains the legacy path
//! until passes are wired to run.

#![allow(dead_code)]

use super::graph::{GraphBuilder, Handle, Img};

pub struct SkyPass;
impl SkyPass {
    pub fn declare(builder: &mut GraphBuilder, color: Handle<Img>) {
        let _ = builder.pass("Sky", |_ctx| {}).writes(color);
    }
}

pub struct MainPass;
impl MainPass {
    pub fn declare(builder: &mut GraphBuilder, color: Handle<Img>, depth: Handle<Img>) {
        let _ = builder.pass("Main", |_ctx| {}).writes(color).writes(depth);
    }
}

pub struct PresentPass;
impl PresentPass {
    pub fn declare(builder: &mut GraphBuilder /*, backbuffer: Handle<Img>*/) {
        let _ = builder.pass("Present", |_ctx| {});
    }
}

pub struct ParticlesPass;
impl ParticlesPass {
    pub fn declare(builder: &mut GraphBuilder, color: Handle<Img>, depth: Handle<Img>) {
        let _ = builder
            .pass("Particles", |_ctx| {})
            .writes(color)
            .writes(depth);
    }
}

pub struct UiPass;
impl UiPass {
    pub fn declare(builder: &mut GraphBuilder, color: Handle<Img>) {
        let _ = builder.pass("UI", |_ctx| {}).writes(color);
    }
}
