@spec
Feature: React Three Fiber both ways: an R3F scene drawn by raea, a raea composition drawn in R3F
  Goal, 2026-11-06: "complete r3f support so that raea can also render to r3f or use r3f as input in raea, two way
  complete r3f support, and measure so that it is complete and nothing is missing". The code is sdk/r3f (raea/r3f for
  authors). Both ways go through raea's scene contract, the `data-3d` elements a frame renders: an R3F scene is read
  into those elements (input), and those elements are drawn as a three.js scene (output). The same units, shapes and
  tone mapping hold on both sides, and both are measured: every kind and class has a row (R3F7) and every case is
  held to the other renderer by SSIM (R3F8).

  Rule: R3F as raea's input

    @done
    Scenario: R3F1. An R3F scene in a raea composition
      Given an R3F scene, the JSX a <Canvas> holds (meshes, lights, cameras, useFrame, refs, primitives)
      When a composition puts it inside <R3F camera={...} shadows> from raea/r3f
      Then raea's engine draws it: each frame R3F's reconciler builds the three.js scene off screen (a stand-in
        renderer, no WebGL), raea steps R3F's clock to the frame and reads the scene into raea's parts
      And useFrame runs once a frame, 1/fps a step, from frame 0; a seek back starts again from 0, so a render, a
        seek and a play give the same frame (test/input.test.tsx "the clock")
      And raea's own hooks work inside it (useCurrentFrame, a context from outside the canvas): the context bridge
        re-provides the composition's contexts to R3F's tree

    @done
    Scenario: R3F2. What is read from three.js
      Then input/kinds.ts has a row for every Object3D and Material class three exports (62 in three 0.180):
        42 drawn, 12 near (a note says what differs), 8 with nothing to draw (a bone, a camera that isn't drawn with,
        a sound, a base class)
      And meshes keep their triangles, normals, uvs (v flipped: Bevy counts from the top), vertex colours, geometry
        groups (a mesh a material); skinned and morphed meshes are posed on the CPU each frame; an InstancedMesh is
        Instances while its copies hold still; Points, Lines, Sprites and three's helpers are drawn
      And lights convert by units.ts, so a three light and the raea light it becomes put the same light on screen; a
        HemisphereLight is an ambient light of its colours' average plus half their difference from straight up
      And textures keep their url; a DataTexture or a canvas becomes a PNG; repeat, offset, rotation and samplers hold

  Rule: raea drawn in R3F and three.js

    @done
    Scenario: R3F3. A raea composition inside an R3F <Canvas>
      Given a raea composition's component
      When an R3F app puts <Raea component={Edit} /> inside its <Canvas>
      Then its 3D is drawn by three.js in that canvas: the component renders as under raea (into a page off screen,
        at its frame), and Scene3d draws the page's parts into a three.js scene R3F holds
      And its camera becomes R3F's, its tone mapping, exposure, shadows, fog, environment and background the canvas's
        (camera={false} and settings={false} leave them to the app)
      And its frame follows R3F's clock at its fps, or `frame` pins it

    @done
    Scenario: R3F4. Scene3d, the three.js host, for any three.js app
      Then sdk/r3f/src/three draws a raea page into a three.js scene without React Three Fiber (sdk/playable's host
        is built on it): Scene3d.sync(page, seconds), fit(width, height), apply(renderer)
      And three/kinds.ts has a row for every kind raea's SDK renders (58): 27 drawn, 11 near, 4 settings, 9 effects
        and 7 that are Bevy's own (a WESL shader or frame pass, a compute pass, a Bevy component or resource,
        volumetric fog and fog volumes)
      And an ECS world (<Entities>) is drawn: its meshes and their transforms come from raea/ecs's own store in
        JavaScript (sdk/app's scene(), the engine's composition of a child under its parent), each mesh's shape by
        three's geometry or core/shape's wasm
      And the 7 that are Bevy's own aren't three.js's to draw: WESL and WGSL run on Bevy's renderer, components and
        resources are Bevy's data, and the fogs are raymarched through Bevy's shadow maps and depth. A page that needs
        them uses raea-web, raea's engine itself in the browser on WebGPU (R3F10); Scene3d names each one it leaves out
        (a warning, and its row)
      And a Model with `at` is a copy of the model at each placement, as raea's engine draws it
      And raea's default tone mapping, TonyMcMapface, is drawn with Bevy's own LUT (sdk/playable's tony.bin) in a
        pass in place of three's output pass; ACES, AgX, Reinhard and none are three's own

    @done
    Scenario: R3F5. raea's solids on the web
      Given a Mesh whose shape is a solid (minus, hull, extrude, Text3D: core/shape's expression)
      Then engine/shape-web compiles core/shape to wasm (tools/shape-wasm.sh, 2.6 MB) and loadShapes() brings it in:
        the solid's own triangles, the same as raea's engine meshes
      And until it loads, a solid waits (it isn't drawn as something else)
      And the public repository's releases carry the module, so an app needn't build Rust: tools/release-web.sh builds
        raea_shape_web.js and raea_shape_web_bg.wasm (and raea-web's raea_web.js and raea_web_bg.wasm, with SHA256SUMS)
        from a commit and attaches them to its GitHub release: every engine release (tag vVERSION; 0.1.0-canary.1 and
        canary.2 were given their own shape code's module on 11-06, without raea-web, which came after them), and a
        pre-release web-SHA for a public commit between them (SHA the public commit's short hash), at
        https://github.com/shinyobjectz/raea/releases/download/TAG/raea_shape_web.js. GitHub serves them as
        downloads (no CORS), so an app serves the two files itself and calls loadShapes("/raea_shape_web.js"), or
        imports the glue and passes the module: loadShapes(await import("./raea_shape_web.js"))

    @done
    Scenario: R3F6. Frame effects
      Then three/effects.ts draws raea's effects with three's EffectComposer in Bevy's order: ambient occlusion,
        outlines, depth of field, bloom (energy-conserving, as Bevy's) and motion blur in the scene's light; Bevy's
        grade (fringes, vignette, white balance, levels) before tone mapping; saturation, looks and lenses after
      And <Raea> runs it whenever the composition has effects; effects={false} hands them to the app's own composer

  Rule: Measured

    @done
    Scenario: R3F7. Nothing is left without a row
      Then test/coverage.test.tsx reads the SDK's sources for every kind it renders and fails on one missing from
        three/kinds.ts, draws every kind three.js draws and fails on a warning, and fails on an effect row the
        composer doesn't draw
      And it walks three's exports and fails on an Object3D or Material class missing from input/kinds.ts
      And it fails on a kind three/kinds.ts calls "engine" that has no raea-web parity case (R3F10)
      And `bun test` in sdk/r3f runs these with the input, round-trip and source-export tests (27 tests)
      And a ShadowMaterial is raea's <Mesh shadow="catch">: a surface that shows only the shadows falling on it, in
        its material's colour and opacity (systems/material shadow_catch.rs in the engine, three's ShadowMaterial in
        Scene3d)

    @done
    Scenario: R3F8. Each way held to the other renderer
      Given test/parity/fixtures.tsx: R3F scenes (in-*), raea compositions (out-*) and the same compositions written
        as R3F source (src-*, R3F9), at 640x360
      When tools/parity.sh renders each with `raea render` (native.tsx) and with three.js in headless Chrome
        (test/parity/page.tsx, which tools/serve.ts builds and serves itself, with the wasm tools/shape-wasm.sh builds),
        and compares the two by SSIM
      Then every case is at least 0.92 (measured 11-07)
        | case        | what                                                                 | SSIM   |
        | in-basic    | a spinning box (useFrame), PBR and clearcoat, shadows, a point light | 0.9917 |
        | in-lights   | spot, hemisphere and area lights, fog, Lambert and flat Phong        | 0.9795 |
        | in-things   | InstancedMesh, multi-material, edge lines, a sprite                  | 0.9917 |
        | in-catch    | a ShadowMaterial floor: only the box's shadow over the background    | 0.9966 |
        | in-lettra   | lettra's MSDF text (TSL) and a TSL gradient on its clock             | 0.9994 |
        | in-lettrafx | lettra's wipe and scramble (an attribute, a uniform array)           | 0.9992 |
        | in-shader   | ShaderMaterials: a rippled sphere, points sized by an attribute      | 0.9989 |
        | in-post     | <EffectComposer>: HueSaturation, Vignette, ToneMapping ACES          | 0.9451 |
        | in-drei     | drei's Text, Environment preset, ContactShadows, Float, useGLTF      | 0.9932 |
        | in-approx   | matcap, normal, Phong specular, dashes, clipping, hemisphere, probe  | 0.9848 |
        | out-basic   | a raea scene, ACES, a moving box                                     | 0.9958 |
        | out-tony    | the same scene in raea's default tone mapping, TonyMcMapface         | 0.9961 |
        | out-lights  | spot and area lights, fog, Text3D (a solid)                          | 0.9633 |
        | out-bloom   | <Bloom intensity={2}>                                                | 0.9295 |
        | out-grade   | <Grade> saturation, contrast, vignette                               | 0.9738 |
        | out-lens    | <LensDistortion k1={0.15}>                                           | 0.9922 |
        | out-catch   | <Mesh shadow="catch">, three's ShadowMaterial                        | 0.9972 |
        | out-entities| <Entities>: a raea/ecs world, a ship and its gun turned by a system  | 0.9972 |
        | src-basic   | out-basic exported as R3F source, drawn by a plain <Canvas>          | 0.9958 |
        | src-tony    | out-tony exported (Tony through raea-effects.ts)                     | 0.9961 |
        | src-lights  | out-lights exported as R3F source (Text3D as its triangles)          | 0.9633 |
        | src-bloom   | out-bloom exported (raea-effects.ts)                                 | 0.9295 |
        | src-grade   | out-grade exported                                                   | 0.9738 |
        | src-lens    | out-lens exported                                                    | 0.9922 |
        | src-catch   | out-catch exported                                                   | 0.9972 |
        | src-entities| out-entities exported                                                | 0.9972 |
      And three r181 rewrote UnrealBloomPass (mrdoob/three.js#31528): three/effects.ts puts r180's kernels, composite
        and blend back on three's pass, so out-bloom holds to the bloom raea's matches (three's frame equals r180's at
        SSIM 0.999998)
      And three's ACES scales its input by 1/0.6 where Bevy's doesn't: the host's exposure takes 0.6 under ACES, and
        an R3F canvas's ACES reads as 0.74 stops more (measured: out-lights 0.927 to 0.963, in-lights 0.927 to 0.979)

  Rule: raea written as R3F

    @done
    Scenario: R3F9. A raea composition exported as React Three Fiber source
      Given a raea composition
      When `bun sdk/r3f/tools/export.ts EDIT.tsx COMPOSITION OUT.tsx [--from N] [--frames N | --all]` runs, or
        toR3FSource(component, options) from @raea/r3f/export
      Then OUT.tsx is a TSX module that needs only react, three and @react-three/fiber: the scene as JSX (meshes with
        three's geometry and materials, lights, fog, background, environment, models by url), what moves as a table a
        frame (position, rotation, scale, visibility, materials, lights, the camera) applied in useFrame, and a default
        export that is a <Canvas> with raea's camera, tone mapping, exposure and shadows
      And a solid (minus, hull, extrude, Text3D) is written as its triangles, meshed by core/shape's wasm
      And when the scene has frame effects or raea's Tony tone mapping, raea-effects.ts is written beside it: the
        composer <Raea> draws them with (three/effects.ts and Bevy's Tony table, needing only three), run by the module
        after each frame's updates; effects that only raea's engine draws (three/kinds.ts "engine") are named in the
        header
      And test/export.test.tsx checks what is written; the src-* cases (R3F8) draw it against raea's render

  Rule: What three.js can't draw, raea-web does

    @done
    Scenario: R3F10. The engine's own kinds held to `raea render` in the browser
      Given sdk/web/test/parity/cases.ts: a case for each kind three/kinds.ts calls "engine", and <Entities>
      When sdk/web/tools/parity.sh renders each with `raea render` and with raea-web (sdk/web, raea's engine on WebGPU)
        in headless Chrome (tools/shoot.ts), and compares the two by SSIM
      Then every case is at least 0.92, and a blank web frame fails whatever its SSIM (measured 11-06)
        | case           | kinds                     | SSIM   |
        | surface-shader | SurfaceShader             | 0.9998 |
        | frame-pass     | FramePass                 | 1.0000 |
        | compute        | Compute                   | 1.0000 |
        | component      | BevyComponent             | 1.0000 |
        | resource       | BevyResource              | 0.9999 |
        | entities       | Entities                  | 0.9760 |
        | fog            | VolumetricFog, FogVolume  | 0.9993 |
      And the first five are sdk/raea/bench/bevy-ts's cases (shader-scene, frame-pass, compute, reflect-on-camera,
        reflect-keyed-resource-still, 64x64); entities and fog are 320x180
      And a page's files (a glTF, a picture) are fetched from assets/ beside the page, Bevy's asset folder on the web
      And measuring it fixed raea-web: a model's file is fetched rather than looked for on disk (runtime/scene3d
        model.rs), a black page background is the 3D's clear rather than a cover over it (rgb(0, 0, 0) was read as
        see-through, here and in Scene3d), and a layered <Canvas3D>'s empty slot isn't drawn

  Rule: What R3F apps are made of draws in raea

    Goal, 2026-11-06: "finish the r3f": custom GLSL shaders, @react-three/postprocessing, the materials and lights that
    were approximate, and a drei-heavy scene, each read by <R3F> and held to three.js by parity (R3F8).

    @done
    Scenario: R3F11. three.js shaders run in raea's engine
      Given a ShaderMaterial or RawShaderMaterial in an <R3F> scene (written by hand, or drei's shaderMaterial())
      When raea reads it (src/input/shader.ts)
      Then its GLSL goes as three would compile it: the #includes expanded from three's ShaderChunk, its defines in
        front, the #ifdef blocks its defines settle decided, its uniforms as this frame's numbers and its textures as
        pictures (a picture made in code as a PNG data: URL)
      And raea-sys-three-shader translates it with naga and runs it with three's built-ins (the matrices, the camera,
        a colour and uvs on every vertex, up to 4 attributes of its own, 64 vec4 uniforms, 4 textures)
      And a shader that includes three's tone mapping is tone-mapped by Bevy's, one that doesn't is shown as three's
        canvas shows it
      And in-shader (a sphere rippled by a uniform set in useFrame, points sized by their own attribute): SSIM 0.9989

    @done
    Scenario: R3F12. @react-three/postprocessing's <EffectComposer> as raea's frame effects
      Given an <EffectComposer> in an <R3F> scene
      When raea reads its effects (src/input/post.ts)
      Then each becomes the raea effect that does the same thing: Bloom; Vignette, ChromaticAberration,
        HueSaturation and BrightnessContrast in one Grade; ToneMapping; DepthOfField and Bokeh; SMAA and FXAA; SSAO
        and N8AO; Pixelation; LensDistortion
      And one raea has nothing for (noise, glitch, god rays ...) is named once and left out (kinds.ts's EFFECTS
        decides every effect postprocessing exports, tested)
      And a composer's frame is tone-mapped only by its ToneMapping effect, as in three
      And in-post (HueSaturation, Vignette, ToneMapping ACES): SSIM 0.945

    @done
    Scenario: R3F13. A drei-heavy scene: <Text>, <Environment preset>, <ContactShadows>, <Float>, useGLTF
      Given test/parity's in-drei case: a knot loaded by useGLTF in a <Float>, troika's <Text> in Inter, drei's
        <ContactShadows> under it and <Environment preset="city"> lighting it
      When raea renders it
      Then three's loaders read their files where a native render has no fetch: FileLoader by the `asset.read` native
        call (a file, or an http(s) URL through raea's cache), so a glTF is parsed in JS; a DataTextureLoader (HDR,
        EXR) hands back the cached file, which raea's engine loads as <Environment src>
      And what an app draws into a render target is done on the CPU where it's a pass raea knows (src/input/targets.ts):
        a MeshDepthMaterial override from an orthographic camera and three's 9-tap blurs, which are drei's
        <ContactShadows>; anything else drawn into a target is named and left empty
      And a mesh whose transform mirrors it is drawn with its triangles rewound (three flips the front face; Bevy
        culls as wound)
      And raea's bundler gives a composition troika-three-text as sdk/r3f/src/input/troika.ts (engine/bundle's
        STAND_INS): its Text holds troika's settings and draws nothing, and src/input/text.ts reads any troika Text as
        raea's <RichText> (size, the face's family from its name table, weight, colour, anchors, maxWidth)
      And a root whose parts mounted late (they suspended) starts again from frame 0 once its loads are cached, and
        Math.random is seeded per root, so drei's <Float> and three's uuids are the same however often a seek restarts
        it (render equals play)
      And in-drei: SSIM 0.993 (Float at speed 0 in the case: three's page draws its phase at random)

    @done
    Scenario: R3F14. The approximate kinds made exact or closer
      Given test/parity's in-approx case
      When raea renders it
      Then MeshMatcapMaterial and MeshNormalMaterial run three's own ShaderLib programs (R3F11), so they look as three
        draws them
      And Phong's specular colour is its F0 (three's Blinn-Phong): Bevy's reflectance and specular tint
      And a LineDashedMaterial is cut into its dashes by the line's distances (computeLineDistances), as three's shader
        cuts it
      And clipping planes (a material's with localClippingEnabled, the renderer's, a ClippingGroup's) cut the mesh's
        triangles, so what three discards isn't drawn (src/input/clip.ts; clipIntersection keeps what is inside any one)
      And drei's MeshTransmissionMaterial draws with Bevy's transmission (its own uniform's, as three's is 0 under it)
      And a LightProbe is an ambient light of its constant band and a light along its linear band (exact for surfaces
        facing that light or sideways, as the hemisphere light is), and its constant band is scaled as three's
        irradiance is (0.886, where 0.282 made it a third as bright)
      And in-approx: SSIM 0.9848 (Phong's F0 is held at Bevy's highest, 0.16, so its orange highlight glints less)

    @done
    Scenario: R3F15. What an R3F scene throws is the composition's error (#131)
      Given an <R3F> whose scene throws: rendering a component, in an effect, in a ref
      Then the frame throws it out of the <R3F> drawing it (src/input/session.ts: the scene under raea's own error
        boundary), so the render is refused naming it, exactly as a throw outside R3F is: "REFUSED the composition threw
        at frame 0: Error: ...", exit 1
      And never as R3F's own root reported it (reportError, else a stack without its message), the scene drawing nothing
        and the render ending with 0 errors
      And three's PMREMGenerator makes no GPU cube map in raea (src/input/pmrem.ts): fromEquirectangular of a panorama
        raea loads is that panorama, lighting the scene as scene.environment does; fromScene (RoomEnvironment) and
        fromCubemap are left out with a warning naming them, the scene drawn without them
      And test/errors.test.tsx holds it

    @done
    Scenario: R3F15b. three's RoomEnvironment lights an R3F scene (#165)
      Given an <R3F> scene whose scene.environment is new PMREMGenerator(gl).fromScene(new RoomEnvironment(), sigma)
        .texture, three's own recipe for soft studio light with no HDR file
      When raea renders it
      Then it is lit by the same room, made by raea's engine: <Environment room blur={sigma}>, which src/input/pmrem.ts
        and lights.ts make of it (its intensity, its turn about +Y, and as the background when the room is that too)
      And the room is three's (systems/environment room.rs): RoomEnvironment's boxes, light boxes and point light,
        each direction from fromScene's cube camera traced to what it hits and lit by three's formulas, blurred by
        sigma as fromScene blurs it, then filtered like any panorama
      And fromScene of any other scene, and fromCubemap, are still left out with a warning naming them
      And raea's own <Environment room> is drawn by three as RoomEnvironment through fromScene (src/three/scene.ts),
        and the source export writes it so (src/export/emit.ts)
      Status (11-16): done. The front face of #131's box, 136,76,58 before (no environment), is 255,189,155; three
        draws 255,195,160. Two grey boxes lit by the room alone at half intensity: 137 and 125 against three's 140 and
        127 (23 and 19 before). three's pixels come from test/room/three.ts, which runs three 0.185's PMREM (the room
        in its 256 cube, the GGX filter chain, textureCubeUV) and MeshStandardMaterial's shading on the CPU, so the
        check needs no browser; test/room/check.sh renders test/room/room.tsx and holds each pixel within 8 levels.
        The room's light at 14 directions matches three's own RoomEnvironment object to 1e-3 (room.rs tests); Bevy's
        filter in place of three's PMREM puts the rest within 3 to 6 levels. Not done: fromScene's options.position
        (a cube camera away from the origin) is drawn from the origin, with a warning

  Rule: What three.js work online is made of draws in raea (0.1.5)

    Goal, 2026-11-15: "ship all of this in 0.1.5". An audit that day rendered 25 probes of what people post online made
    with three.js and R3F (TSL compute particles, render targets, plain three.js with its own renderer, drei's showpiece
    helpers, postprocessing) through 0.1.4's <R3F>: six refused the render, five drew wrong, moving instances cost a
    mesh a copy, and the raea skill told agents R3F doesn't exist here. Each scenario below is one of those, held by a
    case in test/parity (SSIM against three.js in Chrome, R3F8) or a test that refuses the old behaviour.

    @done
    Scenario: R3F16. What three can't draw in raea is named and drawn, never refused
      Given an <R3F> scene with any of: a TSL node material on an InstancedMesh, a compute dispatched through `gl.compute`,
        drei's <MeshReflectorMaterial>, drei's <Environment> with children (Lightformers), drei's <Stars>, drei's <Clouds>
      When raea renders it
      Then the render isn't refused for what raea's stand-in renderer lacks: it answers what three's renderers ask as they
        draw (a WebGPU backend's device limits, WebGLRenderer's `state`, `compute`, a CubeCamera's coordinate system)
      And what raea can't draw yet is named once in a warning and drawn as the documented fallback (flat, or its base
        material), the rest of the scene drawn as three would
      And a ShaderMaterial on Points (drei's <Stars>) draws its points with its shader
      And a texture three loads from an http(s) URL is fetched through raea's cache, never read as a path in the project
      And test/errors.test.tsx renders each of the six without a refusal
      Status (11-15): session.ts's stand-in WebGLRenderer answers coordinateSystem (WebGL's), `state` (an inert GL state:
        every call does nothing), `extensions`, a full `info`, and takes `compute`/`computeAsync` into gl.__raea.computes
        (warned once: R3F22 runs them); tsl.ts's stand-in WebGPURenderer has a backend device with wgpu's limits, any Error
        building a graph is named and the material drawn flat, and a graph reading an attribute the drawn mesh can't carry
        (an InstancedMesh's matrices past 1,024 copies, an InstancedBufferAttribute) is refused there, not by the engine;
        shader.ts gives a sampler2D the material never sets a 1x1 opaque black picture (WebGL's unbound read); texture.ts
        fetches an http(s) picture through raea's cache (`asset.load`). The six probes from the audit render (stars, clouds with the CDN's cloud.png, the
        reflector's plane as its base material, Environment+Lightformer without the cube map, TSL on 100 and 2,000
        copies, gl.compute), each named; test/errors.test.tsx 10/10

    @done
    Scenario: R3F17. A sky at three's far plane is drawn behind the scene
      Given a shader that puts its vertices on the far plane as three does (`gl_Position.z = gl_Position.w`, drei's and
        three's <Sky>, any skybox ShaderMaterial), or a node material's WGSL doing the same
      When raea translates it (Bevy's projection is reverse-Z: the far plane is z = 0)
      Then it is written onto reverse-Z's far plane, so the scene in front of it shows
      And in-sky (three's Sky behind a box): SSIM at least 0.92 against three.js
      Status (11-15): shader.ts farPlane() rewrites `X.z = X.w;` and `gl_Position = X.xyww;` to z = 0 in a ShaderMaterial's
        vertex GLSL and in a node material's vertex WGSL (material.ts, on the part tsl.ts makes); in-sky SSIM 0.9986 (in-shader 0.9935, in-drei 0.9932,
        in-approx 0.9848 unchanged); test/errors.test.tsx holds the rewrite

    @done
    Scenario: R3F18. Agents are taught <R3F>
      Given the raea skill (raea-skills skills/raea) and its source guides (sdk/guides/craft/bevy)
      When an agent reads what 3D in raea can be written with
      Then 3d/r3f.md says when to use <R3F> (an R3F or three.js scene, drei, postprocessing, GLSL or TSL shaders), how to
        set up a project for it, what draws exactly, what differs, and what raea's own parts do better
      And the lines saying useFrame, refs and drei "don't exist" say instead that they don't exist in raea's own 3D parts
        and do inside <R3F>
      And SKILL.md's router sends an agent with three.js or R3F code to 3d/r3f.md
      Status (11-15): sdk/guides/craft/bevy/r3f.md, copied as the skill's 3d/r3f.md (raea-skills #81, pinned at
        89ab557): when to use <R3F> and <Three program>, the project set-up (raea add three @react-three/fiber), an
        example of each, how their loops are stepped from the frame, what draws (by kind, the postprocessing effects
        raea has) and what doesn't yet, each named once in a render's output. bevy.md's router, the skill's 3d.md and
        SKILL.md route three.js work to it; the four lines (bevy.md and 3d.md's rule 1, bevy/scene.md, bevy/animation.md,
        bevy/limits.md) say useFrame, refs and drei don't drive raea's own components and work inside <R3F>. Landed
        1f02317e8.

    @done
    Scenario: R3F19. Plain three.js with its own renderer
      Given a composition whose three.js code makes its own renderer, as single-file demos do: `new
        THREE.WebGLRenderer({ canvas })` or three/webgpu's `WebGPURenderer`, a scene and camera, `renderer.render(scene,
        camera)` from `setAnimationLoop` or requestAnimationFrame, `document.body.appendChild(renderer.domElement)`
      When raea renders or plays it
      Then raea's bundler gives it a renderer that draws nothing and records what it was asked to render, and each frame
        that scene is drawn by raea's engine as <R3F>'s scene is, in the canvas's box
      And its animation loop steps 1/fps a frame from the composition's frame (render equals play)
      And in-vanilla (a plain three.js page): SSIM at least 0.92 against the same page in Chrome
      Status (11-15): `<Three program={fn}>` (raea/r3f, src/input/Three.tsx, program.ts): fn is the page's script as a
        function given a canvas. engine/bundle edit.rs resolves `three` and `three/webgpu` imported by the edit's own
        code (src/, ext/ of the work folder; never the SDK's copies, so the playable host keeps three's real renderer) to
        the r3f copy's vanilla.ts and vanilla-webgpu.ts: three re-exported, its renderer a class whose constructor
        returns R3F's stand-in (renderer.ts, moved out of session.ts with the seeded random in seed.ts, so a
        composition's three needs nothing of React Three Fiber's) wrapped to record a render to the screen and the
        animation loop into the program running it. Each frame steps its loops and requestAnimationFrame callbacks at
        i*1000/fps ms with performance.now() at that time and Math.random seeded by the <Three>'s key; a frame before
        the last runs it again; an async program (await renderer.init()) holds raea's frame and steps from 0 once ready;
        a full-screen pass to the screen (EffectComposer's quad) draws the scene it rendered. test/program.test.ts 7/7,
        edit.rs's own-code test; in-vanilla (test/parity/programs.ts: a knot, a ring of boxes, a sun and shadows, turned
        by the loop's time and by += a call) SSIM 0.9921, and the R3F cases whose `three` is now the stand-in unchanged
        (in-basic 0.9917, in-shader 0.9935, in-drei 0.9932, in-tsl-lit 0.9906, in-instanced 0.9819, in-sky 0.9986).

    @done
    Scenario: R3F20. An InstancedMesh's moving copies are one instance buffer a frame
      Given an InstancedMesh whose matrices or colours change every frame (a field of 20,000 copies moved in useFrame)
      When raea draws it
      Then its copies are one row drawn by Bevy's instancing, their transforms and colours that frame's buffer, never a
        mesh a copy
      And 20,000 moving copies at 640x360 take at most 100 ms a frame (0.1.4: 1,220 ms), 2,000 at most 40 ms (0.1.4: 94)
      And in-instanced (2,000 moving copies with colours): SSIM at least 0.92 against three.js
      # The plan (r3f-gpu, 11-15). 0.1.4's cost is each copy a Mesh with its own BufferMesh: 20,000 copies of a box's
      # arrays as JSON a frame. raea's <Instances> already moves a set frame by frame (its data-at list a frame, the
      # row's inst_all/inst_k, scene3d's sync_instances: one entity a copy sharing the mesh and material, which Bevy's
      # automatic batching draws as one instanced call), so the frame's buffer is one column holding that frame's list,
      # not keyed columns per copy. What it lacks is a colour a copy, and a list that isn't text parsed every frame.
      #   R3F20a. raea/3d <Instances>: an Instance takes `color` (a hex string, or linear [r, g, b]), multiplied with the
      #     material's as three's instanceColor is; a set with colours packs 12 numbers a copy, written as data-at
      #     {"stride":12,"at":[...]}; runtime/look's inst column is then 13 numbers a copy (placement, linear rgb) with
      #     inst_tint 1. scene3d: a tinted set's copies share one TintMaterial (raea-sys-material tint.rs: the standard
      #     material extended by a storage buffer of the copies' colours, read by the copy's MeshTag above the two bits
      #     compose's pass keeps), the buffer rewritten when the frame's list changes; the parent's standard material
      #     copied into its base when it changes (sync_meshes recolours the set as before). compose's sync_passes keeps
      #     a tag's high bits.
      #   R3F20b. sdk/r3f scene.ts: an InstancedMesh with a standard surface is always one <Instances>, still or moving,
      #     coloured or not: its copies from the instance matrices (instanceMatrix.count of them, those past `count` at
      #     scale 1e-6 so the count holds), instanceColor as each copy's colour. A ShaderMaterial or several materials
      #     keep the mesh-a-copy path.
      #   R3F20c. Speed: each render list parsed once into transforms (scene3d Placements), not each frame it shows;
      #     look packs without reformatting text where it can. Measured before and after on 2k and 20k (probes
      #     inst-2k-48f, inst-20k-48f), with RAEA_FRAME_LOG's react/sync/draw split.
      #   R3F20d. in-instanced in test/parity (2,000 moving copies with colours under a directional light), SSIM >= 0.92.
      Status (11-15): built as planned, the coloured list 13 numbers a copy (placement, linear rgb). scene.ts sends
        every InstancedMesh with a standard surface as one <Instances>; raea/3d's Instance takes `color`; runtime/look
        reads data-at as plain numbers; scene3d's copies of a coloured set share one TintMaterial (raea-sys-material
        tint.rs) whose ShaderBuffer of colours is written in place each frame, each copy's MeshTag its number above
        compose's pass bits (sync_passes keeps them). A list is still text parsed once each frame it shows (R3F20c's
        parse-once gave nothing for a list that changes every frame). Measured on this Mac with test/parity/speed.tsx,
        48 frames at 640x360 (0.1.4 in brackets): 2,000 moving copies 25.5 ms a frame (94), 20,000 41.9 ms (1,220);
        coloured every frame, 43.7 and 74.6 ms, of which the TintMaterial's pipeline is 0.5 to 0.7 s on the first
        frame (steady frames 10.5 and 46.8 ms). in-instanced SSIM 0.9819; in-things and in-basic unchanged (0.9917);
        a plain <Instances> scene (bench/rerender shot-1, shot-2) the same as 0.1.4's (SSIM 0.99999, 1). Tests:
        conformance instances (12 numbers a copy), runtime/look a_colour_a_copy, tint's tag bits, test/input.test.tsx
        (moving and coloured copies stay one Instances; a ShaderMaterial is a mesh a copy), test/instances.test.tsx (the
        three.js host's instanceColor).

    @done
    Scenario: R3F21. A lit TSL node material is lit by the scene's lights
      Given the plan
        | crate                 | file                                | lines | job                                                    |
        | raea-sys-three-shader | systems/three-shader/src/surface.rs | 90    | the lit fragment entry: three's surface as Bevy's PbrInput, lit |
        | sdk/r3f               | sdk/r3f/src/input/tsl-lit.ts        | 85    | three's builder writing the surface, the lit contract  |
      And a MeshStandardNodeMaterial or MeshPhysicalNodeMaterial (or a Lambert, Phong or toon one) with nodes of its
        own (colorNode, positionNode, normalNode, roughnessNode, metalnessNode, emissiveNode, opacityNode)
      When raea draws it
      Then three's builder writes its surface with no lights of three's: its output (three's `context.getOutput`) is
        the colour and opacity, the view-space normal and roughness three lights with, the emission and metalness, and
        the point in world space after positionNode, returned by the fragment stage as `three_surface` (tsl-lit.ts)
      And three-shader's lit entry makes Bevy's PbrInput of it and runs the standard material's lighting, fog and tone
        mapping (surface.rs): lit by Bevy's lights and shadows as raea's StandardMaterial is, in three-shader's own
        material rather than SH1's ShaderExt, so the vertex stage is three's (positionNode moves it) and its uniforms
        have 254 slots, not 16
      And a Lambert surface is fully rough and Phong's shininess a roughness, neither metal; a basic, shadow or volume
        node material, or one with a fragmentNode, keeps three's own output
      And in-tsl-lit (a noise-displaced, noise-coloured icosahedron under a directional light): SSIM at least 0.92
      And the status, 2026-11-15: built as planned. in-tsl-lit SSIM 0.9906 against three.js (WebGPURenderer, its WebGL 2
        backend) in Chrome at frame 12, where 0.1.4 drew the same blob black (the 11-15 audit's tsl-displace). Proof:
        sdk/r3f test/tsl.test.ts (the lit contract: three_surface returns base, normal, emissive and world, no three
        light code in the graph, the instance carried flat; a Lambert surface rough and unmetallic) and
        raea-sys-three-shader's surface test; tools/parity.sh runs in-tsl-lit. A displaced mesh's shadow is still its
        undisplaced shape (three-shader's shadow pass is the standard one), and a fragmentNode keeps three's output

    @done
    Scenario: R3F22. TSL compute and storage buffers
      Given the plan
        | crate                 | file                                 | lines | job                                                        |
        | raea-sys-shader       | systems/shader/src/shared.rs         | 130   | storage buffers shared by key, kept, reset to their first contents |
        | raea-sys-compute      | systems/compute/src/three.rs         | 220   | three's kernels: the frame's dispatches run once, in order |
        | raea-sys-three-shader | systems/three-shader/src/storage.rs  | 200   | a node material's storage reads; a mesh drawn as copies    |
        | sdk/r3f               | sdk/r3f/src/input/tsl-compute.ts     | 200   | a ComputeNode's WGSL, each dispatch recorded, the Compute part |
        | sdk/r3f               | sdk/r3f/src/input/tsl-storage.ts     | 90    | a storage attribute's key; a material reading it           |
        | sdk/r3f               | sdk/r3f/src/input/tsl-instances.ts   | 90    | a node material on an InstancedMesh as one mesh of copies  |
      # The plan (r3f-tsl, 11-15). A composition with a compute row already steps every frame from 0, with a fresh App
      # on a seek back (runtime/plugins steps(): a compute isn't frame-pure; engine catch_up), so the engine runs each
      # frame's dispatches once, in order, and a still at frame F runs all of them. What's missing is three's WGSL as a
      # compute, buffers a compute and a material share, and the dispatches as a list a frame.
      #   R3F22a. Engine, compute: a storage buffer shared by key (raea-sys-shader shared.rs: Bevy's ShaderBuffer, made
      #     with its size and first contents, kept from frame to frame, reset to them; a 16-byte one for an empty slot).
      #     A <Compute kernels> (three.rs): each kernel three's own WGSL, its `@group(0)` storage buffers keys of the
      #     shared ones and its uniforms packed by the struct three declares (raea-sys-shader uniform_slots). Its keyed
      #     `runs` column each frame: {epoch, seq, fresh, runs: [[kernel, [x, y, z], {UNIFORM: {field: value}}]]}
      #     (seq: the session's parts in turn), run once for each (epoch, seq) in the frame's compute pass, a fresh one
      #     (a new three session's first) resetting the buffers first; a run whose pipeline or buffer isn't on the GPU
      #     yet is held, never dropped. The computes there are (game_of_life, compute_mesh, bevy-ts compute, sdk/web's)
      #     are unchanged.
      #   R3F22b. Engine, three shader: ThreeShader binds two read-only storage buffers (bindings 11 and 12, the empty
      #     one when unused); a node material's spec lists `storage: [{name, key, size}]` and each stage's
      #     `var<storage, read> NAME` takes its binding (storage.rs). A BufferMesh with `copies` (count, per-copy
      #     attributes, matrices when not all identity) is repeated by fit_mesh count times, the copy's number in its own
      #     attribute `tsl_copy`; such a mesh, and one reading storage, isn't frustum culled (its vertices are placed on
      #     the GPU).
      #   R3F22c. sdk/r3f, compute: a ComputeNode compiled by three's WGSLNodeBuilder against the stand-in, cached per
      #     node and version; renderer.ts's compute and computeAsync (a node or a list) record its kernel, the dispatch
      #     size as three's WebGPU backend works it out and its uniforms' values then (three's node updates, as a
      #     material's); the walk empties gl.__raea.computes into one Compute part a frame (kernels, buffers, runs; the
      #     stand-in's epoch; fresh on its first). <Three program>'s renderer is the same stand-in, so a program's
      #     dispatches are drawn the same way.
      #   R3F22d. sdk/r3f, materials: a storage binding in a node material (`.element(i)`) and an attribute three made of
      #     a storage attribute (`.toAttribute()`) read the shared buffer by its attribute's key (tsl-storage.ts). A node
      #     material on an InstancedMesh (or a Sprite with count above 1) is compiled for copies: three built with no
      #     uniform room, so the copies' matrices and colours are attributes; instance_index is `tsl_copy`, the matrices
      #     identity (baked into the copies), instanceColor and any other per-copy attribute the copy's own.
      #     tsl-instances.ts's nodeInstancedParts makes the one mesh (cached on the geometry, count, instanceMatrix.version
      #     and instanceColor's), undefined for what it doesn't take (several materials, GLSL), from scene.ts's
      #     instancedParts before copyParts (agreed with r3f-gpu, 11-15).
      #   R3F22e. in-tsl-particles: 65,536 particles in a disc, placed by an init compute and orbited each frame by an
      #     update compute at a speed by radius, drawn as one InstancedMesh of small quads (positionNode their storage
      #     position, colorNode a second buffer: one of four arms by the first angle, so the arms wind into a spiral only
      #     if every frame's update ran), additive; against three.js's WebGL 2 backend in headless Chrome (as in-tsl-lit),
      #     frame 12, SSIM at least 0.92.
      Given TSL storage (`instancedArray`, `storage`, `attributeArray`), compute nodes (`Fn(...)().compute(n)`) run with
        `gl.compute` / `computeAsync` in useFrame, and a material reading the buffers (`.element(instanceIndex)`,
        `.toAttribute()`) on an InstancedMesh, Points or Sprite
      When raea renders it
      Then each compute's WGSL (three's WGSLNodeBuilder) runs as a raea <Compute> once each frame it was dispatched, in
        the order dispatched, its storage buffers kept from frame to frame and shared with the materials that read them
      And a render that starts past frame 0 steps every compute from frame 0, so a render equals its play
      And in-tsl-particles (65,536 particles orbited by a compute): SSIM at least 0.92 against three.js on WebGPU
      And the status, 2026-11-15: built as planned, on an InstancedMesh. in-tsl-particles SSIM 0.9510 against three.js
        (WebGPURenderer, its WebGL 2 backend) in Chrome at frame 12: the init and every frame's update ran on raea's GPU
        (the arms wind into the same spiral), drawn as one mesh of 65,536 copies reading both buffers; in-tsl-lit 0.9906
        and in-shader 0.9935 unchanged. Two engine fixes came with it: runtime-scene keys a compute row's `kernels`,
        `shared` and `runs` frame by frame (it keyed only entry, bind and uniforms, so a run after frame 0 never reached
        the engine), and three-shader's unlit entry premultiplies an additive material's colour with no alpha
        (three_blend in tone.wesl: Bevy blends AlphaMode::Add as premultiplied alpha, so three's additive overlaps,
        GLSL ShaderMaterials' too, drew as one layer). Proof: sdk/r3f test/compute.test.tsx (kernels, shared keys, runs
        once a frame with their uniforms, workgroups as three's backend, the copies mesh and its WGSL, the parity swirl),
        raea-sys-compute three.rs and three-shader storage.rs tests (runs packed once by (epoch, seq), copies repeated
        with their number, matrix and values), tools/parity.sh in-tsl-particles. Not yet: Points and a Sprite with a
        count (drawn as before), per-vertex storage reads, indirect dispatch, a compute first dispatched after frame 0
        (runtime-scene makes compute rows from the first frame's items), and a texture bound in a compute. Later the
        same day: an unlit node material's colour is tone-mapped by Bevy's own tone_mapping after the camera's colour
        grading (tone.wesl), as a lit or standard material's is, where it had a curve of three-shader's own without
        the Grade exposure that stands for three's ACES 1/0.6 (R3F25), so it came out darker and redder:
        in-tsl-particles 0.9799 (it was 0.9510, the "saturation" was this), and in-tsl-fbo, a node material reading a
        target raea draws on its GPU (flipsY, onGpu), 0.9397 (0.9125 before); in-tsl-lit, in-lettra, in-tslpost and
        in-tslpost2 unchanged, in-lettrafx 0.9890 (0.9855 on main without it, its last glyph mid-wipe narrower)

    @done
    Scenario: R3F23. Render targets are drawn on the GPU
      Given what an app draws into a render target itself (`gl.setRenderTarget(t); gl.render(scene, camera)`, drei's
        useFBO, <RenderTexture>, <MeshReflectorMaterial>, <MeshPortalMaterial>, three's GPUComputationRenderer, a
        ping-pong between two targets, a feedback or trail effect)
      When raea draws it
      Then that render is a camera of raea's engine drawing that scene into an image the frame's materials read the
        target's texture as, before the cameras that read it, in the order the app drew them
      And a fullscreen quad drawn into a target (a GPGPU pass, a blur) is that quad's shader run into the image
      And in-fbo, in-pingpong (a GPUComputationRenderer wave) and in-reflector: SSIM at least 0.92 against three.js
      # The plan (r3f-gpu, 11-15). The engine already draws more than one camera (the camera rows cam, cam1, ...: each
      # with its order, clear and layers, runtime/look camera.rs, raea-sys-frame camera_pass) and names images cameras
      # draw into ("render:NAME:WxH", runtime/plugins fields.rs, RenderImages), which a three shader's texture reads.
      # What's missing is a camera row drawing into one of them, and a standard material's map reading one.
      #   R3F23a. Engine: a camera row's `target` ("render:NAME:WxH", or ":float" after it for an Rgba16Float image a
      #     GPGPU pass needs) puts the camera on that image (RenderTarget::Image), cleared to its own `bg` (rgba) or not
      #     (`clear` none), not tone-mapped (three writes a target linear), drawing only its `layers`; `order` puts it
      #     before the cameras that read it. A mesh row's maps read "render:NAME" as that image (maps.rs through
      #     RenderImages). raea/3d's cameras take `target` and `layers`.
      #   R3F23b. sdk/r3f: session.ts's `render` while a target is set records a pass {target, scene, camera, clear}
      #     in draw order (gl.__raea.passes, emptied as each frame steps; agreed with lane r3f, 11-15). targets.ts
      #     keeps the CPU passes it does (<ContactShadows>' depth and blur); every other pass becomes its scene's parts
      #     (scene.ts's walk) on a layer of its own, and its camera as a camera part with target
      #     "render:<target uuid>:WxH" (":float" for a Float or HalfFloat target), order before the scene's camera by
      #     draw order, clear the pass scene's background or the renderer's clear colour. A texture whose source is a
      #     target (srcOf, a material's map, a ShaderMaterial's sampler uniform) is "render:<target uuid>".
      #   R3F23c. Ping-pong and feedback: two targets swapped frame by frame are two cameras each drawn on its frames
      #     (`active` false on the others) and a texture that names the frame's; the images last from frame to frame.
      #     A render that starts past frame 0 (a still) steps the passes from frame 0 as R3F22 steps computes (shared
      #     with lane r3f-tsl).
      #   R3F23d. in-fbo (the audit's fbo probe: a torus knot drawn into a useFBO shown on a plane), in-pingpong
      #     (GPUComputationRenderer's wave), in-reflector (<MeshReflectorMaterial>: its mirrored camera's pass, its
      #     blur, and its material's projective read; the read is an onBeforeCompile patch, so it lands with R3F24).
      # Status (11-15): R3F23a and R3F23b written, unit-tested and in the tree. The engine's camera into an image is a
      # "rendercam" row (raea-runtime-look rendercam.rs: its image in `into`, since a live row's `target` names the row
      # a component is on; `on` keyed by step; the ambient light on its own layers, the screen camera's likewise), spawned
      # by raea-render's render_camera (raea-sys-frame camera.rs into_image: IntoImage, no MSAA or dither, Hdr for a
      # float image); every "render:" source resolves through raea-sys-frame's render_image.rs. sdk/r3f: a target is
      # named "render:<texture uuid>@WxH:WxH" (a resized target is a new image), counted as drawn on the GPU once a pass
      # drew into it (targets.ts onGpu: three's PMREM stand-in isn't), read with v turned over as a flipY picture is
      # (texture.ts flipsY: scene.ts, shader.ts; tsl.ts's node textures next); a pass is recorded on the renderer that
      # drew it (targets.ts renderInto, its __raea.passes, emptied by step() as session.ts and program.ts step a frame),
      # its parts taken when it's drawn (two passes of one quad with another uniform each keep their own), and cube or
      # 3D targets stay left out. in-fbo (test/parity/targets.tsx, R3F_CASES="in-fbo:12" until tools/parity.sh lists
      # it): SSIM 0.9557 against three.js at frame 12. What's left of the gap is the unlit plane showing the target, drawn
      # without three's ACES 1/0.6 (Bevy's Exposure scales lit shading only; a plain meshBasicMaterial is as far off): the
      # pass itself matches (a pass camera keeps ev100 9.7, as three's linear pass is lit).
      # Status (11-15, later): R3F23c built. A pass is keyed by its target, what it drew (its scene and the lone mesh's
      # material) and its count, so GPUComputationRenderer's start and step passes into each of its two targets keep their
      # own layer and camera frame to frame. A material's texture follows the target drawn this frame: a map naming render
      # images is keyed (maps, map_k; raea-sys-rendermap), a ShaderMaterial's (and a node material's) through its
      # three:uniforms rows (three-shader's patch, by texture name). A three shader drawn into a target writes its colour as
      # it is (output "target": three tone-maps and encodes only what it draws to the screen). A still with a camera into a
      # render image steps every frame before it (runtime-scene carries_state), so a start drawn at frame 0 is there at 12.
      # Two fixes on the way: the engine's load empties its render images before its rows spawn (runtime/plugins emptied
      # them after, so a three:uniforms row made a second, blank image under the same name and every ShaderMaterial read a
      # target as black), and a flipY=false DataTexture keeps its rows in order (texture.ts bytesOf turned it upside down,
      # on maps and in shaders). R3F23d: in-fbo 0.9941 and in-pingpong 0.9949 against three.js at frame 12, both in
      # tools/parity.sh; the full list of 38 passes. Left: in-reflector, whose material's projective read is an
      # onBeforeCompile patch (R3F24).
      # Status (11-15, done): in-reflector 0.9256 (test/parity/patched.tsx, in tools/parity.sh) with R3F24's patched
      # program. Its mirrored camera copies the screen camera's projection into a camera of its own (fov 50, aspect 1 as
      # made), so a pass camera's lens is now its projection matrix's (passes.ts lensOf: fov, and its aspect where that
      # isn't the target's shape), kept by the engine whatever the image's shape (a rendercam's `aspect`, matchmove's
      # OffAxis). What's left of the gap is the floor, which raea lights lighter than three wherever it is that dark: a
      # plain MeshStandardMaterial of the patch's colour reads (11, 12, 13) against three's (6, 7, 9), as the reflector does.

    @done
    Scenario: R3F24. onBeforeCompile patches
      Given a material patched by onBeforeCompile (drei's MeshWobbleMaterial and MeshDistortMaterial, a patched
        MeshStandardMaterial's vertex displacement)
      When raea draws it
      Then the patched GLSL is what runs: three's program for that material with the patch applied, translated as R3F11's
      And in-patched: SSIM at least 0.92 against three.js
      Status (11-15): built. A MeshStandardMaterial or MeshPhysicalMaterial with an onBeforeCompile of its own is three's
        ShaderLib program handed to it as three hands it (src/input/patched.ts): the material's defines and cloned
        uniforms, patched once a program (again when its version or customProgramCacheKey changes, as three recompiles),
        so a uniform the patch shares (drei's time) or one an app keeps from the patch (userData.shader) is read each
        frame; the material's own values refreshed into it each frame as three's renderer does; the defines three's
        WebGLProgram puts in front for its maps (map, alpha, emissive, roughness, metalness, normal, bump and
        displacement), sides, vertex colours and alpha test, and its light counts at 0. Its lighting is Bevy's, as a lit
        TSL material's is (R3F21): the fragment is cut at <lights_physical_fragment> and writes base, normal and
        roughness, emission and metalness, and the point in world space to four outputs, which raea-sys-three-shader's
        lit GLSL stage returns as its surface (glsl.rs Source lit). A patch past the cut (the light, the output colour)
        is named once and left out; so are aoMap and lightMap, and a physical material's sheen, clearcoat and
        transmission. in-patched 0.9717 (drei's MeshWobbleMaterial and MeshDistortMaterial and an app's own waves kept
        through userData.shader, lit by ambient and a sun) and in-reflector 0.9256 (R3F23), both in tools/parity.sh, and
        the full list of 41 passes, every other case as it was. On
        the way: the white picture a ShaderMaterial gets for a texture with no picture (shader.ts WHITE) was a broken
        PNG (its data cut short, its CRC wrong), so every material given it was refused and drawn in its colour; now a
        decoding one, tested. Tests: test/patched.test.ts (the patched program, patched once, its uniforms each frame,
        maps, a patch past the lights named), glsl.rs a_lit_stage_returns_its_surface, test/shader.test.ts (both
        stand-in pictures decode).

    Scenario: R3F25. Post-processing: three's TSL passes and postprocessing's other effects
      Given three/webgpu's PostProcessing (`pass(scene, camera)` with bloom, GTAO, SSR, TRAA, DOF, film, ... nodes) or
        any effect @react-three/postprocessing exports
      When raea renders it
      Then each becomes raea's effect that does the same (Bloom, AmbientOcclusion, DepthOfField, AntiAliasing, the
        ray-traced reflections) or a <FramePass> raea ships for it: Noise, Scanline, Sepia, DotScreen, ColorAverage,
        ColorDepth, Grid, Glitch, ShockWave, TiltShift, ASCII, LUT, GodRays, Outline
      And none is left out without a warning; in-post2 and in-tslpost: SSIM at least 0.92 against three.js
      Status (11-15, in progress): postprocessing's Noise, Scanline, Sepia, DotScreen, ColorAverage, ColorDepth, Grid,
        Glitch, ShockWave, LUT3D and @react-three/postprocessing's ASCII and TiltShift2 are FramePasses with their own
        shaders in WGSL (sdk/r3f src/input/effects.ts, effects-sampled.ts); TiltShift (one Gaussian for its Kawase blur)
        and GodRays (the sun's disc against the depth prepass, unblurred) are close and say so. A FramePass's uv now spans
        the camera's Viewport, not the Engine's w x (h + 1) target (systems/framepass: patterns drifted a row by the
        frame's foot). three's tone exposure (toneMappingExposure, ACES's 1/0.6) is a Grade `exposure`, Bevy's colour
        grading on every pixel before the curve (new: grade_exposure), not the camera's, which skipped unlit colours (0.74
        stops dark under ACES) and made effects before a composer's ToneMapping read lit and unlit pixels in two scales.
        in-post2 0.9879, in-post3 0.9767; the full parity list unchanged. three/webgpu's PostProcessing (RenderPipeline):
        its render() hands the stand-in renderer a QuadMesh whose fragmentNode is the chain, read for its pass(scene,
        camera), which raea draws as the frame, and its nodes in the order the frame goes through them
        (src/input/tsl-post.ts): bloom as a Bloom added on top (new: Bloom `additive`, bloom_additive, Bevy's additive
        composite; three's UnrealBloom brightens where Bevy's natural bloom trades light), GTAO, DOF, FXAA, SMAA (TRAA as
        SMAA), SSAA and pixelation passes as raea's, film, dotScreen, rgbShift, chromaticAberration, lut3D, sepia and
        bleach as FramePasses doing what their setup() builds (effects-tsl.ts), the output's tone mapping and exposure as
        raea's; the other display nodes and an Fn of the app's own are named once (kinds.ts TSL_POST covers every node
        three ships). A FramePass's frame() read past the edge now takes the edge pixel, not the tag row under the
        Viewport. in-tslpost 0.9737 (bloom), in-tslpost2 0.9233 (sepia, chromatic aberration). Left: Outline (selected
        meshes in a composition pass, once scene.ts is free), Texture, Depth, LUT1D, and the background under a
        composer's or a PostProcessing's tone mapping (three maps it, raea shows it as given)
      Later (11-15): a scene drawn through a composer is drawn into its float buffer by three, where a ShaderMaterial's
        colour is scene light and its tone mapping and colour space includes do nothing; it went as "display" (the
        canvas's value), drawn darker and more saturated than three's, the composer's tone mapping and encoding never
        reaching it. It now goes as "linear" under a composer (shader.ts composing): in-shaderbloom (a ShaderMaterial
        and a faint bloom, test/parity/post2.tsx) 0.9758 to 0.9848, the ball's mean colour 133,66,33 to 186,135,96
        against three's 194,145,110. And the composer's own passes into its targets, which R3F23 drew on raea's GPU,
        aren't drawn under a composer raea draws as its effects (targets.ts passesOf keeps the passes whose pictures
        the frame reads): one bloom was 17 cameras, the scene drawn again into the composer's buffer and 16 quads for
        its luminance and mip chain, and is none; a target the frame shows is still drawn (test/composer.test.tsx).
        in-post's frame is the same to the pixel; the full list of 42 passes

    @done
    Scenario: R3F26. <R3F> in a box smaller than the page (#130)
      Given an <R3F> in a box of its own inside a larger page
      When raea draws it
      Then its camera's aspect and projection are the box's, centred on the box, as R3F's <Canvas> is in a browser
      And #130's five cases each draw the plane at its expected size and place
      Status (11-15): done. The cause was the engine's, not <R3F>'s: a camera in a page element (docs/sdk.feature S3a)
        is drawn as the element's box's own view (a SubCameraView: the box's size as the full view, offset by its
        place), but raea-render's tag_row_view gave every camera on the Engine's target a w x h Viewport on the frames
        before the page had laid the box out, and skipped a camera with a Viewport after that, so the box's view never
        came and the page's was cropped to the box (raea's own camera in a box too). The Viewport it gives is now taken
        off when a box comes (the new crate raea-sys-viewbox, with its tests). #130's cases: 600 x 600 at fov 90, 80 px
        centred in the box (was 120 px centred on the page); fov 32, 278 px (three: 279); 1080 x 1920, 80 px (was
        nothing); 2160 x 3840 with the box at (800, 1600), 80 px in the box. <R3F width height> (the box's size)
        makes R3F's own size, its camera's aspect and an orthographic camera's frame the box's: an orthographic plane
        R3F frames at zoom 40 is 80 px in a 400 x 300 box with them, 40 px without (R3F sizes it from the frame)

    @done
    Scenario: R3F27. R3F's pointer events and controls in a live play
      Given an <R3F> in a game played live: meshes with onClick, onPointerOver, onPointerMove; drei's <OrbitControls>
      When the player moves and clicks the pointer over the 3D
      Then R3F's events fire on the mesh the pointer is over (raycast from raea's pointer through the <R3F>'s camera), and
        controls move the camera, while a render (no pointer) is unchanged
      And a recorded input log replays to the same frames
      Status (11-15): done. raea's input, frame by frame, is sent to R3F's canvas as the DOM events a browser would send
        (sdk/r3f src/input/pointer.ts): the move, the presses, a release and its click, the right button's context
        menu, the wheel, the pointer gone, each against the frame before, with the modifier keys held. The root runs
        R3F's own event manager on a stand-in canvas that is an event target (the renderer's domElement too), whose
        document gets the events that bubble, so R3F raycasts each through its camera and calls the handlers on the
        mesh it hits, and drei's <OrbitControls> (three-stdlib's, listening on the canvas and its document) turn and
        zoom the camera. A step sends its frame's events before it renders, inside flushSync, so what a handler sets is
        drawn that step; a render with no input sends none, and a root stepped again from frame 0 sends the same. An
        <R3F> in a box takes the pointer in its box (page.viewBox, as events3d picks), scaled to its width and height.
        Live play ran every composition on raea's lite runtime, which can't run R3F's reconciler: its bundle failed
        ("use" isn't exported by lite), so an <R3F> never played live. An edit whose own sources import raea/r3f or
        @react-three/ now plays on React, as RAEA_LIVE_REACT=1 does (raea-runtime-live needs_react; an exported game
        the same). Measured: test/pointer.test.tsx 7/7 (over, down, a click with its point, out, a missed click; a drag
        turning OrbitControls' camera and the wheel moving it back; the same input stepped twice ends the same); a
        render with an input log: the ball hovered, clicked, and the box's <R3F> (400 x 400 at 100,100 of 600 x 600)
        the same; bench/render-play's new row sdk/r3f/test/live/pointer.tsx (hover, click, a drag on the empty frame)
        plays and renders the same frames, 1.000000 at frames 6, 12, 18 and 27, and the whole bench 13 of 13 at
        1.000000. Not yet: a double click, touches as pointers, a timeline baked for the player (raea-runtime-bake
        bakes on the lite runtime)
