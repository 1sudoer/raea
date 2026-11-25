// The window render's films (docs/realtime.feature RT1; the plan's experiment 2): RR0's six shots (bench/rerender/
// shots.tsx: a procedural set with grass, trees, a room and a street, a look and a camera per 5 s shot) played round
// for 60 s and for 180 s at 1920x1080, 30 fps. window.sh renders the same 5 s window of each, as the app's draft
// renders the seconds around the playhead (`edit={{ segments, scale }}`), so any difference in its cost is the
// length of the film around it.
import { Composition, registerRoot, useCurrentFrame } from "raea";
import { FPS, Frame, SHOT } from "../../rerender/shots";

function Round() {
  const f = useCurrentFrame();
  return <Frame k={Math.floor(f / SHOT) % 6} t={(f % SHOT) / FPS} />;
}

registerRoot(() => (
  <>
    <Composition id="film-60" component={Round} width={1920} height={1080} fps={FPS} durationInFrames={60 * FPS} />
    <Composition id="film-180" component={Round} width={1920} height={1080} fps={FPS} durationInFrames={180 * FPS} />
  </>
));
