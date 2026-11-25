// three.js ShaderMaterials as raea's ThreeShaderMaterial (src/input/shader.ts, docs/r3f.feature R3F11): the GLSL as
// three would compile it, uniforms as numbers, textures as pictures, Points as quads the engine sizes by gl_PointSize.
import { describe, expect, test } from "bun:test";
import * as THREE from "three";
import { expandIncludes, ownAttributes, shaderPart } from "../src/input/shader";
import { materialOf } from "../src/input/material";

describe("ShaderMaterial → raea's ThreeShaderMaterial", () => {
  test("its GLSL with three's chunks expanded, its defines in front, its uniforms as numbers", () => {
    const m = new THREE.ShaderMaterial({
      defines: { WAVES: 3 },
      uniforms: { uTime: { value: 1.5 }, uColor: { value: new THREE.Color("#ff8000") }, uOffset: { value: new THREE.Vector2(1, 2) }, uM: { value: new THREE.Matrix3() } },
      vertexShader: "#include <common>\nvoid main(){ gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }",
      fragmentShader: "uniform vec3 uColor;\nvoid main(){ gl_FragColor = vec4(uColor, 1.0);\n#include <colorspace_fragment>\n}",
    });
    const part = materialOf(m, false);
    expect(part.type).toBe("ThreeShaderMaterial");
    const p = part.props as Record<string, any>;
    expect(p.glsl.vertex).toContain("#define WAVES 3\n");
    expect(p.glsl.vertex).toContain("#define PI 3.141592653589793");
    expect(p.glsl.fragment).toContain("linearToOutputTexel");
    expect(p.glsl.fragment).not.toContain("#include");
    expect(p.uniforms.uTime).toBe(1.5);
    expect(p.uniforms.uOffset).toEqual([1, 2]);
    expect(p.uniforms.uM).toEqual([1, 0, 0, 0, 1, 0, 0, 0, 1]);
    expect(p.color).toBe("#ff8000");
    expect(p.output).toBe("display");
  });

  test("one that asks for three's tone mapping is scene light, tone-mapped by Bevy", () => {
    const m = new THREE.ShaderMaterial({ vertexShader: "void main(){ gl_Position = vec4(position, 1.0); }", fragmentShader: "void main(){ gl_FragColor = vec4(1.0);\n#include <tonemapping_fragment>\n#include <colorspace_fragment>\n}" });
    const p = shaderPart(m).props as Record<string, any>;
    expect(p.output).toBe("linear");
    expect(p.glsl.fragment).not.toContain("toneMapping");
  });

  test("a texture uniform is a picture; a flipped one is named for the engine to flip", () => {
    const t = new THREE.Texture();
    (t as unknown as { image: unknown }).image = { src: "pics/a.png" };
    const m = new THREE.ShaderMaterial({ uniforms: { map: { value: t } }, vertexShader: "void main(){}", fragmentShader: "uniform sampler2D map; void main(){}" });
    const p = shaderPart(m).props as Record<string, any>;
    expect(p.textures).toEqual({ map: { src: "pics/a.png", colorSpace: "linear" } });
    expect(p.uniforms.map).toBeUndefined();
  });

  test("an include three doesn't have is an error that names it", () => {
    expect(() => expandIncludes("#include <nope>")).toThrow("<nope>");
  });

  test("a geometry's own attributes, the standard four left out", () => {
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute([0, 0, 0, 1, 0, 0], 3));
    g.setAttribute("aRandom", new THREE.Float32BufferAttribute([0.25, 0.75], 1));
    expect(ownAttributes(g)).toEqual({ aRandom: { size: 1, values: [0.25, 0.75] } });
  });
});

describe("the pictures a ShaderMaterial is given where three has none", () => {
  test("are PNGs that decode: one white pixel, one opaque black", async () => {
    const { inflateSync, crc32 } = await import("node:zlib");
    const { WHITE, BLACK } = await import("../src/input/shader");
    const pixel = (url: string) => {
      const png = Buffer.from(url.split(",")[1], "base64");
      const at = png.indexOf("IDAT");
      const len = png.readUInt32BE(at - 4);
      // the chunk's CRC covers its type and its data
      expect(crc32(png.subarray(at, at + 4 + len))).toBe(png.readUInt32BE(at + 4 + len));
      return [...inflateSync(png.subarray(at + 4, at + 4 + len))];
    };
    expect(pixel(WHITE)).toEqual([0, 255, 255, 255, 255]);
    expect(pixel(BLACK)).toEqual([0, 0, 0, 0, 255]);
  });
});
