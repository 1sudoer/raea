@spec
Feature: img2raea: a picture made into raea objects, props, environments and scene data, and held to three.js
  Goal, 2026-11-16: "lets work on the img2threejs as also a way to get 3js perfect parity and how we can expand the
  skill for our own skills like img2raea and how we can do the same with environments, scene data, and what places we
  can use procedural tools or nueral networks or any kidn fo machine learnign to get it right".

  img2threejs (github.com/img2threejs/img2threejs, Apache-2.0, read at 0c0c49bc) is an agent skill that turns one
  picture into a procedural three.js model: a factory, `createXModel(): THREE.Group`, built in gated passes (blockout,
  structure, form, material, surface, lighting, interaction, optimisation) against an ObjectSculptSpec, each pass
  checked by renders in a browser (the "Divine Eye" views, a turntable, self-intersection, attachment and chirality
  gates). Its showcase (img2threejs-showcase, 638b5c73, three ^0.169) has no licence and holds other people's IP, so
  it is measured locally and never committed; the corpus we keep comes from running img2threejs on our own pictures.

  Found on 11-16: a factory runs in raea unchanged as `<R3F><primitive object={createXModel()} /></R3F>` (the Talon
  knife, its tick animation included). So img2threejs's output is already raea's input; what is left is making the
  two renderers agree (parity), giving the skill a raea target (img2raea), and doing the same for places and scenes.

  Rule: three.js parity, measured on img2threejs's output

    @needs
    Scenario: IR1. The showcase as a parity corpus
      Given the showcase's single-file factories (15 on 11-16: a rifle, a bike, three knives, a chest, a house, a
        mascot, a pistol, a shotgun, a humanoid, earbuds' case, a rocket, a truck), fetched at a pinned commit into a
        cache outside git
      When each is centred, scaled to 2 m on its longest side, seen from one camera at 640 x 400, and drawn by raea's
        <R3F> (`raea render`) and by three.js's own <Canvas> in headless Chrome (sdk/r3f/tools/chrome.ts: Chrome only as
        three.js's reference renderer, as R3F8 uses it)
      And under four rigs: neutral (a hemisphere and two directional lights), direct (the two directional lights),
        dark (direct with an ambient light of 0) and lookdev (the author's own lights, moved and scaled with the model)
      Then each pair is scored on the model's own box (the union of both pictures' non-background boxes, padded 8 px):
        SSIM and RMSE there, the frame's SSIM beside them; the frame's SSIM is not the score, as a model that fills a
        tenth of the frame scores 0.99 with its face missing
      And the plan
        | crate | file                        | lines | job                                                                        |
        | tools | tools/img2raea/parity.sh    | 80    | fetch the pinned showcase into ~/.raea/cache, run both sides, score, sheet |
        | tools | tools/img2raea/shoot.ts     | 70    | three.js's side: the page bundled with sdk/r3f's three and R3F, shot       |
        | tools | tools/img2raea/scene.tsx    | 80    | one case's scene for both sides: fit, seeded Math.random, the four rigs     |
        | tools | tools/img2raea/score.sh     | 40    | crop to the model's box, SSIM and RMSE, side-by-side pairs, contact sheets |
      And 11-16, so far: built and run in a scratch folder (not yet in tools/): 60 cases in 6.5 minutes on this Mac,
        raea's renders 5.8 GB peak; 14 of the 15 factories draw (the humanoid throws: IR6); under the dark rig 12 of the
        14 score 0.95 or better on the crop (the rocket 0.81: IR4; the chest 0.94: its gold edges' highlights); under
        the neutral rig the earbuds' case, the rocket and the chest fall to 0.80-0.84 (IR3); the mascot scores 0.976
        with its face missing (IR5), so a per-part or edge score has to join SSIM before 0.95 can mean "the same"
      And not yet diagnosed: the bike's tyre lettering is missing in raea (a canvas texture repeated -1 x 1, so a
        negative repeat is the first suspect) and the truck's ground ring loses its soft glow (additive blending is
        mapped, input/material.ts, so something else)

    @done
    Scenario: IR2. No ambient light means none
      Given a three.js scene with no AmbientLight, HemisphereLight or LightProbe
      Then raea draws it with no ambient light either
      And today it doesn't: <R3F> sends no ambient, so the camera falls back to Bevy's GlobalAmbientLight (white, 80),
        which raea never sets; a metal then shows its colour everywhere where three.js shows black outside its
        highlights (the earbuds' copper lid, the chest's gold corners, a knife's blade)
      And measured 11-16: the earbuds' case, direct rig, crop SSIM 0.83; with an explicit <ambientLight intensity={0}>
        0.996; the chest 0.88 to 0.94, the Gerber knife 0.94 to 0.98
      And the fix is one of: sdk/r3f's input/lights.ts sends AmbientLight 0 when the scene has none, or raea's engine
        sets GlobalAmbientLight to 0 (a raea scene without an ambient light would then be darker too: decide which)
      And 11-16, built in sdk/r3f (input/scene.ts convert): a scene with no AmbientLight, HemisphereLight or LightProbe
        gets an AmbientLight of 0, white so it lends no colour (raea sums a camera's ambient lights and takes the first
        non-white one's colour, so a composition's own AmbientLight beside the <R3F> is kept as it was); the engine's
        GlobalAmbientLight is unchanged (Shane's call, through the 0.1.6 release session). Measured: under the direct
        rig the earbuds' case 0.83 to 0.996, the Gerber knife 0.94 to 0.98, the chest 0.88 to 0.94, the mascot 0.96
        to 0.99; R3F8's 43 cases as before (one, in-drei, 0.9935 to 0.9943); test/input.test.tsx "ambient light"

    @needs
    Scenario: IR3. three.js's ambient lights only diffuse
      Given a three.js AmbientLight or HemisphereLight
      Then it lights a surface's diffuse colour only, as three.js does (irradiance x diffuse; a metal has none)
      And today raea's AmbientLight is Bevy's, which also lights specular through the environment BRDF, so under a
        hemisphere light a metal glows evenly: the earbuds' case, neutral rig, crop SSIM 0.80
      And the fix: a diffuse-only ambient for lights read from three.js (an ambient term raea's PBR shader can switch,
        or the hemisphere light as an irradiance-only environment)

    @needs
    Scenario: IR4. Anisotropic metal
      Given a MeshPhysicalMaterial with anisotropy and anisotropyRotation (the rocket's brushed steel: 0.65, pi/2)
      Then raea's highlight stretches along the brushing as three.js's does
      And today the steel blows out in hard white and black halves: crop SSIM 0.81; with anisotropy 0, 0.94
      And what to check first: the mesh's tangents (Bevy's anisotropy reads ATTRIBUTE_TANGENT; a three.js geometry
        has none unless computed), then the rotation's sense and the strength's mapping

    @needs
    Scenario: IR5. Overlays drawn without the depth test
      Given meshes with material.depthTest false and a renderOrder above the body's (the mascot's eyes, mouth and
        cheeks: decals whose vertices lie on the body shell, drawn on top in renderOrder 3 and 4)
      Then raea draws them after the meshes before them, over what is already drawn, as three.js does
      And today they are depth-tested and lost inside the shell: the face is missing (crop SSIM 0.976, which hides it:
        the face is a small part of the box; IR1's sheet shows it); sdk/r3f passes depthTest on for a patched or
        shader material (input/patched.ts, input/shader.ts) but not for the standard ones (input/material.ts), and
        renderOrder is read only for render targets (input/targets.ts)
      And 11-16, built in sdk/r3f: a basic material drawn without the depth test is three's own basic program
        (shader.ts builtinPart, as matcap and normal are), a standard or physical one its patched program (patched.ts),
        and any ThreeShaderMaterial without the depth test goes with the transparent ones, after everything opaque, as
        raea's opaque pass has no order; the mascot's face is drawn (crop SSIM 0.96 to 0.99 under the direct rig);
        test/patched.test.ts "a material drawn without the depth test"
      And left: renderOrder among such overlays (the eye's highlight over the eye) is still raea's transparent sort,
        by distance; three's order needs a sort bias from renderOrder in raea-sys-three-shader (Material::depth_bias),
        an engine change

    @needs
    Scenario: IR6. A browser's globals a factory reads
      Given a factory that reads window.location.search (the humanoid's debug switches) or other browser globals
      Then raea's JS host has a location (about:blank's: an empty search), so the factory runs
      And today the composition throws at frame 0: "undefined is not an object (evaluating 'window.location.search')"

  Rule: img2raea, the skill

    @needs
    Scenario: IR7. img2threejs with raea as its target
      Given img2threejs's emission target socket (docs/EMISSION_TARGET_SOCKET.md: `--target <kind>`, a plugin step
        with `provides`, run after the model is action-ready)
      When a raea target plugin is added, installed as ~/.img2/plugins/raea/ and read by their forge/_shared/targets.py:
        plugin.json `{"schema": 1, "id": "raea", "requires": {"harness": RANGE, "coreApi": 1}, "capabilities":
        [{"from": "sculpt-spec", "to": "raea-tsx"}]}`, and a steps.json row `{"id": "emit-raea", "command":
        "{plugin_dir}/emit.sh {workspace} {spec}", "provides": {"version": 1, "from": "sculpt-spec", "to": "raea-tsx",
        "artifact": {"kind": "raea-tsx", "path": ".img2/artifacts/raea/model.tsx"}}}`, its gates in gates.json
      Then `--target raea-tsx` writes a raea composition that draws the factory (`<R3F><primitive
        object={createXModel()} /></R3F>`, its tick from useFrame) and its lights, beside the factory, the same bytes
        on every run (their socket runs a target twice and refuses a mismatch)
      And raea is the render authority for the gates that need pictures:
        | img2threejs gate          | raea                                                                                  |
        | Divine Eye views          | `raea render EDIT.tsx --review DIR`: hero, front, right, top, iso, low, colour and clay |
        | turntable                 | a composition turning the model, `raea render` to frames                              |
        | self-intersection         | `raea scene lint` (clipping), `raea scene explain`                                    |
        | attachment anchor         | `raea scene lint`: what rests on what, what floats or sinks                           |
        | surface and lighting pass | `raea render --aov` (normals, depth) and the light meter                              |
        | chirality                 | the review's front and right views against the reference                              |
      And the gates also pass IR1's parity on the model, so what the skill approved in raea looks the same in three.js

    @needs
    Scenario: IR8. img2raea in raea's own skill
      Given the raea skill (skills/, the public raea-skills)
      Then it teaches the same method natively, credited to img2threejs (Apache-2.0: its NOTICE kept with what is
        adapted): the picture read into a spec of parts (ObjectSculptSpec's fields), then built in passes in raea's
        own vocabulary, each pass checked by `raea render --review` before the next
      And raea's vocabulary for it: core/shape's CSG for hard surfaces, core/sdf for organic ones, core/grow for the
        numerous (leaves, rocks, terrain), raea's standard material and systems/procmat for surfaces
      And a picture's thing can come out as either: a three.js factory raea draws (IR7), or a raea part

    @needs
    Scenario: IR9. Tried on our own pictures
      Given pictures we make (`raea gen image`, under $10) of a prop, a hard-surface thing and a character
      When img2raea builds each
      Then each is scored against its picture from the picture's own view: the silhouette's IoU (SAM 2's mask of the
        picture against raea's alpha render), and SigLIP's similarity of the two crops (on the CPU: Core ML's GPU path
        gets SigLIP wrong on this Mac)
      And these become the kept corpus (CC0, ours), in IR1's runner beside the showcase

  Rule: Scene data

    Scenario: IR10. A three.js scene's geometry in scene lint, explain and export
      Given an R3F or <Three program> scene, whose geometry raea holds as <BufferMesh> shape "buffer:{positions,
        indices, normals, uvs}"
      Then contact, `raea scene lint|explain` and `raea scene export` read it as its triangles, and export writes each
        such part as GLB (POSITION, NORMAL, TEXCOORD_0) and OBJ (v, vn, vt), without Manifold, as it needn't be closed
      And 11-16: built on branch img2raea (core/shape/src/buffer.rs; raea-sys-mesh re-exports buffer_tris), unit tests
        in core-shape and interchange pass; before it every such part stopped `raea scene` with "unexpected ':' in a
        shape"; the knife's end-to-end export waits for a raea rebuild

    @needs:IR10
    Scenario: IR11. Materials go with the geometry
      Given a part `raea scene export` writes as a mesh
      Then its GLB carries its material (base colour, metallic, roughness, emissive, the maps raea has as files), so
        Blender and Godot open it looking as it does in raea, not grey

    @needs:IR10
    Scenario: IR12. An import measured both ways
      Given a scene exported by `raea scene export` (scene.json and its meshes)
      When it is opened in three.js and in Blender
      Then it is held to raea's own render by IR1's scoring from the same camera, so import and export are measured
        as the R3F parity is

  Rule: Environments

    @needs
    Scenario: IR13. A place from one picture
      Given a picture of a place (a room, a street, a clearing)
      Then it is made in raea in stages, each checked against the picture from its camera:
        | stage      | what                                                                             | how                                        |
        | camera     | the lens's field of view and the camera's height and tilt                         | MoGe-2's point map (MIT, ViT-S 35M)        |
        | layout     | the ground and walls as planes, so things stand on them                          | planes fitted to the point map (RANSAC)    |
        | things     | a mask and a name for each thing                                                  | SAM 2.1 (Apache-2.0), OWLv2 (Apache-2.0)   |
        | props      | each thing as a part at its depth and size                                        | img2raea (IR8), or an image-to-3D model    |
        | far away   | what has no depth worth building: a panorama or splat behind                      | <Environment>, <Splat> (raea splat)        |
        | light      | the sun's and sky's direction and colour, then exposure                           | the picture's shading; systems/light-meter |
        | contact    | nothing floats, sinks or clips                                                    | `raea scene lint`                          |
      And what can be made procedurally is: terrain, rocks and trees by core/grow, grass by systems/grass, floors and
        walls by systems/procmat, the picture giving their settings rather than their meshes

  Rule: Where procedural tools, neural networks and measurement belong

    @needs
    Scenario: IR14. The map
      Then each step uses what gets it right on this Mac (16 GB first) under a commercial-permissive licence:
        | step                      | method                                          | model or tool                                   | licence                     |
        | parity score              | measure                                         | crop SSIM and RMSE now; LPIPS or DINO next      | ours                        |
        | a thing's mask            | neural                                          | SAM 2.1 (Core ML build exists)                  | Apache-2.0                  |
        | a thing's name            | neural                                          | OWLv2; Apple Vision (systems/vision)            | Apache-2.0; system          |
        | depth, lens, normals      | neural                                          | MoGe-2 ViT-S; Depth Anything V2 Small, DA3 Small, Base, Mono-L, Metric-L | MIT; Apache-2.0 |
        | parts of a thing          | procedural, fitted                              | core/shape CSG, core/sdf, core/grow             | ours                        |
        | a fit's settings          | search, judged by measure                       | render, score against the picture, adjust (CMA-ES or coordinate search) | ours |
        | a whole object's mesh     | neural, as a reference or a start               | TripoSG (geometry only, about 8 GB)             | MIT                         |
        | a character               | neural                                          | Idol (sheet3d's route, about 3 GB)              | as docs/character.feature   |
        | how close to the picture  | neural, as a critic                             | SigLIP 2 (on the CPU), silhouette IoU           | Apache-2.0; ours            |
        | a surface's look          | procedural, fitted                              | systems/procmat's kinds, colours from the mask's lit pixels | ours           |
      And left out, by licence or size: Depth Anything V2 Base and Large and DA3 Large and Giant (CC BY-NC), Depth Pro
        and SHARP (research only), Hunyuan3D (not the EU, UK or South Korea), TRELLIS.2 (MIT but 24 GB: a reference to
        compare against, not a step), SAM 3D Objects (SAM Licence, no published memory, reported out of memory at 20 GB)
      And neural models make or name things; measurement decides whether they are right; procedural parts are what is
        kept, so a result can be edited as raea's own parts rather than as a mesh no one can change
