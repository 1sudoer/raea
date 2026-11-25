// RR0's film (docs/rerender.feature): six 5 s shots at 1920x1080, 30 fps (900 frames), each with its own look and
// camera, on one procedural set (no media files, so it renders the same on macOS, Windows and Linux). Its forms:
//   "film"            one composition, one camera that jumps at the cuts and a look that changes at every cut: what
//                     RR1 makes render correctly (before RR1 a render holds the first frame's look for all six)
//   "shot-1".."shot-6" each shot as its own composition, as agents split films today; bench.sh renders these as
//                     chunks and joins them, the baseline the overhaul is measured against
//   "cut"             the film with each shot a Sequence of its own, so its timeline knows the cuts (RR3: the video
//                     splits into segments there); the same frames as "film"
//   "cut-long"        "cut" four times over, 2 minutes (3,600 frames): RR3's join is timed on it
// Shot 4 (noir) is the one change.sh edits, as a note would.
import { Composition, Sequence, registerRoot } from "raea";
import { FPS, Film, SHOT, SHOTS, one } from "./shots";

const ONE = SHOTS.map((_, k) => one(k));
/** `n` shots in turn, each a Sequence, the six over and over. */
const cut = (n: number) =>
  function Cut() {
    return (
      <>
        {Array.from({ length: n }, (_, i) => {
          const Shot = ONE[i % 6];
          return <Sequence key={i} from={i * SHOT} durationInFrames={SHOT}><Shot /></Sequence>;
        })}
      </>
    );
  };

registerRoot(() => (
  <>
    <Composition id="film" component={Film} width={1920} height={1080} fps={FPS} durationInFrames={SHOT * 6} />
    {SHOTS.map((_, k) => <Composition key={k} id={`shot-${k + 1}`} component={one(k)} width={1920} height={1080} fps={FPS} durationInFrames={SHOT} />)}
    <Composition id="cut" component={cut(6)} width={1920} height={1080} fps={FPS} durationInFrames={SHOT * 6} />
    <Composition id="cut-long" component={cut(24)} width={1920} height={1080} fps={FPS} durationInFrames={SHOT * 24} />
  </>
));
