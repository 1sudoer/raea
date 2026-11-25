@spec
Feature: Sound for picture: the local models on any Mac, open libraries and packs one command away, music composed
  Asked for on 2026-11-16: rewrite the music and sound notes so that SAO and TokenSynth can be downloaded, every open
  instrument library and sound-effect pack raea knows is easy to reach, and music is never a default: "it tends to
  result in drones and pads and not actually design real complex music, scoring, sound effects, diegetic and
  non-diegetic."

  Where it stands (2026-11-19): `raea setup-audio` installs both models through tools/sao/setup.py and
  tools/tokensynth/setup.py, run by uv on a Mac, Windows or Linux with nothing installed first (no Homebrew, git
  or FFmpeg; raea fetches uv itself when the PATH has none), and `raea sounds` reaches four libraries (sdk/raea/library/sounds/catalog.json:
  salamander, fluid, tonejs-instruments, vcsl). The guides teach deciding what each scene is heard as, spotting the
  cut and composing for it (sdk/guides/craft/motion/music.md, sound.md), and list every vetted source with its licence
  and how to fetch it (motion/sources.md). What those guides had to write down as a gap is below.

  Rule: The local models install on any Mac, and say what they are

    Scenario: SN1. setup-audio works on a clean Mac
      Given a Mac with no uv, no git and no Python of its own
      When `raea setup-audio` runs
      Then each setup gets uv and git through tools/ensure.sh, which every release ships, instead of the PATH
      And TokenSynth's code is cloned at a pinned commit and its weights fetched at a pinned revision, each file
        checked against its SHA-256, as SAO's already are
      And a missing tool, a full disk or a failed download is named in the error with the one command that fixes it
      And it is proven from a release build on a fresh macOS user account, with the sizes it reports

    Scenario: SN2. raea meets SAO's licence, and says what is left for the person
      Given Stable Audio Open Small is under Stability AI's Community License (stability.ai/community-license-agreement),
        which isn't an open licence, and raea hands the model on to everyone who uses raea, so the licence's duties for
        whoever distributes it, or a product that uses it, are raea's
      When setup-audio installs it
      Then it writes a copy of the agreement and a NOTICE file beside the model (~/.cache/raea-sao), the notice
        holding the attribution the licence gives word for word ("This Stability AI Model is licensed under the
        Stability AI Community License, Copyright © Stability AI Ltd. All Rights Reserved"), and saying that the
        model raea installs is a changed one and how (a Core ML conversion from a Hugging Face mirror)
      And raea shows "Powered by Stability AI" where it documents SAO (the manual's setup-audio page, the app's about
        page)
      And raea's maker has registered its commercial use with Stability, and knows the licence ends for raea when
        raea and its affiliates make more than USD 1 million a year, from anything, unless Stability grants it an
        enterprise licence
      And `raea setup-audio --check` names each model's licence and where its copy is
      And what applies to the person is decided and written in motion/sources.md: whether the licence's clause for
        someone who gets the model "as part of an integrated end user product" takes them out of its commercial
        section, and what remains (the outputs are theirs as far as the law allows; no training another generative
        model on them; Stability's acceptable use policy)
      And until then the guides tell the agent to name the licence and link it before an SAO sound goes into a
        commercial piece, and to say nothing about what it requires of the person

    Scenario: SN3. The app offers the local models before a piece needs them
      Given the raea app on a Mac without the models
      When a piece first needs SAO or TokenSynth
      Then the app says what will be downloaded (its size, licence and folder) and installs it on a yes, with progress
      And a render never blocks on a download of gigabytes it didn't say it would make

  Rule: Open libraries and packs are one command away

    Scenario: SN4. raea sounds reaches the vetted packs
      Given the packs motion/sources.md lists as fetched by hand
      When `raea sounds get` names one, an instrument of it or a sound in it (VSCO 2 CE's celli, Kenney's impacts,
        Groove MIDI's funk grooves)
      Then it downloads only that, once, into ~/.cache/raea-sounds, checked against a pinned SHA-256
      And it copies what the piece uses into media/ with its LICENCE.txt and the credit line it needs
      And it converts what raea can't read (Ogg, Opus) to WAV on the way in
      And a pack whose licence forbids passing it on (Sonniss's bundles, Pixabay's and Mixkit's sounds) is never
        mirrored: raea links to it and indexes the person's own download
      And nothing non-commercial, no-derivatives, Sampling+, share-alike or GPL is in the catalogue
      And until it does, motion/sources.md gives a one-line fetch for every pack with a stable address, into
        ~/Music/raea-sounds/PACK/ with its licence beside it
      And Freesound is searched from raea only under a commercial licence for its API from Freesound's owners

    Scenario: SN5. A sampled instrument keeps its dynamics and round robins
      Given a library with several velocity layers and round robins a note (VCSL, VSCO 2 CE, Salamander V3)
      When `raea sounds get` fetches it, or a composition loads its SFZ map
      Then every layer comes, and one Sampler plays the layer each note's velocity asks for and turns through the
        round robins
      And `humanize` on a Part, a Sequence or a Loop moves each note by a seeded few milliseconds, the same every render

    Scenario: SN6. A MIDI file plays on sampled instruments
      Given a MIDI file: a Groove MIDI drum performance, or a public-domain score exported from MuseScore
      When a composition imports it (`import groove from "./media/grooves/funk-92.mid"`)
      Then it gets the file's tracks and notes in raea/music's MidiFile shape (parseMidi's), to play on Samplers and
        Players, the same shape motion/sources.md's converter writes as JSON today
      And a drum track maps onto a kit by note number, in General MIDI's map or a named kit's (Roland's, which
        Groove MIDI was recorded on, adds hi-hat edges at 22 and 26)
      And a note-on with no note-off ends where the file does, as the converter has it, instead of being dropped
      And a SoundFont bank (.sf2, .sf3) can play it in raea, without a render outside it

  Rule: Music is composed for the cut, and a drone is caught

    Scenario: SN7. raea check says when a cue is a drone
      Given a render whose music bus holds a cue longer than 15 s
      When `raea check` measures it from the render's note events
      Then it reports, by timecode, a cue that holds still, by these measures: the distinct pitches its lead plays in
        8 bars, the longest any one chord holds, its note onsets a beat over 4 bars across all its parts (the notes of
        a chord struck together count as one onset, and a bar with no onset counts as a held one), the spread of its
        velocities, how many timbres it uses, and whether it ends on a cut or fades out
      And its first fixtures are the cues in the guides' own examples (motion/music's cue and game loop, motion/sound's
        cue, the game bench's loop), which must all pass whatever their length, and a held pad and a sine sub under
        a long reverb, which must fail
      And its thresholds are set from those fixtures, approved scores and known drones before it ranks anything above
        P3 (a first guess, not measured: under 5 lead pitches in 8 bars, a chord held past 2 bars, under 1 onset a
        beat, a velocity spread under 8 of 127)
