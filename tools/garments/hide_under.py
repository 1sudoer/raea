# The body with the skin wholly under cloth left out, so it can't poke through as it moves; and each garment with what
# lies wholly under the garments worn over it left out too (GARMENT.glb... innermost first, each rewritten in place):
# chars/teal's tee came through his open jacket in patches, both draped on the bare body (11-16); now the tee shows
# only where the jacket leaves it uncovered, its open front and below its hem.
#   raea blender run hide_under.py BODY.glb OUT.glb GARMENT.glb...
# A body vertex is under cloth when a ray out along its normal meets a garment within REACH (6 cm); a face goes when
# its vertices and all their neighbours are under cloth (the edge of the cut stays, so no gap shows at a hem). A
# garment's vertex is under the garments outside it when a ray either way along its normal meets one within REACH
# (cloth's normals are not all outward); NO_LAYERS=1 leaves garments whole. FOLLOW=NAME,... names the garments the
# cloth follower carries (raea sdk/body/tools/worn.ts): their cloth below the crotch swings off what it lies on at rest,
# so it hides nothing (11-17: a greatcoat's skirt swung clear of the trousers it had hidden at rest, and the holes left
# in them showed the coat's inside, ragged at the cut faces' edges).
import bpy, bmesh, sys, os
from mathutils import Vector
from mathutils.bvhtree import BVHTree

a = sys.argv[sys.argv.index("--") + 1:]
body_glb, out, garments = a[0], a[1], a[2:]
REACH = float(os.environ.get("REACH", "0.06"))
CLOSE, LATERAL = 0.03, 0.002                                       # a garment under another: within 3 cm, straight under its face
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=body_glb)
skin = max((o for o in bpy.data.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
keep = set(bpy.data.objects)
FOLLOW = {n for n in os.environ.get("FOLLOW", "").split(",") if n}
# the crotch, as the follower's garment reader takes it (sdk/body/tools/garment.ts readWorn: the thighs' joints, 8 cm down)
arm = next((o for o in bpy.data.objects if o.type == "ARMATURE"), None)
legs = [arm.matrix_world @ arm.data.bones[b].head_local for b in ("LeftLeg", "RightLeg", "thigh_l", "thigh_r") if arm and b in arm.data.bones]
CROTCH = min(p.z for p in legs) - 0.08 if legs else None
if FOLLOW and CROTCH is None: sys.exit("hide_under: FOLLOW given but the body has no thigh joints to find its crotch by")


def cover(into, o, path):
    """a garment's mesh added to what hides (in the world), less a followed garment's cloth below the crotch"""
    t = bmesh.new(); t.from_object(o, bpy.context.evaluated_depsgraph_get()); t.transform(o.matrix_world)
    if os.path.splitext(os.path.basename(path))[0] in FOLLOW:
        bmesh.ops.delete(t, geom=[f for f in t.faces if all(v.co.z < CROTCH for v in f.verts)], context="FACES")
    m = bpy.data.meshes.new("t"); t.to_mesh(m); t.free(); into.from_mesh(m)


dg = bpy.context.evaluated_depsgraph_get()
cb = bmesh.new()
for gpath in garments:
    before = set(bpy.data.objects); bpy.ops.import_scene.gltf(filepath=gpath)
    for o in set(bpy.data.objects) - before:
        if o.type == "MESH":
            cover(cb, o, gpath)
bvh = BVHTree.FromBMesh(cb)
W = skin.matrix_world; N = W.to_3x3()
bm = bmesh.new(); bm.from_mesh(skin.data); bm.verts.ensure_lookup_table()
under = [False] * len(bm.verts)
for v in bm.verts:
    p = W @ v.co; nrm = (N @ v.normal).normalized()
    hit = bvh.ray_cast(p + nrm * 0.0005, nrm, REACH)
    under[v.index] = hit[0] is not None
deep = [under[v.index] and all(under[e.other_vert(v).index] for e in v.link_edges) for v in bm.verts]
gone = [f for f in bm.faces if all(deep[v.index] for v in f.verts)]
total = len(bm.faces)
bmesh.ops.delete(bm, geom=gone, context="FACES")
bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
bm.to_mesh(skin.data); bm.free()
for o in bpy.data.objects:
    o.select_set(o in keep and o.type in ("MESH", "ARMATURE"))
bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", use_selection=True, export_animations=False, export_yup=True, export_skins=True)
print(f"hide_under: {len(gone)} of {total} faces under cloth left out: {out}")


# the garments, each under the ones worn over it (innermost first: the last has nothing over it)
def load_mesh(path):
    before = set(bpy.data.objects); bpy.ops.import_scene.gltf(filepath=path)
    return [o for o in set(bpy.data.objects) - before]


if os.environ.get("NO_LAYERS") != "1":
    for k, gpath in enumerate(garments[:-1]):
        bpy.ops.wm.read_factory_settings(use_empty=True)
        mine = load_mesh(gpath)
        cloth = max((o for o in mine if o.type == "MESH"), key=lambda o: len(o.data.vertices))
        ob = bmesh.new()
        for opath in garments[k + 1:]:
            for o in load_mesh(opath):
                if o.type == "MESH":
                    cover(ob, o, opath)
                    bpy.data.objects.remove(o)
                elif o.type != "ARMATURE":
                    bpy.data.objects.remove(o)
        tree = BVHTree.FromBMesh(ob)
        W = cloth.matrix_world; N = W.to_3x3()
        bm = bmesh.new(); bm.from_mesh(cloth.data); bm.verts.ensure_lookup_table()
        under = [False] * len(bm.verts)
        for v in bm.verts:
            p = W @ v.co; nrm = (N @ v.normal).normalized()
            # met along its normal, or lying close (within CLOSE) under or over a garment over it: layered cloth all
            # but touches, and at a shoulder the normal points up past a sleeve beside it (11-16: 3,129 of chars/teal's
            # tee's vertices within 2 cm of his jacket, 1,667 met along their normals). Under or over its face, not
            # past its edge: the nearest point on it is straight across (off to the side by under LATERAL), where past
            # an open front, a hem or a cuff it is the edge, off to the side. (A distance from the edges kept a band of
            # tee along the jacket's open front that came through it in white flecks, at 8 mm as at 1.5 cm)
            hit = any(tree.ray_cast(p + d * nrm * 0.0005, d * nrm, REACH)[0] is not None for d in (1, -1))
            if not hit:
                q, nq, _, _ = tree.find_nearest(p, CLOSE)
                if q is not None:
                    off = p - q
                    hit = (off - off.dot(nq) * nq).length < LATERAL
            under[v.index] = hit
        # (a face under the garment over it goes when its own corners are under: kept to the skin's rule, all their
        # neighbours too, a ring of tee was left round each hidden patch and came through the jacket in specks)
        gone = [f for f in bm.faces if all(under[v.index] for v in f.verts)]
        total = len(bm.faces)
        bmesh.ops.delete(bm, geom=gone, context="FACES")
        bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
        bm.to_mesh(cloth.data); bm.free()
        for o in bpy.data.objects:
            o.select_set(o in mine and o.type in ("MESH", "ARMATURE"))
        bpy.ops.export_scene.gltf(filepath=gpath, export_format="GLB", use_selection=True, export_animations=False, export_yup=True, export_skins=True)
        print(f"hide_under: {os.path.basename(gpath)}: {len(gone)} of {total} faces under the garments over it left out")
