"""
Headless Blender export to GLB with embedded textures and one animation per NLA strip.

Usage (from repository root):

  BLENDER=/Applications/Blender.app/Contents/MacOS/Blender \
  "$BLENDER" -b /path/to/input.blend --python scripts/blender/export_glb_clean.py -- \
    --in /path/to/input.blend \
    --out assets/models/red_wyvern/RedDragon2021.textured.glb \
    --strip-cams --strip-lights --strip-empties \
    --pack --push-actions

Notes
- This script does NOT bake UDIMs; it exports whatever images are referenced. Use --pack
  to ensure images are embedded into GLB when possible.
- It prefers existing Armature NLA strips. If none exist and --push-actions is set, it
  will push Actions that target pose bones to the armature NLA (one strip per action).
"""

import bpy
import sys
import os
import argparse


def parse_args():
    argv = sys.argv
    if "--" in argv:
        argv = argv[argv.index("--") + 1 :]
    else:
        argv = []
    p = argparse.ArgumentParser()
    p.add_argument("--in", dest="infile", required=True)
    p.add_argument("--out", dest="outfile", required=True)
    p.add_argument("--prefer-armature", dest="arm_hint", default="")
    p.add_argument("--strip-cams", action="store_true")
    p.add_argument("--strip-lights", action="store_true")
    p.add_argument("--strip-empties", action="store_true")
    p.add_argument("--apply-modifiers", action="store_true")
    p.add_argument("--triangulate", action="store_true")
    p.add_argument("--pack", action="store_true")
    p.add_argument("--push-actions", action="store_true")
    return p.parse_args(argv)


def open_input(path: str):
    ext = os.path.splitext(path)[1].lower()
    if ext == ".blend":
        bpy.ops.wm.open_mainfile(filepath=path)
    elif ext == ".fbx":
        bpy.ops.import_scene.fbx(filepath=path, automatic_bone_orientation=True)
    elif ext in (".gltf", ".glb"):
        bpy.ops.import_scene.gltf(filepath=path)
    else:
        raise RuntimeError(f"Unsupported input format: {ext}")


def visible_meshes():
    return [o for o in bpy.data.objects if o.type == "MESH" and o.visible_get()]


def find_armature(meshes, hint: str):
    cand = None
    hint_l = hint.lower() if hint else ""
    # prefer parent armature or armature modifiers
    for m in meshes:
        if m.parent and m.parent.type == "ARMATURE":
            if hint_l and hint_l in m.parent.name.lower():
                return m.parent
            cand = cand or m.parent
        for mod in m.modifiers:
            if getattr(mod, "type", None) == "ARMATURE" and getattr(mod, "object", None):
                a = mod.object
                if a and a.type == "ARMATURE":
                    if hint_l and hint_l in a.name.lower():
                        return a
                    cand = cand or a
    # fallback: first armature in scene
    if not cand:
        cand = next((o for o in bpy.data.objects if o.type == "ARMATURE"), None)
    return cand


def strip_objects(args):
    to_delete = []
    for ob in list(bpy.data.objects):
        if args.strip_cams and ob.type == "CAMERA":
            to_delete.append(ob)
        if args.strip_lights and ob.type == "LIGHT":
            to_delete.append(ob)
        if args.strip_empties and ob.type == "EMPTY":
            to_delete.append(ob)
    if to_delete:
        bpy.ops.object.select_all(action="DESELECT")
        for ob in to_delete:
            try:
                ob.select_set(True)
            except Exception:
                pass
        try:
            bpy.ops.object.delete()
        except Exception:
            pass


def select_export_set(arm, meshes):
    bpy.ops.object.select_all(action="DESELECT")
    if arm:
        bpy.context.view_layer.objects.active = arm
        arm.select_set(True)
    for m in meshes:
        try:
            m.select_set(True)
        except Exception:
            pass


def normal_images_to_noncolor():
    for img in bpy.data.images:
        name_l = img.name.lower()
        if any(k in name_l for k in ("normal", "_nrm", "-n", "_n")):
            try:
                img.colorspace_settings.name = "Non-Color"
            except Exception:
                pass


def push_actions_to_nla(arm: bpy.types.Object):
    arm.animation_data_create()
    nla = arm.animation_data.nla_tracks
    # if NLA already has strips, leave them (exporter will use them)
    has_strips = any(t.strips for t in nla)
    if has_strips:
        return
    # build set of actions that target pose bones
    def is_pose_action(act: bpy.types.Action) -> bool:
        for fc in act.fcurves:
            if fc.data_path.startswith("pose.bones["):
                return True
        return False

    pose_actions = [a for a in bpy.data.actions if is_pose_action(a)]
    if not pose_actions:
        return
    for act in pose_actions:
        tr = nla.new()
        tr.name = act.name
        start = int(act.frame_range[0])
        try:
            strip = tr.strips.new(act.name, start, act)
            strip.name = act.name
        except Exception:
            # continue on bad actions
            try:
                nla.remove(tr)
            except Exception:
                pass


def main():
    args = parse_args()
    # Read in file
    open_input(args.infile)

    # Optionally strip objects
    strip_objects(args)

    # Collect export set
    meshes = visible_meshes()
    arm = find_armature(meshes, args.arm_hint)
    if not arm:
        raise RuntimeError("No Armature found for export")

    # Selection for export
    select_export_set(arm, meshes)

    # Minor material hygiene
    normal_images_to_noncolor()
    if args.pack:
        try:
            bpy.ops.file.pack_all()
        except Exception:
            pass

    # Ensure NLA strips exist (if requested)
    if args.push_actions:
        push_actions_to_nla(arm)

    # GLB export
    os.makedirs(os.path.dirname(args.outfile), exist_ok=True)
    bpy.ops.export_scene.gltf(
        filepath=args.outfile,
        export_format='GLB',
        use_selection=True,
        export_yup=True,
        export_texcoords=True,
        export_normals=True,
        export_tangents=False,
        export_materials='EXPORT',
        export_image_format='AUTO',
        export_animations=True,
        export_skins=True,
        export_morph=True,
        export_nla_strips=True,
        export_force_sampling=True,
        export_cameras=False,
        export_lights=False,
    )
    print(f"Exported GLB: {args.outfile}")


if __name__ == "__main__":
    main()
