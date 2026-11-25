// A game with sound (raea/tone): a ball run by a state machine (sdk/raea/src/machine.ts), its landings firing a
// MembraneSynth one-shot on their exact frame (<Sfx>: seeded pitch and volume variation, at most 4 voices, the oldest
// stolen), a coin every 45th frame (an SAO sound, made once, round-robin over two pitches); a chiptune loop on the
// music bus (synths on purpose, so the bench needs no sample files: a square lead that asks for two bars and answers
// for two, a bass walking C Am F G, a plucked arpeggio of each chord), ducked 12 dB under a voice line on the voice
// bus (voice.wav, made by the check: `say`, not committed).
// The render's bounce and a play's tap must be the same samples (sdk/react/bench/tone/check.sh).
import { AbsoluteFill, Audio, Composition, Mix, Sequence, machine, registerRoot, staticFile, useCurrentFrame, useGameEvents, useGameState } from "raea";
import type { Snapshot } from "raea/machine";
import * as Tone from "raea/tone";

const FLOOR = 300;

// ---- the game: a ball whose life is a machine --------------------------------------------------------------------
interface Ball { y: number; vy: number; sm: Snapshot }
const ball = machine<Ball, null>()({
  id: "ball",
  initial: "fall",
  states: {
    fall: { tick: (b) => ({ ...b, vy: b.vy + 0.8, y: Math.min(FLOOR, b.y + b.vy + 0.8) }), always: { to: "land", when: (b) => b.y >= FLOOR } },
    land: { tick: (b) => ({ ...b, y: FLOOR, vy: -Math.max(9, Math.abs(b.vy) * 0.8) }), after: { 1: "rise" } },
    rise: { tick: (b) => ({ ...b, vy: b.vy + 0.8, y: b.y + b.vy + 0.8 }), always: { to: "fall", when: (b) => b.vy >= 0 } },
  },
});
interface Game { ball: Ball; frame: number }
const INITIAL: Game = { ball: { y: 40, vy: 0, sm: ball.start() }, frame: 0 };
const fold = (g: Game, _input: unknown, frame: number): Game => ({ ball: ball.step(g.ball, null), frame });
const landed = (b: Game, a: Game) => ball.state(a.ball) === "land" && ball.state(b.ball) !== "land";

// ---- the sound -----------------------------------------------------------------------------------------------------
Tone.getTransport().bpm.value = 100;
Tone.buses.music.duck(Tone.buses.voice, { amount: -12, threshold: -45, attack: 0.04, release: 0.4 });
Tone.buses.music.volume.value = -4;
const thump = Tone.vary(new Tone.MembraneSynth({ pitchDecay: 0.03, octaves: 4, envelope: { attack: 0.001, decay: 0.25, sustain: 0 }, maxPolyphony: 4, poly: true }), { pitch: 0.5, volume: 2 }).connect(Tone.buses.sfx);
const coin = new Tone.Sampler({ urls: { C5: Tone.sao("a bright arcade coin chime, short", { seconds: 1 }) }, release: 0.2 }).connect(Tone.buses.sfx);
const lead = new Tone.Synth({ oscillator: { type: "square" }, envelope: { attack: 0.005, decay: 0.12, sustain: 0.3, release: 0.08 }, volume: -16 }).connect(Tone.buses.music);
const bass = new Tone.Synth({ oscillator: { type: "triangle" }, envelope: { attack: 0.005, decay: 0.2, sustain: 0.5, release: 0.1 }, volume: -6 }).connect(Tone.buses.music);
const pluck = new Tone.PluckSynth({ volume: -10 }).connect(new Tone.FeedbackDelay("8n", 0.25).connect(Tone.buses.music));
const loop = (part: Tone.Part) => {
  part.loop = true; // set before start, as in Tone
  part.loopEnd = "4m";
  part.start(0);
};
// the lead: a question over C and Am, then its answer, the same rhythm moved onto F and G, ending on the D that
// leads back into the loop's first E
loop(new Tone.Part((t, n) => lead.triggerAttackRelease(n.note, n.dur, t, n.vel), [
  { time: "0:0", note: "E5", dur: "8n", vel: 0.8 }, { time: "0:0:2", note: "G5", dur: "8n", vel: 0.5 }, { time: "0:1:2", note: "E5", dur: "8n", vel: 0.6 },
  { time: "0:2", note: "C5", dur: "4n", vel: 0.75 }, { time: "0:3", note: "D5", dur: "8n", vel: 0.55 },
  { time: "1:0", note: "E5", dur: "4n", vel: 0.8 }, { time: "1:1", note: "C5", dur: "8n", vel: 0.5 }, { time: "1:1:2", note: "A4", dur: "8n", vel: 0.6 }, { time: "1:2", note: "A4", dur: "4n", vel: 0.7 },
  { time: "2:0", note: "F5", dur: "8n", vel: 0.8 }, { time: "2:0:2", note: "A5", dur: "8n", vel: 0.5 }, { time: "2:1:2", note: "F5", dur: "8n", vel: 0.6 },
  { time: "2:2", note: "C5", dur: "4n", vel: 0.75 }, { time: "2:3", note: "D5", dur: "8n", vel: 0.55 },
  { time: "3:0", note: "D5", dur: "4n", vel: 0.8 }, { time: "3:1", note: "B4", dur: "8n", vel: 0.5 }, { time: "3:1:2", note: "G4", dur: "8n", vel: 0.6 }, { time: "3:2", note: "B4", dur: "8n", vel: 0.7 }, { time: "3:3", note: "D5", dur: "4n", vel: 0.65 },
]));
// the bass: a chord a bar, root on one and fifth on three, then a passing note into the next root
const CHORDS: [string, string, string][] = [["C2", "G2", "B1"], ["A1", "E2", "G1"], ["F1", "C2", "A1"], ["G1", "D2", "B1"]];
loop(new Tone.Part((t, n) => bass.triggerAttackRelease(n.note, n.dur, t, n.vel), CHORDS.flatMap(([root, fifth, pass], bar) => [
  { time: `${bar}:0`, note: root, dur: "4n.", vel: 0.85 },
  { time: `${bar}:2`, note: fifth, dur: "4n", vel: 0.6 },
  { time: `${bar}:3:2`, note: pass, dur: "8n", vel: 0.5 },
])));
// the pluck: each chord's notes in eighths, up and back
const ARPS = [["C4", "E4", "G4", "C5"], ["A3", "C4", "E4", "A4"], ["F3", "A3", "C4", "F4"], ["G3", "B3", "D4", "G4"]];
loop(new Tone.Part((t, n) => pluck.triggerAttackRelease(n.note, "8n", t), ARPS.flatMap((arp, bar) =>
  [0, 1, 2, 3, 2, 1, 2, 3].map((k, i) => ({ time: `${bar}:${i >> 1}:${(i & 1) * 2}`, note: arp[k] })),
)));
Tone.getTransport().start();

function Game() {
  const frame = useCurrentFrame();
  const g = useGameState(INITIAL, fold);
  const lands = useGameEvents(INITIAL, fold, landed);
  const coinNow = frame > 0 && frame % 45 === 0;
  return (
    <AbsoluteFill style={{ background: "#0e1620" }}>
      <Mix lufs={-14} ceiling={-1.5} />
      <div style={{ position: "absolute", left: 300, top: g.ball.y, width: 40, height: 40, borderRadius: 20, background: "#f2c14e" }} />
      <div style={{ position: "absolute", left: 0, right: 0, top: FLOOR + 40, height: 4, background: "#557" }} />
      <Tone.Sfx when={lands.includes(frame)} play={(t) => thump.triggerAttackRelease("C2", "8n", t)} />
      <Tone.Sfx when={coinNow} play={(t, f) => coin.triggerAttackRelease((f / 45) % 2 ? "C5" : "G5", "8n", t)} />
      <Sequence from={90} durationInFrames={150}>
        <Audio src={staticFile("voice.wav")} bus="voice" />
      </Sequence>
    </AbsoluteFill>
  );
}

registerRoot(() => <Composition id="game" component={Game} width={640} height={400} fps={30} durationInFrames={300} />);
