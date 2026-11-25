// #164: inside <R3F>, an object scaled to 0 draws nothing, as three draws it. Green is scaled to 0 three ways (a group's
// scale={0}, a mesh's [0, 0, 0], and a pop-in from 0 before it starts); the red box is the control, drawn at half size,
// so a frame with no green says the zero scales drew nothing, not that nothing was drawn. Before the fix every green
// box drew at full size (three's Matrix4.decompose gives a matrix with no volume the scale 1).
import { AbsoluteFill, Composition, interpolate, registerRoot, useCurrentFrame } from "raea";
import { R3F } from "raea/r3f";

// lit: a standard material under a light casting shadows onto a floor, as a character is drawn
function Scene({ lit }: { lit: boolean }) {
  const pop = interpolate(useCurrentFrame(), [10, 20], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const green = lit ? <meshStandardMaterial color="#00ff00" /> : <meshBasicMaterial color="#00ff00" />;
  return (
    <AbsoluteFill style={{ background: "#0000ff" }}>
      <R3F flat={!lit} shadows={lit} camera={{ position: [0, 0, 5], fov: 50 }}>
        {lit && <directionalLight position={[2, 4, 5]} intensity={3} castShadow />}
        {lit && <ambientLight intensity={0.5} />}
        {lit && (
          <mesh position={[0, 0, -1.5]} receiveShadow>
            <planeGeometry args={[20, 20]} />
            <meshStandardMaterial color="#0000ff" />
          </mesh>
        )}
        <mesh position={[-1.4, -1.4, 0]} scale={0.5} castShadow>
          <boxGeometry />
          {lit ? <meshStandardMaterial color="#ff0000" /> : <meshBasicMaterial color="#ff0000" />}
        </mesh>
        <group scale={0}>
          <mesh>
            <boxGeometry args={[2, 2, 2]} />
            {green}
          </mesh>
        </group>
        <mesh position={[1.4, 1.4, 0]} scale={[0, 0, 0]}>
          <boxGeometry />
          {green}
        </mesh>
        <mesh position={[-1.4, 1.4, 0]} scale={pop}>
          <boxGeometry />
          {green}
        </mesh>
      </R3F>
    </AbsoluteFill>
  );
}
registerRoot(() => (
  <>
    <Composition id="zero" component={() => <Scene lit={false} />} width={160} height={160} fps={30} durationInFrames={1} />
    <Composition id="zero-lit" component={() => <Scene lit />} width={160} height={160} fps={30} durationInFrames={1} />
  </>
));
