//! three.js's compute nodes (docs/r3f.feature R3F22): <R3F>'s gl.compute dispatches as a <Compute kernels shared runs>
//! (sdk/r3f's tsl-compute.ts). Each kernel is three's own WGSL, compiled as written: its `@group(0)` storage buffers the
//! shared ones by key (shared.rs: the node materials reading them bind the same buffers), its
//! uniforms packed from each dispatch's values by the struct three declares (raea-sys-shader uniform_slots), its entry
//! `main`. Three keyed columns follow the frame: `kernels` ({id: {wgsl, buffers: {NAME: KEY}}}) and `shared` ({KEY: {size,
//! f32 | u32 | i32}}) change when a new kernel is dispatched, `runs` every frame: {epoch, seq, fresh, runs: [[kernel, [x,
//! y, z], {UNIFORM: {field: value}}]]}, the dispatches since the frame before.
//!
//! A frame's runs are dispatched once (their (epoch, seq) is remembered), in order, in the frame's compute pass before
//! the cameras draw; `fresh` (a three session's first frame) writes the buffers back to their first contents first. A
//! composition with a compute steps every frame from 0 (raea-runtime-plugins steps(): a compute isn't frame-pure), so
//! a still at frame F runs every frame's dispatches before it, and a seek back starts a new App. A run whose pipeline
//! or buffer isn't on the GPU yet is held for the next frame, with every run after it, never dropped.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use bevy::prelude::*;
use bevy::render::storage::ShaderBuffer;
use raea_sys_shader::shared::{first_contents, SharedBuffers};
use serde_json::Value;

use crate::written::{decls, Decl};

/// A kernel, built: its shader, declarations, uniforms as declared, source and each storage buffer's key by name.
#[derive(Debug)]
pub struct Kernel {
    pub shader: u128,
    pub decls: Vec<Decl>,
    declared: Vec<(String, String)>,
    src: String,
    buffers: BTreeMap<String, String>,
}

/// One dispatch, packed: the kernel's shader and declarations, its workgroups, each uniform's bytes and each storage
/// buffer's shared buffer, by name.
#[derive(Clone, Debug)]
pub struct Run {
    pub kernel: Arc<Kernel>,
    pub workgroups: [u32; 3],
    pub uniforms: BTreeMap<String, Vec<u8>>,
    pub buffers: BTreeMap<String, AssetId<ShaderBuffer>>,
}

/// A <Compute kernels>: its kernels by id, the columns last followed, the (epoch, seq) last run, and this frame's runs.
#[derive(Clone, Debug, Default)]
pub struct Three {
    pub kernels: HashMap<u64, Arc<Kernel>>,
    given: (String, String),
    pub seen: Option<(i64, i64)>,
    pub runs: Vec<Run>,
}

/// The entry point every kernel has (three's WGSLNodeBuilder writes `fn main`).
pub const ENTRY: &str = "main";

/// A kernel from its spec ({wgsl, buffers: {NAME: KEY}}): every storage buffer it declares has a key, it binds no
/// storage texture, and its entry is `main`.
fn kernel(world: &mut World, v: &Value) -> Result<Kernel, String> {
    let src = v.get("wgsl").and_then(Value::as_str).ok_or("a kernel is {wgsl, buffers}")?;
    let d = decls(src)?;
    let keys: BTreeMap<String, String> = v.get("buffers").and_then(Value::as_object).into_iter().flatten().filter_map(|(k, x)| Some((k.clone(), x.as_str()?.to_string()))).collect();
    for x in &d {
        match x {
            Decl::Buffer { name, .. } if !keys.contains_key(name) => return Err(format!("its storage buffer {name} has no key")),
            Decl::Storage { name, .. } => return Err(format!("{name}: a kernel binds storage buffers and uniforms, not storage textures")),
            _ => {}
        }
    }
    if !src.contains(&format!("fn {ENTRY}(")) && !src.contains(&format!("fn {ENTRY} (")) {
        return Err(format!("a kernel's entry is fn {ENTRY}"));
    }
    let declared = raea_sys_shader::bindings::uniforms(src)?;
    let shader = raea_sys_shader::register(world, src.to_string(), &[], "raea three compute", "raea_sys_compute");
    Ok(Kernel { shader, decls: d, declared, src: src.to_string(), buffers: keys })
}

/// The `kernels` and `shared` columns, when they've changed: new kernels built, new shared buffers made.
fn follow_kernels(world: &mut World, t: &mut Three, kernels: &str, shared: &str) -> Result<(), String> {
    if t.given.0 != kernels {
        let all: serde_json::Map<String, Value> = serde_json::from_str(kernels).map_err(|e| format!("kernels: {e}"))?;
        for (id, v) in &all {
            let id: u64 = id.parse().map_err(|_| format!("a kernel's id is a number, got {id}"))?;
            if t.kernels.get(&id).is_some_and(|k| Some(k.src.as_str()) == v.get("wgsl").and_then(Value::as_str)) {
                continue;
            }
            let k = kernel(world, v).map_err(|e| format!("kernel {id}: {e}"))?;
            t.kernels.insert(id, Arc::new(k));
        }
        t.given.0 = kernels.to_string();
    }
    if t.given.1 != shared {
        let all: serde_json::Map<String, Value> = serde_json::from_str(shared).map_err(|e| format!("shared: {e}"))?;
        world.resource_scope::<SharedBuffers, _>(|world, mut s| -> Result<(), String> {
            let mut assets = world.resource_mut::<Assets<ShaderBuffer>>();
            for (key, v) in &all {
                s.get_or_make(&mut assets, key, first_contents(v).map_err(|e| format!("shared {key}: {e}"))?)?;
            }
            Ok(())
        })?;
        t.given.1 = shared.to_string();
    }
    Ok(())
}

/// This frame's `runs` packed, when its (epoch, seq) hasn't been run; a fresh one writes the buffers back first.
fn follow_runs(world: &mut World, t: &mut Three, runs: &str) -> Result<(), String> {
    t.runs.clear();
    if runs.is_empty() {
        return Ok(());
    }
    let v: Value = serde_json::from_str(runs).map_err(|e| format!("runs: {e}"))?;
    let (Some(epoch), Some(seq)) = (v.get("epoch").and_then(Value::as_i64), v.get("seq").and_then(Value::as_i64)) else {
        return Err("runs: its epoch and seq are numbers".into());
    };
    let at = (epoch, seq);
    if t.seen == Some(at) {
        return Ok(());
    }
    t.seen = Some(at);
    if v.get("fresh").and_then(Value::as_bool) == Some(true) {
        let keys: Vec<String> = t.kernels.values().flat_map(|k| k.buffers.values().cloned()).collect();
        world.resource_scope::<SharedBuffers, _>(|world, s| s.reset(&mut world.resource_mut::<Assets<ShaderBuffer>>(), keys.iter().map(String::as_str)));
    }
    let shared = world.resource::<SharedBuffers>();
    for (i, r) in v.get("runs").and_then(Value::as_array).into_iter().flatten().enumerate() {
        let id = r.get(0).and_then(Value::as_u64).ok_or_else(|| format!("run {i}: [kernel, workgroups, uniforms]"))?;
        let k = t.kernels.get(&id).ok_or_else(|| format!("run {i}: kernel {id} isn't one it has"))?.clone();
        let workgroups = crate::groups_of(r.get(1)).ok_or_else(|| format!("run {i}: workgroups are [x, y, z], each 1 to 65535"))?;
        let given = r.get(2).and_then(Value::as_object).cloned().unwrap_or_default();
        let uniforms = raea_sys_shader::uniform_bytes(&k.src, &k.declared, &given, &[]).map_err(|e| format!("run {i}: {e}"))?;
        let mut buffers = BTreeMap::new();
        for (name, key) in &k.buffers {
            buffers.insert(name.clone(), shared.get(key).ok_or_else(|| format!("run {i}: shared buffer {key} isn't made (the shared column names it)"))?.id());
        }
        t.runs.push(Run { kernel: k, workgroups, uniforms, buffers });
    }
    Ok(())
}

/// Each <Compute kernels>'s columns this frame (raea-sys-frame's keyed strings) followed: its kernels and shared
/// buffers, and the runs to dispatch this frame (none when this frame's were run).
pub fn follow(world: &mut World) {
    // last frame's runs were extracted and taken: none until this frame's are followed
    let Some(mut c) = world.get_resource_mut::<crate::Computes>() else { return };
    if !c.list.iter().any(|k| k.three.is_some()) {
        return;
    }
    c.list.iter_mut().filter_map(|k| k.three.as_mut()).for_each(|t| t.runs.clear());
    let cols: Vec<(usize, [String; 3])> = {
        let Some(frame) = world.get_resource::<raea_sys_frame::FrameRows>() else { return };
        let Some(c) = world.get_resource::<crate::Computes>() else { return };
        c.list.iter().enumerate().filter(|(_, k)| k.three.is_some()).filter_map(|(i, k)| {
            let r = frame.cols.id.iter().position(|x| *x == k.id)?;
            Some((i, ["kernels", "shared", "runs"].map(|col| crate::column(frame, r, col))))
        }).collect()
    };
    if cols.is_empty() {
        return;
    }
    world.init_resource::<SharedBuffers>();
    for (i, [kernels, shared, runs]) in cols {
        let (id, mut t) = {
            let mut c = world.resource_mut::<crate::Computes>();
            let k = &mut c.list[i];
            (k.id.clone(), k.three.take().unwrap_or_default())
        };
        if let Err(e) = follow_kernels(world, &mut t, &kernels, &shared).and_then(|_| follow_runs(world, &mut t, &runs)) {
            error!("Compute {id}: {e}");
            t.runs.clear();
        }
        world.resource_mut::<crate::Computes>().list[i].three = Some(t);
    }
}

/// A <Compute kernels>'s spec ({three: true}): its kernels and runs arrive in its keyed columns (follow).
pub fn build(id: &str) -> crate::Compute {
    crate::Compute { shader: 0, workgroups: [1, 1, 1], slots: [Vec4::ZERO; raea_sys_material::shader_material::SLOTS], buffers: vec![], images: vec![], written: None, three: Some(Three::default()), id: id.to_string(), names: vec![], given: String::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frames_runs_are_packed_once_by_epoch_and_seq() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default())).init_asset::<Shader>().init_asset::<ShaderBuffer>().init_resource::<SharedBuffers>();
        let w = app.world_mut();
        let src = "struct B { value: array<vec4<f32>> };\n@group(0) @binding(0)\nvar<storage, read_write> NodeBuffer_1 : B;\nstruct O { nodeUniform1 : f32, nodeUniform2 : u32 };\n@group(0) @binding(1)\nvar<uniform> object : O;\n@compute @workgroup_size(64)\nfn main(@builtin(global_invocation_id) g: vec3<u32>) { if (g.x >= object.nodeUniform2) { return; } NodeBuffer_1.value[g.x].x += object.nodeUniform1; }\n";
        let kernels = serde_json::json!({ "7": { "wgsl": src, "buffers": { "NodeBuffer_1": "tsl:0" } } }).to_string();
        let shared = serde_json::json!({ "tsl:0": { "size": 64 } }).to_string();
        let runs = serde_json::json!({ "epoch": 0, "seq": 0, "fresh": true, "runs": [[7, [1, 1, 1], { "object": { "nodeUniform1": 0.5, "nodeUniform2": 4 } }]] }).to_string();
        let mut t = Three::default();
        follow_kernels(w, &mut t, &kernels, &shared).unwrap();
        follow_runs(w, &mut t, &runs).unwrap();
        assert_eq!(t.runs.len(), 1);
        let u = &t.runs[0].uniforms["object"];
        assert_eq!((f32::from_ne_bytes(u[0..4].try_into().unwrap()), u32::from_ne_bytes(u[4..8].try_into().unwrap())), (0.5, 4));
        assert_eq!(t.runs[0].buffers["NodeBuffer_1"], w.resource::<SharedBuffers>().get("tsl:0").unwrap().id());
        // the same frame drawn again runs nothing; a kernel with a storage buffer no key names is refused
        follow_runs(w, &mut t, &runs).unwrap();
        assert!(t.runs.is_empty());
        let bad = serde_json::json!({ "8": { "wgsl": src, "buffers": {} } }).to_string();
        assert!(follow_kernels(w, &mut t, &bad, &shared).unwrap_err().contains("no key"));
    }
}
