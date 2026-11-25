# SOMA's template body (NVlabs/SOMA-X, Apache-2.0) as a raea body folder: body.glb skinned to flat joints under
# SOMA's names (the engine places each joint itself), and skeleton.json (rig "soma": the 78 named joints first,
# then the 32 twist joints as helpers; metres, Y up, facing +Z; skin samples per part). `raea clothes` makes the
# machine's copy once (in raea's models folder, soma/body) from SOMA-X's files beside it.
#   raea blender run tools/clothes/body.py SOMA_template_rig.usda SOMA_neutral.npz OUT_DIR
import sys, os, json, random
import bpy, numpy as np
from mathutils import Matrix, Vector

usd, npz, out = sys.argv[sys.argv.index("--") + 1:][:3]
os.makedirs(out, exist_ok=True)
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.wm.usd_import(filepath=usd)
src = next(o for o in bpy.data.objects if o.type == "ARMATURE")
keep = [o for o in bpy.data.objects if o.type == "MESH" and not o.name.endswith(("_lo", "_xlo")) and o.name != "c_fullBody_hi"]
for o in [o for o in bpy.data.objects if o.type == "MESH" and o not in keep]:
    bpy.data.objects.remove(o)

W = src.matrix_world
bones = [b for b in src.data.bones if b.name != "Root"]  # parents first; SOMA's Root is a fixed frame at the floor, not a joint
# the body's joints first (what a clip turns: raea's clips hold the first joints of a skeleton), then the head's
# ends, eyes, jaw and the fingers, then the twist joints
BODY = ["Hips", "Spine1", "Spine2", "Chest", "Neck1", "Neck2", "Head"] + [s + n for s in ("Left", "Right") for n in ("Shoulder", "Arm", "ForeArm", "Hand")] \
    + [s + n for s in ("Left", "Right") for n in ("Leg", "Shin", "Foot", "ToeBase")]
named = [b for b in bones if "Twist" not in b.name]
named = [next(b for b in named if b.name == n) for n in BODY] + [b for b in named if b.name not in BODY]
order = named + [b for b in bones if "Twist" in b.name]
idx = {b.name: i for i, b in enumerate(order)}
head = {b.name: W @ b.head_local for b in bones}  # Blender world, metres

# SHAPE="k:sigmas,k:sigmas": the body moved along SOMA's own shape model (SOMA_neutral.npz: component k by so many
# of its standard deviations; centimetres, Y up), for a person of another build. Each joint moves with the skin it
# carries: the mean of that skin's move, each vertex counted by its weight on the joint and by its nearness (a
# vertex 8 cm off counts 0.6 of one at the joint), so a limb narrowed round its bone leaves its joint where it was
# and a body made shorter takes its joints down with it (SOMA's own layer regresses them from the shaped mesh; this
# stands in for it; THE GAMBIT's compositions/people-body-build.tsx shows a shaped body beside the template, an arm
# raised). A joint carrying no skin (the eyes, the ends) moves as its parent does; the feet are kept on the floor.
SHAPE = [(int(a), float(b)) for a, b in (kv.split(":") for kv in os.environ.get("SHAPE", "").split(",") if kv)]
shape_d = None
if SHAPE:
    zz = np.load(npz)
    d = sum(c * float(np.sqrt(zz["eigenvalues"][k])) * zz["shapedirs"][k].reshape(-1, 3) for k, c in SHAPE) / 100.0
    shape_d = np.stack([d[:, 0], -d[:, 2], d[:, 1]], 1)                     # SOMA's axes (Y up, +Z ahead) to Blender's world
    sk0 = next(o for o in keep if o.name.startswith("c_skin") and len(o.data.vertices) == 18056)
    V0 = np.array([tuple(sk0.matrix_world @ v.co) for v in sk0.data.vertices])
    shape_d[:, 2] -= (V0[:, 2] + shape_d[:, 2]).min() - V0[:, 2].min()     # the soles where they were
    gname0 = {g.index: g.name for g in sk0.vertex_groups}
    by_joint = {}
    for v in sk0.data.vertices:
        for g in v.groups:
            if g.weight > 1e-3:
                by_joint.setdefault(gname0[g.group], []).append((v.index, g.weight))
    moved = {}
    for b in bones:                                                          # parents first
        # SOMA hangs a limb's skin on its twist joints, none on the elbow, knee, shoulder or hip joint itself: a joint
        # takes the skin of the two segments that meet at it (its own twist joints and its parent's). Without them the
        # woman's elbow (11-17) moved as her shoulder did, 3 cm off her shortened arm, and sleeves cut for the arm's
        # angle hung beside it.
        near = {b.name} | {c.name for c in b.children if "Twist" in c.name} | ({c.name for c in b.parent.children if "Twist" in c.name} if b.parent else set())
        acc = {}
        for n_ in near:
            for i, w_ in by_joint.get(n_, []):
                acc[i] = acc.get(i, 0.0) + w_
        vw = list(acc.items())
        if vw:
            ii = np.array([i for i, _ in vw]); w = np.array([w_ for _, w_ in vw])
            w = w * np.exp(-np.sum((V0[ii] - np.array(head[b.name])) ** 2, 1) / (2 * 0.08 ** 2))
            moved[b.name] = (shape_d[ii] * w[:, None]).sum(0) / max(w.sum(), 1e-12) if w.sum() > 1e-9 else None
        if moved.get(b.name) is None:
            moved[b.name] = moved.get(b.parent.name, np.zeros(3)) if b.parent else np.zeros(3)
    for n, m in moved.items():
        head[n] = head[n] + Vector(tuple(float(x) for x in m))
    print("shaped joints: Head moved", [round(float(x) * 100, 1) for x in moved["Head"]], "cm, LeftArm", [round(float(x) * 100, 1) for x in moved["LeftArm"]], "cm")


def R(v):  # Blender's world (Z up, facing -Y) to raea's (Y up, facing +Z)
    return [round(v[0], 5), round(v[2], 5), round(-v[1], 5)]


def tail_of(b):
    kids = [c for c in b.children if "Twist" not in c.name]
    if "Twist" in b.name:  # a twist joint ends at the next one along its segment, or at the segment's end
        sib = sorted([c for c in b.parent.children if "Twist" in c.name], key=lambda c: c.name)
        k = sib.index(b)
        if k + 1 < len(sib):
            return head[sib[k + 1].name]
        far = [c for c in b.parent.children if "Twist" not in c.name]
        return head[far[0].name] if far else head[b.name] + Vector((0, 0, 0.02))
    if kids:
        # a trunk joint ends at the next joint up; a hand at its middle finger
        pick = next((c for c in kids if c.name.endswith(("Spine1", "Spine2", "Chest", "Neck1", "Neck2", "Head", "HeadEnd", "Middle1"))), kids[0])
        return head[pick.name]
    d = head[b.name] - head[b.parent.name] if b.parent and b.parent.name in head else Vector((0, 0, 0.03))
    return head[b.name] + d.normalized() * 0.02


joints = [{"name": b.name, "parent": idx[b.parent.name] if b.parent and b.parent.name in idx else -1, "head": R(head[b.name]), "tail": R(tail_of(b)), **({"helper": True} if "Twist" in b.name else {})} for b in order]

# the flat rig: every joint a short bone pointing up, none a child of another
arm_data = bpy.data.armatures.new("soma")
rig = bpy.data.objects.new("soma", arm_data)
bpy.context.scene.collection.objects.link(rig)
bpy.context.view_layer.objects.active = rig
bpy.ops.object.mode_set(mode="EDIT")
for b in order:
    e = arm_data.edit_bones.new(b.name)
    e.head = head[b.name]; e.tail = head[b.name] + Vector((0, 0, 0.02)); e.roll = 0
bpy.ops.object.mode_set(mode="OBJECT")

# (SHAPE, above: the skin moved by the shape, its joints already moved with it)
skin = None
for o in keep:
    me = o.data
    me.transform(o.matrix_world)
    if shape_d is not None and o.name.startswith("c_skin") and len(me.vertices) == 18056:
        for v, dv in zip(me.vertices, shape_d):
            v.co += Vector(tuple(float(x) for x in dv))
        print("shaped", SHAPE, "largest move mm", round(float(np.linalg.norm(shape_d, axis=1).max()) * 1000, 1))
    elif shape_d is not None:                                                # the eyes: with the head
        me.transform(Matrix.Translation(Vector(tuple(float(x) for x in moved["Head"]))))
    o.parent = None
    o.matrix_world = Matrix.Identity(4)
    for m in list(o.modifiers):
        o.modifiers.remove(m)
    o.modifiers.new("rig", "ARMATURE").object = rig
    o.parent = rig
    for p in me.polygons:
        p.use_smooth = True
    if o.name.startswith("c_skin"):
        skin = o

# CUT=height (metres): the body without its own head, for a head delivered as a part (docs/character.feature CH20):
# the skin is cut level at that height through the neck, and the eyes go. SKIN="r,g,b" (sRGB) is the skin's colour.
CUT = float(os.environ.get("CUT", "0")) or None
if CUT and shape_d is not None:                                              # the template's cut height, moved with the neck
    CUT = round(CUT + float(moved["Neck1"][2] + moved["Neck2"][2]) / 2, 4)
if CUT:
    import bmesh
    for o in [o for o in keep if o is not skin]:
        bpy.data.objects.remove(o)
    keep = [skin]
    bm = bmesh.new(); bm.from_mesh(skin.data)
    # a level plane through the neck: the skin is cut along it (new vertices on the plane, their weights between
    # their edge's ends), and everything above goes
    bmesh.ops.bisect_plane(bm, geom=bm.verts[:] + bm.edges[:] + bm.faces[:], plane_co=(0, 0, CUT), plane_no=(0, 0, 1), clear_outer=True, dist=1e-5)
    bm.to_mesh(skin.data); bm.free()
    print("cut at", CUT, "verts left", len(skin.data.vertices))
tone = [float(x) ** 2.2 for x in os.environ.get("SKIN", "0.81,0.80,0.79").split(",")]
mat = bpy.data.materials.new("skin"); mat.use_nodes = True
b = next(n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED")
b.inputs["Base Color"].default_value = (*tone, 1); b.inputs["Roughness"].default_value = 0.6
eye = bpy.data.materials.new("eye"); eye.use_nodes = True
next(n for n in eye.node_tree.nodes if n.type == "BSDF_PRINCIPLED").inputs["Base Color"].default_value = (0.05, 0.05, 0.06, 1)
for o in keep:
    o.data.materials.clear(); o.data.materials.append(mat if o is skin else eye)

# skin samples, by the part the vertex mostly follows
PART = {"Hips": "hips", "Spine1": "waist", "Spine2": "torso", "Chest": "chest", "Neck1": "neck", "Neck2": "neck", "Head": "head", "Jaw": "head",
        "Shoulder": "chest", "Arm": "upperArm", "ForeArm": "foreArm", "Hand": "palm", "Leg": "thigh", "Shin": "shin", "Foot": "foot", "ToeBase": "toes"}
COUNT = {"hips": 120, "torso": 120, "chest": 120, "waist": 94, "head": 80, "palm": 80, "neck": 30}


def part_of(name):
    side = "L" if name.startswith("Left") else "R" if name.startswith("Right") else ""
    n = name[len("Left"):] if side == "L" else name[len("Right"):] if side == "R" else name
    if "Twist" in n:
        n = n[:n.index("Twist")]
    if n.startswith("Hand") and n != "Hand":  # a finger: HandIndex1 is in the palm, 2 to 4 are its three phalanges
        f = n[4:]
        finger = next(x for x in ("Thumb", "Index", "Middle", "Ring", "Pinky") if f.startswith(x))
        k = f[len(finger):]
        if finger == "Thumb":
            return (f"thumb{k}" if k in ("1", "2", "3") else "thumb3"), side
        return ("palm" if k == "1" else f"{finger.lower()}{min(int(k) - 1, 3)}" if k.isdigit() else f"{finger.lower()}3"), side
    return PART.get(n, "chest"), side


# Each sample is written on the joints a pose turns. SOMA hangs skin on joints that no pose, clip or solver turns:
# the twist joints along a limb (all of an arm's and a leg's skin), the bone in the palm under each finger and the
# end joint past each fingertip. Left on those, a hand on a cup was 12 samples of palm with no forearm and its
# fingertips 2 cm short. The part still says where the skin is (a palm bone's is "palm").
above = {j["name"]: joints[j["parent"]]["name"] for j in joints if j["parent"] >= 0}
PALM = tuple(f"Hand{f}1" for f in ("Index", "Middle", "Ring", "Pinky"))


def carrier(n):
    while n in above and ("Twist" in n or n.endswith("End") or n.endswith(PALM)):
        n = above[n]
    return n


gname = {g.index: g.name for g in skin.vertex_groups}
by_part = {}
for v in skin.data.vertices:
    ws = sorted(((g.weight, gname[g.group]) for g in v.groups if gname[g.group] in idx), reverse=True)[:3]
    if not ws:
        continue
    tot = sum(w for w, _ in ws)
    part, side = part_of(ws[0][1])
    j0, on = carrier(ws[0][1]), {}
    for w, n in ws:
        on[carrier(n)] = on.get(carrier(n), 0) + w
    ws = [(on[j0], j0)] + sorted(((w, n) for n, w in on.items() if n != j0), reverse=True)
    by_part.setdefault((part, side), []).append({"part": part, "side": side, "j": [idx[n] for _, n in ws], "w": [round(w / tot, 4) for w, _ in ws],
                                                 "o": R(v.co - head[j0]), "n": R(v.normal)})
random.seed(7)
surface = []
for (part, side), vs in sorted(by_part.items()):
    n = COUNT.get(part, 30 if side else 60)
    if side and part in COUNT:
        n //= 2
    surface += random.sample(vs, min(n, len(vs)))

from pxr import Usd, UsdSkel
stage = Usd.Stage.Open(usd)
sk = next(UsdSkel.Skeleton(p) for p in stage.Traverse() if p.IsA(UsdSkel.Skeleton))
jn = [str(j).split("/")[-1] for j in sk.GetJointsAttr().Get()]
bind = {}
for n, m in zip(jn, sk.GetBindTransformsAttr().Get()):
    u, _, vt = np.linalg.svd(np.array(m).reshape(4, 4).T[:3, :3]); bind[n] = u @ vt
z = np.load(npz)
tp = dict(zip([str(x) for x in z["t_pose_world__v0.2.0__joint_names"]], z["t_pose_world__v0.2.0"].astype(float)))
tpose = {}
for n in idx:
    if n in tp:
        q = Matrix((tp[n] @ bind[n].T).tolist()).to_quaternion()
        tpose[n] = [round(q.x, 6), round(q.y, 6), round(q.z, 6), round(q.w, 6)]
zs = [v.co.z for v in skin.data.vertices]
eyes = (head["LeftEye"] + head["RightEye"]) / 2
skel = {"name": "soma", "rig": "soma", "body": {"source": "NVlabs/SOMA-X assets/SOMA_template_rig.usda, Apache-2.0"}, "height": round(max(zs) - min(zs), 4),
        "joints": joints, "landmarks": {"eyes": R(eyes), "sitDrop": 0.09}, "captured": len(BODY), "tpose": tpose, "surface": surface}
json.dump(skel, open(os.path.join(out, "skeleton.json"), "w"), separators=(",", ":"))
if CUT:
    # the open edge the cut left, in order round the neck, for the head's mesh to end on: each vertex's place, normal
    # and the joints that carry it
    import bmesh
    bm = bmesh.new(); bm.from_mesh(skin.data); bm.verts.ensure_lookup_table()
    edge = [e for e in bm.edges if e.is_boundary and all(abs(v.co.z - CUT) < 1e-4 for v in e.verts)]
    nxt = {}
    for e in edge:
        a_, b_ = e.verts
        nxt.setdefault(a_.index, []).append(b_.index); nxt.setdefault(b_.index, []).append(a_.index)
    ring = [edge[0].verts[0].index]
    while True:
        more = [v for v in nxt[ring[-1]] if v not in ring]
        if not more:
            break
        ring.append(more[0])
    gname2 = {g.index: g.name for g in skin.vertex_groups}
    def carried(i):
        ws = sorted(((g.weight, gname2[g.group]) for g in skin.data.vertices[i].groups if g.weight > 1e-4), reverse=True)[:4]
        t = sum(w for w, _ in ws)
        return {n: round(w / t, 4) for w, n in ws}
    json.dump({"units": "metres, Y up, +Z ahead", "cut": CUT, "closed": len(ring) == len(nxt),
               "ring": [{"at": R(bm.verts[i].co), "normal": R(bm.verts[i].normal), "weights": carried(i)} for i in ring]},
              open(os.path.join(out, "collar.json"), "w"), indent=1)
    print("collar ring", len(ring), "of", len(nxt), "edge vertices")
    bm.free()
print("clothes: SOMA's body, joints", len(joints), "named", len(named), "samples", len(surface), "height", skel["height"], "floor", round(min(zs), 4))

for o in bpy.data.objects:
    o.select_set(o is rig or o in keep)
bpy.data.objects.remove(src)
bpy.ops.export_scene.gltf(filepath=os.path.join(out, "body.glb"), export_format="GLB", use_selection=True, export_animations=False, export_yup=True, export_skins=True)
# joint.glb: the small stand-in <Character> puts at each joint for the skin's bones to follow (a body folder has one)
for o in bpy.data.objects:
    o.select_set(False)
bpy.ops.mesh.primitive_cube_add(size=0.01)
bpy.context.active_object.select_set(True)
bpy.ops.export_scene.gltf(filepath=os.path.join(out, "joint.glb"), export_format="GLB", use_selection=True, export_animations=False, export_yup=True)
print("saved", os.path.join(out, "body.glb"))
