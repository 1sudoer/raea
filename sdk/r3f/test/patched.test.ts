// A lit material patched by onBeforeCompile as three's own program with the patch applied (src/input/patched.ts,
// docs/r3f.feature R3F24): patched once as three patches it, its uniforms the material's each frame, its fragment cut
// where three's lights begin and writing the surface raea's engine lights.
import { describe, expect, spyOn, test } from "bun:test";
import * as THREE from "three";
import { materialOf } from "../src/input/material";

/** drei's MeshWobbleMaterial's patch: the vertex turned about y by a uniform the material keeps */
class Wobble extends THREE.MeshStandardMaterial {
  time = { value: 0 };
  compiled = 0;
  override onBeforeCompile(shader: THREE.WebGLProgramParametersWithUniforms) {
    this.compiled++;
    shader.uniforms.time = this.time;
    shader.vertexShader = `uniform float time;\n${shader.vertexShader}`.replace("#include <begin_vertex>", "float theta = sin( time + position.y ) / 2.0;\nvec3 transformed = vec3( position ) * mat3( cos( theta ), 0, sin( theta ), 0, 1, 0, -sin( theta ), 0, cos( theta ) );");
  }
}

const props = (m: THREE.Material) => materialOf(m, false).props as Record<string, any>;

describe("a material patched by onBeforeCompile", () => {
  test("is its patched program, its surface lit by raea's engine", () => {
    const m = new Wobble({ color: "#ff8000", roughness: 0.3, metalness: 0.2 });
    const part = materialOf(m, false);
    expect(part.type).toBe("ThreeShaderMaterial");
    const p = part.props as Record<string, any>;
    expect(p.glsl.lit).toBe(true);
    expect(p.output).toBe("linear");
    expect(p.glsl.vertex).toContain("float theta = sin( time + position.y ) / 2.0;");
    expect(p.glsl.vertex).toContain("#define STANDARD");
    // cut at three's lights: the surface written, none of three's lighting left
    expect(p.glsl.fragment).toContain("layout(location = 3) out highp vec4 raeaWorld;");
    expect(p.glsl.fragment).toContain("raeaNormal = vec4( normal, roughnessFactor );");
    expect(p.glsl.fragment).not.toContain("RE_Direct");
    expect(p.glsl.fragment).not.toContain("directionalLights");
    expect(p.uniforms.roughness).toBeCloseTo(0.3);
    expect(p.uniforms.metalness).toBeCloseTo(0.2);
    expect(p.uniforms.diffuse).toEqual(new THREE.Color("#ff8000").toArray());
    // only what the program declares: none of three's lights or environment
    expect(p.uniforms.ambientLightColor).toBeUndefined();
    expect(Object.keys(p.textures)).toEqual([]);
  });

  test("is patched once, as three compiles it; its uniforms are read each frame", () => {
    const m = new Wobble();
    expect(props(m).uniforms.time).toBe(0);
    m.time.value = 2.5;
    m.roughness = 0.75;
    const p = props(m);
    expect(p.uniforms.time).toBe(2.5);
    expect(p.uniforms.roughness).toBe(0.75);
    expect(m.compiled).toBe(1);
    m.needsUpdate = true;
    props(m);
    expect(m.compiled).toBe(2);
  });

  test("takes its maps as three's program does, read with v as given", () => {
    const pic = new THREE.DataTexture(new Uint8Array([255, 0, 0, 255]), 1, 1);
    pic.needsUpdate = true;
    pic.offset.set(0.5, 0);
    const m = new Wobble({ map: pic, side: THREE.DoubleSide });
    const p = props(m);
    expect(p.glsl.fragment).toContain("#define USE_MAP");
    expect(p.glsl.fragment).toContain("#define MAP_UV uv");
    expect(p.glsl.fragment).toContain("#define DOUBLE_SIDED");
    expect(Object.keys(p.textures)).toEqual(["map"]);
    expect(p.uniforms.mapTransform.slice(6, 8)).toEqual([0.5, 0]);
  });

  test("a patch past three's lights is named once and left out", () => {
    const warn = spyOn(console, "warn").mockImplementation(() => {});
    const m = new THREE.MeshPhysicalMaterial();
    m.onBeforeCompile = (s) => {
      s.fragmentShader = s.fragmentShader.replace("#include <dithering_fragment>", "gl_FragColor.rgb = vec3( 1.0 );");
    };
    const p = props(m);
    props(m);
    expect(p.glsl.vertex).toContain("#define PHYSICAL");
    expect(p.glsl.fragment).not.toContain("gl_FragColor.rgb = vec3( 1.0 );");
    expect(warn.mock.calls.filter((c) => String(c[0]).includes("past three's lighting")).length).toBe(1);
    warn.mockRestore();
  });

  test("an unpatched standard material stays raea's own", () => {
    expect(materialOf(new THREE.MeshStandardMaterial(), false).type).toBe("MeshStandardMaterial");
  });
});

describe("a material drawn without the depth test (docs/img2raea.feature IR5)", () => {
  test("is three's own program, drawn after what is opaque, as three draws it over what it drew before", () => {
    for (const m of [new THREE.MeshBasicMaterial({ color: 0xff0000, depthTest: false, depthWrite: false }), new THREE.MeshStandardMaterial({ depthTest: false })]) {
      const part = materialOf(m, false);
      expect(part.type).toBe("ThreeShaderMaterial");
      expect(part.props.depthTest).toBe(false);
      expect(part.props.transparent).toBe(true);
    }
    expect(props(new THREE.MeshBasicMaterial({ color: 0xff0000, depthTest: false })).color).toBe("#ff0000");
    expect(materialOf(new THREE.MeshBasicMaterial(), false).type).toBe("MeshBasicMaterial");
  });
});
