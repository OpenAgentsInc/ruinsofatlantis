
# Dragons in Bevy — Runtime Handling, Authoring, and NPC Integration (with code)

This dossier explains how our Bevy vertical slice loads, spawns, and animates dragons from glTF (GLB), how the headless Blender scripts build the dragon prototypes, and how to integrate dragons as NPCs alongside the existing wizard scene/NPC patterns. It includes code excerpts from the repository so the flow is concrete.

## Summary

- Authoring: DragonProto_v2.glb is produced headlessly in Blender with minimal rig (spine, neck/head, tail chain, wings) and multiple Actions pushed to NLA for separate clips.
- Loading: The Bevy slice loads the GLTF with cameras/lights disabled and animations enabled, spawns Scene0, prunes helper meshes by name, finds the GLTF‑provided AnimationPlayer, builds an AnimationGraph with all clips, and rotates through them automatically.
- NPC context: The existing wizard scene builds ring spawns with palette bases and per‑instance animation indices/time offsets; the server demo registers NPCs (Npc component) and steps AI; replication can drive visuals.
- Path forward: Register dragons as NPC types by mirroring the wizard pipeline: prefab + instance placement + per‑instance animation selection; wire into replication and/or local spawners.

## Authoring (Blender → GLB)

DragonProto v2 extends the basic prototype:

- Rig: `spine`, `neck`→`head`, `tail.01..tail.04`, `wing.R`, `wing.L`.
- Skinning: automatic weights; single mesh bound to the armature.
- Clips: `Flap`, `Bank`, `Idle`, `Look`, `TailSwing` — each Action pushed to NLA so exporters emit distinct glTF animations.
- Export: GLB, selection‑only (mesh+armature), lights/cameras excluded, `export_animation_mode='NLA_TRACKS'`, `export_animations=True`, `export_bake_animation=True`, `export_force_sampling=True`.

See: docs/dragon-proto-workflow.md for details and commands; source notes in RedWyvern docs (DRAGON_PROTO_V2.md).

## Runtime (Bevy) — Loading and Spawning

The Bevy slice app defaults to DragonProto_v2 and sets glTF loader settings to keep the scene clean and animation‑ready.

Code: apps/roa_slice_bevy/src/lib.rs

```rust
const DEFAULT_DRAGON: &str = "models/DragonProto_v2.glb";

fn setup_slice(mut commands: Commands, cfg: Res<SliceConfig>, assets: Res<AssetServer>) {
    if cfg.zone_picker { return; }
    // Load GLTF with cameras/lights disabled; animations enabled
    let dragon_gltf: Handle<Gltf> = assets.load_with_settings(
        DEFAULT_DRAGON,
        |s: &mut bevy_gltf::GltfLoaderSettings| {
            s.load_cameras = false;
            s.load_lights = false;
            s.load_animations = true;
        },
    );
    commands.insert_resource(DragonGltf(dragon_gltf));
    info!("Slice: requested dragon glTF={}", DEFAULT_DRAGON);
}
```

We spawn Scene0 once the Gltf asset is loaded:

```rust
fn spawn_dragon_scene(
    mut commands: Commands,
    dragon: Res<DragonGltf>,
    gltfs: Res<Assets<Gltf>>,
    assets: Res<AssetServer>,
    mut spawned: Local<bool>,
) {
    if *spawned { return; }
    let Some(g) = gltfs.get(&dragon.0) else { return; };
    if let Some(first_scene) = g.scenes.get(0).cloned() {
        let xf = Transform::from_xyz(0.0, 1.5, 0.0).with_scale(Vec3::splat(1.0));
        commands.spawn((SceneRoot(first_scene), xf, IsDragon));
        info!("dragon: spawned Scene0");
        *spawned = true;
        return;
    }
    // fallback via label
    let scene0 = assets.load(GltfAssetLabel::Scene(0).from_asset(DEFAULT_DRAGON));
    commands.spawn((SceneRoot(scene0), Transform::from_xyz(0.0, 1.5, 0.0).with_scale(Vec3::splat(1.0)), IsDragon));
    *spawned = true;
}
```

We remove stray helper meshes by name once (cube, plane, grid… and light/sphere/sun/emiss):

```rust
fn prune_dragon_extras(
    mut commands: Commands,
    mut done: Local<bool>,
    q_root: Query<(Entity, Option<&Children>), With<IsDragon>>,
    q_children: Query<&Children>,
    q_name_mesh: Query<(Option<&Name>, Option<&Mesh3d>)>,
) {
    if *done { return; }
    let Ok((_root, kids)) = q_root.single() else { return; };
    let mut stack: Vec<Entity> = Vec::new();
    if let Some(k) = kids { for c in k.iter() { stack.push(c); } }
    let mut removed = 0;
    while let Some(e) = stack.pop() {
        if let Ok(children) = q_children.get(e) { for c in children.iter() { stack.push(c); } }
        if let Ok((maybe_name, maybe_mesh)) = q_name_mesh.get(e) {
            if maybe_mesh.is_some() {
                if let Some(name) = maybe_name {
                    let s = name.as_str().to_ascii_lowercase();
                    if s.contains("cube")||s.contains("block")||s.contains("plane")||s.contains("grid")||s.contains("floor")||
                       s.contains("helper")||s.contains("light")||s.contains("lamp")||s.contains("sphere")||s.contains("sun")||s.contains("emiss") {
                        commands.entity(e).despawn();
                        removed += 1;
                    }
                }
            }
        }
    }
    if removed > 0 { info!("pruned {} helper mesh(es) under dragon", removed); }
    *done = true;
}
```

## Runtime (Bevy) — Animation: Use All Clips, Rotate Automatically

We attach an AnimationGraph with a clip node for each glTF animation and drive the GLTF’s own AnimationPlayer entity (created by the loader) so skinning targets resolve correctly.

```rust
#[derive(Component, Clone)]
struct DragonAnimSeq { nodes: Vec<AnimationNodeIndex>, idx: usize }
#[derive(Component)]
struct DragonAnimController; // marker on the driven player entity

fn prepare_dragon_animation(
    mut commands: Commands,
    dragon: Res<DragonGltf>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    q_dragon: Query<(Entity, Option<&Children>), With<IsDragon>>,
    q_children: Query<&Children>,
    q_players: Query<Entity, With<AnimationPlayer>>,
    mut done: Local<bool>,
) {
    if *done { return; }
    let Some(g) = gltfs.get(&dragon.0) else { return; };
    if g.animations.is_empty() { info!("dragon: no animations found in GLTF"); *done = true; return; }

    let mut graph = AnimationGraph::new();
    let mut seq_nodes = Vec::new();
    for h in g.animations.iter() {
        let n = graph.add_clip(h.clone(), 1.0, graph.root);
        seq_nodes.push(n);
    }
    let handle = graphs.add(graph);

    // Find the existing AnimationPlayer inserted by the GLTF loader
    let Some((root, kids)) = q_dragon.iter().next() else { return; };
    let mut stack = Vec::new();
    if let Some(k) = kids { for c in k.iter() { stack.push(*c); } }
    let mut player_ent = None;
    while let Some(e) = stack.pop() {
        if q_players.get(e).is_ok() { player_ent = Some(e); break; }
        if let Ok(ch) = q_children.get(e) { for c in ch.iter() { stack.push(*c); } }
    }
    let target = player_ent.unwrap_or(root);

    let mut ecmd = commands.entity(target);
    if q_players.get(target).is_err() { ecmd.insert(AnimationPlayer::default()); }
    ecmd.insert(AnimationGraphHandle(handle))
        .insert(DragonAnimSeq { nodes: seq_nodes, idx: 0 })
        .insert(DragonAnimController);
}

fn kickoff_dragon_animation(
    mut q: Query<(&mut AnimationPlayer, &mut DragonAnimSeq), With<DragonAnimController>>,
) {
    if let Ok((mut player, seq)) = q.single_mut() {
        if let Some(&node) = seq.nodes.get(seq.idx) {
            if !player.is_playing_animation(node) {
                player.start(node);
                info!("dragon: playing animation node {:?}", node);
            }
        }
    }
}

fn cycle_dragon_animation(
    mut q: Query<(&mut AnimationPlayer, &mut DragonAnimSeq), With<DragonAnimController>>,
) {
    if let Ok((mut player, mut seq)) = q.single_mut() {
        if seq.nodes.is_empty() { return; }
        let cur = seq.nodes[seq.idx];
        let finished = player.animation(cur).map(|a| a.is_finished()).unwrap_or(true);
        if finished {
            seq.idx = (seq.idx + 1) % seq.nodes.len();
            let next = seq.nodes[seq.idx];
            player.stop_all();
            player.start(next);
            info!("dragon: switched animation to node {:?}", next);
        }
    }
}
```

## Input and Motion (domain)

We reuse the portable domain crate for input → commands → controller → transform state. In FixedUpdate we map WASD/mouse to domain `Command`s, apply them to a simple `DragonController`, then apply the domain `TransformState` back onto the Bevy `Transform` of the dragon root for basic locomotion.

See: apps/roa_slice_bevy/src/lib.rs (input_to_domain_commands, sys_apply_commands_to_controller, apply_transformstate_to_transforms) and crates/roa_domain (character.rs, input.rs, sim_time.rs).

## Existing Wizard Scene (reference) — Building NPC Visuals

The wizard scene assembles instances with per‑instance palette bases and per‑instance animation selection. This is a good template for multi‑NPC spawns.

Code: crates/render_wgpu/src/gfx/scene.rs (excerpt)

```rust
// Assign palette bases and animations: PC idle in Still; ring wizards PortalOpen (staggered)
let joints_per_wizard = skinned_cpu.joints_nodes.len() as u32;
let mut wizard_anim_index: Vec<usize> = Vec::with_capacity(wiz_instances.len());
let mut wizard_time_offset: Vec<f32> = Vec::with_capacity(wiz_instances.len());
for (i, inst) in wiz_instances.iter_mut().enumerate() {
    inst.palette_base = (i as u32) * joints_per_wizard;
    if i == 0 { wizard_anim_index.push(1); wizard_time_offset.push(0.0); } else {
        wizard_anim_index.push(0); // PortalOpen
        let ring_idx = i - 1; wizard_time_offset.push(ring_idx as f32 * 0.5);
    }
}
```

Takeaways we’ll reuse for dragons as NPCs:
- Keep a compact set of per‑instance parameters (animation selection, time offset, palette base when GPU‑skinning is applicable).
- Batch instance placement with clean buffers (buffer_init + COPY_DST updates).
- If replicated, accept NPC state from server and build visuals in one go.

## NPC systems (server demo) — registration and stepping

Server‑side NPCs use `Npc` components and simple AI that seeks wizards. The demo server boots a zone encounter, spawns the PC and NPCs, steps AI, and emits replication.

Code: crates/ecs_core/src/components.rs (excerpt)

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Npc { pub radius: f32, pub speed_mps: f32, pub damage: i32, pub attack_cooldown_s: f32 }
```

Code: crates/platform_winit/src/lib.rs (demo server wiring; excerpt)

```rust
let mut srv = server_core::ServerState::new();
let pc0 = wiz_now.first().copied().unwrap_or(glam::vec3(0.0, 0.6, 0.0));
if srv.pc_actor.is_none() { let _ = srv.spawn_pc_at(pc0); }
if let Some(slug) = detect_zone_slug() { let _ = server_core::zones::boot_with_zone(&mut srv, slug.as_str()); }
...
srv.step_authoritative(dt);
let asnap = srv.tick_snapshot_actors(tick64);
```

## Registering Dragons as NPCs (plan)

- Authoring: keep dragon rigs ≤256 joints and export per‑clip animations; one mesh per type; name GLBs predictably (e.g., `DragonProto_v2.glb`, `DragonGlide.glb`).
- Runtime registry: add a small registry mapping dragon types → GLTF path + default scale/offset and available clip names (read from GLTF and cached).
- Spawning multiple dragons:
  - Option A (client demo): create N entities, each with `SceneRoot` for the dragon scene, attach `DragonAnimSeq`, and stagger start indices/time.
  - Option B (replicated): on server snapshots, build dragon visuals similarly to how we build zombie/wizard visuals, with per‑instance params; select clip based on NPC state (idle, patrol, engage).
- Input/AI: re‑use `Npc` AI for simple seek/orbit; for aerial motion, extend the controller with yaw/pitch smoothers and altitude clamps (domain crate is the right place).

## Practical tips

- Use loader settings to keep the scene clean (no cameras/lights) and avoid stray “dots.”
- Always find and drive the GLTF‑inserted `AnimationPlayer`; don’t attach a new one to the root or targets won’t match.
- For many dragons, consider pooling SceneRoots (spawning multiple GLTF scenes is heavy). An alternative is a dedicated instance path akin to wizards with GPU palette skinning where applicable.

## Next steps

- Introduce a `dragon_types.toml` registry (type → GLB + scale) and a Bevy resource to cache clip handles per type.
- Add a spawner that places M dragons (mixed types), seeds `DragonAnimSeq` with different starting `idx`, and spaces them in 3D.
- Server path: define a `NpcKind::Dragon(type_id)` and emit replication; client builds visuals keyed by type.

If you want, I can scaffold the `dragon_types` registry, a spawner system, and a tiny UI toggle to cycle dragons and switch clips manually.

## With‑Code Appendix (240 KiB style excerpt)

----- CONTEXT -----
Bundle: Dragons in Bevy — code and docs referenced by this dossier, formatted
as file blocks similar to our 240 KiB sharing bundles.

Contents (excerpt)
- apps/roa_slice_bevy/src/lib.rs — Bevy slice loader/spawner/animation
- crates/roa_domain/src/{character.rs,input.rs,sim_time.rs} — domain controller
- docs/dragon-proto-workflow.md — authoring/export workflow
----- END CONTEXT -----

----- /Users/christopherdavid/code/ruinsofatlantis/apps/roa_slice_bevy/src/lib.rs -----
```rust
// Bevy slice entry: loading DragonProto_v2, pruning helpers, building graph for all clips,
// and rotating the AnimationPlayer across them.
// (Full file in repo; excerpted for brevity.)

// ... imports ...
const DEFAULT_DRAGON: &str = "models/DragonProto_v2.glb";

#[derive(Component, Clone)]
struct DragonAnimSeq { nodes: Vec<AnimationNodeIndex>, idx: usize }
#[derive(Component)]
struct DragonAnimController;

fn setup_slice(mut commands: Commands, cfg: Res<SliceConfig>, assets: Res<AssetServer>) {
    if cfg.zone_picker { return; }
    let dragon_gltf: Handle<Gltf> = assets.load_with_settings(
        DEFAULT_DRAGON,
        |s: &mut bevy_gltf::GltfLoaderSettings| {
            s.load_cameras = false; s.load_lights = false; s.load_animations = true;
        },
    );
    commands.insert_resource(DragonGltf(dragon_gltf));
}

fn spawn_dragon_scene(/* ... */) { /* spawn Scene0; set IsDragon; position/scale */ }

fn prune_dragon_extras(/* ... */) { /* despawn Cube/Plane/Grid/Light/Lamp/Sphere/Sun/Emiss */ }

fn prepare_dragon_animation(
    mut commands: Commands,
    dragon: Res<DragonGltf>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    q_dragon: Query<(Entity, Option<&Children>), With<IsDragon>>,
    q_children: Query<&Children>,
    q_players: Query<Entity, With<AnimationPlayer>>,
    mut done: Local<bool>,
) {
    if *done { return; }
    let Some(g) = gltfs.get(&dragon.0) else { return; };
    if g.animations.is_empty() { *done = true; return; }
    let mut graph = AnimationGraph::new();
    let mut seq_nodes = Vec::new();
    for h in g.animations.iter() { seq_nodes.push(graph.add_clip(h.clone(), 1.0, graph.root)); }
    let handle = graphs.add(graph);
    // find GLTF's AnimationPlayer in spawned hierarchy
    let Some((root, kids)) = q_dragon.iter().next() else { return; };
    let mut stack = Vec::new(); if let Some(k)=kids { for c in k.iter(){ stack.push(*c); } }
    let mut player_ent=None; while let Some(e)=stack.pop(){ if q_players.get(e).is_ok(){player_ent=Some(e);break;} if let Ok(ch)=q_children.get(e){for c in ch.iter(){stack.push(*c);} } }
    let target = player_ent.unwrap_or(root);
    let mut ecmd = commands.entity(target);
    if q_players.get(target).is_err() { ecmd.insert(AnimationPlayer::default()); }
    ecmd.insert(AnimationGraphHandle(handle))
        .insert(DragonAnimSeq { nodes: seq_nodes, idx: 0 })
        .insert(DragonAnimController);
}

fn kickoff_dragon_animation(mut q: Query<(&mut AnimationPlayer, &mut DragonAnimSeq), With<DragonAnimController>>){
    if let Ok((mut player, seq)) = q.single_mut() {
        if let Some(&node) = seq.nodes.get(seq.idx) { if !player.is_playing_animation(node) { player.start(node); } }
    }
}

fn cycle_dragon_animation(mut q: Query<(&mut AnimationPlayer, &mut DragonAnimSeq), With<DragonAnimController>>){
    if let Ok((mut player, mut seq)) = q.single_mut() {
        if seq.nodes.is_empty() { return; }
        let cur = seq.nodes[seq.idx];
        let finished = player.animation(cur).map(|a| a.is_finished()).unwrap_or(true);
        if finished { seq.idx = (seq.idx + 1) % seq.nodes.len(); let next = seq.nodes[seq.idx]; player.stop_all(); player.start(next); }
    }
}
```

----- /Users/christopherdavid/code/ruinsofatlantis/crates/roa_domain/src/character.rs -----
```rust
//! Minimal character/dragon controller components and systems.
use bevy_ecs::prelude::*;
use bevy_reflect::Reflect;
use crate::Command;

#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct DragonController { pub speed_fwd: f32, pub speed_strafe: f32, pub speed_up: f32, pub yaw: f32, pub pitch: f32 }
impl Default for DragonController { fn default() -> Self { Self { speed_fwd: 12.0, speed_strafe: 8.0, speed_up: 8.0, yaw: 0.0, pitch: 0.0 } } }

#[derive(Component, Reflect, Debug, Default, Clone, Copy)]
pub struct TransformState { pub pos: glam::Vec3, pub rot_yaw_pitch_roll: glam::Vec3 }

pub fn sys_apply_commands_to_controller(
    mut q: Query<(&mut DragonController, &mut TransformState)>,
    mut ev: MessageReader<Command>, sim: Res<crate::SimTime>) {
    let dt = sim.dt;
    for (mut ctrl, mut tf) in q.iter_mut() {
        for e in ev.read() { match *e {
            Command::MoveAxes { x, y } => { let yaw = ctrl.yaw; let fwd = glam::Vec3::new(yaw.sin(),0.0,-yaw.cos()); let right = glam::Vec3::new(fwd.z,0.0,-fwd.x); tf.pos += (fwd*y*ctrl.speed_fwd + right*x*ctrl.speed_strafe)*dt; }
            Command::LookDelta { dx, dy } => { ctrl.yaw += dx*0.002; ctrl.pitch=(ctrl.pitch+dy*0.002).clamp(-1.2,1.2); tf.rot_yaw_pitch_roll = glam::vec3(ctrl.yaw, ctrl.pitch, 0.0); }
            Command::Ascend(a)=>{ tf.pos.y += a*ctrl.speed_up*dt; }
            Command::Descend(a)=>{ tf.pos.y -= a*ctrl.speed_up*dt; }
            _ => {}
        } }
    }
}
```

----- /Users/christopherdavid/code/ruinsofatlantis/crates/roa_domain/src/input.rs -----
```rust
//! Input command events for the domain.
use bevy_ecs::prelude::*; use bevy_reflect::Reflect;
#[derive(Message, Debug, Clone, Reflect)]
pub enum Command { MoveAxes{ x:f32, y:f32 }, LookDelta{ dx:f32, dy:f32 }, Ascend(f32), Descend(f32), Takeoff, Land, AttackPrimary }
```

----- /Users/christopherdavid/code/ruinsofatlantis/crates/roa_domain/src/sim_time.rs -----
```rust
//! Simple simulation time resource for fixed-step logic.
use bevy_ecs::prelude::*; use bevy_time::Time;
#[derive(Resource, Debug, Clone, Copy)] pub struct SimTime { pub tick:u64, pub dt:f32 }
impl Default for SimTime { fn default()->Self{ Self{ tick:0, dt:1.0/60.0 } } }
pub fn tick_sim_time(mut sim: ResMut<SimTime>, time: Res<Time>) { sim.tick = sim.tick.saturating_add(1); sim.dt = time.delta_secs(); }
```

----- /Users/christopherdavid/code/ruinsofatlantis/docs/dragon-proto-workflow.md -----
```markdown
# DragonProto Workflow — Programmatic Dragon Builds (Blender → GLB → Bevy)
(See full file in repo; excerpt)
- Headless Blender build: geometry → rig → skin → Actions; push Actions to NLA
- Export GLB: selection-only, no cams/lights, animations baked & force-sampled
- Verify GLB has skins/animations; integrate under assets/models; run slice
```
