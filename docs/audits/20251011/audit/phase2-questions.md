# Phase‑Two Questions

These are clarifications to ensure PR 12+ align with your expectations.

- ExecCtx contents: OK to gradually expand ExecCtx (currently `{ renderer, encoder }`) instead of requiring `GpuCtx/SurfaceCtx/Attachments/Pipelines/BgCache/UploadRing` immediately? I can wire those as they materialize in later PRs.
- Pipelines adoption: Do you want `Pipelines` (typed handles) threaded through `ExecCtx` starting with PR 13 (Sky/Present), or keep using existing raw fields for the first extraction, then wrap later?
- BindGroup cache & Upload ring: Any existing cache/ring patterns you want me to follow, or should I scaffold `BgCache`/`UploadRing` as fresh modules under `renderer/`?
- Hazard scope: For now, validation checks per‑pass read+write conflicts. Should I also add a simple cross‑pass hazard check (write→read without an explicit resolve) in PR 12, or leave that for when we alias resources in PR 16?
- Tests: Is it acceptable to keep tests CPU‑only for PR 12–15, with optional GPU timestamp queries behind a feature later (per Phase‑Two goals)?
- Present/Surface: For PR 13, do you prefer presenting via the surface’s `frame.present()` as today, or routing the swapchain view as a declared `backbuffer` handle through the graph’s present pass (first option seems lower churn)?
