# The mannequin a worn hard piece is drawn on (docs/character.feature CH49): SOMA's body at rest, flat grey, one view
# without perspective from three-quarters ahead, the camera written beside it, so the dressed figure Idol makes from
# the picture can be laid back on the body.
#   raea blender run tools/garments/wear_mannequin.py BODY.glb OUT_DIR [AZ=30] [EL=6] [SIZE=1024] [KEY=0]
# KEY=1 draws the body flat magenta, for pieces that leave skin bare (a hat, a helmet, a breastplate alone): the image
# model is told to keep bare parts magenta, and wear_fit.py cuts what comes back that colour.
import json, math, os, sys
import bpy
from mathutils import Vector

a = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else sys.argv[-2:]
src, out = a[0], a[1]
az, el, size = (float(a[2]) if len(a) > 2 else 30.0), (float(a[3]) if len(a) > 3 else 6.0), (int(a[4]) if len(a) > 4 else 1024)
key = len(a) > 5 and a[5] == "1"
os.makedirs(out, exist_ok=True)
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=src)
# (the importer adds a 2 m sphere for its bone shapes: dropped, or the frame is set by it)
for o in [o for o in bpy.data.objects if o.type == "MESH" and o.name.startswith("Icosphere")]:
    bpy.data.objects.remove(o)
meshes = [o for o in bpy.data.objects if o.type == "MESH"]
sc = bpy.context.scene
sc.render.engine = "BLENDER_WORKBENCH"
sh = sc.display.shading
sh.light = "STUDIO"; sh.color_type = "SINGLE"; sh.single_color = (0.42, 0.42, 0.42); sh.show_cavity = True; sh.cavity_type = "BOTH"; sh.show_shadows = False
if key: sh.single_color = (0.85, 0.0, 0.85); sh.show_cavity = False
sc.world = bpy.data.worlds.new("w"); sc.world.color = (0.578, 0.578, 0.578)
sc.view_settings.view_transform = "Standard"
sc.render.resolution_x = sc.render.resolution_y = size
dg = bpy.context.evaluated_depsgraph_get()
pts = [o.matrix_world @ v.co for o in meshes for v in o.evaluated_get(dg).data.vertices]
lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
centre = (lo + hi) / 2
cam_d = bpy.data.cameras.new("c"); cam_d.type = "ORTHO"; cam_d.ortho_scale = (hi.z - lo.z) * 1.12
cam = bpy.data.objects.new("c", cam_d); sc.collection.objects.link(cam); sc.camera = cam
# Blender's frame: z up, the body faces -y. The camera sits ahead of the body, turned `az` degrees to its left, `el` up.
A, E = math.radians(az), math.radians(el)
d = Vector((math.sin(A) * math.cos(E), -math.cos(A) * math.cos(E), math.sin(E)))
cam.location = centre + d * 10
cam.rotation_euler = (-d).to_track_quat("-Z", "Y").to_euler()
sc.render.filepath = os.path.join(out, "mannequin.png")
bpy.ops.render.render(write_still=True)
# the camera in glTF's frame (y up, +z ahead of the body), as the creature flow writes its own
json.dump({"az": az, "el": el, "centre": [centre.x, centre.z, -centre.y], "ortho_scale": cam_d.ortho_scale, "size": size,
           "height": hi.z - lo.z, "frame": "glTF: y up, +z the body's ahead, x its left"}, open(os.path.join(out, "mannequin.cam.json"), "w"))
print("wrote", sc.render.filepath)
