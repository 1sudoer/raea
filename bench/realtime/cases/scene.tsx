// raster.tsx's scene, shared with caption.tsx: a few parts on a floor under a sun, the camera circling them over 5 s,
// the hero cube's colour a prop.
import { useCurrentFrame } from "raea";
import { Camera3d } from "raea/camera";
import { AmbientLight, DirectionalLight } from "raea/light";
import { Cuboid, Mesh3d, Sphere } from "raea/mesh";
import { StandardMaterial } from "raea/pbr";
import { Transform } from "raea/transform";

export function Scene({ color = "#d04a3a" }: { color?: string }) {
  const f = useCurrentFrame();
  const a = 0.4 + f * 0.012;
  return (
    <>
      <Camera3d transform={Transform.fromXyz(Math.cos(a) * 6, 2.6, Math.sin(a) * 6).lookingAt([0, 0.6, 0], [0, 1, 0])} projection={{ Perspective: { fov: (45 * Math.PI) / 180 } }} />
      <DirectionalLight name="sun" transform={Transform.fromXyz(4, 8, 3).lookingAt([0, 0, 0], [0, 1, 0])} illuminance={6000} shadow_maps_enabled />
      <AmbientLight brightness={120} />
      <Mesh3d name="floor" transform={Transform.fromXyz(0, -0.05, 0)}><Cuboid half_size={[8, 0.05, 8]} /><StandardMaterial base_color="#a8a196" perceptual_roughness={0.9} /></Mesh3d>
      <Mesh3d name="hero" transform={Transform.fromXyz(0, 0.6, 0)}><Cuboid half_size={[0.6, 0.6, 0.6]} /><StandardMaterial base_color={color} perceptual_roughness={0.45} /></Mesh3d>
      <Mesh3d name="ball" transform={Transform.fromXyz(1.6, 0.45 + 0.25 * Math.abs(Math.sin(f / 9)), 0.8)}><Sphere radius={0.45} /><StandardMaterial base_color="#3a7bd0" perceptual_roughness={0.3} metallic={0.2} /></Mesh3d>
      <Mesh3d name="post" transform={Transform.fromXyz(-1.5, 0.9, -1)}><Cuboid half_size={[0.2, 0.9, 0.2]} /><StandardMaterial base_color="#e2c044" perceptual_roughness={0.7} /></Mesh3d>
    </>
  );
}
