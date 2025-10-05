Status: Proposed

Title: Initial Clothing System — Wizard Robes for UBC Characters

Problem
- Our UBC-based player/NPC characters (male/female) currently render as a single skinned body without wearable garments. We want a lightweight, deterministic, and renderer‑friendly way to dress characters in wizard robes that work with our existing animation set and pipeline.

Goals (MVP)
- Dress all UBC characters (male/female) in a wizard robe garment that animates correctly with our current skeleton and clips.
- Keep the runtime simple: no cloth simulation and no dynamic tearing/pins; the robe is a skinned mesh bound to the same skeleton.
- Avoid visible body poke‑through with a predictable, deterministic approach (masking under‑garment regions or using prepared body LODs with hidden areas).
- Preserve performance budgets (add ≤1 skinned submesh per character; stay within current skinning buffers/pipelines).

Non‑Goals (Future)
- Full outfit layering, blendshape tailoring, or cloth physics.
- Runtime garment authoring/tools; we’ll integrate pre‑authored robe meshes prepared for UBC rigs.

Background & Assumptions
- The “Universal Base Characters” (UBC) packs provide rig‑compatible male/female meshes with multi‑material submeshes. Our viewer already aggregates all submeshes for the dominant skin and merges animations from AnimationLibrary.glb.
- Garments that are authored for the same skeleton can be loaded as separate GLTF/GLB assets and bound to the same palette/sampling path (skinned matrices).
- To prevent poke‑through, common industry patterns are: (1) author a robe slightly offset and thicker, (2) hide body polygons under the garment via vertex masks/material slots, or (3) export a “body-with-robe” variant with under‑areas removed.

Proposed Design (Phase 1)
1) Asset structure
   - Place robe meshes under `assets/models/clothing/robes/wizard/` with UBC‑compatible skin (same joint names, bind pose).
   - Include materials (baseColor, normal, ORM). Use alpha mask for trims if needed; two‑sided only for thin edges.
   - Provide two variants initially (male/female proportions) if required by the vendor export; otherwise a single unisex if it binds correctly.

2) Runtime composition
   - Load the base UBC body and the robe GLTF for each dressed character.
   - Bind both to the same per‑character palette (same joint matrices); robe has its own material bind group and is drawn as an additional skinned submesh.
   - Masking: prefer asset‑level “body-with-robe” exports that remove under‑polygons. If unavailable, support a simple “body segment hide list” (e.g., node/primitive names) to skip drawing those submeshes on dressed characters.

3) Animation compatibility
   - No retargeting changes: robe tracks the same skeleton and joints.
   - The existing sampler (CPU palette generation) remains unchanged; the robe reads the same instance palette base.

4) Materials & variants
   - Start with one wizard robe material (baseColor+normal+ORM) with 2 colorways (e.g., Blue/Crimson). Add a small tint uniform if the material pipeline already supports it; otherwise ship two textures.
   - Keep alpha usage to masked trims; avoid full alpha blending for the body of the robe.

5) Renderer integration (scoped)
   - Extend the character draw path to accept an extra skinned submesh per dressed character: draw order body → robe → accessories (future).
   - Reuse existing skinned pipeline; ensure material BG for the robe is distinct from body.
   - Update the instance struct usage: robe shares `palette_base` with the body.

6) Data & config
   - Add a tiny config describing whether a character is “dressed” and, optionally, which robe variant/color to use. (File: `data/config/clothing.toml`, e.g., default=wizard_robe_blue.)
   - For now, dress all UBC characters by default (PC + sorceress NPC); wizard NPCs (legacy rig) remain unchanged.

7) Tooling (viewer)
   - Add toggles in the model viewer to load/unload the robe on UBC characters and switch color variants. This aids quick QA of poke‑through across clips.

Acceptance Criteria (MVP)
- PC (UBC male) and Sorceress (UBC female) render with a wizard robe that animates correctly with idle/walk/sprint/casting.
- No obvious poke‑through on standard poses/locomotion under our current camera distances.
- Default build compiles and passes CI; performance budgets remain within current limits (≤1 added skinned draw per dressed character).
- Viewer can preview a UBC model dressed with the robe and switch one color variant.

Implementation Plan (Phased)
P1 — Assets & composition
- Import robe GLTF(s) under `assets/models/clothing/robes/wizard/` with LFS.
- Add a minimal clothing descriptor (per-character: enabled + variant).
- Load robe asset alongside UBC body; create robe VB/IB + material BG.
- Draw robe after body with same palette base; keep shaders unchanged.

P2 — Masking / poke‑through mitigation
- Prefer “body-with-robe” meshes from export; if not, add a hide list for known body submeshes (eyes/eyelashes remain visible as needed).
- Verify across Idle/Walk/Sprint/Cast.

P3 — Viewer support
- Buttons to toggle Robe On/Off and Variant A/B; log any missing joints or materials.

Testing
- CPU-only unit tests (skinning):
  - Given a synthetic skeleton and two skinned meshes (body + robe) sharing joints, sample a clip and assert robe vertices transform consistently with body (hash of transformed positions for a known clip/time).
  - Validate that the hide list omits expected body submeshes for dressed characters.
- Integration spot-checks:
  - Sanity render under Idle/Walk/Sprint/Cast; snapshot in viewer.

Performance & Budgets
- Each dressed character adds one skinned submesh draw. Target ≤0.2 ms aggregate at current scene scale on mid‑GPU.
- Materials: 1 additional bind group per dressed character; negligible memory overhead compared to base body.

Risks & Mitigations
- Poke‑through in extreme poses — keep MVP to standard locomotion + casting; document known edge cases.
- Skeleton mismatches — verify joint names between robe and body; log actionable errors in viewer and at load.
- Material sorting — ensure robe uses the same pipeline (no blended body); avoid overdraw spikes.

Docs & Ownership
- Update `src/README.md` to mention clothing composition under gfx.
- Add a short `docs/systems/clothing.md` for skeleton compatibility, masking policy, and performance guidance.
- Owners: Graphics (render_wgpu), Assets (LFS), Tools team (viewer toggles).

Out of Scope / Future
- Full outfit sets and mix‑and‑match layering.
- Cloth simulation and runtime tailoring.
- Dynamic swaps at runtime (for now load-on-start only).

---

Absolutely—let’s keep this *simple, deterministic, and production‑friendly* for the UBC (Quaternius) characters while leaving a clean path to fancier tailoring later.

---

## Practical considerations for a UBC clothing system (start simple, scale later)

### 1) How “tailoring” works at MVP

* **Skeleton‑fit first.** UBC bodies share a humanoid rig. If a robe is skinned to **the same joint names and bind pose**, it automatically conforms to any UBC character that uses that skeleton (including bone‑length/scale variations).
* **Proportion safety without morphs.** To handle slight width differences (regular/teen/superhero), start with:

  * A **small garment “inflate” offset** (1–3 mm) along vertex normals at *bind* time (baked) or via a **shader parameter** (see WGSL snippet below).
  * Optional **male/female robe variants** if one mesh can’t cover all shapes cleanly.
* **Hide the body under the robe.** Deterministic poke‑through fix:

  * Prefer *exported “body‑with‑robe”* (under‑areas removed).
  * If that’s unavailable, support a **hide list** of body primitives (torso/hips/upper‑legs) per outfit so we simply don’t draw them.

> MVP rule of thumb: one extra skinned draw (robe), zero cloth physics, zero runtime morphs.

### 2) Accessories (wands, belts, hats)

* Define a **socket table** per skeleton (simple JSON/TOML). Sockets are transforms relative to specific bones (e.g., `r_hand`, `spine2_back`, `head_top`).
* Accessories are **static or rigid meshes** parented to sockets (no skinning). For straps/belts later, you can convert them to skinned with limited bones.

### 3) Asset expectations (UBC friendly)

* Format: **glTF 2.0** preferred (UBC supports glTF/FBX; CC0 license).
* Mesh budget: keep robe ≤ **10–15k tris**, one material if possible.
* Materials: **PBR** (baseColor, normal, ORM). Use **alpha‑masked trims** only; no blended robe body.
* Skeleton: identical joint names & bind pose to UBC. Keep the robe’s armature at the **same root orientation**.

### 4) Export notes (Blender quick path)

1. Duplicate UBC body → model robe (Solidify for thickness, then **Apply**).
2. **Transfer weights** from body to robe (nearest face interpolated), clean weights.
3. Bind to **the same Armature** (don’t add new bones).
4. Make a *body‑with‑robe‑cut* variant (delete hidden faces) **or** author a **hide list**.
5. Export glTF: *Apply Modifiers*, *Tangents*, *Skin Weights*, *Armature*, meters scale 1.0.

### 5) Rendering & performance

* Add exactly **one additional skinned submesh** per dressed character.
* Draw order: **body → robe → accessories** (opaque), then usual post.
* Keep robe in the **same skinning path** and palette as body; no new pipelines.

### 6) Visual safety rails

* **TAA ghosting**: mark robe material as *non‑reactive* except edges; clamp history near thin trims.
* **Overdraw**: avoid double‑sided robe except cuffs/hood edges (alpha‑clip if needed).
* **LOD**: robe shares body LOD switches (or 1 LOD) for now.

---

## Minimal API & data (Rust ECS, pseudocode)

**Components**

```rust
struct SkeletalRig { skeleton_id: SkeletonId, palette_base: u32 /* etc. */ }

struct Outfit {
    garments: Vec<GarmentInstance>,
    accessories: Vec<AccessoryInstance>,
}

struct GarmentInstance {
    mesh: Handle<Mesh>,              // skinned
    material: Handle<Material>,
    inflate_mm: f32,                 // 0.0–3.0; shader param
    hide_rules: HideRules,           // e.g. ["Body.Torso", "Body.Hips", "Body.UpperLegs"]
}

struct AccessoryInstance {
    mesh: Handle<Mesh>,              // rigid
    material: Handle<Material>,
    socket: SocketId,                // e.g. "r_hand"
    local: Mat4,                     // fine offset/rotation
}

enum HideRules { None, NamedPrims(Vec<String>) }
struct SocketId(pub &'static str);
```

**Sockets definition (per skeleton) — `assets/skeletons/ubc_sockets.toml`**

```toml
[r_hand] bone="Hand.R"  offset=[[0,0,0],[0,0,0,1]]
[l_hand] bone="Hand.L"  offset=[[0,0,0],[0,0,0,1]]
[head_top] bone="Head"  offset=[[0,0.11,0],[0,0,0,1]]
[spine_back] bone="Spine2" offset=[[0,-0.03,-0.08],[0,0,0,1]]
```

**Clothing descriptor — `assets/clothing/wizard_robe/descriptor.toml`**

```toml
id = "wizard_robe_A"
skeleton = "ubc_humanoid"
mesh = "assets/clothing/wizard_robe/wizard_robe_A.glb#Mesh0"
material = "assets/clothing/wizard_robe/wizard_robe_A.mat"
inflate_mm = 1.5
hide = ["Body.Torso","Body.Hips","Body.UpperLegs"]
variants = ["blue","crimson"]
```

**Attach robe to a character (system)**

```rust
fn attach_outfit(mut q: Query<(Entity, &SkeletalRig, Option<&mut Outfit>), Added<SkeletalRig>>) {
    for (e, rig, outfit) in &mut q {
        let robe = load_garment("wizard_robe_A", rig.skeleton_id);
        let wand = AccessoryInstance {
            mesh: load_mesh("assets/props/wand/wand_a.glb#Mesh0"),
            material: load_mat("assets/props/wand/wand_a.mat"),
            socket: SocketId("r_hand"),
            local: Mat4::from_translation(Vec3::new(0.02, -0.01, 0.0)),
        };
        let mut o = outfit.cloned().unwrap_or(Outfit { garments: vec![], accessories: vec![] });
        o.garments.push(robe);
        o.accessories.push(wand);
        commands.entity(e).insert(o);
    }
}
```

**WGSL vertex tweak (inflate before skin)**

```wgsl
struct GarmentPush { inflate_mm: f32; _pad: vec3<f32>; };
@group(2) @binding(0) var<uniform> garment: GarmentPush;

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var pos_obj = in.position + normalize(in.normal) * (garment.inflate_mm * 0.001);
    let skinned = skin(pos_obj, in.joint_indices, in.joint_weights); // your existing skin()
    // ...
}
```

*(Do the offset in bind/object space and then skin—keeps the thickness consistent under animation.)*

---

## 📌 GitHub Issue (copy‑paste)

**Status:** Proposed
**Title:** Initial Clothing System — Wizard Robes for UBC Characters

### Problem

Our UBC‑based characters currently render as a single skinned body with no wearables. We need a **lightweight and deterministic** way to dress every UBC character in a wizard robe and attach a wand. Runtime must remain simple (no cloth sim) and integrate with our existing skeleton/animation and Rust/wgpu renderer.

### Goals (MVP)

* Dress **all UBC characters (male & female)** in a wizard robe that animates correctly with our current skeleton and clips.
* **No physics.** Robe is a skinned mesh bound to the **same** skeleton as the body.
* **Deterministic poke‑through mitigation** via hidden body regions or cut‑body exports.
* **Accessories support:** attach a rigid **wand** to a right‑hand socket.
* **Performance:** ≤ **+1 skinned draw** and ≤ **+1 rigid draw** per dressed character; keep within current skinning/UBO limits.

### Non‑Goals (MVP)

* Layered outfits, runtime authoring, cloth simulation, runtime shape tailoring/blendshapes, material variant UI.

### Background & Assumptions

* UBC assets are CC0, ~13k tri bodies, rigged, provided in glTF/FBX; we already play animations from the UBC/Universal Animation Library set.
* We render skinned characters via a shared palette; materials are standard PBR (baseColor/normal/ORM); TAA is enabled.
* Sockets will be defined as transforms relative to existing bones (no extra bones required).

### Proposed Design

1. **Assets**

   * `assets/clothing/wizard_robe/`

     * `wizard_robe_A.glb` (skinned to UBC skeleton; start with a unisex mesh—fall back to male/female variants if needed)
     * `wizard_robe_A.mat` (+ `blue` / `crimson` material variants)
     * `descriptor.toml` (skeleton id, mesh path, material path, `inflate_mm`, `hide` list)
   * `assets/props/wand/wand_a.glb` (+ material)
   * `assets/skeletons/ubc_sockets.toml` (socket → bone mapping with offsets)

2. **Runtime composition**

   * Load body and robe as **independent skinned submeshes** bound to the same palette (`palette_base`).
   * Draw order: **body → robe → accessories** (opaque).
   * Apply a **bind‑space inflate** (1–3 mm) in the garment shader (uniform `inflate_mm`).
   * Apply **hide rules**: skip rendering named body primitives under the robe *or* load a pre‑cut “body‑with‑robe” mesh if provided.

3. **Accessories (wand)**

   * Introduce **socket system**: resolve socket → (bone, local offset). Parent wand as a rigid draw using the animated bone matrix * × local transform*.

4. **Data**

   * New `Outfit` component with `garments[]` and `accessories[]`.
   * Robe/variant selection via `data/config/clothing.toml` (default robe/color per character type).

5. **Renderer integration**

   * Allow an extra skinned draw per character instance reusing current skinning pipeline and palette.
   * Add a tiny bind group for `GarmentPush` (`inflate_mm`) or fold into per‑material params.

6. **Tools**

   * Model viewer toggles: **Robe On/Off**, Variant select (Blue/Crimson), **Hide Rules On/Off**, **Sockets preview** (draw small axes).

### Acceptance Criteria

* UBC male + UBC female **both** display the wizard robe correctly across **Idle / Walk / Run / Cast** animations.
* No visible body poke‑through at default camera distances; extreme acrobatics may clip (documented).
* Wand appears in right hand with stable orientation during locomotion and casting.
* Scene perf impact: **≤0.2 ms GPU** per 10 dressed characters on our mid‑tier PC test; PS5 perf parity remains within budget.
* CI: viewer snapshots (4 clips × 2 bodies) pass visual diff threshold; loader prints **zero** missing‑joint warnings.

### Implementation Plan

**P1 — Assets & loader (2–3 days)**

* [ ] Import `wizard_robe_A.glb` + materials; validate joint names against UBC skeleton.
* [ ] Add `descriptor.toml` and parse into `ClothingDescriptor`.
* [ ] Implement `Outfit` component and loader that attaches robe + wand based on config.
* [ ] Draw robe as an additional skinned submesh sharing the body’s palette.

**P2 — Hide rules & inflate (2–3 days)**

* [ ] Implement **hide list**: optional primitive‑name filter to skip body draws under robe.
* [ ] Add **garment inflate** uniform and WGSL displacement before skinning.
* [ ] Viewer toggles to flip hide rules and tweak `inflate_mm` (0–3 mm).

**P3 — Sockets & accessories (1–2 days)**

* [ ] Implement `ubc_sockets.toml`; load and resolve per skeleton.
* [ ] Attach wand to `r_hand` socket with a per‑asset local offset.
* [ ] Viewer overlay to visualize sockets.

**P4 — QA & content pass (1–2 days)**

* [ ] Snapshot tests: male/female × (Idle/Walk/Run/Cast).
* [ ] Verify no missing joints/materials; log actionable errors.
* [ ] Add “Known Issues” (extreme crouch twist may clip at hip hem).

### Testing

* **Unit (skinning parity):** given a known pose, transformed robe vertex hashes match body bone transforms (rig consistency).
* **Integration:** golden‑image diffs for four animations; screen‑space poke‑through analyzer (optional debug: highlight body depth > robe depth within 1–2 px).

### Performance

* **Draws:** +1 skinned (robe) +1 rigid (wand) per dressed character.
* **Materials:** +1 PBR bind per robe; accessories share existing PBR pipeline.
* **Memory:** robe VB/IB within 0.5–1.0 MB; textures 512–1k for MVP.

### Risks & Mitigations

* **Robe fit fails on one body type** → ship male/female variants; enable `inflate_mm` per character.
* **Overdraw on robe hems** → alpha‑clip trims; avoid double‑sided except edges.
* **Socket drift (retargeted clips)** → lock sockets to bones (not retarget helpers); expose per‑accessory local offsets.

### Docs & Ownership

* [ ] `docs/systems/clothing.md` (assets, sockets, hide rules, inflate).
* **Owners:** Graphics (render_wgpu), Tools (viewer), Content (assets).

### Out of Scope (Future)

* Cloth simulation, layered outfits, runtime morph tailoring, material customization UI, dynamic runtime swap pools.

---

### Bonus: “Known good” robe build checklist for content

* Single armature, identical joint names to UBC.
* Final mesh has **applied transforms** and **triangulated** polygons.
* Weight normalization; no >4 influences/vertex (or match engine limit).
* *Optional:* Export **male** and **female** robe variants if a single unisex mesh clips in QA.
* Provide a **cut‑body** export *or* a clear hide‑list mapping for UBC primitives.

If you want, I can also drop a tiny robe test scene spec (camera path + clips + timing) you can paste into your test runner to produce the acceptance snapshots.
