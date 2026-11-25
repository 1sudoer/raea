# Clothes laid on SOMA's own skin by procedure (raea docs/character.feature CH45b, CH45c), the making and wearing
# behind `raea clothes cover` and `raea clothes wear`. A garment is data on SOMA's own vertices, by their numbers in
# SOMA's template (so it rides any SOMA body): which are covered, how far the cloth stands off each, and its
# colour there. (Sewn garments are tools/garments; garments measured from drawn pictures were taken out on 11-15.)
#
#   cover: raea blender run tools/clothes/garment.py cover BODY.glb RIG.usda OUT.json PART[:SHARE],... #RRGGBB STAND_M [UNDER.json]
#     a garment by procedure: the skin of the parts named (each limb as far along as its share, from the end that joins the rest), in one colour, so
#     far off the skin (and a layer's thickness off UNDER).
#   wear: raea blender run tools/clothes/garment.py wear BODY.glb RIG.usda OUT_DIR GARMENT.json...   (innermost first)
#     writes OUT_DIR/<name>.glb for each garment (the covered skin pushed out, with the body's own weights) and
#     OUT_DIR/body.glb, the body with the skin that is wholly under cloth left out.
import sys, os, json
import bpy, bmesh, numpy as np
from mathutils import Vector, kdtree
from mathutils.bvhtree import BVHTree

args = sys.argv[sys.argv.index("--") + 1:]
# a hard piece (cover --hard [--sole MM] [--sole-colour #RRGGBB] [--panels #RRGGBB,...]): footwear, which keeps its own
# shape instead of the skin's (see make_hard)
FLAGS = {}
for k_ in ("--sole", "--sole-colour", "--panels"):
    if k_ in args:
        i_ = args.index(k_); FLAGS[k_] = args[i_ + 1]; del args[i_:i_ + 2]
if "--hard" in args:
    args.remove("--hard"); FLAGS["--hard"] = True
mode, body_glb, rig_usd = args[:3]
OFF, THICK = 0.004, 0.003  # least stand-off, a layer's own thickness (metres)


def canon():
    """SOMA's template vertices at rest, Blender's frame, metres: the numbers a garment is kept by"""
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.wm.usd_import(filepath=rig_usd)
    o = next(o for o in bpy.data.objects if o.type == "MESH" and o.name.startswith("c_skin_mid"))
    return np.array([o.matrix_world @ v.co for v in o.data.vertices])


def load_body():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=body_glb)
    for o in [o for o in bpy.data.objects if o.type == "MESH" and not any(m.type == "ARMATURE" for m in o.modifiers)]:
        bpy.data.objects.remove(o)
    return max((o for o in bpy.data.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))


C = canon()
tree = kdtree.KDTree(len(C))
for i, p in enumerate(C):
    tree.insert(Vector(p), i)
tree.balance()
# each of the body's vertices: its number in the template, found by its rest place. A body of another shape has
# its vertices elsewhere, so NEUTRAL names the template-shaped body it was built beside (same export, same order).
neutral = os.environ.get("NEUTRAL")
if neutral:
    keep_path, body_glb = body_glb, neutral
    ref = load_body()
    cid = np.array([tree.find(ref.matrix_world @ v.co)[1] for v in ref.data.vertices])
    body_glb = keep_path
skin = load_body()
me, W = skin.data, skin.matrix_world
co = np.array([W @ v.co for v in me.vertices]); no = np.array([(W.to_3x3() @ v.normal).normalized() for v in me.vertices])
n = len(co)
if neutral:
    assert len(cid) == n, f"the shaped body has {n} vertices, its neutral twin {len(cid)}"
else:
    cid = np.array([tree.find(Vector(p))[1] for p in co])
bmw = bmesh.new(); bmw.from_mesh(me); bmw.transform(W); bvh = BVHTree.FromBMesh(bmw); bmw.verts.ensure_lookup_table()
ring = [[e.other_vert(v).index for e in v.link_edges] for v in bmw.verts]
bmw.free()

def export(obj, path, also=()):
    for o in bpy.data.objects:
        o.select_set(o is obj or o in also or o.type == "ARMATURE")
    kw = dict(filepath=path, export_format="GLB", use_selection=True, export_animations=False, export_yup=True, export_skins=True)
    try:
        bpy.ops.export_scene.gltf(**kw, export_vertex_color="ACTIVE", export_all_vertex_colors=False)
    except TypeError:
        bpy.ops.export_scene.gltf(**kw)


def make_hard(g, covered, stand_v):
    """A hard piece (footwear) as its own mesh, not the skin's: each foot's covered skin, stood off as the garment says,
    closed in its convex hull (the toes and the foot's small forms go under it: a cover laid on the skin kept them, so
    shoes read as bare feet, chars/teal's sneakers and chars/wren's boots, 11-16), remeshed even (the foot's own
    triangles bunch at the toes, and a sole's line across them came out in stripes), its top over the leg opened as
    the collar, its corners rounded. Under it a sole, flat and set out, in its own colour up the side; a toe cap, a heel
    and a band in the panels' colours. Skinned like the skin it covers (the nearest covered vertices' weights)."""
    hard, sole = g["hard"], g["hard"]["sole_m"]
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    J = {b_.name: np.array(arm.matrix_world @ b_.head_local) for b_ in arm.data.bones}
    sk = bmesh.new(); sk.from_mesh(skin.data); sk.transform(W); skin_bvh = BVHTree.FromBMesh(sk); sk.free()
    base = tuple(float(x) for x in np.mean(g["colour_srgb"], axis=0))
    groups = [{gg.group: gg.weight for gg in v.groups} for v in me.vertices]
    out = bmesh.new(); uv = out.loops.layers.uv.new("UVMap"); pal = [base]; wts = []
    for side in ("Left", "Right"):
        foot, toe = J.get(side + "Foot"), J.get(side + "ToeBase")
        if foot is None or toe is None:
            continue
        other = J.get(("Right" if side == "Left" else "Left") + "Foot")
        idx = np.where(covered & (np.linalg.norm(co - foot, axis=1) < (np.linalg.norm(co - other, axis=1) if other is not None else 9)))[0]
        if len(idx) < 8:
            continue
        P = co[idx] + no[idx] * stand_v[idx, None]
        hb = bmesh.new()
        for p_ in P:
            hb.verts.new(Vector(p_))
        bmesh.ops.convex_hull(hb, input=list(hb.verts), use_existing_faces=False)
        bmesh.ops.delete(hb, geom=[v for v in hb.verts if not v.link_faces], context="VERTS")
        bmesh.ops.recalc_face_normals(hb, faces=list(hb.faces))
        tm = bpy.data.meshes.new("hull"); hb.to_mesh(tm); hb.free()
        to = bpy.data.objects.new("hull", tm); bpy.context.scene.collection.objects.link(to)
        rm = to.modifiers.new("even", "REMESH"); rm.mode = "VOXEL"; rm.voxel_size = 0.004
        em = bpy.data.meshes.new_from_object(to.evaluated_get(bpy.context.evaluated_depsgraph_get()))
        bpy.data.objects.remove(to); bpy.data.meshes.remove(tm)
        bm = bmesh.new(); bm.from_mesh(em); bpy.data.meshes.remove(em)
        bmesh.ops.triangulate(bm, faces=list(bm.faces))
        # the collar: the hull's top over the leg (faces facing up whose nearest skin is not covered) left open
        kd = kdtree.KDTree(n)
        for i in range(n):
            kd.insert(Vector(co[i]), i)
        kd.balance()
        bm.normal_update()
        cap = [f for f in bm.faces if f.normal.z > 0.2 and not covered[kd.find(f.calc_center_median())[1]]]
        bmesh.ops.delete(bm, geom=cap, context="FACES")
        bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
        held = {v for v in bm.verts if v.is_boundary}
        for it in range(40):                                            # its corners rounded (Taubin: no shrinking)
            lam = 0.5 if it % 2 == 0 else -0.53
            nxt = {v: v.co + (sum((e.other_vert(v).co for e in v.link_edges), Vector()) / len(v.link_edges) - v.co) * lam
                   for v in bm.verts if v not in held and v.link_edges}
            for v, c in nxt.items():
                v.co = c
        for v in bm.verts:                                              # never nearer the skin than 2 mm
            q, nq, _, _ = skin_bvh.find_nearest(v.co)
            if q is not None and (v.co - q).dot(nq) < 0.002:
                v.co = v.co + nq * (0.002 - (v.co - q).dot(nq))
        # the sole: the bottom laid flat a little below the bare foot (a thick sole all below it would sink the
        # person into the floor), set out a few millimetres, its colour a sole's thickness up the side
        z0 = float(np.percentile(co[idx, 2], 1))
        zb = z0 - min(sole, 0.008)
        ax = toe - foot; ax[2] = 0; ax /= np.linalg.norm(ax) + 1e-9
        mid = co[idx, :2].mean(0)
        for v in bm.verts:
            if v.co.z < zb + 0.006:
                v.co.z = zb
            if v.co.z < zb + 0.8 * sole:
                o_ = np.array(v.co[:2]) - mid; o_ /= np.linalg.norm(o_) + 1e-9
                v.co.x += o_[0] * 0.003; v.co.y += o_[1] * 0.003
        along = lambda p_: float((np.array(p_[:2]) - foot[:2]) @ ax[:2])
        al = [along(v.co) for v in bm.verts]; a0, a1 = min(al), max(al)
        tip = max(bm.verts, key=lambda v: along(v.co) - abs(v.co.z - (zb + sole + 0.02))).co.copy()
        heel = min(bm.verts, key=lambda v: along(v.co) + abs(v.co.z - (zb + sole + 0.03))).co.copy()
        pn = hard["panels_srgb"]
        cols = {}
        for f in bm.faces:
            c_ = f.calc_center_median(); h = c_.z - zb; t = (along(c_) - a0) / max(a1 - a0, 1e-6)
            if h < sole:
                cols[f] = hard["sole_srgb"]
            elif pn and (c_ - tip).length < 0.055:
                cols[f] = pn[0]                                         # the toe cap
            elif len(pn) > 1 and (c_ - heel).length < 0.05:
                cols[f] = pn[1]                                         # the heel
            elif len(pn) > 2 and 0.3 < t < 0.72 and abs(h - (sole + 0.012 + 0.05 * (t - 0.3))) < 0.009:
                cols[f] = pn[2]                                         # a band along the side, rising to the heel
        # into the one mesh, with each vertex's weights: its nearest covered skin's, by inverse distance
        ck = kdtree.KDTree(len(idx))
        for k_, i in enumerate(idx):
            ck.insert(Vector(co[i]), int(i))
        ck.balance()
        vmap = {}
        for v in bm.verts:
            nv = out.verts.new(v.co); vmap[v] = nv
            near = ck.find_n(v.co, 4); w = {}
            for _, i, d in near:
                for gi, gw in groups[i].items():
                    w[gi] = w.get(gi, 0.0) + gw / max(d, 1e-4)
            tot = sum(w.values()) or 1.0
            wts.append({gi: x / tot for gi, x in w.items()})
        out.verts.ensure_lookup_table()
        for f in bm.faces:
            nf = out.faces.new([vmap[v] for v in f.verts])
            c = tuple(cols.get(f, base))
            if c not in pal:
                pal.append(c)
            for l in nf.loops:
                l[uv].uv = ((pal.index(c) + 0.5) / 16, 0.5)
        bm.free()
    out.transform(W.inverted())
    mesh = bpy.data.meshes.new(g["name"]); out.to_mesh(mesh); out.free()
    for p_ in mesh.polygons:
        p_.use_smooth = True
    obj = bpy.data.objects.new(g["name"], mesh); bpy.context.scene.collection.objects.link(obj)
    obj.parent = skin.parent; obj.matrix_parent_inverse = skin.matrix_parent_inverse.copy(); obj.matrix_basis = skin.matrix_basis.copy()
    vg = [obj.vertex_groups.new(name=x.name) for x in skin.vertex_groups]
    for vi, w in enumerate(wts):
        for gi, x in w.items():
            if x > 1e-3:
                vg[gi].add([vi], x, "REPLACE")
    md = obj.modifiers.new("Armature", "ARMATURE"); md.object = arm
    return obj, pal


def wear(out_dir, garments):
    os.makedirs(out_dir, exist_ok=True)
    under = np.zeros(n, bool)
    for path in garments:
        g = json.load(open(path))
        stand = np.zeros(len(C)); col = np.zeros((len(C), 3)); has = np.zeros(len(C), bool)
        has[g["vertices"]] = True; stand[g["vertices"]] = g["stand"]; col[g["vertices"]] = g["colour_srgb"]
        covered = has[cid]
        if g.get("hard"):
            piece, pal = make_hard(g, covered, stand[cid])
        else:
            piece, pal = skin.copy(), None
            piece.data = skin.data.copy(); bpy.context.scene.collection.objects.link(piece)
        if pal is None:                                                 # (cut from the skin, stood off it)
            bm = bmesh.new(); bm.from_mesh(piece.data); bm.verts.ensure_lookup_table()
            # the body's own open edges (its UV seams, where glTF splits a vertex in two): kept where they are below
            seam = bm.verts.layers.int.new("seam")
            for v in bm.verts:
                v[seam] = int(v.is_boundary)
            layer = bm.loops.layers.color.new("cloth")
            for f in bm.faces:
                for l in f.loops:
                    l[layer] = (*[float(x) ** 2.2 for x in col[cid[l.vert.index]]], 1.0)
            for v in bm.verts:
                v.co += v.normal * float(stand[cid[v.index]])
            bmesh.ops.delete(bm, geom=[f for f in bm.faces if not all(covered[v.index] for v in f.verts)], context="FACES")
            bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
            # the hem: a garment's open edge runs along the body's triangles, a stair of them; each edge vertex is drawn
            # toward the middle of its two neighbours along the edge, a few times, which evens the stair into a line
            edge = [v for v in bm.verts if v.is_boundary and not v[seam]]
            for _ in range(4):
                to = {}
                for v in edge:
                    nb = [e.other_vert(v) for e in v.link_edges if e.is_boundary and not e.other_vert(v)[seam]]
                    if len(nb) == 2:
                        to[v] = v.co * 0.5 + (nb[0].co + nb[1].co) * 0.25
                for v, c in to.items():
                    v.co = c
            bm.to_mesh(piece.data); bm.free()
        # its colour: a picture in the body's UVs, baked from the views it was measured from (the garment's own UVs
        # are the body's, spread to fill that picture); a garment made by procedure has no views and is its one colour
        if "views" in g["source"]:
            sys.exit(f"clothes: {g['name']} was measured from drawn pictures, which raea no longer wears: sew it (raea clothes make) or lay it on the skin (raea clothes cover)")
        tex, lo, hi = np.tile(np.mean(g["colour_srgb"], axis=0).astype(np.float32), (4, 4, 1)), np.zeros(2), np.ones(2)
        if pal:                                                         # (its colours: a palette of 16 cells, make_hard's)
            tex = np.ones((4, 64, 3), np.float32)
            for k, c in enumerate(pal[:16]):
                tex[:, k * 4:(k + 1) * 4] = c
        uvl = piece.data.uv_layers[0].data
        for l in uvl:
            l.uv = ((l.uv[0] - lo[0]) / max(hi[0] - lo[0], 1e-9), (l.uv[1] - lo[1]) / max(hi[1] - lo[1], 1e-9))
        im = bpy.data.images.new(g["name"], tex.shape[1], tex.shape[0], alpha=False)
        im.pixels = np.concatenate([tex, np.ones((*tex.shape[:2], 1), np.float32)], axis=2).reshape(-1)  # (rows bottom up, as v runs)
        im.filepath_raw = os.path.join(out_dir, g["name"] + ".png"); im.file_format = "PNG"; im.save()
        mat = bpy.data.materials.new(g["name"]); mat.use_nodes = True
        bsdf = next(x for x in mat.node_tree.nodes if x.type == "BSDF_PRINCIPLED")
        a = mat.node_tree.nodes.new("ShaderNodeTexImage"); a.image = im
        mat.node_tree.links.new(a.outputs["Color"], bsdf.inputs["Base Color"]); bsdf.inputs["Roughness"].default_value = 0.85
        piece.data.materials.clear(); piece.data.materials.append(mat)
        export(piece, os.path.join(out_dir, g["name"] + ".glb"))
        bpy.data.objects.remove(piece)
        under |= covered
        print(f"clothes: wore {g['name']} over {int(covered.sum())} of the body's vertices: {os.path.join(out_dir, g['name'] + '.glb')}")
    # the skin wholly under cloth is left out: a vertex covered, with every neighbour covered too
    deep = under & np.array([all(under[j] for j in ring[i]) for i in range(n)])
    bm = bmesh.new(); bm.from_mesh(skin.data); bm.verts.ensure_lookup_table()
    gone = [f for f in bm.faces if all(deep[v.index] for v in f.verts)]
    total = len(bm.faces)
    bmesh.ops.delete(bm, geom=gone, context="FACES")
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
    bm.to_mesh(skin.data); bm.free()
    # with the body's other parts (a whole SOMA's eyes; a headless body has none)
    export(skin, os.path.join(out_dir, "body.glb"), [o for o in bpy.data.objects if o.type == "MESH" and o is not skin])
    print(f"clothes: the body without the skin wholly under cloth ({len(gone)} of {total} faces left out): {os.path.join(out_dir, 'body.glb')}")


# the most a garment measured from pictures stands off each part (metres; the trunk 8 cm)
STAND_CAP = {"upperArm": 0.03, "foreArm": 0.025, "hand": 0.01, "thigh": 0.04, "shin": 0.035, "foot": 0.015, "toes": 0.01, "neck": 0.03, "head": 0.03}
LIMB = {"Arm": ("upperArm", "ForeArm"), "ForeArm": ("foreArm", "Hand"), "Leg": ("thigh", "Shin"), "Shin": ("shin", "Foot"), "Foot": ("foot", "ToeBase")}
TRUNK = {"Hips": "hips", "Spine1": "waist", "Spine2": "torso", "Chest": "chest", "Shoulder": "chest", "Neck1": "neck", "Neck2": "neck", "ToeBase": "toes", "ToeEnd": "toes"}
PARTS = sorted(set(TRUNK.values()) | {v[0] for v in LIMB.values()} | {"hand", "head"})


def part_of(name):
    """a joint's part (hips, waist, torso, chest, neck, head, upperArm, foreArm, hand, thigh, shin, foot, toes) and,
    for a limb, the joint it starts at and the one it ends at"""
    side = "Left" if name.startswith("Left") else "Right" if name.startswith("Right") else ""
    j = name[len(side):]
    if "Twist" in j:  # a twist joint is its limb's
        j = j[:j.index("Twist")]
    if j.startswith("Hand"):
        return "hand", None
    if j in LIMB:
        return LIMB[j][0], (side + j, side + LIMB[j][1])
    return TRUNK.get(j, "head"), None


def cover(out, spec, colour, stand_m, under=None):
    """A garment by procedure, with no picture: the skin of the parts named, each limb as far along as its share
    says ("foreArm:0.5", the half nearest the elbow; "foot,shin:0.4", the shin's lower 40%), in one colour, standing off the skin by stand_m (and off the
    layer beneath by a layer's thickness)."""
    want = {}
    for s in spec.split(","):
        p, _, share = s.strip().partition(":")
        if p not in PARTS:
            raise SystemExit(f"clothes: no part {p!r}; the parts are {', '.join(PARTS)}")
        want[p] = float(share) if share else 1.0

    # a limb partly covered is covered from the end that joins the rest of the garment: a sleeve's forearm from the
    # elbow (upperArm,foreArm:0.5), a sock's or a boot's shin from the ankle (foot,shin:0.4). Counted from the top
    # alone, chars/teal's socks covered his knees and left his ankles bare (11-16)
    below = {"thigh": "shin", "shin": "foot", "upperArm": "foreArm", "foreArm": "hand"}
    above = {"shin": "thigh", "thigh": "hips", "foreArm": "upperArm", "upperArm": "chest"}
    def from_below(p):
        return below.get(p) in want and above.get(p) not in want
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    head = {b.name: np.array(arm.matrix_world @ b.head_local) for b in arm.data.bones}
    gname = {g.index: g.name for g in skin.vertex_groups}
    covered = np.zeros(n, bool)
    for i, v in enumerate(me.vertices):
        if not v.groups:
            continue
        top = gname[max(v.groups, key=lambda g: g.weight).group]
        p, limb = part_of(top)
        if p not in want:
            continue
        if limb and want[p] < 1.0 and limb[0] in head and limb[1] in head:
            a, b = head[limb[0]], head[limb[1]]
            t = float((co[i] - a) @ (b - a)) / max(float((b - a) @ (b - a)), 1e-9)
            if (t < 1.0 - want[p]) if from_below(p) else (t > want[p]):
                continue
        covered[i] = True
    for _ in range(2):  # the edge evened without wearing it away: a vertex changes only when two thirds of its neighbours differ
        k = np.array([sum(covered[j] for j in ring[i]) / max(len(ring[i]), 1) for i in range(n)])
        covered = np.where(k > 2 / 3, True, np.where(k < 1 / 3, False, covered))
    # a hem allowance: one ring past the parts' edge, so garments on neighbouring parts overlap instead of leaving a gap
    covered = covered | np.array([any(covered[j] for j in ring[i]) for i in range(n)])
    rgb = [int(colour.lstrip("#")[k:k + 2], 16) / 255 for k in (0, 2, 4)]
    ids = sorted(set(int(c) for c in cid[covered]))
    stand = np.full(len(ids), max(stand_m, OFF))
    if under and "stand" not in under:
        # a sewn garment (raea clothes make) isn't laid on the skin's vertices, so there's no stand to go over: it
        # crashed here (KeyError 'stand', 11-17, boots --over sewn trousers)
        raise SystemExit(f"cover: --over {under.get('name')}: it is sewn, not laid on the skin; make this one without "
                         "--over and list it before the sewn one in `raea clothes wear`, which layers them in that order")
    if under:
        u = np.zeros(len(C)); u[under["vertices"]] = under["stand"]
        stand = np.maximum(stand, u[ids] + THICK)
    g = {"name": os.path.splitext(os.path.basename(out))[0], "topology": "SOMA template, c_skin_mid, 18056 vertices", "units": "metres", "layer": (under["layer"] + 1) if under else 0,
         **({"over": under["name"]} if under else {}),
         "source": {"how": "raea clothes cover (tools/clothes/garment.py cover)", "parts": spec, "colour": colour},
         "vertices": ids, "stand": [round(float(x), 5) for x in stand], "colour_srgb": [[round(x, 4) for x in rgb]] * len(ids),
         "stray": [round(float(max(0.0, x - 0.012) * 0.6), 5) for x in stand]}
    if FLAGS.get("--hard"):
        srgb = lambda h: [round(int(h.lstrip("#")[k:k + 2], 16) / 255, 4) for k in (0, 2, 4)]
        g["hard"] = {"sole_m": float(FLAGS.get("--sole", "10")) / 1000, "sole_srgb": srgb(FLAGS.get("--sole-colour", "#e8e2d4")),
                     "panels_srgb": [srgb(h) for h in FLAGS["--panels"].split(",")] if FLAGS.get("--panels") else []}
        g["stray"] = [0.0] * len(ids)                                   # (a hard piece does not swing)
    json.dump(g, open(out, "w"), separators=(",", ":"))
    print(f"clothes: {g['name']}: layer {g['layer']}, {len(ids)} of {len(C)} template vertices covered ({spec}), {colour}, {stand.min() * 1000:.1f} to {stand.max() * 1000:.1f} mm off what is under it")


if mode == "cover":
    cover(args[3], args[4], args[5], float(args[6]), json.load(open(args[7])) if len(args) > 7 else None)
else:
    wear(args[3], args[4:])
