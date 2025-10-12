//! Typed wrappers for bind group layouts and render pipelines.

use std::marker::PhantomData;

#[derive(Clone)]
pub struct Bgl<T> {
    pub raw: wgpu::BindGroupLayout,
    _t: PhantomData<T>,
}

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
pub enum TerrainPass {}
pub enum InstancedPass {}
pub enum SkyPass {}
pub enum AoPass {}
pub enum SsgiPass {}
pub enum SsrPass {}
pub enum BloomPass {}
