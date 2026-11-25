// R3F's pointer events and drei's <OrbitControls> in a live play (docs/r3f.feature R3F27): the ball turns yellow under
// the pointer and green once clicked, and a drag on the empty frame turns the camera. bench/render-play plays it with
// pointer.json's input and renders the input it logged: the same frames (render = play).
import { useState } from "react";
import { Composition, registerRoot } from "raea";
import { R3F } from "raea/r3f";
import { OrbitControls } from "@react-three/drei";

function Ball() {
  const [hot, setHot] = useState(false);
  const [on, setOn] = useState(false);
  return (
    <mesh position={[-1.2, 0, 0]} onPointerOver={() => setHot(true)} onPointerOut={() => setHot(false)} onClick={() => setOn((v) => !v)}>
      <sphereGeometry args={[0.8, 32, 16]} />
      <meshBasicMaterial color={on ? (hot ? "#88ff99" : "#22dd44") : hot ? "#ffcc00" : "#4466ff"} />
    </mesh>
  );
}

const Pointer = () => (
  <R3F flat camera={{ position: [0, 0, 5], fov: 50 }}>
    <color attach="background" args={["#111"]} />
    <OrbitControls enableDamping={false} />
    <Ball />
    <mesh position={[1.2, 0, 0]}>
      <boxGeometry args={[1.2, 1.2, 1.2]} />
      <meshNormalMaterial />
    </mesh>
  </R3F>
);

registerRoot(() => <Composition id="pointer" component={Pointer} width={640} height={360} fps={30} durationInFrames={60} />);
