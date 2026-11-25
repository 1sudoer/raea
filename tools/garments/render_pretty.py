# A worn garment rendered to look at: smooth shading, a fabric material, soft studio light, Cycles on the GPU.
#   raea blender run render_pretty.py BODY.glb OUT_DIR DIR...     (each DIR holds <name>.glb and <name>.bake.json)
import bpy, sys, os, glob, json, math
from mathutils import Vector

a = sys.argv[sys.argv.index("--") + 1:]
body_glb, out_dir, dirs = a[0], a[1], a[2:]
os.makedirs(out_dir, exist_ok=True)
bpy.ops.wm.read_factory_settings(use_empty=True)
sc = bpy.context.scene
for eng in ("BLENDER_EEVEE_NEXT", "BLENDER_EEVEE"):  # Cycles compiles Metal kernels silently for minutes on first use
    try: sc.render.engine = eng; break
    except TypeError: pass
ee = sc.eevee
for k, v in (("taa_render_samples", 64), ("use_shadows", True), ("use_raytracing", True), ("use_gtao", True), ("shadow_ray_count", 2)):
    if hasattr(ee, k): setattr(ee, k, v)
print("render: engine", sc.render.engine, flush=True)
sc.render.resolution_x, sc.render.resolution_y = 900, 1350
sc.view_settings.view_transform = "AgX"; sc.view_settings.look = "AgX - Medium High Contrast"; sc.view_settings.exposure = -0.6

bpy.ops.import_scene.gltf(filepath=body_glb)
for o in [o for o in bpy.data.objects if o.type == "MESH" and not any(m.type == "ARMATURE" for m in o.modifiers)]:
    bpy.data.objects.remove(o)
skin = max((o for o in bpy.data.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
sm = bpy.data.materials.new("mannequin"); sm.use_nodes = True
b = next(x for x in sm.node_tree.nodes if x.type == "BSDF_PRINCIPLED")
b.inputs["Base Color"].default_value = (0.5, 0.49, 0.47, 1); b.inputs["Roughness"].default_value = 0.45
skin.data.materials.clear(); skin.data.materials.append(sm)
for p in skin.data.polygons: p.use_smooth = True

w = bpy.data.worlds.new("w"); sc.world = w; w.use_nodes = True
w.node_tree.nodes["Background"].inputs[0].default_value = (0.9, 0.9, 0.88, 1); w.node_tree.nodes["Background"].inputs[1].default_value = 0.35
bpy.ops.mesh.primitive_plane_add(size=30); floor = bpy.context.object
fm = bpy.data.materials.new("floor"); fm.use_nodes = True
fb = next(x for x in fm.node_tree.nodes if x.type == "BSDF_PRINCIPLED"); fb.inputs["Base Color"].default_value = (0.82, 0.81, 0.79, 1); fb.inputs["Roughness"].default_value = 0.9
floor.data.materials.append(fm)
def light(name, loc, power, size):
    l = bpy.data.lights.new(name, "AREA"); l.energy = power; l.size = size
    o = bpy.data.objects.new(name, l); sc.collection.objects.link(o); o.location = loc
    o.rotation_euler = (Vector((0, 0, 1.0)) - Vector(loc)).to_track_quat("-Z", "Y").to_euler()
light("key", (-2.2, -2.6, 3.0), 900, 2.5); light("fill", (2.8, -1.8, 1.6), 260, 3.0); light("rim", (0.8, 3.0, 2.6), 500, 1.5)
cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam")); sc.collection.objects.link(cam); sc.camera = cam
cam.data.lens = 70; cam.location = (1.55, -4.6, 1.05)
cam.rotation_euler = (Vector((0, 0, 0.92)) - cam.location).to_track_quat("-Z", "Y").to_euler()

for d in dirs:
    g = glob.glob(os.path.join(d, "*.glb")); bj = glob.glob(os.path.join(d, "*.bake.json"))
    if not g or not bj: continue
    name = os.path.basename(g[0])[:-4]; colour = json.load(open(bj[0]))["colour"]
    before = set(bpy.data.objects); bpy.ops.import_scene.gltf(filepath=g[0]); new = set(bpy.data.objects) - before
    cloth = [o for o in new if o.type == "MESH"]
    rgb = [(int(colour[i:i + 2], 16) / 255) ** 2.2 for i in (1, 3, 5)]
    m = bpy.data.materials.new(name); m.use_nodes = True
    p = next(x for x in m.node_tree.nodes if x.type == "BSDF_PRINCIPLED")
    p.inputs["Base Color"].default_value = (*rgb, 1); p.inputs["Roughness"].default_value = 0.82
    for k, v in (("Sheen Weight", 0.6), ("Sheen Roughness", 0.4)):
        if k in p.inputs: p.inputs[k].default_value = v
    for o in cloth:
        o.data.materials.clear(); o.data.materials.append(m)
        for f in o.data.polygons: f.use_smooth = True
        s = o.modifiers.new("thick", "SOLIDIFY"); s.thickness = 0.0025; s.offset = 1
    sc.render.filepath = os.path.join(out_dir, name + ".png"); bpy.ops.render.render(write_still=True)
    print(f"render: {name}", flush=True)
    for o in new: bpy.data.objects.remove(o)
