@spec
Feature: video2raea sound: a film's soundtrack split into dialogue, music and effects, and each layer written as code
  Goal, 2026-11-17: "have our own version of reverse engineer anything video skill or threejs one we were playing
  with too that bring it to raea and then just take the audio layers demuxed and separated and then map them to the
  code."

  img2raea (docs/img2raea.feature) turns a picture into raea objects. This is the same for a video's sound: the mix
  is split into the stems films are mixed from (dialogue, music, effects), and each stem becomes the raea code that
  would make it: dialogue as <Speech> lines from the transcript, music as a beat map and a cue composed to it
  (`raea song`, sdk/guides/craft/motion/music.md), effects as one placed call per sound (its frame, the take or
  prompt that makes it, its gain) and, through the foley adapter, the control curves that keep a new sound on the
  old one's timing.

  The test set is our own films, so every answer is known: tools/foley.ts-style mixes log each placement, and
  `raea render --manifest` lists every sound a render played (element, time, gain, model and prompt).

  Rule: our own films are the answer key

    @wip
    Scenario: VS1. The goose film as the first answer key
      Given projects/swamp-goose's mix (the song and 205 effect placements from 32 takes, tools/foley.ts) levelled to
        -14 LUFS under a limiter (tools/mix.sh)
      And the effects-only stem from the same code (STEM=foley), and a key that logs each take's first sample and
        whether it was placed by its peak (173 of the 205 are, so the logged time is not the take's start)
      When the mix is separated, its effects stem's onsets found (the foley lab's meter) and each matched within 30 ms
        to the onsets the code predicts (every placed take's own onsets inside the span it plays)
      Then measured on 11-17 (lab: ~/.cache/raea-foley/lab/goose, bin/sep_score.py and bin/eventmap.py):
        | stem                         | effects SI-SDR | events found | loud events found | onsets the code explains | take named (top 1) |
        | the true effects stem        | -              | 72%          | 77%               | 89%                      | 57%                |
        | MRX, adapted-loudness        | -5.1 dB        | 61%          | 65%               | 59%                      | 17%                |
        | MRX, default                 | -5.1 dB        | 43%          | 55%               | 61%                      | 24%                |
        | the mix, unseparated         | -12.7 dB       | 27%          | 28%               | 56%                      | 4%                 |
      And always naming the commonest take (a footstep, 82 of the 205) scores 29-47% on the same matches, so naming
        from a separated stem is below that baseline: separation finds the events, the log-mel matcher can't yet say
        which take each one is through the music that leaks into the stem
      And the mix sits 87 samples (2 ms) behind its stems: the limiter's lookahead, removed before scoring

  Rule: separation

    @needs
    Scenario: VS2. A separator that runs on the Mac and keeps its licence
      Given the candidates read on 11-17: MRX (MERL, MIT, weights in the repo; 90 s in 5 s on the CPU), DBS-TIGER
        (Apache-2.0; one 3 s chunk ran past 3 minutes on the CPU, its window code fails on MPS), TIGER-DnR (Apache-2.0,
        an 8-bit ONNX of 7 MB exists), MVSEP's Demucs4 for CDX23 (MIT code; the weights' terms to read), BandIt
        (CC BY-NC weights: never), SAM-Audio (Meta's SAM License: not on the permissive list)
      And every DnR-trained model's training data mixes LibriSpeech, FMA and FSD50K, whose clips carry their own
        licences: noted beside the model, as ml-tracks.feature notes HTDemucs's MUSDB
      When each is run on VS1's mix
      Then the one kept beats MRX's effects SI-SDR and events found on VS1, and runs 90 s of audio in under 90 s on a
        16 GB Mac

  Rule: effects as code

    @needs
    Scenario: VS3. Naming a sound through the leak
      Given VS1's separated effects stem
      When each onset is named from the film's own takes (or, on a reference with no takes, given a prompt)
      Then the take is right more often than always naming the commonest take, scored on VS1's matches
      And candidates are tried cheapest first: the log-mel matcher with the music stem subtracted from the segment,
        CLAP or YAMNet embeddings (FP2f already loads both), then a text-queried separator run per candidate take

    @needs
    Scenario: VS4. A reference re-sounded on its own timing
      Given a separated effects stem
      When its loudness, onset and centroid curves drive the foley adapter (out/train_v2, the scene gate's G1-G4) with
        a prompt of the person's choosing
      Then the new effects pass G1 and G4 against the stem's own onsets, and the code written is one foley() call per
        span with the curves' digest, not a pasted clip

  Rule: licences

    @spec
    Scenario: VS5. Timing only from someone else's film
      Given a reference the person doesn't own
      Then only its timing and curves are kept: every sound in the code is generated or comes from a permissive pack,
        and nothing from the reference's audio trains a model (training stays on CC0, our own sounds and SAO's
        outputs, docs/sound.feature SN2)

  Rule: open footage, read from the picture

    @wip
    Scenario: VS6. Human interaction: Greatest Hits re-sounded as raea code
      Given Greatest Hits (Owens et al. 2016, CC BY 4.0, credited): videos of a hand hitting and scratching materials
        with a drumstick, their recorded sound, and human labels for each action's time, material and hit or scratch;
        single recordings read out of the 20 GB zip by HTTP range (remotezip), five on 11-17 (metal, cloth, plastic,
        ceramic, glass, wood; 149 labelled hits)
      When hits are read from the picture (the stick's motion reversing: optical flow on the fastest 1% of pixels;
        MediaPipe's hand landmarker alongside, which sees a hand in 13-61% of frames as it leaves the frame's edge),
        from the sound (onsets), and from both (an onset kept where the picture shows an impact within 67 ms)
      Then measured on 11-17 against the labels (lab bin/gh_hits.py, gh_fuse.py):
        | read from          | labelled hits found | of what it finds, real actions | time error |
        | the sound          | 100%                | 6-27% (hits only)              | 9 ms       |
        | the picture        | 61-90%              | 11-23% (hits only)             | 20-34 ms   |
        | both               | 72%                 | 63% (all actions)              | 9-18 ms    |
      And one clip (2015-09-12-04-15-35-1, 84 s) written as raea code (gh_recreate.py: 79 hits, each an <Audio> of an
        SA3 take for its material at its time and level, over the clip's picture, muted) and rendered by `raea render`
        in 16 s: every hit sounds within 10 ms of where the code puts it; the render has a sound at 70% of the
        labelled hits (the original recording: 100%), 17.5 ms off on average; loudness envelope r 0.45 against the
        original
      And with no labels at all (gh_recreate.py --events sound --kinds), the same clip re-sounded and rendered:
        | step                                  | labelled actions with a sound | envelope r |
        | picture and sound must agree, labels  | 70%                           | 0.45       |
        | sound decides when (within 20 dB)     | 99%                           | 0.72       |
        | sound when, picture what (no labels)  | 99%                           | 0.74       |
        the sound says when, the picture only what and where: a hit the picture can't confirm is still placed
      And material without labels (gh_material.py, zero-shot over the dataset's 17 materials, 207 labelled hits):
        | read from                                         | right |
        | always the commonest (plastic)                    | 32%   |
        | CLAP on the hit's sound                           | 20%   |
        | SigLIP 2 on the frame                             | 55%   |
        | SigLIP 2 on frame and crop, summed over ±2 s hits | 56%   |
        | kNN on the other clips' labels                    | 21%   |
        and hit or scratch from CLAP 82% (always hit: 76%)
      And the hits the picture misses are not off screen (gh_offscreen.py: of 43 missed, none inside a swing out of the
        frame and back; 1 of 9 looked at has no stick in view): they are small taps where the stick barely moves and
        fast swings blurred to a smear, which whole-frame flow at 228 px can't follow. Next: track the stick's tip
        (TAPIR, BootsTAPIR or TAPNext, DeepMind, code and weights Apache-2.0), where it stops is the hit and what it
        stops on is the material

    @needs
    Scenario: VS7. Steps from a skeleton
      Given a walk with the feet in frame and footsteps in its sound, under an open licence
      When MediaPipe's pose landmarker's heels give each foot's contacts (lab bin/pose_steps.py)
      Then the contacts line up with the effects stem's onsets clearly better than the same steps shifted
      And Tears of Steel does not serve: tried on 11-17 (clips 008 and 009), its walks have the feet below the frame
        (MediaPipe guesses them), its mix has no footstep track, and its soundtrack is CC BY-ND (measured in the lab
        only, never published); AUC 0.58 and 0.66 on 20 and 10 steps

  Rule: what raea needs

    @needs
    Scenario: VS8. <Audio> starts between frames
      Given hits known to 10 ms and a 29.97 fps composition (a frame is 33 ms)
      When each is placed with <Sequence from={frame}>
      Then today it lands on the next frame unless the take is padded and trimmed by the fraction (startFrom in
        fractional frames, which raea's renderer honours: VS6's 79 hits within 10 ms); a start in seconds on <Audio>
        or <Sequence> would make placement exact without the padding
