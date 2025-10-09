# bevy_gltf: Notes and Takeaways

This summarizes how Bevy’s `bevy_gltf` loads glTF 2.0 compared to our current prototype loader, and what we can adopt.

## What Bevy Loads and How
- Plugin + AssetLoader: `GltfPlugin` registers a `GltfLoader` that outputs a top‑level `Gltf` asset plus handles for scenes, nodes, meshes, primitives, skins, materials, images, animations.
- Labels everywhere: each sub‑asset is addressable (e.g., `GltfAssetLabel::Primitive{ mesh, primitive }`, `Texture(index)`). This enables partial loads and reuse.
- Buffers/images: supports `Source::View` (embedded) and `Source::Uri` (external). Data URIs are decoded; regular URIs are resolved relative to the glTF file.
- Loader settings: flags control which categories load and which worlds retain assets (`RenderAssetUsages`), default sampler override, and coordinate conversion (`use_model_forward_direction`).

## Geometry & Attributes
- Per‑primitive meshes: preserves primitive topology (triangle list/strip, etc.) and index width (u8/u16/u32).
- Attribute conversion: robust path handling positions, normals, tangents, colors, joints/weights, multiple UV sets; custom vertex attributes supported.
- Auto‑fixups: generates flat normals if missing (triangle lists), and tangents when materials require them (normal/clearcoat normal maps).
- Morph targets: reads morph target positions/normals/tangents and builds a `MorphTargetImage` asset.

## Skinning
- Skins are assets: `SkinnedMeshInverseBindposes` (palette of inverse bind matrices) + `SkinnedMesh` ECS components per node. Runtime updates are handled by Bevy’s render pipeline.

## Materials & Textures
- StandardMaterial mapping: base color, metallic/roughness, normal, occlusion, emissive; plus optional extensions behind features (clearcoat, anisotropy, specular, transmission/volume).
- UV channels: selects UV0/UV1 based on the texture’s `texCoord`; warns for channels > 1.
- Texture transforms: reads `KHR_texture_transform`; applies it on base‑color; warns for differing transforms on other texture kinds.
- Samplers: builds an `ImageSamplerDescriptor` from glTF sampler (wrap/filter). Honors global default sampler (including anisotropy rules).

## Scenes, Nodes, Cameras/Lights
- Builds Bevy `Scene` with hierarchical `Transform`s. Can optionally spawn cameras (perspective/ortho) and lights (directional/point/spot). Has coordinate‑conversion option for forward direction.

## Animations
- Parses all glTF animations (translations/rotations/scales) with proper interpolation (linear/step; cubic where applicable). Emits `AnimationClip`s and wires `AnimationPlayer` targets.

## Differences vs Our Loader
- Assetization: Bevy splits the glTF into many typed assets; we currently flatten one mesh (wizard) and one static mesh (ruins) into CPU buffers.
- Attributes/UVs: Bevy supports multiple UV sets and custom attributes; we pick a single UV set from baseColorTexture and planar‑fallback if invalid.
- Materials: Bevy covers full PBR + extensions; we sample only base‑color (SRGB), no metallic/roughness/normal/occlusion/emissive yet.
- Texture transforms: Bevy applies `KHR_texture_transform` (base‑color) and warns on others; we only just added a minimal transform uniform for base‑color.
- Samplers: Bevy mirrors glTF wrap/filter and has a global default; we hard‑code linear + repeat and nearest mips for now.
- Geometry fixups: Bevy auto‑generates normals/tangents when needed; we rely on source data (no generation yet).
- Morph/Animation: Bevy loads morph targets and all animations; we parse only a few named clips and sample linearly; no morph targets.
- Skinning: Bevy uses ECS components and renderer integration; we upload palettes to a storage buffer and do GPU skinning in our WGSL (works, but it’s bespoke).
- Draco: Bevy does not decode Draco at load time; our loader includes a CLI decompression fallback using `gltf-transform` (and a native Draco decoder for JSON path).

## Action Items We Can Adopt
- Asset boundaries: keep CPU/GPU split but introduce typed assets (Mesh, Material, Texture, Skin) to avoid flattening and enable per‑primitive draws.
- Texture pipeline: map glTF sampler wrap/filter; add metallic/roughness/normal/occlusion/emissive; honor `uv_channel` per texture.
- Texture transforms: keep uniform path; also log/warn when non‑base transforms exist (as Bevy does) until supported.
- Normals/tangents: add optional generation for missing data (flat normals, mikktspace tangents) behind a feature flag.
- Animations: generalize to load all channels with proper interpolation; keep small helper to pick “portal” demo clips.
- Coordinate conversion: add a toggle to convert forward direction for compatibility with non‑Y‑up content.
- Samplers: introduce a default sampler config and allow per‑texture overrides from glTF.
- Index types: preserve u16/u32 indices instead of forcing u16; avoid unnecessary re‑indexing.
- Diagnostics: mirror Bevy’s warnings (skinned mesh on non‑skinned node, UV channel > 1, differing transforms).

## References
- Bevy source: `crates/bevy_gltf/src` (loader, gltf_ext/*, vertex_attributes.rs, convert_coordinates.rs).
- Key APIs: `texture_handle`, `texture_sampler`, `texture_transform_to_affine2`, `needs_tangents`, `convert_attribute`, `GltfLoaderSettings`.
