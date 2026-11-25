// #162's composition, which bench/at-time/check.sh draws: one face, its left half grey by Bevy's globals.time and its
// right half by a <Material> uniform the frame gives its time in seconds (each at a quarter of it), so the halves match
// only when Time is the frame drawn's. "time" holds nothing a plugin carries, so a still draws only its own frame
// (#77); "stepped" is the same face with an animation player on it (it plays nothing), which Bevy carries from frame
// to frame, so a still is stepped to through every frame before it.
import { Composition, registerRoot, useCurrentFrame } from "raea";
import { Transform } from "raea/transform";
import { Camera3d } from "raea/camera";
import { Cuboid, Mesh3d } from "raea/mesh";
import { BevyComponent } from "raea/3d";
import { Material } from "@raea/render/bevy3d";

function Shot({ stepped }: { stepped: boolean }) {
  const f = useCurrentFrame();
  return (
    <>
      <Mesh3d transform={Transform.fromXyz(0, 0, 0)}>
        <Cuboid half_size={[2, 2, 0.01]} />
        <Material fragment="time.wesl" uniforms={{ seconds: f / 30 }} />
        {stepped ? <BevyComponent type="bevy_animation::AnimationPlayer" value={{ active_animations: {} }} /> : null}
      </Mesh3d>
      <Camera3d transform={Transform.fromXyz(0, 0, 3).lookingAt([0, 0, 0])} />
    </>
  );
}

const Pure = () => <Shot stepped={false} />;
const Stepped = () => <Shot stepped />;

registerRoot(() => (
  <>
    <Composition id="time" component={Pure} width={64} height={64} fps={30} durationInFrames={120} />
    <Composition id="stepped" component={Stepped} width={64} height={64} fps={30} durationInFrames={120} />
  </>
));
