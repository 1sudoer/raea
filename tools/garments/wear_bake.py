# Worn hard pieces (wear_fit.py's npz) as a part for SOMA (docs/character.feature CH49): each vertex weighted wholly to
# the bone its face was given, on SOMA's own armature, coloured by Idol's vertex colours, exported as one glb with a
# picture from three-quarters ahead beside it.
#   raea blender run tools/garments/wear_bake.py SOMA_BODY.glb PIECES.npz OUT.glb [METAL=0.0]
import math, os, sys
import bpy, numpy as np
from mathutils import Vector

a = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else sys.argv[-3:]
body, npz, out = a[0], a[1], a[2]
metal = float(a[3]) if len(a) > 3 else 0.0
z = np.load(npz)
V, F, C, fb, names = z["V"], z["F"], z["C"], z["fbone"], [str(n) for n in z["names"]]
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=body)
arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
skin = [o for o in bpy.data.objects if o.type == "MESH"]
# the pieces: split per face's bone (a vertex shared by two bones' faces is doubled, so each piece is rigid on its own)
verts, faces, cols, vbone = [], [], [], []
for b in np.unique(fb):
    fs = F[fb == b]; used = np.unique(fs); remap = {int(u): i + len(verts) for i, u in enumerate(used)}
    verts += [(float(V[u, 0]), float(-V[u, 2]), float(V[u, 1])) for u in used]      # glTF (y up) to Blender (z up)
    cols += [tuple(C[u]) for u in used]; vbone += [int(b)] * len(used)
    faces += [tuple(remap[int(i)] for i in f) for f in fs]
me = bpy.data.meshes.new("worn"); me.from_pydata(verts, [], faces); me.update()
for p in me.polygons: p.use_smooth = True
ca = me.color_attributes.new("Col", "FLOAT_COLOR", "POINT")
# (Idol's colours are display values: made linear here, as a FLOAT_COLOR attribute holds linear light)
lin = lambda x: x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4
for i, c in enumerate(cols): ca.data[i].color = (lin(c[0]), lin(c[1]), lin(c[2]), 1.0)
ob = bpy.data.objects.new("worn", me); bpy.context.scene.collection.objects.link(ob)
groups = {}
for i, b in enumerate(vbone):
    n = names[b]
    g = groups.get(n) or ob.vertex_groups.new(name=n); groups[n] = g
    g.add([i], 1.0, "REPLACE")
ob.parent = arm
mod = ob.modifiers.new("arm", "ARMATURE"); mod.object = arm
mat = bpy.data.materials.new("worn"); mat.use_nodes = True
bsdf = next(n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED")
attr = mat.node_tree.nodes.new("ShaderNodeVertexColor"); attr.layer_name = "Col"
mat.node_tree.links.new(attr.outputs["Color"], bsdf.inputs["Base Color"])
bsdf.inputs["Metallic"].default_value = metal; bsdf.inputs["Roughness"].default_value = 0.35 if metal > 0 else 0.6
me.materials.append(mat)
# a picture: the pieces over the grey body, from three-quarters ahead
sc = bpy.context.scene
sc.render.engine = "BLENDER_WORKBENCH"
sh = sc.display.shading; sh.light = "STUDIO"; sh.color_type = "VERTEX"; sh.show_cavity = True
for o in skin:
    o.color = (0.35, 0.35, 0.35, 1)
sc.world = bpy.data.worlds.new("w"); sc.world.color = (0.578, 0.578, 0.578)
sc.view_settings.view_transform = "Standard"; sc.render.resolution_x = sc.render.resolution_y = 1024
cam_d = bpy.data.cameras.new("c"); cam_d.type = "ORTHO"; cam_d.ortho_scale = 2.05
cam = bpy.data.objects.new("c", cam_d); sc.collection.objects.link(cam); sc.camera = cam
A, E = math.radians(30), math.radians(6)
d = Vector((math.sin(A) * math.cos(E), -math.cos(A) * math.cos(E), math.sin(E)))
cam.location = Vector((0, 0, 0.9)) + d * 10; cam.rotation_euler = (-d).to_track_quat("-Z", "Y").to_euler()
sc.render.filepath = os.path.splitext(out)[0] + ".png"
bpy.ops.render.render(write_still=True)
# the glb: the armature and the pieces only
for o in skin: bpy.data.objects.remove(o)
for o in [o for o in bpy.data.objects if o.type == "MESH" and o.name.startswith("Icosphere")]: bpy.data.objects.remove(o)
bpy.ops.object.select_all(action="DESELECT"); arm.select_set(True); ob.select_set(True)
bpy.ops.export_scene.gltf(filepath=out, use_selection=True, export_skins=True, export_apply=False)
print(f"wear: {out}: {len(verts)} vertices, {len(faces)} faces on {len(groups)} bones ({', '.join(sorted(groups))})")
