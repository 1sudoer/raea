# raea Light: what real-time renderers do, and what raea should take from them

Research for [realism.feature](realism.feature) (LT2-LT14), written 2026-11-17. The target is this Mac's M5 Pro on
Metal, through wgpu 30 and Bevy 0.20. Windows and Linux RTX and RDNA cards come second. raea draws live games and
offline films with the same renderer, and gambit's 1917 Berlin street is the main scene.

How to read it. A cost is given in milliseconds, with the GPU and resolution its source states. "No figure" means the
source gives none. "(inference)" marks a conclusion of this document rather than of a source. Links sit on the
claims they support, and claims about the repo cite a path and line.

## (a) The ten findings that matter most for raea

1. **Every shipped system gathers light in three stages. D6 has the right shape but skips the first stage.** A ray
   is marched in screen space first. If that misses, it is traced in hardware against a simpler scene. It ends in a
   world-space radiance cache. Lumen works this way ([SIGGRAPH 2022][lumen22]), and so do AMD's GI-1.0
   ([paper][gi10]), Snowdrop in Avatar ([DF][snowdrop-df]), Assassin's Creed Shadows ([DF][acs-df]) and Tiny Glade
   ([DF][tinyglade-df]). D6's two levels, a world cache and screen probes, match this. But no step from LT2 to LT14
   has a screen trace. It is the cheapest ray, and the only one that sees every material exactly as raster drew it.

2. **Drop the surface cache (D5, LT4).** Unreal 5.9's Lumen World Cache replaces Lumen's cards in the hardware-ray
   path. It is a sparse world-space hash grid that path tracing fills. It handles skinned, WPO (world position
   offset: vertices the material moves) and small meshes, gives more bounces and uses much less memory
   ([Epic][worldcache]). GI-1.0, NVIDIA's SHaRC and Bevy's Solari use hash-grid caches too ([GI-1.0][gi10],
   [SHaRC][sharc], [Solari 0.19][solari19]). Cards need a raster capture of every mesh from up to six sides, through
   every MaterialExtension (D4, D5). Epic has stopped paying that cost in its hardware path.

3. **raea already has a working hash-grid world cache in its tree, and LT13 would delete it.**
   - The cache lives in `vendor/bevy_solari/src/realtime/world_cache_{update,query,compact}.wesl`. It holds 2^20
     cells of 84 bytes each, about 88 MB (`realtime/prepare.rs:46-48`). Cells grow with view distance, queries are
     jittered, and unused cells expire.
   - A stochastic update of 40k cells took 0.47 ms in Bistro, against 2.65 ms to update every cell
     ([Solari 0.19][solari19]; the GPU isn't named).
   - Solari's GPU-driven TLAS setup (`scene/binder/setup_tlas_instances.wesl`) and its light tiles
     (`realtime/presample_light_tiles.wesl`) are in the tree too.
   - "No Solari" (D2) should mean no Solari *renderer*. LT3, LT6 and LT13 should take these parts into raea's GI, not
     write them again.

4. **Nested octahedral probe grids (D6, LT3) are the older and leakier choice.**
   - DDGI keeps a depth map per probe and still can't stop light leaking through zero-thickness walls
     ([RTXGI][rtxgi]).
   - A hash grid puts its cells on the surfaces that rays actually hit. Each cell can also be split by surface
     direction: Lumen's World Cache uses six normal bins ([Epic][worldcache]).
   - Solari stops leaks by using its finest cell whenever a hit is closer than a cell's size
     ([Solari 0.19][solari19]).
   - Berlin's facades are thin shells around rooms, which is exactly the case that leaks (inference).

5. **The ray scene must be much coarser than the raster scene.**
   - Nanite traces fallback meshes, not its own geometry ([walkthrough][nanite-walk]). Snowdrop traces a
     lower-detail BVH with averaged materials ([GDC 2024][snowdrop-gdc]). Tiny Glade traces a coarse proxy
     ([DF][tinyglade-df]).
   - Lumen asks for fewer than 100k TLAS instances on consoles, because the TLAS is rebuilt every frame
     ([Lumen performance guide][lumen-perf]).
   - An early Battlefield V scene had 20,200 instances and 5,000 BLAS rebuilds, and took 64 ms ([GTC 2019][bfv]).
   - systems/rt, by contrast, puts every visible Bevy mesh into the TLAS, with one BLAS per mesh asset
     (`systems/rt/src/lib.rs:80-112`).

6. **On Metal, wgpu's ray queries take Apple's slower path, and wgpu can't refit.**
   - naga 30 emits `intersection_query` (`naga-30.0.1/src/back/msl/ray.rs:23`). Apple says to prefer the
     `intersector` API: intersection queries use more scratch memory and turn the reorder stage off
     ([Apple tech talk][apple-rt-m3]).
   - wgpu 30 has `ALLOW_UPDATE`, but documents it as "currently unimplemented and will build as normal"
     ([wgpu][wgpu-rt-types]).
   - Metal itself can refit an acceleration structure ([Apple][mtl-refit]) and store per-primitive data in it
     ([Apple][mtl-rt]).
   - So the two or three hottest trace kernels should be raea's own MSL, through passthrough, the way docs/gpu.md
     already runs MetalFX.

7. **systems/rt rebuilds its whole ray scene every frame, and its instances carry no data.**
   - It calls `create_tlas` anew each frame (`lib.rs:202`) and gives every instance custom data 0 (`lib.rs:210`).
   - It builds each BLAS with `PREFER_FAST_TRACE` and doesn't compact it (`lib.rs:160`).
   - Every build is its own submit (`lib.rs:187`, `lib.rs:214`).
   - Compaction alone saves about half of acceleration-structure memory on NVIDIA ([RTXMU][rtxmu]). It took
     Indiana Jones's dynamic BLASes from 1027 MB to 606 MB ([NVIDIA][ij-omm]).
   - M5 cuts acceleration-structure alignment from 16 KB to 1 KB, so many small BLASes now cost little
     ([Apple M5 talk][apple-rt-m5]).

8. **The Mac has no ray reconstruction, so raea's GI has to need little denoising.**
   - With path tracing on, Cyberpunk on a Mac with MetalFX looks "pretty messy" next to DLSS Ray Reconstruction
     ([Digital Foundry][cp-df]).
   - Solari 0.20 turned ReSTIR off by default, because DLSS-RR 4.5 cleans up without it. It has no denoiser on Metal
     ([Solari 0.20][solari20]).
   - An SVGF denoiser for diffuse and specular costs 4.0-4.23 ms at 1080p on an M5 Pro in Chrome's WebGPU
     ([pmndrs][pmndrs]). That is most of D13's 5 ms.
   - Lumen filters between its probes instead. A 3x3 filter in probe space removes as much noise as a 48x48 filter in
     screen space ([SIGGRAPH 2022][lumen22]).
   - MetalFX's denoised upscaler ([Apple][mfx-denoised]) should be the last stage, not a stand-in for caches that are
     already low in noise.

9. **Keep a short-range AO. LT10 as written is wrong.**
   - Lumen runs a full-resolution ShortRangeAO at every tier, its 60 fps High tier included, because its screen probes
     are downsampled and miss contact detail ([UE 5.2 notes][ue52], [performance guide][lumen-perf56]).
   - Epic measured the screen-space version at 0.12 ms, against 1.75 ms traced in hardware ([UE 5.2 notes][ue52]; no
     GPU named).
   - Metro Exodus Enhanced Edition dropped SSAO, but its GI rays are cast per pixel ([DF][metro-df]). raea's screen
     probes, one every 16 pixels (D6), are not.

10. **The frame budget is spent before GI starts.**
    - FLOOR3's raster frame already takes 18.0 ms at 1080p on this Mac (realism.feature, LT1's table), over D13's
      16.7 ms.
    - Lumen's 60 fps tier spends 2.8-4.3 ms on GI and reflections on PS5, at 1/16 of a ray per pixel
      ([SIGGRAPH 2022][lumen22]). Fortnite fits them into an average of 4 ms ([Epic][fortnite]).
    - With ray tracing on, an M4 Max trails an RTX 4060 a little ([Digital Foundry][cp-df]). The M5 Pro is up to 35%
      faster at ray tracing than the M4 Pro ([Apple][m5pro]), and the M4 Pro ran Cyberpunk with RT at 57% of the M4
      Max's frame rate ([TweakTown][cp-tt]). That puts the M5 Pro at roughly three quarters of an M4 Max in ray
      tracing (inference).
    - D13's 5 ms GI budget holds only under three conditions: Lumen-High ray density, GI traced below the internal
      resolution, and MetalFX upscaling. It also needs the raster frame cut to about half of FLOOR3's 18.0 ms. Speed
      (LT12) has to be designed from LT2 on.

**Is Lumen-style the wrong choice for an M-series Mac?** Lumen's software half is: mesh distance fields, the global
SDF and the cards. Epic deprecated software detail tracing in 5.7 ([Tom Looman][ue57]). Its developers call software
Lumen "hopefully a thing of the past" ([DF on the Witcher 4 demo][witcher4]). Lumen's hardware half, as Unreal 5.9
ships it, is the right choice: screen traces, hardware rays and the hash-grid World Cache.
- Apple GPUs trace in hardware from the M3 on ([Apple][apple-rt-m3]).
- The M5 adds hardware instance transforms ([Apple][apple-rt-m5]).
- A design with no cards and no SDFs also suits a 16 GB unified-memory Mac (inference).

## (b) Techniques

### Global illumination

#### Lumen (Unreal Engine 5)

**What it is.**
- **Tracing order.** Lumen traces in screen space first, then with software rays (mesh SDFs and a global SDF) or
  hardware rays, and falls back to the sky ([technical details][lumen-tech]).
- **Surface cache.** A hit takes its light from a surface cache: up to 12 cards per mesh, captured by raster and lit
  by radiosity ([technical details][lumen-tech]).
- **Final gather.** Rays start from screen probes at 1/16 resolution in each dimension. A world radiance cache at
  about 1/256 resolution backs them up, and contact AO runs at full resolution ([SIGGRAPH 2022][lumen22]).
- **Probe placement.** Probes sit on pixels and subdivide where geometry is detailed. Their placement is jittered
  and accumulated over time ([SIGGRAPH 2022][lumen22]).
- **Latency by distance.** Each range of the gather tolerates a different delay: contact AO (16 pixels) gets none,
  screen probes cover 2-16 m, the world cache reaches to 200 m, and the sky tolerates the most
  ([SIGGRAPH 2022][lumen22]).

**Cost.** On PS5 ([SIGGRAPH 2022][lumen22]):

| Scene | Tracing | High, 1/16 ray a pixel | Epic, 1/4 ray a pixel |
|---|---|---|---|
| Lumen in the Land of Nanite | software | 2.77 ms: surface cache 0.47, final gather 2.07, reflections 0.23 | 4.63 ms |
| Lyra | software | 4.29 ms, of which reflections 2.28 | 8.70 ms |
| The Matrix Awakens | hardware, with far field | 6.38 ms: surface cache 1.03, final gather 3.05, reflections 2.30 | 11.31 ms |

- **Consoles at Epic scalability.** Epic gives about 8 ms for GI and reflections at 1080p internal, and aims the
  High tier at 60 fps ([technical details][lumen-tech]).
- **Fortnite Chapter 4.** It fits GI and reflections into an average of 4 ms at 60 fps, at about 55-60% of 4K
  internal resolution ([Epic][fortnite]).
- **Unreal 5.6.** Hardware tracing came down to software's budget ([5.6 notes][ue56]):
  - short-range AO moved to half resolution and runs twice as fast;
  - surface cache updates were halved;
  - downsampled integration is about 3x faster, saving 0.3-0.5 ms at 1080p on console.
- **Unreal 5.7.** It turns half-resolution integration on at High ([Tom Looman][ue57]).
- **Scalability settings** ([5.6 performance guide][lumen-perf56]):
  - probe spacing is 32 px at High, 16 at Epic and 8 at Cinematic;
  - each probe traces an 8x8 octahedron;
  - 300 radiance-cache probes are updated a frame at High and at Epic.

**Quality and failure modes.**
- **Ghosting.** The temporal filter rejects history by depth and normal, not by neighbourhood clamping. That makes it
  stable but slow to react, so it ghosts behind moving objects. Epic speeds up accumulation where traces hit fast
  movers ([SIGGRAPH 2022][lumen22]).
- **Screen-edge darkening.** Screen traces darken corners in a way that moves with the screen's edge. Users call it
  the "demon shadow" ([forum][lumen-forum]).
- **Foliage.** The surface cache projects badly onto it ([UE 5.2 notes][ue52]).
- **Instance overlap.** Overlapping instances slow hardware tracing, and the TLAS is rebuilt every frame
  ([performance guide][lumen-perf]).

**Where it shipped.**
- Fortnite ([Epic][fortnite]).
- The Matrix Awakens ([SIGGRAPH 2022][lumen22]).
- The Witcher 4 demo, which ran hardware Lumen at 60 fps on PS5 ([DF][witcher4]).

#### Lumen World Cache (Unreal Engine 5.9)

**What it is.** A sparse world-space hash grid for hardware ray tracing. Path tracing fills it, and it replaces the
surface cache entirely ([Epic][worldcache]).
- **Cells.** About 25.6 cm at 10 m from the camera, growing with distance (CellDensity 20). Each cell has six
  normal-direction bins. The grid holds at most 2^20 entries.
- **Per frame.** One path per 8x8-pixel tile, of up to three bounces. Each path takes one light sample chosen by
  energy, through the same culling MegaLights uses. The last bounce reads last frame's cache.
- **Lifetime.** A cell accumulates over 16 frames and is evicted after 64 frames unused.
- **Scheduling.** It runs on async compute.

**Cost.** The docs give no figure.

**Quality and failure modes.** The docs list these ([Epic][worldcache]):
- **Gains.** It handles skeletal, WPO, spline and small meshes. It gives more bounces and near-instant light after a
  camera cut, costs nothing per instance on the CPU, and uses much less memory than cards.
- **Lag.** It trails changes in the scene.
- **Unsupported.** Translucency, water, foliage and the far field.
- **Material evaluation at hits.** It needs Full or Simplified material mode, not Inline.

**Where it shipped.** In the engine from 5.9. No shipped game was found.

#### AMD GI-1.0

**What it is.**
- **Screen probes.** One 8x8 octahedral probe per 8x8-pixel tile. A quarter of the probes are recomputed each frame,
  and the rest are upscaled in time.
- **World cache.** A hash-grid cache sits behind the probes.
- **Refinements.** Ray guiding, parallax-corrected reuse, and reprojection in local shared memory.

Sources: [paper][gi10], [GDC 2023][gi10-gdc].

**Cost.**
- 1.93-3.12 ms in total on an RX 6900 XT at 1080p ([paper][gi10]).
- In Gas Station, 1.932 ms in total, of which ray tracing is only 0.235 ms.
- Kitchen takes 3.5 ms and Sponza 4.2 ms, with direct light included.
- Inside Unreal it took 2.4 ms with hardware traversal, against software Lumen's 2.6 ms.

**Quality and failure modes.** The extracts read for this note don't cover them.

**Where it shipped.** It is an AMD research release. No shipped game was found.

**Why it matters to raea.** Most of its time is shading and filtering, not traversal. This is the closest public
design to what D6 wants (inference).

#### AMD FidelityFX Brixelizer GI

**What it is.** Sparse SDF bricks built in compute. The radiance cache is a 256^3 R11G11B10 atlas fed from last
frame's lit image, and screen probes feed spherical harmonics per brick ([Brixelizer][brix], [GDC 2024][brix-gdc],
[manual][brix-manual]).

**Cost.** No figure was found.

**Quality and failure modes.** Off-screen light depends on the scene, because the cache is fed from what the screen
showed. Tiny geometry over-shadows ([manual][brix-manual]).

**Where it shipped.** AMD's SDK. No game was found.

**For raea.** It needs no hardware rays, and every raea target has them. It is a fallback idea at most (inference).

#### DDGI and RTXGI

**What it is.** A grid of probes. Each probe traces rays every frame and stores irradiance and depth (mean and mean
squared) on octahedral maps. A Chebyshev test against the depth limits leaks ([JCGT 2019][ddgi]).

**Cost.**
- 2.6 ms in total on an RTX 2080 Ti at 1080p: ray cast 0.8 ms, shade 0.4, probe update 0.7, sample 0.5. Glossy
  reflections add 2.4 ms. The grid was 32x8x32 probes at 64 rays each ([JCGT 2019][ddgi]).
- NVIDIA quotes 1-2 ms on a 2080 Ti and 5-20 MB of memory ([RTXGI][rtxgi]).

**Quality and failure modes.** It can't stop leaks through walls of zero thickness, it lags on low-end GPUs, and it
needs a separate solution for glossy reflections ([RTXGI][rtxgi]).

**Where it shipped.**
- Metro Exodus Enhanced Edition, for the second bounce onward ([4A][metro-4a]).
- Assassin's Creed Shadows, as "DDGI-like probe cascades" for later bounces ([DF][acs-df]).

#### NVIDIA SHaRC

**What it is.** A spatial-hash radiance cache. An update pass traces one path per 5x5-pixel block, about 4% of
pixels, and writes the results into the hash. A resolve pass follows, and later paths end early by reading the
cache ([SHaRC guide][sharc]).

**Cost.**
- The guide gives no figure.
- In Indiana Jones on an RTX 5080, SharcUpdate took 1.99 ms, and 0.89 ms with opacity micromaps ([NVIDIA][ij-omm]).

**Quality and failure modes.** The guide's limits weren't extracted for this note.

**Where it shipped.** The path-tracing mode of Indiana Jones and the Great Circle ([NVIDIA][ij-omm]).

#### Neural Radiance Cache (NRC)

**What it is.** A small neural network (an MLP), trained online on path-traced samples and queried at a path's
second vertex ([paper][nrc]).

**Cost.**
- In the paper's tests, queries took 1.59 ms and training 1.11 ms on average, against DDGI's 0.58 ms and 1.37 ms.
  DDGI traced about 1M update rays, while NRC trained on 65k records ([paper][nrc]).
- On a mobile Xclipse GPU at 480x270, it took about 4 ms to infer and 4 ms to train ([mobile NRC][nrc-mobile]).
- No figure exists for Apple GPUs. The M5 Pro has Neural Accelerators in each GPU core ([Apple][m5pro]), so a small
  network may become cheap there (inference; untested).

**Quality and failure modes.** The extracts read for this note don't cover them.

**Where it shipped.** AMD's FSR Redstone includes "Radiance Caching" as a 0.9 preview ([AMD][redstone]).

#### Solari (Bevy)

**What it is.**
- Bevy's experimental hardware-ray renderer: ReSTIR for direct and indirect light, a hash-grid world cache, and light
  tiles.
- In 0.20, ReSTIR runs as one unified pass and is off by default, since DLSS-RR 4.5 makes it unneeded. It traces 6-10
  rays per pixel, builds its TLAS on the GPU with a compute copy, and keeps the previous frame's TLAS to fix shadow
  lag ([Solari 0.20][solari20]).
- It runs on Metal without a denoiser ([Bevy 0.20][bevy020]).

**Cost.**
- Updating every world-cache cell took 2.65 ms in Bistro. Updating 40k cells stochastically took 0.47 ms
  ([Solari 0.19][solari19]; no GPU named).
- On this Mac, FLOOR3 at 1080p took 47.6 ms with Solari, against 18.0 ms in raster (realism.feature, M11).
- If Solari traced 6-10 rays per pixel there, its extra 30 ms bought 12-21M rays. That is 0.4-0.7 billion rays per
  second, shading and resampling included (inference). It is an upper bound on the cost of a ray here, and LT2's
  measurement replaces it.

**Quality and failure modes.**
- It lands 0.96 stops from the reference, almost all of it a single bias a stop dark (M11).
- It skips skinned and morphed meshes (RT5) and ignores point and rect lights (M8).
- RT4 measured MetalFX's denoised upscaler on Solari at 1280x720 on this Mac, by temporal second difference: a still
  camera went from 3.16 to 0.022 and an orbiting one from 4.29 to 0.47. Forward-drawn meshes, which have nothing in
  the G-buffer, smeared under it (docs/raytracing.feature, RT4).

**Where it shipped.** Bevy, as an experimental feature.

#### ReSTIR GI and its cheaper variants

**What it is.** Indirect-light samples are resampled from reservoirs shared across neighbouring pixels and across
frames.

**Cost.**
- World-space ReSTIR adds about 0.8-2.5 ms on an RX 6900 XT and an RTX 3080 ([AMD][wsrestir]).
- ReSTIR PT Enhanced runs 2.3x faster than the 2022 ReSTIR PT ([ACM][restirpt]).

**Quality and failure modes.**
- Tiny Glade skipped ReSTIR because its memory traffic is too heavy for weak GPUs ([DF][tinyglade-df],
  [GPC 2024][tinyglade-gpc]).
- Solari 0.20 turned it off by default once DLSS-RR could clean the image instead ([Solari 0.20][solari20]). On a Mac,
  with no DLSS-RR, the case for it is stronger, but it pays the same bandwidth cost (inference).

**Where it shipped.** Solari. No game-specific source was read.

#### Radiance Cascades

**What it is.** Cascades of probes, each finer in angle and coarser in space as distance grows ([paper][rc]).

**Cost.** No ms figure for 3D.

**Quality and failure modes.**
- In 3D, the finest cascade (cascade 0) costs about as much as voxelising the whole scene.
- Tracing world-space intervals in hardware would cost "an order of magnitude higher" ([paper][rc]).
- Neither fits a city.

**Where it shipped.** Path of Exile 2, in screen space with screen ray marching ([paper][rc]).

#### Surfels (Frostbite's GIBS)

**What it is.** Surfels spawned from the G-buffer and backed by a probe clipmap. About 6k probes are updated a frame,
by priority. It handles skinned characters ([2024 talk][gibs24], [2021 talk][gibs21]).

**Cost.** It started at 6-7 ms and was optimised until it could ship. The shipped figure is in the 2024 talk's
slides and wasn't extracted here ([2024 talk][gibs24]).

**Quality and failure modes.** The extracts read for this note don't cover them.

**Where it shipped.** EA titles, per the 2024 talk ([2024 talk][gibs24]).

#### Voxel, SDF and proxy tracing (Tiny Glade, Teardown)

**What it is.**
- **Tiny Glade** traces one ray per 4x4 pixels: a screen march first, then a coarse proxy BVH, then screen radiance
  at the hit. It stores the result as spherical harmonics, filters it spatially, and runs on a GTX 1060
  ([DF][tinyglade-df], [GPC 2024][tinyglade-gpc]).
- **Teardown** traces voxels for AO, soft shadows and specular occlusion, but has no GI ([80.lv][teardown-80],
  [frame teardown][teardown-acko]).

**Cost.** Neither source gives an ms figure.

**Quality and failure modes.** The extracts read for this note don't cover them.

**Where it shipped.** Both games.

**Lesson for raea.** A coarse proxy plus screen radiance is enough for a convincing diffuse bounce.

### Shipped games

| Game | How it does GI | Notes and measured costs | Sources |
|---|---|---|---|
| Metro Exodus EE | RT GI per pixel, from every light. DDGI-style probes give the later bounces. | SSAO removed. | [4A][metro-4a], [DF][metro-df] |
| Alan Wake 2 | RT direct light, path-traced indirect, DLSS-RR. | Foliage animated by skinning. OMMs and shader execution reordering cut RT cost by a third. | [Remedy][aw2], [DirectX blog][dx-omm] |
| Cyberpunk 2077 on Mac | MetalFX upscaling with dynamic resolution at 50-80%. "For this Mac" targets 30 or 60 fps. Path tracing on high-end chips, with MetalFX frame interpolation and denoising. | M4 Max (40 cores): 31.2 fps at 1080p with RT Overdrive and MetalFX on automatic. At 1600p with RT and MetalFX Performance: M4 Max 57.5 fps, M4 Pro (20 cores) 32.8 fps. | [WWDC26][wwdc26-cp], [DF][cp-df], [Pokde][cp-pokde], [TweakTown][cp-tt] |
| Avatar: Frontiers of Pandora (Snowdrop) | Screen trace, then HW trace against a lower-detail BVH with averaged materials, then probes. GI at quarter to full resolution, stored as SH, denoised with a recurrent blur. | Alpha test replaced by blue-noise dithered rejection with one alpha per instance. Long-range and contact shadows traced. Impostor boxes in the BVH ray-marched. | [DF][snowdrop-df], [GDC 2024][snowdrop-gdc], [GDC 2024 BVH][snowdrop-bvh] |
| Assassin's Creed Shadows | Screen rays, then DXR world rays, then DDGI-like probe cascades, then a denoiser. | Avoids any-hit shaders. Software RT for the hideout on PCs without RT hardware. Mac: M4 MacBook Air 23 fps at Low (MetalFX from about 500p to 900p); M4 Max about 30 fps at Ultra High and 50 at High. | [DF][acs-df], [PC Guide][acs-pcg], [Notebookcheck][acs-nbc] |
| Indiana Jones and the Great Circle | RT GI is mandatory and is the only lighting path. The path-tracing mode uses SHaRC. | RTX 5080: TraceMain 7.90 ms, 3.58 ms with OMMs; SER took 4.07 ms to 3.08 ms. | [raytracedb][ij-rt], [NVIDIA OMM][ij-omm], [NVIDIA SER][ij-ser] |
| Frostbite (EA) | GIBS surfels with a probe clipmap. | See the surfels entry. | [2024][gibs24] |
| Fortnite | Lumen, 4 ms at 60 fps. | See the Lumen entry. | [Epic][fortnite] |
| Call of Duty | GTAO for AO. | No source on current CoD GI was found in this pass. | [Activision][gtao] |

### Ambient occlusion and contact

#### GTAO and XeGTAO

**What it is.** Horizon-based screen-space AO that matches a ground-truth integral. It can output a bent normal, the
average open direction, for specular occlusion and probe lookups ([Activision][gtao], [XeGTAO][xegtao]).

**Cost.**
- 0.5 ms on PS4 at 1080p ([Activision][gtao]).
- XeGTAO at High: 0.56 ms at 1080p on an RTX 2060, and 1.4 ms at 4K on an RTX 3070. Bent normals add 25%
  ([XeGTAO][xegtao]).

**Quality and failure modes.** It sees only what is on screen: occluders off screen or behind the depth buffer don't
count. Its radius is in screen space.

**Where it shipped.** Call of Duty ([Activision][gtao]) and Intel's open-source version ([XeGTAO][xegtao]).

#### Visibility bitmasks (SSILVB)

**What it is.** GTAO's horizons replaced by a bitmask of occluded sectors. This gives surfaces a thickness, and adds
one bounce of screen-space indirect light ([paper][ssilvb]).

**Cost.** On an RTX 2080 at 1080p ([paper][ssilvb]):
- AO: 0.49-1.13 ms, which is 0.01-0.02 ms over GTAO, plus 0.3 ms to denoise.
- Indirect light: 1.07 ms at half resolution, and 1.23-4.33 ms at full.

**Quality and failure modes.** Screen-space limits as for GTAO, but thin objects stop over-darkening
([paper][ssilvb]).

**Where it shipped.** A paper with public code. No shipped game was found.

#### Traced AO

**What it is.** AO from short shadow rays against the TLAS.

**Cost.**
- Lumen's hardware-traced bent normal took 1.75 ms, against 0.12 ms for the screen-space one ([UE 5.2 notes][ue52]).
- raea's own traced AO shoots 128 rays per pixel by default and renders 90 frames of 1080p in 20 s
  (`systems/rt/src/traced_ao.rs:6-8`). That makes it an offline effect, not a live one.

**Quality and failure modes.** M7's unexplained 7-8% brightening of a face is a known fault of raea's version
(realism.feature, M7).

**Where it shipped.** Lumen keeps it as a reference only, being "too slow to use" ([UE 5.2 notes][ue52]).

#### AO from the GI

Lumen keeps a full-resolution ShortRangeAO on top of its GI at every scalability tier. It is screen-space directional
occlusion with an option for bent normals, and it is applied after the temporal filter so that it doesn't lag
([UE 5.2 notes][ue52], [performance guide][lumen-perf56], [NVIDIA UE 5.4 guide][nv-ue54], [SIGGRAPH 2022][lumen22]).
Metro Exodus EE removed SSAO because its GI rays are cast per pixel ([DF][metro-df]). The lesson: AO can only come
out of the GI when the GI samples at pixel density near contacts.

#### Bend's screen-space shadows

**What it is.** Short, sun-direction marches through the depth buffer, run in compute, that add contact shadows the
shadow map is too coarse to give. The code is public ([Bend][bend]).

**Cost.** No ms figure was found.

**Quality and failure modes.** Only on-screen geometry casts.

**Where it shipped.** Days Gone and Ghost of Tsushima ([Bend][bend]).

**For raea.** These are the shadows under noses and feet, and between fingers, that a 1080p sun shadow misses
(inference).

### Direct light

#### Virtual shadow maps (VSM)

**What it is.** A 16k virtual shadow map split into 128x128 pages. Only the pages the screen needs are allocated,
and they are cached between frames. Moving lights and WPO invalidate them ([Epic][vsm]).

**Cost.** About 3.1 ms on PS5 and 2.1 ms on an RTX 4070 at 1080p (third-party figures, [StraySpark][vsm-ss]).

**Quality and failure modes.** A moving sun defeats the cache: Fortnite's time of day hurt it
([Epic, Fortnite][vsm-fn]).

**Where it shipped.** Fortnite and Unreal 5 games.

#### MegaLights

**What it is.** Stochastic direct lighting. Each pixel samples a few lights, traces their shadow rays (screen first,
then hardware), shades and denoises. It replaces all shadowing and shading of local lights, with VSM as the
fallback for foliage ([SIGGRAPH 2025][megalights], [Epic docs][megalights-docs]).

**Cost.** 5.51 ms on PS5 at 1080p, at one sample per pixel with 941 lights ([SIGGRAPH 2025][megalights]):

| Stage | ms |
|---|---|
| Sampling | 0.7 |
| Screen trace | 0.47 |
| Hardware trace | 1.35 |
| Shading | 0.55 |
| Denoising | 0.96 |
| Translucency and volumetrics | the remainder |

**Quality and failure modes.** Noise that the denoiser has to hide. Foliage falls back to VSM
([Epic docs][megalights-docs]).

**Where it shipped.** In Unreal Engine 5.

**For raea.** Gambit at dusk has a gas lamp every few tens of metres, but FRAME_BUDGET allows two shadowed point
lights (budgets.ts). MegaLights' shape is the way to light that street (inference).

#### Ray-traced shadows with denoising

**Cost.**
- NVIDIA's SIGMA shadow denoiser takes 0.40 ms on an RTX 4080 at 1440p ([NRD][nrd]).
- A WebGPU shadow denoiser takes 0.55-0.72 ms on an M5 Pro at 1080p ([pmndrs][pmndrs]).
- Snowdrop traces its long-range and contact shadows ([DF][snowdrop-df]).

#### Many-light sampling

**What it is.** Pick one or a few lights per sample, in proportion to their contribution:
- Lumen's World Cache takes one light sample chosen by energy ([Epic][worldcache]).
- Solari presamples light tiles (`realtime/presample_light_tiles.wesl`).
- MegaLights does the same for direct light ([SIGGRAPH 2025][megalights]).

D7 and LT6's "each with a shadow ray" doesn't scale past a few lights. LT6's clause "a few lights sampled by their
power" is the right one.

### Reflections

#### Hybrid screen-space and traced reflections

**What it is.** Reflections march screen space first and fall back to a traced ray where the screen has no answer.
The two are blended by a confidence value ([Far Cry 6][fc6]). Stochastic SSR (SSSR) importance-samples the
roughness lobe and reuses rays between neighbouring pixels ([SIGGRAPH 2015][sssr]).

**Cost.**
- Battlefield V's traced reflections took 6.29 ms in total on its talk's RTX setup ([GTC 2019][bfv]).
- Far Cry 6's costs are in its slides and weren't extracted here ([GDC][fc6]).

**Quality and failure modes.**
- Screen-space misses objects that are off screen or behind other objects.
- Mixing the two methods shows a seam unless they are blended by confidence ([Far Cry 6][fc6]).

**Where it shipped.** Battlefield V and Far Cry 6.

#### Lumen reflections

**What it is.** Reflections are traced only below a roughness of 0.4 (MaxRoughnessToTrace 0.4,
[performance guide][lumen-perf]). Rougher surfaces reuse the screen radiance cache (8x8 per probe). Glossy rays are
shortened and finish in the world radiance cache (32x32 per probe). Reflection hits sample the scene colour, to hide
gaps where screen traces gave up ([SIGGRAPH 2022][lumen22], [UE 5.2 notes][ue52]).

**Cost.**
- Reusing the cache for rough reflections cut their cost by 50-70%.
- Shortening glossy rays saved 16% on roads ([SIGGRAPH 2022][lumen22]).
- Reflections take 0.23-2.30 ms of the PS5 totals above.

**Quality and failure modes.** Vertical streaks under foreground objects, where screen traces were interrupted. 5.2
reduced them by sampling the scene colour at hits ([UE 5.2 notes][ue52]).

**Where it shipped.** Everywhere Lumen did.

### Denoising and reconstruction

#### Temporal upscalers

- **TSR and FSR.** No per-pass figures were gathered for this note.
- **MetalFX's temporal scaler.** It takes a dynamic input size and a reactive mask. Apple advises keeping the scale
  at 2x or below ([WWDC25][wwdc25-211]).
- **Cyberpunk on Mac.** It runs MetalFX dynamic resolution between 50% and 80% of the output
  ([Pokde][cp-pokde]). Digital Foundry finds MetalFX competitive with PC upscalers that use machine learning, but
  softer ([DF][cp-df]).
- **Cost.** Apple publishes no ms figure for MetalFX.

#### MetalFX's denoised upscaler and frame interpolator

**What it is.**
- **Denoised upscaler.** MTLFXTemporalDenoisedScaler takes noisy colour plus guide textures: diffuse albedo, specular
  albedo, normals, roughness, depth, motion, and specular hit distance ([Apple][mfx-denoised],
  [WWDC25][wwdc25-211]). WWDC26 shows it in Maxon's Redshift Live path tracer ([WWDC26][wwdc26-neural]).
- **Frame interpolator.** It takes two frames, motion and depth, and goes after tonemapping ([WWDC25][wwdc25-205]).

**Cost.** No ms figure is published.

**Quality and failure modes.** Cyberpunk's path tracing through it looked messy next to DLSS-RR ([DF][cp-df]). In
raea, forward-drawn meshes smeared under it because they have no G-buffer (RT4).

#### Ray reconstruction (DLSS-RR, FSR Ray Regeneration)

**What it is.** A neural denoiser and upscaler combined, trained for ray-traced input.
- **DLSS-RR.** It is what lets Solari 0.20 drop ReSTIR ([Solari 0.20][solari20]). It is not on the Mac.
- **FSR Redstone.** AMD's equivalent ships Ray Regeneration 1.1, and Crimson Desert uses it ([AMD][redstone]).

Both are options for raea's Windows path (inference).

#### SVGF, A-SVGF and NRD

**Cost.**
- **SVGF.** 10.2 ms at 1080p on 2017 hardware ([HPG 2017][svgf]). A 2026 optimisation made it 1.3-2.5x faster
  ([JCGT 2026][svgf-opt]).
- **A-SVGF.** Its temporal pass takes under 2 ms on a Titan Xp ([KIT][asvgf]).
- **NRD,** on an RTX 4080 at 1440p ([NRD][nrd]):
  - ReBLUR: 2.55 ms, or 3.40 ms with spherical harmonics;
  - ReLAX: 3.25 ms;
  - SIGMA: 0.40 ms.
- **On this Mac's chip.** An M5 Pro in Chrome WebGPU at 1080p ([pmndrs][pmndrs]):
  - diffuse plus specular SVGF: 4.0-4.23 ms;
  - diffuse only: 2.18 ms;
  - shadows: 0.55-0.72 ms;
  - AO: 1.32-1.67 ms.

**For raea.** A full-screen à-trous denoiser at 1080p costs 2-4 ms on this Mac. raea's GI should therefore filter
in probe space and in the cache, and keep the full-screen passes for shadows and reflections only (inference).

#### Cache-first designs

Lumen ([SIGGRAPH 2022][lumen22]), GI-1.0 ([paper][gi10]) and Tiny Glade ([DF][tinyglade-df]) filter few samples in
probe or spherical-harmonics space and accumulate them in time. None of them ships a full-screen SVGF on its GI.

### Geometry and LOD

#### Nanite

**What it is.**
- **Clusters.** Meshes are split into clusters arranged in a DAG of simplifications. A view-dependent cut through the
  DAG chooses the detail each frame.
- **Visibility buffer.** Clusters are drawn into a visibility buffer, and materials are shaded afterwards.
- **Small triangles.** These go through a compute software rasteriser ([SIGGRAPH 2021][nanite]).
- **Ray tracing.** It uses fallback meshes, or a BLAS streamed out from a coarser cut of the DAG
  ([walkthrough][nanite-walk]).
- **Foliage.** Nanite Assemblies cut one tree from 3.5 GB to 29 MB, and Nanite Voxels draw distant foliage
  ([Epic][nanite-foliage]).

**Cost.** On PS5 at about 1404p, the visibility buffer took about 2.5 ms and material evaluation about 2 ms
([SIGGRAPH 2021][nanite]). GPU-driven materials are covered in [GDC 2024][nanite-mat].

**Quality and failure modes.** The traced scene differs from the drawn one. That is why Lumen needs screen traces to
hide the mismatch ([forum][lumen-forum]).

**Where it shipped.** Unreal 5 games. On Mac, Nanite runs on M2 and later through SM6, on macOS 15 or later; Epic
won't support the M1 ([Epic][ue-mac]).

#### Meshlets and mesh shaders

- **Bevy meshlets.** Bevy's virtual geometry (the `meshlet` feature) is still experimental. It needs
  `SHADER_INT64_ATOMIC_MIN_MAX` ([Bevy 0.15][bevy015]), which wgpu supports on Metal with MSL 2.4 or later
  ([wgpu features][wgpu-features]).
- **Bevy mesh shaders.** Bevy 0.20 added base support, with no StandardMaterial integration yet ([Bevy 0.20][bevy020]).
- **wgpu.** Mesh shaders on Metal need passthrough shaders, because naga compiles them only for Vulkan
  ([wgpu features][wgpu-features]).
- **Hardware.** The M5 Pro has hardware mesh shading ([Apple][m5pro]).
- **Strand hair.** For hair, a compute software rasteriser beat mesh shaders 4.4x: 3.1 ms against 13.6 ms on an RTX
  5080 at 1080p ([paper][hair-swr]).

#### GPU-driven rendering and two-phase occlusion culling

**What it is.** The GPU culls objects and writes indirect draws. Two-phase culling first draws last frame's visible
objects, then builds a depth pyramid (HZB) from them and tests the rest against it.

**Cost.** RedLynx drew 250k objects in 2.3 ms of GPU and 0.2 ms of CPU on Xbox One at 1080p ([SIGGRAPH 2015][gpudriven]).

**Where it shipped.** In Bevy since 0.16 ([Bevy 0.16][bevy016]). Two limits matter for raea:
- Bevy's docs say occlusion culling is "currently incompatible with deferred shading" ([Bevy docs][bevy-occl]). D4
  makes every material deferred.
- Bevy 0.16's platform table marks multi-draw and GPU culling as unsupported on Metal ([Bevy 0.16][bevy016]). No
  source said whether 0.20 changed this, so check on this Mac before relying on it.

#### HLOD and impostors

- **HLOD.** World Partition's HLOD merges a region's meshes into one proxy, with its materials baked
  ([Epic][hlod]).
- **Octahedral impostors.** Each object is captured from a hemisphere or sphere of directions into an atlas.
  Fortnite's trees used a 2048 base-colour and a 1024 normal atlas each, about 6.8 MB per tree, and saved 500k
  triangles ([ShaderBits][impostors]).

#### Virtual texturing

No cost figures were gathered. Bevy plans texture and mesh streaming ([fifth birthday post][jms-5th]). VIEW_BYTES
(640 MB, budgets.ts) caps gambit's geometry instead.

#### LOD for the ray scene

**What it is.**
- **Compaction.** Saves up to about 50% of memory ([NVIDIA][nv-bp], [RTXMU][rtxmu]).
- **Builds.** Build the TLAS every frame with fast trace, refit deforming meshes, spread rebuilds over frames, and
  update only BLASes in use ([NVIDIA][nv-bp]).
- **Refit.** Intel's Jungle Ruins saved 40-45% with refits ([Intel][intel-jr]).
- **Partitioned TLAS (PTLAS).** Rebuilds only the partitions that changed: 7x faster updates in Jungle Ruins
  ([Intel][intel-jr]). Moving objects go in a global partition ([NVIDIA whitepaper][nv-ptlas]).
- **Clustered geometry (CLAS).** Builds a BLAS from prebuilt clusters of up to 256 triangles. In a DXR preview test
  scene, a build went from 788 ms to 3.11 ms ([DirectX][clas]). Cluster templates reuse topology across frames, so
  animated characters rebuild fast ([NVIDIA whitepaper][nv-ptlas]). RTX Mega Geometry draws Zorah at 15.5 ms on an
  RTX 5090 at 4K with DLSS Quality ([RTXMG][rtxmg]).
- **Opacity micromaps (OMMs).** They took Indiana Jones's main trace from 7.90 ms to 3.58 ms ([NVIDIA][ij-omm]).
- **Displacement micromaps (DMMs).** Blackwell's whitepaper puts Mega Geometry forward for detailed geometry
  instead ([NVIDIA][blackwell]).

**Where it applies.**
- CLAS and PTLAS need no new hardware on DirectX, with AMD, Intel and NVIDIA drivers ([DirectX][clas]).
- No Metal equivalent of CLAS, PTLAS or OMMs was found.
- In Unreal, skinned BLASes are rebuilt every frame, at a cost proportional to their triangles
  ([UE ray tracing guide][ue-rtperf]).

### Dynamic geometry

#### Skinned meshes into a BLAS

- **Usual method.** Skin into a vertex buffer in compute, then refit the BLAS or rebuild it fast ([NVIDIA][nv-bp],
  [UE ray tracing guide][ue-rtperf]).
- **On wgpu.** wgpu has no refit ([wgpu][wgpu-rt-types]), so on wgpu it can only be a rebuild with
  PREFER_FAST_BUILD. Metal itself refits ([Apple][mtl-refit]), through `as_hal`.
- **Lumen World Cache.** It is listed as handling skeletal meshes without cards ([Epic][worldcache]).
- **Alan Wake 2.** It animates foliage through skinning ([Remedy][aw2]).
- **Cost on Metal.** No figure was found. LT2 should measure one.

#### Vertex animation textures (VAT)

**What it is.** Baked vertex positions per frame, stored in a texture and read in the vertex shader. Instancing is
cheap, and no skeleton is evaluated.

**Cost.** No ms figure was found. GPU Gems 3's skinned instancing is the older equivalent ([GPU Gems 3][gpugems-crowd]).

#### Crowds

- **AC Unity.** Up to 10,000 NPCs on screen, with 40 full AIs and 120 high-resolution models ([GDC][acu]).
- **Hitman: Absolution.** 1,200 people simulated and 500 on screen, for 8 ms of GPU on PS3 ([GDC 2012][hitman]).
- **Ray-traced crowds.** Ray tracing benefits from reusing skeleton samples across a crowd ([Evolve][evolve]).
- **Research.** A 2026 paper animates ray-traced characters through tetrahedral cages ([ACM][tetcage]), and another
  uses PTLAS for foliage ([ACM][ptlas-foliage]).

#### Cloth

- **Learned deformers.** EA's Swish, a neural cloth deformer in Madden NFL 21, costs 20 µs of CPU, 115 µs of GPU
  deform and 5 µs of normal maps per instance on console ([SIGGRAPH 2021][swish]). Unity's Time Ghost deforms 120k
  vertices in 0.8 ms of GPU with a learned model ([Unity][timeghost]). Unreal's ML Cloth predicts a cloth shape from
  the pose: a low-frequency PCA part plus a nearest-neighbour high-frequency part ([Epic][ue-mlcloth]).
- **Simulated cloth.** Chaos Cloth's cost grows with particles, substeps and self-collision. Its LODs turn simulation
  off for distant characters ([Epic][chaos]).
- **For gambit.** The output is a deformed mesh, which goes into a BLAS like skinning's output does (inference).

#### Hair and skin

- **Strand hair.** In Indiana Jones it costs 1.64-2.39 ms in total ([SIGGRAPH 2025][ij-hair]):
  - Xbox Series S at 1080p: 2.39 ms;
  - PS5 at 1800p: 1.95 ms;
  - PS5 Pro at 2160p: 1.64 ms.
- **Hair through ray tracing.** NVIDIA's linear swept spheres trace hair about 2x faster than strips, with about 5x
  less memory. They are hardware-accelerated on Blackwell only ([NVIDIA][lss]).
- **Path-traced skin.** Subsurface scattering in NVIDIA's RTX Character Rendering SDK costs a 2.38 ms kernel on an
  RTX 5090 at 1080p ([SIGGRAPH 2025][sss25]).
- **Screen-space SSS.** No figure was gathered for separable screen-space subsurface scattering.

### Apple silicon, Metal and wgpu

#### Hardware ray tracing

- **M3 and later.** M3 and A17 GPUs (Apple family 9) have a hardware intersector. Apple's advice: prefer the
  `intersector` API, keep payloads small and prefer opaque triangles ([tech talk][apple-rt-m3]). Intersectors work
  in render shaders only on Apple-silicon GPUs ([Apple][mtl-rt]).
- **M5.** Third-generation RT, which adds:
  - hardware instance transforms;
  - intersection-function buffer indexing in hardware, up to 70% less time;
  - 1 KB acceleration-structure alignment, down from 16 KB ([M5 talk][apple-rt-m5]).
- **M5 Pro.** Up to 35% faster in ray tracing than the M4 Pro, with hardware mesh shading and a Neural Accelerator in
  each GPU core ([Apple][m5pro]).
- **Base M5.** 153 GB/s of memory bandwidth ([Apple][m5]).
- **Profiling.** Apple's GPU counters include ray-tracing counters ([tech talk][apple-prof]).

#### Metal 4 and acceleration structures

- **Build flags.** Metal 4 adds builds that prefer fast intersection or minimal memory, set per build. Builds already
  offered refit, larger scenes and fast build ([WWDC25][wwdc25-211]).
- **Per-primitive data.** Data stored inside the acceleration structure arrives with the hit and saves a lookup by
  primitive ID ([Apple][mtl-rt]).
- **Intersection-function buffers.** They map DirectX shader tables onto Metal ([WWDC25][wwdc25-211]).
- **Through wgpu.** None of these is reachable through wgpu (inference from [wgpu's spec][wgpu-rt]).

#### Tile-based deferred rendering (TBDR)

- **Single-pass deferred.** Apple GPUs keep a pass's render targets in tile memory. A deferred renderer can write its
  G-buffer, light it with programmable blending and never store it, if the G-buffer is memoryless ([Apple][tbdr],
  [Apple sample][tbdr-sample], [WWDC20][wwdc20-tbdr]).
- **Limits.** At most 8 colour attachments. A fragment memory barrier flushes tile memory, which costs time
  ([Apple][tbdr]).
- **Measured gain.** A Slang port measured single-pass deferred at 1.6x faster with no lights and 1.08-1.3x with
  lights, at 3600x2368, saving about 100 bytes per pixel of G-buffer traffic ([slang-metal-tbdr][slang-tbdr]).
- **In wgpu.** wgpu 30 has TRANSIENT_ATTACHMENT, which maps to memoryless, but Bevy doesn't set it (docs/gpu.md).
- **The catch for raea.** Screen probes, short-range AO and MetalFX read the G-buffer from compute. So raea's
  G-buffer must be stored, and the lever is to keep it thin, not memoryless (inference).

#### AAA games on Mac

- **Cyberpunk 2077** uses Apple's Metal shader converter and MetalFX. Its numbers are in the shipped-games table
  ([WWDC26][wwdc26-cp], [Apple][cp-apple]).
- **Assassin's Creed Shadows** runs with RT GI on Macs ([PC Guide][acs-pcg], [Notebookcheck][acs-nbc]).
- **Unreal on Mac.** Epic publishes a table of which Macs get Lumen hardware RT and Nanite ([Epic][ue-mac]).

#### wgpu ray queries on Metal

- **What wgpu 30 has.** Ray queries only; RT pipelines are still in development ([spec][wgpu-rt]). Metal support
  landed in [PR #8071][wgpu-8071].
- **What it lacks.** ALLOW_UPDATE builds as normal, so there is no refit. Compaction is supported
  (`prepare_compaction_async`, `compact_blas`). Instance custom data is 24 bits ([types][wgpu-rt-types]).
- **Open issue.** The tracking issue lists "Metal BLAS -> TLAS builds need synchronisation" (#9215)
  ([#6762][wgpu-6762]).
- **Shaders.** naga's MSL uses `intersection_query` (`naga-30.0.1/src/back/msl/ray.rs:23`).
- **No pipeline cache.** PIPELINE_CACHE is unimplemented on Metal ([wgpu features][wgpu-features]).

### Bevy's roadmap

- **Solari.** It runs on Metal without a denoiser, and the 0.20 notes welcome a MetalFX ray-reconstruction
  contribution ([Bevy 0.20][bevy020]). In 0.20 it unified ReSTIR, made ReSTIR optional and built its TLAS on the GPU
  ([Solari 0.20][solari20]). Animated meshes are future work (RT5).
- **Irradiance volumes.** Baked only, ambient cubes, with no visibility test ([docs][bevy-irr], M13).
- **Virtual geometry.** Still experimental, and its author may return to it ([fifth birthday post][jms-5th]).
- **Plans.** A rework of the material API, texture and mesh streaming, and HDR ([fifth birthday post][jms-5th]).
- **Mesh shaders.** In since 0.20 ([Bevy 0.20][bevy020]).
- **GPU occlusion culling.** In since 0.16, but not with deferred shading ([Bevy docs][bevy-occl]).

## (c) Recommendations, ranked, mapped onto LT2-LT14

Ranked by how much each change decides the outcome. Each item names its step and says whether the plan is right,
wrong, timid, or missing something.

1. **LT2 is right but too narrow. Widen it to measure everything that sets the budget.**
   - Measure, by GPU timestamps on Berlin:
     - TLAS build and BLAS build time;
     - the probe pass at 64k-512k rays, through wgpu's `intersection_query` and through an owned MSL kernel using
       `intersector` (finding 6);
     - the raster frame, broken down by pass.
   - FLOOR3's raster frame is already 18.0 ms. Until it is understood, no GI budget means anything (finding 10).
   - The ray range is right. Lumen High's 1/16 of a ray per pixel at 1440x810 internal is 73k rays, and a full
     quarter-rate probe set plus world-cache paths comes to about 250k ([SIGGRAPH 2022][lumen22]; arithmetic in (e)).

2. **LT3 is missing the ray scene's plumbing. Add it before records.**
   - Keep one TLAS object and rebuild it in place, on the GPU, from an instance buffer, as Solari's
     `setup_tlas_instances.wesl` and Solari 0.20 do ([Solari 0.20][solari20]).
   - Make each instance's 24-bit custom index point into a record table ([wgpu][wgpu-rt-types]).
   - Compact static BLASes ([RTXMU][rtxmu]).
   - Build BLASes on a per-frame budget, in the frame's own encoder, not their own submits (`lib.rs:187`, `214`).
   - Merge each streamed tile into a few BLASes. Cap instances with a distance and size test. Lumen's console cap is
     100k ([performance guide][lumen-perf]); a much lower target suits this Mac (inference; LT2 sets it).
   - Watch wgpu #9215 on Metal sync ([wgpu][wgpu-6762]).

3. **LT3 and D6 are wrong about the cache's form. Make the world cache a hash grid, taken from Solari.**
   - Replace "probes on nested grids, octahedral irradiance and distance" with cells on surfaces, sized by distance,
     with normal bins ([Epic][worldcache], [Solari 0.19][solari19]).
   - Fill it with one path per 8x8 tile, at most three bounces, ending in last frame's cache.
   - Start from `vendor/bevy_solari/src/realtime/world_cache_*.wesl`. It is already in the tree, runs on wgpu, and
     fixes leaks by refining near hits.
   - Keep LT2's flat stand-in and its irradiance-volume bridge only as a measuring step. Bevy's volume has no
     visibility test (M13), so it leaks whatever feeds it.

4. **LT8 is right but late, and screen traces are missing. Move screen traces and screen probes up, straight after
   LT3.**
   - The on-screen bounce is most of what a street shot shows.
   - Probes every 16 px at the internal resolution (Lumen Epic's spacing; High uses 32), traced as an 8x8 octahedron
     ([performance guide][lumen-perf56]).
   - Refresh a quarter of them each frame, as GI-1.0 does ([paper][gi10]).
   - Trace in screen space first, then the TLAS, then end in the world cache.
   - Filter in probe space ([SIGGRAPH 2022][lumen22]).
   - The 0.25-stop bar is right.

5. **LT4 is wrong. Don't build a surface cache. Give hits their material in three stages.**
   - **(i)** The instance record carries each submesh's averaged albedo and emission, as Snowdrop does
     ([GDC 2024][snowdrop-gdc]). That alone makes snow bounce white and a lit window glow.
   - **(ii)** Bake a low-resolution albedo-and-emission texture per asset, once, at import, through the deferred pass.
     This is for meshes whose look is procedural (procmat, wear). It is a bake, not a runtime card capture.
   - **(iii)** Later, evaluate a simplified form of procmat in the hit kernel. This is Lumen's "Simplified material"
     mode ([Epic][worldcache]).
   - On-screen hits read last frame's lit colour through the screen trace, so they need none of this.
   - D4 stays: the G-buffer is still needed for screen probes, AO and MetalFX. D5 goes.

6. **LT5 and D11 are wrong about the reference. A reference must not share the GI's shortcuts.**
   - D11 bounces the reference from the surface cache. A reference that shares the cache's simplification can't
     measure that simplification.
   - Have the path tracer evaluate full materials at every bounce, where they can be evaluated. Where they can't yet
     (hooks, M2), use LT5's stage (ii) textures, and measure that error on FLOOR3 against Bevy's <Pathtracer>.
   - The same tracer is the film renderer, so its cost matters less than its truth (inference).

7. **LT10 is wrong. Rewrite it as "occlusion is counted once", not "occlusion from the GI alone".**
   - Keep a full-resolution short-range AO with bent normals, as Lumen does ([UE 5.2 notes][ue52]). Use GTAO or
     visibility bitmasks ([XeGTAO][xegtao], [SSILVB][ssilvb]). Apply it after the probes' temporal filter
     ([SIGGRAPH 2022][lumen22]), only to indirect light.
   - Add Bend-style contact shadows for the sun ([Bend][bend]).
   - Retire Bevy's SSAO and the 128-ray traced AO in live play. Traced AO stays as a reference tool (`traced_ao.rs`).

8. **LT7 is the right goal; plan its mechanism now.**
   - Skin once in compute into a buffer that raster and the BLAS both read. Bevy skins in the vertex shader of each
     pass, so this also saves skinning in the shadow pass and prepass (inference).
   - Trace each character through a reduced proxy (its LOD1, a few thousand triangles), rebuilt every frame with
     PREFER_FAST_BUILD while wgpu can't refit ([wgpu][wgpu-rt-types]). Move to an owned Metal refit once LT2 shows
     rebuilds cost too much ([Apple][mtl-refit]).
   - Hair cards go into the ray scene with Snowdrop's dithered alpha: one alpha per instance, rejected by blue noise
     ([GDC 2024][snowdrop-gdc]). Metal has no OMMs (finding 7).
   - The crowd beyond the near ring is in (d).
   - "CPU twins" (LT7's Given) cost CPU-to-GPU bandwidth every frame (inference). Use them only as RT5's trial.

9. **LT6 is right, but too small for gaslit streets.**
   - Sample lights by their power through light tiles. Solari's `presample_light_tiles.wesl` is in the tree. Give one
     shadow ray per hit, not one per light ([Epic][worldcache]).
   - Plan MegaLights-style stochastic direct light as a later step, for dusk on Berlin's avenues
     ([SIGGRAPH 2025][megalights]). FRAME_BUDGET's two shadowed point lights can't light them (budgets.ts).

10. **LT9 is right. Add the known savings.**
    - Trace only below a roughness of 0.4 ([performance guide][lumen-perf]).
    - Run SSR first, with a confidence blend into traced rays ([Far Cry 6][fc6]).
    - Let rough surfaces reuse the screen probes (50-70% cheaper), and shorten glossy rays into the world cache
      ([SIGGRAPH 2022][lumen22]).

11. **LT12 is in the wrong place. Speed is a design input from LT2 on, not a final step.**
    - Fix the internal resolution at 67-75%, with MetalFX's temporal scaler at 2x or below ([WWDC25][wwdc25-211]).
    - Trace GI at the internal resolution or below.
    - Keep the G-buffer thin, since it can't be memoryless ([Apple][tbdr]).
    - Treat frame interpolation as a bonus for 120 Hz displays, never as budget ([WWDC25][wwdc25-205]).
    - Measure every tier.

12. **LT11 is right.** For costs, see (b) and (e). For realism, strand hair is a later step; Indiana Jones spends
    1.6-2.4 ms on it ([SIGGRAPH 2025][ij-hair]).

13. **LT13 is wrong in part.** Removing the Solari renderer is right. Removing its world cache, light tiles and TLAS
    setup is not: take those into systems/rt first (finding 3).

14. **LT14 is right.** Add GPU ms per pass to the stops, so a commit that keeps the image but loses a millisecond
    shows it.

**Missing from the plan.**
- **A far field.** Rays and sky occlusion beyond about 200 m need LOD2 or an HLOD proxy in the ray scene. Lumen
  traces HLOD for this ([technical details][lumen-tech]).
- **GI in fog and glass.** FRAME_BUDGET's fog volume (budgets.ts) and window glass should read the world cache.
  Lumen gives translucency its own GI volume ([performance guide][lumen-perf56]).
- **A film mode.** The same scene and caches with more rays and more accumulation, then the reference path tracer
  for finals.
- **Windows and Linux.** A pluggable denoiser slot, so DLSS-RR or FSR Ray Regeneration can take MetalFX's place
  there ([AMD][redstone]).
- **Culling that works with deferred.** See (d) ([Bevy docs][bevy-occl]).

**Revised order (inference).**
1. LT2, widened.
2. LT3: the ray-scene plumbing, then the hash-grid cache.
3. Records with stage (i) materials.
4. Screen traces and screen probes (LT8).
5. Short-range AO and contact shadows (LT10, rewritten).
6. LT6.
7. LT7.
8. LT9.
9. LT5's reference.
10. Stage (ii) materials.
11. LT11.
12. LT13.

LT12's targets and LT14 hold throughout.

## (d) LOD, culling and geometry for 1917 Berlin and its crowds

### What gambit's budgets imply

These figures come from `~/Studio/raea/projects/gambit/world/budgets.ts`. The arithmetic is this document's.

- **Near and middle tiles.** A street camera holds at most 4 LOD0 tiles and about 30 LOD1 tiles.
  - At the per-tile caps, that is 3.6M + 3.0M = 6.6M triangles, over FRAME_BUDGET's 6M before any LOD2.
  - At the measured 510k triangles per LOD0 tile, it is about 2.0M + 3.0M = 5.0M.
  - So culling the LOD1 tiles hidden behind facades decides whether the budget holds.
- **LOD2.** It reaches 6 km. If the map slice really reaches that far, it is about 1,800 tiles, 36M triangles, 7,000
  draws and 2.8 GB.
  - LOD2 needs a merged level over blocks of 1-2 km (an HLOD of LOD2), or the slice must stay small. "The rest of the
    slice" in budgets.ts suggests the slice is small. Confirm that before relying on it.
- **People.**
  - At the main avenue's 250 people per tile, density is 0.004 per m². FRAME_BUDGET's 40 skinned characters then
    cover a radius of about 55 m, and 400 middle ones about 180 m.
  - PEOPLE_RANGE's 150 m near range is a simulation range. Drawing must use its own, smaller rings.
- **Props.** 6,000 per tile only work as GPU instances, culled by distance and size.

### Raster scene

1. **Tiles.** Keep the three LODs with hysteresis.
   - Draw one draw per material per tile, as budgets.ts already merges.
   - Add an HLOD of LOD2 over blocks of 1-2 km, with baked material atlases ([Epic][hlod]).
   - Use octahedral impostors for distant landmarks: towers, the Dom ([ShaderBits][impostors]).
2. **Occlusion.**
   - Bevy's two-phase culling doesn't work with deferred shading ([Bevy docs][bevy-occl]). GPU culling may not run on
     Metal at all ([Bevy 0.16][bevy016]).
   - The city is static. So bake, per 64 m cell (CELL_M), the tiles visible from it, during the map bake. Draw only
     those.
   - A street canyon hides most LOD1 tiles, which is where the 6M-triangle budget is won (inference).
   - Bevy's frustum culling stays. An HZB pass of raea's own can come later, for the props.
3. **Props.**
   - Instance them per kind, cull them by distance and screen size, and draw them only in LOD0 tiles.
   - Small props drop out at LOD1.
4. **Triangle density.** LOD0's cornices, sills and frames make small triangles near the camera's edge of range.
   - Bevy's meshlet renderer is experimental ([Bevy 0.15][bevy015]), and mesh shaders on Metal need passthrough
     ([wgpu features][wgpu-features]). Neither is a dependency for 0.1.x (inference).
   - Tighten LOD0's range or LOD1's detail if LT2's raster breakdown shows triangle-bound passes.
5. **Textures.** There is no virtual texturing in Bevy. Hold textures per tile and per LOD within VIEW_BYTES, and
   stream with the tiles (budgets.ts STREAM).

### Ray scene

1. **A separate, coarser scene, built from the same tiles** ([walkthrough][nanite-walk], [GDC 2024][snowdrop-gdc]).

   | Range | Ray scene holds | Built |
   |---|---|---|
   | The camera's LOD0 tiles | LOD0, merged into a few BLASes per tile, see-through parts masked (RT6) | once at stream-in, compacted |
   | LOD1 tiles, to 700 m | LOD1, one BLAS per tile | once at stream-in, compacted |
   | Beyond 700 m | LOD2 or its HLOD, as far field for sky occlusion and distant bounce | once |
   | Props | a BLAS per prop kind, instanced, only for props over about 0.5 m and within LOD0 tiles | once per kind |

   Small props, cornice detail and window frames come through screen traces and short-range AO, not rays.

2. **Instances.**
   - Tiles contribute tens of instances, props hundreds to low thousands, and characters 40.
   - Keep the total in the low thousands (inference; LT2 sets the cap). At that size a full TLAS rebuild every frame
     is cheap, so a partitioned TLAS, which Metal lacks, isn't needed.
3. **Builds.**
   - BLAS builds happen at stream-in, at most two tiles a frame (STREAM.loadsPerFrame), within a fixed ms budget. Only
     characters rebuild every frame.
   - Compact every static BLAS ([RTXMU][rtxmu]).
4. **Alpha.**
   - Glass and see-through instances stay masked out of the TLAS, as RT6 already does.
   - Foliage and hair cards use dithered alpha ([GDC 2024][snowdrop-gdc]).
5. **Records.** Each instance points into a table holding:
   - the mesh's vertex and index slices;
   - each submesh's averaged albedo and emission;
   - an index into its baked albedo texture, once stage (ii) lands.

### Crowds

| Ring (main avenue at peak) | Raster | Ray scene | Update |
|---|---|---|---|
| Near, about 0-55 m, up to 40 people | skinned, LOD0, hair cards, cloth by learned deformer | LOD1 proxy, skinned in compute, rebuilt every frame | every frame |
| Middle, about 55-180 m, up to 400 people | instanced, VAT or skinned instancing, no skeleton ([GPU Gems 3][gpugems-crowd]) | none, or one shared proxy BLAS per pose bucket, instanced (inference; untested) | animation at a lower rate |
| Far, beyond 180 m | impostors or billboards | none | none |

- **Skinning.** Skin near characters once a frame into a buffer that raster and the BLAS share.
- **Cloth.** The learned approach costs about 0.1 ms of GPU per character on console ([SIGGRAPH 2021][swish]).
  Simulated cloth only for the hero ([Epic][chaos]).
- **Pose buckets.** Instead of a BLAS per character, keep a few posed proxy BLASes per walk cycle, and instance the
  nearest pose for each middle character. That is close to Evolve's reuse of skeleton samples ([Evolve][evolve]).
- **Middle-ring occlusion.** People in the middle ring are small on screen. Screen traces and short-range AO darken
  the street round them, so leaving them out of the ray scene loses little (inference).

### Order of work (inference)

1. Measure raster and the ray scene on Berlin (LT2).
2. Add the per-cell visibility bake and prop instancing.
3. Build the ray scene from merged tiles with compaction.
4. Add near characters' proxies (LT7).
5. Add the middle crowd as instanced VAT.
6. Add the HLOD of LOD2, and impostors.

## (e) Cost targets per pass on an M-series Mac at 1080p

These are targets for the M5 Pro at a 1920x1080 output, 60 fps, live. The internal resolution is 1440x810 (75%),
upscaled by MetalFX. No source measures these passes on this GPU. Each target is grounded in the nearest published
figure, named in the table, and LT2 recalibrates them all.

| Pass | Target, ms | Nearest published figure |
|---|---|---|
| Scene update, and compute skinning of 40 characters | 0.4 | Indiana Jones skins its hair in 0.14-0.17 ms on consoles ([SIGGRAPH 2025][ij-hair]) |
| BLAS builds: character proxies, plus at most 2 streamed tiles | 0.6 | no Metal figure; spread builds over frames ([NVIDIA][nv-bp]) |
| TLAS build, a few thousand instances, on the GPU | 0.2 | no Metal figure |
| Depth prepass and G-buffer at 1440x810 | 3.5 | Nanite: 2.5 ms visibility buffer and 2 ms materials on PS5 at about 1404p ([SIGGRAPH 2021][nanite]) |
| Sun shadow (cascades or virtual) and 2 shadowed point lights | 2.0 | VSM 2.1 ms on an RTX 4070 at 1080p ([StraySpark][vsm-ss]) |
| Sun contact shadows | 0.2 | no figure ([Bend][bend]) |
| Deferred direct lighting | 0.8 | no figure |
| Short-range AO with bent normals, half resolution | 0.4 | XeGTAO 0.56 ms at full 1080p on an RTX 2060 ([XeGTAO][xegtao]) |
| World cache: about 18k paths, update and compact | 0.8 | Solari 0.47 ms for 40k cells, GPU unnamed ([Solari 0.19][solari19]) |
| Screen traces and screen-probe rays | 1.2 | Lumen High final gather 2.07 ms on PS5, integration included ([SIGGRAPH 2022][lumen22]) |
| Probe filter, integration and temporal accumulation | 0.8 | GI-1.0 1.9-3.1 ms in total on an RX 6900 XT at 1080p ([paper][gi10]) |
| Reflections: SSR, then traced below roughness 0.4, half resolution | 1.0 | Lumen reflections 0.23-2.30 ms on PS5 ([SIGGRAPH 2022][lumen22]) |
| Fog volume, 64 steps, lit from the world cache | 0.6 | no figure |
| Transparents, glass and particles | 0.5 | no figure |
| MetalFX temporal upscale, 1440x810 to 1920x1080 | 1.0 | Apple publishes none; measure it |
| Post-processing (tonemap, bloom, grade) and UI | 0.8 | no figure |
| **Total** | **14.8** | leaves 1.9 ms of the 16.7 ms frame |

- **GI's share.** World cache 0.8 + screen probes 1.2 + filtering 0.8 + reflections 1.0 + short-range AO 0.4 =
  4.2 ms, within D13's 5 ms.
- **The raster target.** About 8 ms for prepass, shadows, lighting and post, against FLOOR3's 18.0 ms today (LT1).
  Closing that gap is the first job.
- **Ray counts behind these targets.** At 1440x810, 16 px probe spacing gives 90x51 = 4,590 probes. At 64 rays each,
  that is 294k rays when all refresh, or 73k at a quarter a frame. One world-cache path per 8x8 tile is about 18k
  paths. At up to three bounces with a shadow ray each, that is about 110k rays. Reflections below roughness 0.4, at
  half resolution, add tens of thousands. The total is about 200-300k rays a frame, the middle of LT2's range. At the
  0.4-0.7 billion rays a second inferred from M11, tracing alone is about 0.3-0.75 ms. So shading, memory traffic and
  filtering, not traversal, decide whether the targets hold (inference).
- **Memory.** The world cache is about 88 MB at Solari's size (`prepare.rs:46-48`). Hold the ray scene's BLASes to a
  budget LT2 sets, after compaction. Geometry stays within VIEW_BYTES (640 MB). On a 16 GB Mac, the total GPU working
  set should stay near 2 GB (inference).
- **Floor tier, 30 fps on smaller Macs.**
  - Internal resolution 67% (1280x720).
  - Probe spacing 32 px (Lumen High's), and world-cache paths per 16x16 tile.
  - No traced reflections: SSR and probes only.
  - Same passes otherwise.
- **Film mode.** Same scene and caches, with probe spacing 8 px, every probe each frame, and accumulation over
  frames. Finals go through the reference path tracer (LT5), which has no frame budget.

[lumen22]: https://advances.realtimerendering.com/s2022/SIGGRAPH2022-Advances-Lumen-Wright%20et%20al.pdf
[lumen-tech]: https://dev.epicgames.com/documentation/unreal-engine/lumen-technical-details-in-unreal-engine
[lumen-perf]: https://dev.epicgames.com/documentation/unreal-engine/lumen-performance-guide-for-unreal-engine?lang=en-US
[lumen-perf56]: https://dev.epicgames.com/documentation/en-us/unreal-engine/lumen-performance-guide-for-unreal-engine?application_version=5.6
[lumen-forum]: https://forums.unrealengine.com/t/weird-ambient-occlusion-in-corners-in-ue5-lumen/1305381?page=2
[worldcache]: https://dev.epicgames.com/documentation/unreal-engine/lumen-world-cache-in-unreal-engine?lang=en-US
[fortnite]: https://www.unrealengine.com/tech-blog/lumen-brings-real-time-global-illumination-to-fortnite-battle-royale-chapter-4
[ue52]: https://dev.epicgames.com/documentation/unreal-engine/unreal-engine-5.2-release-notes
[ue56]: https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engine-5-6-release-notes
[ue57]: https://tomlooman.com/unreal-engine-5-7-performance-highlights
[witcher4]: https://www.digitalfoundry.net/articles/digitalfoundry-2025-witcher-4-unreal-engine-5-deep-dive-tech-interview
[nv-ue54]: https://dlss.download.nvidia.com/uebinarypackages/Documentation/UE5+Raytracing+Guideline+v5.4.pdf
[gi10]: https://arxiv.org/html/2310.19855v1
[gi10-gdc]: https://gpuopen.com/gdc-presentations/2023/GDC-2023-Two-Level-Radiance-Caching-GI.pdf
[brix]: https://gpuopen.com/fidelityfx-brixelizer/
[brix-gdc]: https://gpuopen.com/download/GDC2024_GI_with_AMD_FidelityFX_Brixelizer.pdf
[brix-manual]: https://gpuopen.com/manuals/fidelityfx_sdk/techniques/brixelizer-gi
[ddgi]: https://jcgt.org/published/0008/02/01/paper.pdf
[rtxgi]: https://developer.nvidia.com/blog/rtx-global-illumination-part-i
[metro-4a]: https://www.4a-games.com.mt/4a-dna/in-depth-technical-dive-into-metro-exodus-pc-enhanced-edition
[metro-df]: https://www.digitalfoundry.net/articles/digitalfoundry-2021-inside-metro-exodus-enhanced-edition-pc-exclusive
[sharc]: https://github.com/NVIDIA-RTX/RTXGI/blob/main/Docs/SharcGuide.md
[nrc]: https://arxiv.org/html/2106.12372v2
[nrc-mobile]: https://dl.acm.org/doi/10.1145/3757376.3771399
[solari19]: https://jms55.github.io/posts/2026-04-12-solari-bevy-0-19
[solari20]: https://jms55.github.io/posts/2026-09-18-solari-bevy-0-20
[jms-5th]: https://jms55.github.io/posts/2025-09-03-bevy-fifth-birthday
[wsrestir]: https://gpuopen.com/download/SA2021_WorldSpace_ReSTIR.pdf
[restirpt]: https://dl.acm.org/doi/full/10.1145/3804494
[rc]: https://github.com/Raikiri/RadianceCascadesPaper/blob/main/RadianceCascades.tex
[gibs24]: https://www.advances.realtimerendering.com/s2024/content/EA-GIBS2/Apers_Advances-s2024_Shipping-Dynamic-GI.pdf
[gibs21]: https://advances.realtimerendering.com/s2021/SIGGRAPH%20Advances%202021%20-%20Surfel%20GI.pdf
[tinyglade-df]: https://www.digitalfoundry.net/articles/digitalfoundry-2024-tiny-glade-proves-the-power-of-bespoke-game-engines-and-graphics-tech
[tinyglade-gpc]: https://graphicsprogrammingconference.com/archive/2024/
[teardown-80]: https://80.lv/articles/teardown-developer-breaks-down-multiplayer-and-voxel-destruction-tech
[teardown-acko]: https://acko.net/blog/teardown-frame-teardown
[snowdrop-df]: https://www.digitalfoundry.net/articles/digitalfoundry-2023-avatar-frontiers-of-pandora-and-snowdrop-the-big-developer-tech-interview
[snowdrop-gdc]: https://media.gdcvault.com/gdc2024/Slides/GDC+slide+presentations/Kuenlib_Quentin_Raytracing_In_Snowdrop.pdf
[snowdrop-bvh]: https://media.gdcvault.com/gdc2024/Slides/GDC+slide+presentations/Koshlo_Oleksandr_Snowdrop_Custom_BVH.pdf
[acs-df]: https://www.digitalfoundry.net/articles/digitalfoundry-2025-assassins-creed-shadows-tech-qa-rtgi-shader-compilation-taa-and-more
[acs-pcg]: https://www.pcguide.com/news/assassins-creed-shadows-releases-on-mac-with-disappointing-performance-m4-benchmarks-show/
[acs-nbc]: https://www.notebookcheck.net/Cyberpunk-AC-Shadows-on-Apple-s-M4-Max-Gaming-performance-comparable-to-the-RTX-5060-Laptop.1166765.0.html
[aw2]: https://www.remedygames.com/article/how-northlight-makes-alan-wake-2-shine
[dx-omm]: https://devblogs.microsoft.com/directx/d3d12-opacity-micromaps
[wwdc26-cp]: https://developer.apple.com/videos/play/wwdc2026/356
[cp-apple]: https://developer.apple.com/news?id=eb0rfgn9
[cp-df]: https://www.digitalfoundry.net/articles/digitalfoundry-2025-cyberpunk-2077-apple-mac-review-an-impressive-thoughtful-port-from-cd-projekt-red
[cp-pokde]: https://pokde.net/blog/testing-cyberpunk-2077-on-mac-studio
[cp-tt]: https://www.tweaktown.com/news/106804/cyberpunk-2077-runs-at-1600p-61fps-on-macbook-pro-16-with-apple-m4-max-chip-and-ray-tracing/index.html
[ij-rt]: https://raytracedb.com/news/indiana-jones-great-circle-ray-tracing-settings
[ij-omm]: https://developer.nvidia.com/blog/path-tracing-optimizations-in-indiana-jones-opacity-micromaps-and-compaction-of-dynamic-blass
[ij-ser]: https://developer.nvidia.com/blog/path-tracing-optimization-in-indiana-jones-shader-execution-reordering-and-live-state-reductions
[ij-hair]: https://advances.realtimerendering.com/s2025/content/Strand%20Hair%20in%20IJGC%20-%20Final%20Slides%20%28Post-Conference%29.pdf
[gtao]: https://www.activision.com/cdn/research/PracticalRealtimeStrategiesTRfinal.pdf
[xegtao]: https://github.com/GameTechDev/XeGTAO
[ssilvb]: https://arxiv.org/html/2301.11376v2
[bend]: https://www.bendstudio.com/blog/inside-bend-screen-space-shadows
[vsm]: https://dev.epicgames.com/documentation/unreal-engine/virtual-shadow-maps-in-unreal-engine
[vsm-ss]: https://www.strayspark.studio/blog/virtual-shadow-map-optimization-open-worlds-ue5-7
[vsm-fn]: https://www2.unrealengine.com/tech-blog/virtual-shadow-maps-in-fortnite-battle-royale-chapter-4
[megalights]: https://advances.realtimerendering.com/s2025/content/MegaLights_Stochastic_Direct_Lighting_2025.pdf
[megalights-docs]: https://dev.epicgames.com/documentation/unreal-engine/megalights-in-unreal-engine?lang=en-US
[bfv]: https://developer.download.nvidia.com/video/gputechconf/gtc/2019/presentation/s91023-it-just-works-ray-traced-reflections-in-battlefield-v.pdf
[fc6]: https://gpuopen.com/download/GDC_Performant_Reflective_Beauty_Hybrid_Ray_Traced_Reflections_In_Far_Cry_6.pdf
[sssr]: https://advances.realtimerendering.com/s2015
[nrd]: https://github.com/NVIDIA-RTX/NRD
[svgf]: http://cwyman.org/papers/hpg17_svgf.pdf
[svgf-opt]: https://jcgt.org/published/0015/01/02/paper.pdf
[asvgf]: https://cg.ivd.kit.edu/publications/2018/adaptive_temporal_filtering/adaptive_temporal_filtering.pdf
[pmndrs]: https://pmndrs.github.io/denoiser/docs/guides/realtime-denoisers
[redstone]: https://www.amd.com/en/developer/resources/technical-articles/2026/upscale-everything-super-resolution-across-amd-hardware-.html
[wwdc25-211]: https://developer.apple.com/videos/play/wwdc2025/211
[wwdc25-205]: https://developer.apple.com/videos/play/wwdc2025/205
[mfx-denoised]: https://developer.apple.com/documentation/metalfx/mtlfxtemporaldenoisedscalerdescriptor
[wwdc26-neural]: https://developer.apple.com/videos/play/wwdc2026/359
[apple-rt-m3]: https://developer.apple.com/videos/play/tech-talks/111375
[apple-rt-m5]: https://developer.apple.com/videos/play/tech-talks/111431
[apple-prof]: https://developer.apple.com/videos/play/tech-talks/111374
[m5pro]: https://www.apple.com/newsroom/2026/03/apple-debuts-m5-pro-and-m5-max-to-supercharge-the-most-demanding-pro-workflows
[m5]: https://www.apple.com/newsroom/2025/10/apple-unleashes-m5-the-next-big-leap-in-ai-performance-for-apple-silicon
[mtl-rt]: https://developer.apple.com/documentation/metal/ray-tracing-with-acceleration-structures
[mtl-refit]: https://developer.apple.com/documentation/metal/mtlaccelerationstructurecommandencoder/refit%28sourceaccelerationstructure:descriptor:destinationaccelerationstructure:scratchbuffer:scratchbufferoffset:%29
[tbdr]: https://developer.apple.com/documentation/metal/tailor-your-apps-for-apple-gpus-and-tile-based-deferred-rendering
[tbdr-sample]: https://developer.apple.com/documentation/metal/rendering-a-scene-with-deferred-lighting-in-swift
[wwdc20-tbdr]: https://developer.apple.com/videos/play/wwdc2020/10632
[slang-tbdr]: https://github.com/rudybear/slang-metal-tbdr
[ue-mac]: https://www.unrealengine.com/tech-blog/bringing-unreal-engine-on-macos-up-to-feature-parity-with-windowsprogress-report
[wgpu-rt]: https://github.com/gfx-rs/wgpu/blob/v30/docs/api-specs/ray_tracing.md
[wgpu-rt-types]: https://wgpu.rs/doc/src/wgpu_types/ray_tracing.rs.html
[wgpu-8071]: https://github.com/gfx-rs/wgpu/pull/8071
[wgpu-6762]: https://github.com/gfx-rs/wgpu/issues/6762
[wgpu-features]: https://wgpu.rs/doc/wgpu_types/struct.Features.html
[nanite]: https://advances.realtimerendering.com/s2021/Karis_Nanite_SIGGRAPH_Advances_2021_final.pdf
[nanite-walk]: https://traceyang.com/en/2026/06/11/nanite-end-to-end-pipeline-walkthrough-qa
[nanite-foliage]: https://dev.epicgames.com/documentation/en-us/unreal-engine/nanite-foliage?lang=en-US
[nanite-mat]: https://media.gdcvault.com/gdc2024/Slides/GDC+slide+presentations/Nanite+GPU+Driven+Materials.pdf
[gpudriven]: https://advances.realtimerendering.com/s2015/aaltonenhaar_siggraph2015_combined_final_footer_220dpi.pdf
[hlod]: https://dev.epicgames.com/documentation/unreal-engine/world-partition---hierarchical-level-of-detail-in-unreal-engine?lang=en-US
[impostors]: https://shaderbits.com/blog/octahedral-impostors
[nv-bp]: https://developer.nvidia.com/blog/best-practices-for-using-nvidia-rtx-ray-tracing-updated
[rtxmu]: https://developer.nvidia.com/blog/reducing-acceleration-structure-memory-with-nvidia-rtxmu
[intel-jr]: https://community.intel.com/t5/Blogs/Tech-Innovation/Client/Path-Tracing-Massive-Dynamic-Geometry-in-Jungle-Ruins/post/1691062
[clas]: https://devblogs.microsoft.com/directx/clas-ptlas-preview/
[nv-ptlas]: https://github.com/NVIDIA-RTX/Whitepapers/blob/main/dx_clusters_and_ptlas/README.md
[rtxmg]: https://github.com/NVIDIA-RTX/RTXMG
[blackwell]: https://images.nvidia.com/aem-dam/Solutions/geforce/blackwell/nvidia-rtx-blackwell-gpu-architecture.pdf
[ue-rtperf]: https://dev.epicgames.com/documentation/unreal-engine/ray-tracing-performance-guide-in-unreal-engine?application_version=5.8
[acu]: https://www.gdcvault.com/play/1022411
[hitman]: https://media.gdcvault.com/gdc2012/slides/Programming%20Track/Fauerby_Kasper_CrowdsInHitman.pdf
[gpugems-crowd]: https://developer.nvidia.com/gpugems/gpugems3/part-i-geometry/chapter-2-animated-crowd-rendering
[evolve]: https://www.evolvebenchmark.com/blog-posts/ray-tracing-animated-crowds
[tetcage]: https://dl.acm.org/doi/10.1145/3820014
[ptlas-foliage]: https://dl.acm.org/doi/10.1145/3820021
[swish]: https://history.siggraph.org/wp-content/uploads/2022/06/2021-Talks-Lewin_Swish.pdf
[timeghost]: https://discussions.unity.com/t/making-of-time-ghost-cloth-deformation-with-unity-sentis/1537842
[ue-mlcloth]: https://dev.epicgames.com/documentation/unreal-engine/machine-learning-cloth-simulation-overview?application_version=5.6
[chaos]: https://dev.epicgames.com/community/learning/tutorials/KWeX/unreal-engine-a-tech-artist-s-playbook-for-chaos-performance
[hair-swr]: https://arxiv.org/html/2607.04230v1
[lss]: https://developer.nvidia.com/blog/render-path-traced-hair-in-real-time-with-nvidia-geforce-rtx-50-series-gpus
[sss25]: https://advances.realtimerendering.com/s2025/content/sss-siggraph-2025-advances-published.pdf
[bevy015]: https://bevy.org/news/bevy-0-15/
[bevy016]: https://bevy.org/news/bevy-0-16
[bevy020]: https://bevy.org/news/bevy-0-20/
[bevy-occl]: https://dev-docs.bevy.org/bevy/render/occlusion_culling/struct.OcclusionCulling.html
[bevy-irr]: http://docs.rs/bevy/latest/bevy/pbr/irradiance_volume/index.html
