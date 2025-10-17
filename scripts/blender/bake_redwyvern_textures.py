"""
Bake UDIM-style materials to single 0–1 albedo maps per material and export GLB.
Usage (invoked by scripts/bake_redwyvern_textured_glb.sh):
  blender -b <source.blend> --python this.py -- --size 4096 --out <path.glb>

Notes:
  - Expects Cycles; will switch renderer and enable baking.
  - For each mesh material slot, creates an Image node (RGBA) and bakes
    Combined->Color (albedo-only). For UDIMs, Blender will resolve tiles when
    baking from the original node graph.
  - Packs images and exports a GLB with embedded textures.
"""
import bpy, sys, argparse, os

def parse_args():
    argv = sys.argv
    if "--" in argv:
        argv = argv[argv.index("--") + 1 :]
    else:
        argv = []
    ap = argparse.ArgumentParser()
    ap.add_argument("--size", type=int, default=4096)
    ap.add_argument("--out", required=True)
    return ap.parse_args(argv)

def ensure_cycles():
    bpy.context.scene.render.engine = 'CYCLES'
    prefs = bpy.context.preferences.addons.get('cycles')
    bpy.context.scene.cycles.device = 'CPU'

def make_image(name, size):
    img = bpy.data.images.new(name=name, width=size, height=size, alpha=True, float_buffer=False)
    img.colorspace_settings.name = 'sRGB'
    return img

def bake_material_albedo(obj, mat, size):
    if mat is None: return
    if mat.use_nodes is False:
        mat.use_nodes = True
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    # Add an Image Texture node for bake target
    img = make_image(f"Bake_{mat.name}", size)
    img_node = nodes.new('ShaderNodeTexImage')
    img_node.image = img
    img_node.select = True
    nodes.active = img_node
    # Bake diffuse color only
    bpy.ops.object.select_all(action='DESELECT')
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    try:
        bpy.ops.object.mode_set(mode='OBJECT')
    except Exception:
        pass
    bpy.ops.object.bake(type='DIFFUSE', pass_filter={'COLOR'}, use_clear=True, margin=8)
    # Keep node for export (Image node plugged into baseColor by exporters)
    return img

def main():
    args = parse_args()
    ensure_cycles()
    # Select all mesh objects and bake per material slot
    for obj in bpy.data.objects:
        if obj.type != 'MESH':
            continue
        bpy.ops.object.select_all(action='DESELECT')
        bpy.context.view_layer.objects.active = obj
        obj.select_set(True)
        for slot in obj.material_slots:
            bake_material_albedo(obj, slot.material, args.size)
        obj.select_set(False)
    # Pack resources to embed images
    bpy.ops.file.pack_all()
    # Export GLB (embedded images)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    bpy.ops.export_scene.gltf(filepath=args.out, export_format='GLB', export_apply=True, export_texcoords=True, export_normals=True, export_tangents=False, export_materials='EXPORT', export_image_format='AUTO', export_yup=True)

if __name__ == "__main__":
    main()
