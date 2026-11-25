// R3F20's speed bars (docs/r3f.feature): an InstancedMesh of 2,000 and 20,000 copies moved every frame, 48 frames at
// 640x360, as the 11-15 audit drew them on 0.1.4 (1,220 ms a frame at 20,000, 94 at 2,000). `raea render speed.tsx
// --composition inst-20k-48f -o out.mp4` prints the ms a frame; the bars are 100 and 40. The -tint compositions colour
// every copy every frame too.
import { useMemo, useRef } from "react";
import { Composition, registerRoot } from "raea";
import { R3F } from "raea/r3f";
import { useFrame } from "@react-three/fiber";
import * as THREE from "three";
import { SIZE } from "./fixtures";

function Field({ n, tint }: { n: number; tint: boolean }) {
  const ref = useRef<THREE.InstancedMesh>(null!);
  const o = useMemo(() => new THREE.Object3D(), []);
  const c = useMemo(() => new THREE.Color(), []);
  const side = Math.ceil(Math.sqrt(n));
  useFrame(({ clock }) => {
    const t = clock.elapsedTime;
    for (let i = 0; i < n; i++) {
      const x = (i % side) - side / 2, z = Math.floor(i / side) - side / 2;
      o.position.set(x * 0.12, Math.sin(x * 0.3 + t * 2) * Math.cos(z * 0.3) * 0.4, z * 0.12);
      o.updateMatrix();
      ref.current.setMatrixAt(i, o.matrix);
      if (tint) ref.current.setColorAt(i, c.setHSL((x / side + t * 0.1) % 1, 0.7, 0.5));
    }
    ref.current.instanceMatrix.needsUpdate = true;
    if (tint) ref.current.instanceColor!.needsUpdate = true;
  });
  return (
    <>
      <color attach="background" args={["#0a0a12"]} />
      <ambientLight intensity={0.3} />
      <directionalLight position={[3, 5, 2]} intensity={2.5} />
      <instancedMesh ref={ref} args={[undefined as never, undefined as never, n]}>
        <boxGeometry args={[0.08, 0.08, 0.08]} />
        <meshStandardMaterial color="#66ccff" />
      </instancedMesh>
    </>
  );
}

const CASES: Record<string, { n: number; tint: boolean; camera: [number, number, number] }> = {
  "inst-2k-48f": { n: 2000, tint: false, camera: [0, 4, 6] },
  "inst-20k-48f": { n: 20000, tint: false, camera: [0, 8, 12] },
  "inst-2k-tint-48f": { n: 2000, tint: true, camera: [0, 4, 6] },
  "inst-20k-tint-48f": { n: 20000, tint: true, camera: [0, 8, 12] },
};

registerRoot(() => (
  <>
    {Object.entries(CASES).map(([id, k]) => (
      <Composition key={id} id={id} component={() => <R3F camera={{ position: k.camera, fov: 45 }}><Field n={k.n} tint={k.tint} /></R3F>} width={SIZE.width} height={SIZE.height} fps={SIZE.fps} durationInFrames={48} />
    ))}
  </>
));
