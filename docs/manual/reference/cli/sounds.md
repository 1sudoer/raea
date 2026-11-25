---
title: "raea sounds"
summary: "Search open sample libraries for instruments and drums, and fetch one for a Sampler."
order: 83
sources: ["apps/tools/src/sound/library.rs"]
---

`raea sounds` searches open, freely licensed sample libraries by the words producers use ("kick", "hihat", "upright
piano") and lists what matches. `raea sounds get` fetches one instrument's samples into the project, ready for a
Tone.js `Sampler`. For a pitched instrument it takes one sample per note, at the middle dynamic.

```sh
raea sounds piano
raea sounds get vcsl/upright-piano-knight --notes C2,C3,C4,C5 -o media/piano/
raea sounds get vcsl/bass-drum-1 --all
```

The ids are exact: `raea sounds kick` lists the kicks by their ids (`vcsl/bass-drum-1`, `-2`, `-3-legacy`). A drum or a
hit comes as its first 12 takes unless `--all` is given. The four libraries are Salamander's grand piano, FluidR3's
General MIDI set and tonejs-instruments (all three CC-BY 3.0: credit them), and the Versilian Community Sample Library
(CC0). Larger libraries, effect packs and MIDI grooves are fetched by hand: `raea guide sources` lists them with
their licences and the command that fetches each.

<!-- generated:begin by tools/manual-ref/cli.ts from `raea help --json`; edit only the flag descriptions -->

## Usage

```sh
raea sounds [WORDS] | sounds get PACK/INSTRUMENT [--notes ..|--all] [-o DIR]   (open sample libraries for a Sampler)
```

Headless: it needs nothing running and opens no window. Takes up to 2 arguments besides its flags.

## Flags

| Flag | Takes | What it does |
| --- | --- | --- |
| `--all` | – | Fetch every sample: a pitched instrument's every sampled note (still one take a note, at the middle dynamic), a drum's every take. |
| `--notes` | `..\|--all` | Fetch only these notes, comma-separated (`C2,C4`). |
| `-o` | `DIR` | The folder the samples go to. |

<!-- generated:end -->
