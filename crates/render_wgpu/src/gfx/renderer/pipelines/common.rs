//! Typed wrappers for bind group layouts and render pipelines.

use std::marker::PhantomData;

#[derive(Clone)]
#[allow(dead_code)]
pub struct Bgl<T> {
    pub raw: wgpu::BindGroupLayout,
    _t: PhantomData<T>,
}

#[allow(dead_code)]
pub struct Pipeline<T> {
    pub raw: wgpu::RenderPipeline,
    _t: PhantomData<T>,
}

impl<T> std::ops::Deref for Bgl<T> {
    type Target = wgpu::BindGroupLayout;
    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl<T> std::ops::Deref for Pipeline<T> {
    type Target = wgpu::RenderPipeline;
    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

// Tag types for typed pipelines
#[allow(dead_code)]
pub enum TerrainPass {}
#[allow(dead_code)]
pub enum InstancedPass {}
#[allow(dead_code)]
pub enum SkyPass {}
#[allow(dead_code)]
pub enum AoPass {}
#[allow(dead_code)]
pub enum SsgiPass {}
#[allow(dead_code)]
pub enum SsrPass {}
#[allow(dead_code)]
pub enum BloomPass {}
