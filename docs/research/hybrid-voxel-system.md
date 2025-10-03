Great question. Here’s a clear, practical way to think about **“all‑voxel world”** vs a **hybrid mesh+voxel** approach (with pre‑voxelized assets), plus concrete implications for performance, visuals, authoring, physics, streaming, and your current engine.

---

## TL;DR (recommendation)

* **Keep a hybrid:** meshes for most things, **voxels only where you need live destruction** (terrain zones + destructible props).
* **Pre‑voxelize** destructible assets offline (object‑space voxel data embedded/sidecar) so runtime doesn’t do slow tri→voxel work; spawn proxies instantly on first hit.
* Consider **voxel terrain** in hot areas (battle arenas / set pieces), but **not the entire world** (memory + streaming).
* Characters, FX, foliage, small props stay as meshes.

This gets you the fun parts of voxels—**instant destruction, debris, stable collisions, easy boolean ops**—without the worst costs (memory, bake times, “blocky” look everywhere).

---

## What “everything is voxels” buys you

**Pros**

* **Unified destructive geometry:** every surface can be carved/fractured—no proxy swapping, no special cases.
* **Simple boolean edits:** explosions, digging, building are just write ops to a grid; trivial to record/replay as “carve spheres”.
* **Stable collision queries:** one representation for gameplay (raycasts, overlaps, bullet impact).
* **Emergent simulation:** heat diffusion, fire spread, fluids, erosion can live on the grid.
* **Artist‑proof destruction:** no fragile pre‑fracture authoring; gameplay drives the geometry.

**Cons / costs**

* **Memory explodes with resolution & volume.** Example: a modest 100 m × 100 m × 20 m play space, at 10 cm voxels:

  * dims = 1000 × 200 × 1000 = **200,000,000 voxels**
  * 1 byte occupancy → **~200 MB** raw (no materials/metadata), plus chunk and meshing overhead.
  * With materials (say 2 bytes) + hashes + GPU meshes, you can easily exceed **500 MB** just for that region.
* **Visual fidelity trade‑off.** Small curves and thin details look chunky unless you use higher res or advanced meshing (dual contouring with Hermite data), both more expensive.
* **Rendering cost & meshing time.** Realtime chunk meshing, uploads, and draw calls become a primary cost (though greedy meshing helps a lot).
* **Lighting complexity.** PBR looks great on meshes with UVs; voxels tend to prefer triplanar mapping and need different AO/lighting tricks.
* **Authoring friction.** Most existing assets are meshes; fully voxel requires a different art pipeline, tools, and preview.
* **Streaming & persistence.** World‑scale voxels require chunk streaming (I/O, compression), LODs, and robust save/load of edits.

**Bottom line:** All‑voxel is fantastic for *sandbox/destruction‑first* games. For a stylized RPG arena with set pieces, it’s often overkill.

---

## What a **hybrid** gets you (the path you’re on)

**Pros**

* **High‑fidelity visuals** for most content (meshes + PBR) with **selective** voxel destructibility.
* **Manageable memory/perf**: you only pay the voxel cost where gameplay needs it.
* **Fast iteration:** keep importing GLTFs; mark only some as destructible; everything else is untouched.
* **Deterministic networking & saves:** replicate compact carve ops per destructible ID instead of shipping entire grids.

**Cons**

* Requires a **mesh→voxel** step (you already have it). Doing it *offline* solves runtime stalls.
* Two render paths (mesh and voxels) to maintain; but they’re already in your renderer.

---

## If you *do* want “everything voxels,” what you’d add

* **Sparse bricking + streaming:** 8³ / 16³ bricks, only allocate populated bricks; compress on disk; stream by distance.
* **Adaptive resolution:** coarse voxels outside the action; refined in “interesting” regions (or on demand as explosions approach).
* **Better meshing:** dual contouring / transvoxel for smoother silhouettes; still works in wgpu, but more code.
* **Lighting pipeline:** triplanar PBR, cheap AO, maybe probe‑based GI; baking for voxels is different from lightmaps.
* **Editor & tools:** in‑engine voxel paint/boolean tools; or offline prevoxelizer integrated with DCC pipeline.

That’s a sizeable engineering track.

---

## Pre‑voxelized assets (highly recommended next step)

This gives you most benefits instantly:

* **Offline voxelize** any asset you mark “destructible” → produce **object‑space** voxel data:

  * `voxel_m` (meters/voxel), `dims` (X,Y,Z), material id, chunk dims, and **surface mask** (or SDF).
  * Option A (fast & small): **surface-only + implicit interior fill** (we already fill via parity).
  * Option B (simple): binary solid grid.
* **Store** it as a **GLTF extension** or sidecar binary. Example GLTF `extras` snippet:

```json
{
  "extensions": {
    "RA_voxel_proxy": {
      "voxel_m": 0.10,
      "dims": [120, 96, 120],
      "chunk": [16, 16, 16],
      "material": "stone_ruin_01",
      "layout": "bricks8",
      "bricks": [
        // array of brick records: {coord:[bx,by,bz], mask: base64(512 bits)}
      ]
    }
  }
}
```

* **Runtime:** when the instance is first hit:

  * Skip tri→voxel; **instantiate the voxel grid immediately** from the extension.
  * Mesh a few chunks right away (you already burst‑mesh).
  * Carves and debris work as they do now.

**Why it helps**

* No 3‑second stalls; no proxy “box” fallbacks; silhouettes match art perfectly.
* Reused assets share the same voxel template (great for many ruins).
* You can ship multiple **LOD voxelizations** per asset (vm 5 cm, 10 cm, 20 cm) and choose one at spawn based on expected distance/perf.

---

## Terrain strategy (middle ground)

* Keep **terrain as voxels** in *combat areas* (or along a spline corridor) so the ground can crater, tunnel, or deform.
* Keep **far terrain** as mesh with heightmap or static mesh.
* This creates a compelling “the world breaks” feeling without turning the whole map into a data problem.

---

## Engine implications (concrete and immediate)

1. **Registry format**

   * Add `DestructVoxelAsset` (dims, voxel_m, chunk, material, brick data).
   * Map `DestructInstance` → either `mesh_id` or `voxel_asset_id` (prefer voxel when present).

2. **Loader**

   * In SceneBuild, when a GLTF node has `RA_voxel_proxy`, load it into `destruct_meshes_vox` and record `voxel_asset_id` on each instance.
   * Precompute `world_min/world_max` from local AABB (already added).

3. **Spawn**

   * `get_or_spawn_proxy(did)` picks **voxel asset path** if available; only falls back to tri→voxel for older content.

4. **Materials & shading**

   * Use **triplanar mapping** for voxel chunks (already typical).
   * Allow per‑voxel **material IDs** (stone, wood, metal) → different debris mass/drag, sound, color.

5. **Budgets (good defaults on M2‑class)**

   * Voxel size (destructibles): **0.08–0.12 m** (8–12 cm) looks good up close.
   * Chunk size: **16³** (nice balance for CPU meshing).
   * First‑frame burst mesh: **64 chunks** max (you’re doing similar).
   * Per‑frame budget: **16–32 chunks** remeshed.
   * Debris cap per impact: **~64–128** cubes, TTL ~2–3 s.

6. **Persistence & multiplayer**

   * Keep your current **carve‑ops log** per `DestructibleId` (center+radius per impact).
   * On load, instantiate the pre‑voxelized grid and **replay** the ops (you already implemented replay primitives).

---

## Numbers to ground expectations

* **Single ruin** (≈ 12×10×12 m) at 10 cm voxels: ~120×100×120 ≈ **1.44 M cells**.

  * Surface‑only bricks storage (8³) often **<5–15 MB** compressed per asset template, reused across instances.
  * Runtime instantiated grid (binary occupancy in RAM): 1–2 bytes/voxel → **1.4–2.9 MB** per *active* proxy (plus meshed chunk VBs/IBs).
  * With your chunk meshing and caching, this is **comfortable**.

* **Whole arena 100×100×20 m** at 10 cm voxels: 200 M cells → **200–400 MB** raw grid **alone**, plus meshes.

  * Feasible only with sparse streaming and/or coarser voxels outside the action.

---

## What you *lose* if you go all‑voxel

* **Fine curved silhouettes** and crisp UV‑based material detail (unless you pay with tiny voxels and advanced meshing).
* **Authoring convenience**: artists lose native mesh fidelity; every asset must be voxel‑ready; more pipeline work.
* **Performance headroom** you might prefer to spend on AI, FX, or larger scenes.

---

## A sensible roadmap

1. **Now**: Flip to **pre‑voxelized assets** for ruins (and the next few destructibles). Keep your current multi‑proxy system; it will just instantiate instantly.
2. **Next**: Mark terrain patches in combat areas as voxel zones (world‑space). Let Fireball and big spells deform terrain.
3. **Later** (if desired): Add LOD voxelization and streaming to expand playable voxel space. Consider dual contouring for smoother visuals.

---

## Quick checklist you can hand to the dev agent

* [ ] Add `DestructVoxelAsset` and GLTF `RA_voxel_proxy` loader.
* [ ] Prefer `voxel_asset_id` in `get_or_spawn_proxy(did)`; skip runtime tri→voxel.
* [ ] Keep current carve/debris/collider/meshing pipeline; **no logic changes needed**.
* [ ] Provide a tiny CLI: `xtask voxelize --in assets/ruins.gltf --vm 0.10 --chunk 16 --out assets/ruins.vox.json` (or embed as GLTF extension).
* [ ] Default budgets: 64 burst chunks on spawn; 16–32 per frame steady; debris cap 96.

---

If you want, I can sketch the GLTF extension schema and the minimal loader changes next, or a small heuristic for choosing `voxel_m` per asset based on typical camera distance (so silhouettes look smooth without over‑voxelizing).
