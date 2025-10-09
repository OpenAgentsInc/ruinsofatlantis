use super::Renderer;

/// GPU container for static Zone batches (instances, clusters, etc.).
///
/// v0 is intentionally minimal — we only use this as a capability flag and a
/// future home for uploaded buffers.
pub struct GpuZoneBatches {
    pub slug: String,
}

pub fn upload_zone_batches(
    _r: &Renderer,
    zp: &client_core::zone_client::ZonePresentation,
) -> GpuZoneBatches {
    GpuZoneBatches {
        slug: zp.slug.clone(),
    }
}
