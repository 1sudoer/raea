@spec
Feature: A render in layers: captions, graphics, footage and sound apart, for CapCut, Premiere, Resolve and raea's timeline
  wackyza, 2026-11-16: "export a rendered video, in all its component layers (video, audio, graphics, all separate) to
  Premiere, Resolve and Capcut ... keep graphics on one overlay track ... And captions, if applicable, should be their
  own track. I think this separation would be valuable for the internal (in-app) timeline as well ... the best order
  of events might be to get the layers rendering really robust, then be able to push it to the in-app timeline and
  all the ext editors".

  wackyza, 2026-11-16, on text: "sometimes text is complexly integrated into graphics and separating it out is liable
  to cause more problems than good. Whereas I'm confident captions should always be their own layer." So text is part
  of the graphics layer. A text layer of its own may come later, opt-in only (an element marked
  data-raea-role="text"), never guessed.

  A cut finished in another editor needs its parts, not one flattened movie. `raea export capcut` (docs/capcut.feature)
  already hands CapCut the footage and sounds as clips of their own files and everything raea draws as one
  transparent overlay. This spec takes that split out of the CapCut export, adds captions as a layer of their own,
  proves it against the render, and gives it to every editor from one place: CapCut, Premiere Pro and DaVinci
  Resolve, and the layout of raea's own timeline.

  What there is to build on (read 11-16):
    - apps/capcut: the manifest's frames grouped into runs of footage and sound (plan.rs), the overlay rendered with
      the footage hidden (RAEA_HIDE_MEDIA=1) and what's under it (RAEA_HIDE_MEDIA=under), written as a CapCut draft
      (io/capcut). Its plan is written in CapCut's types, so no other editor can use it as it is.
    - systems/layers: hide.rs hides layers by key (RAEA_HIDE_LAYERS) and the footage (RAEA_HIDE_MEDIA) as page CSS.
    - io/alpha: transparent movies without ffmpeg (CC1): ProRes 4444, HEVC with alpha, QuickTime Animation, PNG.
    - sdk/timeline: the captions operation (TL7c) puts lines on a track named Captions, and read.ts writes each line
      into the composition as a `text` op, the same op a title is written with, so once in the code a caption and a
      title can't be told apart.

  The layers, top to bottom, and what each is:
    | layer    | what it holds                                                  | what the editor gets                 |
    | captions | the words said, as raea draws them, marked (LA1)               | a transparent movie + an .srt beside |
    | graphics | everything else the page draws over the footage: text, shapes | a transparent movie                  |
    | footage  | each <Video>'s file, at its trims, place and size (CC2a)       | clips of the original files          |
    | behind   | what's drawn behind a picture in picture (its card), between   | a transparent movie for each file    |
    |          | it and the footage under it (LA2c)                             | shown as a picture in picture        |
    | under    | what's drawn beneath the footage: the backgrounds of what      | an opaque movie, left out when all   |
    |          | holds it (CC2b), and the 3D, which the whole page is drawn over | black                                |
    | sound    | each sound clip, and footage's own sound linked to its clip    | audio clips of the original files    |
  A layer with nothing in it is left out.

  The passes cost time only at export. Rendering a composition, in the app or with `raea render`, draws it once, as
  it does today; the caption mark is an attribute in the code and draws nothing; raea's timeline reads the code (TL0c)
  and renders no layer. Only `raea export` (layers, capcut, xml) renders the layers.

  Background:
    Given a composition renders with --manifest (render.json), as docs/capcut.feature's Background says
    And the reference render is the composition as `raea render` draws it, every layer at once; an export renders it
      itself, with --manifest, every time

  Rule: LA1. Captions are marked, so every pass and every tool tells them from the rest
    Scenario: LA1a. A caption carries data-raea-role="caption"
      Given words drawn as captions: lines the captions operation (TL7c) wrote, or a caption the SDK or an agent draws
      Then the element holding each caption's words carries data-raea-role="caption", and everything inside it is
        part of it (a word highlighted as it's said is part of its caption)
      And the page keeps the mark (runtime/page elements.rs keeps only the data-raea- props it names)
      And the captions operation's `text` ops say they are captions (caption: true), so the code they write can keep
        the mark (raea-feed writes it) and a timeline read back from the code puts them on the Captions track again
      And the raea skill's caption guide says to mark captions this way
      And nothing is a caption unless it is marked: words that aren't marked are graphics

  Rule: LA2. Each layer is a pass of its own, rendered transparent, only when exported
    Scenario: LA2o. A render that isn't an export draws once
      Given a composition with captions, graphics and footage
      When it is rendered in the app or with `raea render`
      Then it is drawn in one pass, in the same time as before this spec, and no layer is written

    Scenario: LA2a. The passes
      When the composition is exported in layers
      Then it is rendered --alpha once for each layer that has something in it, with RAEA_LAYER naming the layer:
        | RAEA_LAYER | draws                                                                         |
        | captions   | only what is marked a caption, and none of the 3D                             |
        | graphics   | what RAEA_HIDE_MEDIA=1 draws today, less captions and the 3D                  |
        | under      | what RAEA_HIDE_MEDIA=under draws today and the 3D, less captions; opaque      |
      And the page draws nothing inside a hidden element, so the captions pass clears what holds a caption (its
        background, border, shadow and outline) and hides the rest, as RAEA_HIDE_MEDIA=under clears what holds footage
      And the 3D is drawn first and the whole page over it, footage included, so it is under the footage: the layer
        under the page draws it, and since --alpha clears the 3D's clear colour (the page's background over 3D) to
        nothing, that layer is rendered opaque, the bottom of every timeline
      And the 3D is Bevy's, not the page's, so a layer that draws none of it leaves its parts out of the frame's scene
        (runtime/scene items_of) and keeps its cameras, which render an empty view
      And the under pass runs when there is footage or 3D (the manifest's has3d)
      And an element whose `:has()` was matched is matched again when a node is inserted or removed under it
        (vendor/blitz-dom mutator.rs), as Chrome does: a caption a <Sequence> put on the page at frame 10 stayed hidden
        under the captions pass's `:not(:has(...))`, and so would footage that comes in later under RAEA_HIDE_MEDIA=under
      And the captions pass runs only when the manifest says a caption was drawn
      And RAEA_HIDE_MEDIA keeps working as it does, so `raea export capcut` is unchanged until LA4b
      And a pass is cut into clips where it draws something, frames that draw nothing a gap (CC2b)

    Scenario: LA2b. Captions also go out as SubRip
      Given the manifest says, for each frame, the words of each caption drawn there
      Then each run of frames a caption shows one line of words is an entry of NAME.srt: its words as the page shows
        them (a word highlight doesn't start a new entry), its first frame and the frame after its last, in timecode
      And a line whose words come in one by one (each frame's words starting with the last's) is one entry, its words
        whole
      And the .srt is written beside the export for the person to upload to a network or keep, and no editor's
        project imports it (the captions movie is what the timeline shows, in raea's own style)

    Scenario: LA2c. What's behind a picture in picture sits between it and the footage under it
      Given footage that doesn't fill the frame (a picture in picture) over other footage, on a card: an element
        holding it whose background is drawn over the footage under it
      Then for each file shown as a picture in picture, RAEA_LAYER=behind:FILE draws the backgrounds of what holds that
        file and no other footage; what holds other footage too is cleared, not hidden; the rest is hidden
      And its clips are kept only where that file is a picture in picture, on a track named "behind picture in
        picture", above the footage that fills the frame and below the pictures in picture
      And the under layer, beneath all the footage, can't hold it: footage that fills the frame would cover it (the
        multi-clip test's card behind C at 4-6 s: 23.2 dB with it in the under layer, 37.7 with it behind C)
      And where a picture in picture is the only footage, what holds it also holds the page's background, so the
        backing layer and the under layer both draw it: the same picture, but a picture in picture moved in an editor
        leaves its card behind in the under layer

  Rule: LA3. The layers put back together are the render
    Scenario: LA3a. The fixtures' layers composited over each other match their reference render
      Given fixtures: full-frame footage with a lower third (a bar and words) and captions; a picture in picture on a
        coloured card; captions with a word highlight; 3D with a title and a caption
      When each is exported in layers and composited bottom to top (under, footage, graphics, captions)
      Then every frame matches the reference render within the alpha codec's error (CC1a's numbers), checked at least
        at each clip's first, middle and last frame
      And this is a test of the method, not of every export: apps/layers composite.rs, run by hand on a machine with a
        GPU (`cargo test -p raea-app-layers composite -- --ignored`, RAEA_BIN the raea), the fixtures in
        apps/layers/fixtures, the footage engine/rune/media/tests/fixtures/clip.mp4; RAEA_LAYERS_KEEP_FRAMES keeps
        each pass's frames for it
      And on 11-16 (this Mac, M-series, a source build): highlight 33.0-35.1 dB, lower-third 34.1-35.1, pip-card
        33.2-33.5, three-d 39.1-39.8; the floor is 30 dB (the reference is H.264, and the footage is decoded and scaled
        by the test, not the page)
      And RAEA_LAYERS_PROJECT=DIR/EDIT.tsx checks a project of its own in place of the fixtures: on 11-16 a 10 s cut of
        three raea-rendered clips (cuts, one clip at two trims, a picture in picture on a card, four captions, a lower
        third and a title that come and go, its own sound and a muted clip over a music bed) at 36.2-42.7 dB

    Scenario: LA3b. What doesn't carry is said, not dropped quietly
      Then captions always go on top: a caption drawn under something else is still the top layer in the export
      And CC2e's notes (filters, rotations, masks, notes and <Mix>) are said as they are

  Rule: LA4. One layered timeline, written by `raea export layers`, read by every editor's export
    Scenario: LA4a. `raea export layers EDIT.tsx -o DIR` writes the layers and their timeline
      Then DIR holds layers.json, each layer's movie, NAME.srt when there are captions, reference.mp4 (the reference
        render, to compare the editor's timeline against; on no timeline) and render.json, its manifest
      And footage whose file has sound but no linked sound clip plays silent there: the composition doesn't play its
        sound at that place
      And layers.json says the frame rate, size and length, and the tracks top to bottom, each with its layer and
        its clips: the file, where it starts and how long, its first source frame, its place and size, its gain and
        fades, and the id of the clip it's linked to (footage and its own sound)
      And it says what CC2e notes, the same lines the command prints
      And it names no editor's format: CapCut's draft and the XML are written from it

    Scenario: LA4c. Footage that moves, scales, is cut or fades keeps its keys
      Given footage a box it's in clips (`overflow: hidden`), or whose box or opacity changes over its run (a split
        screen: the take rising into the top of the frame while the box holding it shrinks to the seam)
      Then render.json's video entry says what of its box shows (`clip`), the clip each box up its containing blocks
        that clips it leaves, read from the layout, raea's drawing unchanged; one clipped away entirely isn't listed
      And layers.json's clip has its first frame's clip and keys: its box, clip and opacity at its first and last
        frames and at the frames on either side of each change, a straight line between two keys being each frame between
      And whether it's the main footage or a picture in picture is read from what of it shows, not its box
      And each editor's export keys it in its own terms (the XML's keyframes, LA5a; CapCut's, docs/capcut.feature CC2f),
        and says what it can't (CapCut: a cut that changes over the run)

    Scenario: LA4d. Every editor places a picture as raea drew it, from one reading of it
      Given footage of any shape (upright, horizontal, square) in a composition of any size (1080 x 1920, 4K)
      Then where the picture is drawn and what of it is kept is worked out once (raea_app_layers::plan::picture), as
        blitz-paint draws it: sized by its object-fit, placed by its object-position (render.json's `position` when
        it isn't centred, each axis as px plus a share of the free space), cut where its box or a box it's in cuts it
      And only cuts inside the frame are crops: an edge on or past the frame's cuts nothing, the frame cutting it
        anyway, so an editor reframing the clip finds the rest of the picture there, not a crop
      And each editor's export turns that into its own terms (the XML's motion, CapCut's place)
      And footage composed at its own size (a 4K edit of 4K takes) is at 100 in every editor
      And LA3a composites it back as the render on 11-17 at 35 dB and over, in a 1080 x 1920 and a 2160 x 3840
        composition alike (telluride-layers/positioning-test: a horizontal take covering the frame and panned inside
        it, a contained picture in picture sliding, a phone's take in a card cutting its sides, pushed in, and the split
        close): `RAEA_LAYERS_PROJECT=EDIT.tsx RAEA_LAYERS_COMPOSITION=ID cargo test -p raea-app-layers composite -- --ignored`

    Scenario: LA4b. The CapCut export reads it
      Then `raea export capcut` exports the composition in layers into out/capcut/NAME (raea-app-layers) and writes
        layers.json as a CapCut draft (apps/capcut from_layers.rs): its tracks bottom first, the footage CapCut's main
        track, then "raea overlay" (the graphics), then "raea captions"; "raea under" below the footage when there is one
      And a footage clip's linked sound is the clip's own volume, as CapCut keeps a video's sound, which its Extract
        audio separates; footage with no linked sound is silent
      And for a composition with no captions the draft is the one it was before, less the 3D, which is now under the
        footage as raea draws it
      And reference.mp4 is copied into the draft's folder, on no track, where CC2d kept it only beside the draft;
        every export's folder holds it, so the person can compare what the editor plays with what raea rendered

  Rule: LA5. Premiere Pro and DaVinci Resolve take the same XML
    Scenario: LA5a. `raea export xml EDIT.tsx` writes an FCP7 XML project
      Then it writes a folder: NAME.premiere.xml and NAME.resolve.xml (xmeml version 5), the layers' movies, the .srt (LA2b) and reference.mp4,
        which the sequence doesn't use
      And the sequence has the composition's frame rate and size, its video tracks bottom to top under, footage
        (main, then each picture in picture on a track above), graphics, captions, and its audio tracks below
      And each clip of the original footage points at its file where it is, never copied (CapCut's draft copies,
        since sandboxed CapCut reads only its own folder, CC2d), at its trim, scaled and placed to its box (Basic
        Motion), at its gain (Audio Levels)
      And a video clip and its own sound are linked clipitems, so they move together, and each editor's own control
        separates them: in Premiere, Unlink; in Resolve, Link Clips off (Cmd/Ctrl+Shift+L); in CapCut, Extract audio
      And sound is laid out as Premiere writes it (read from an XML Premiere Pro 26.2.2 exported): the sequence
        explodedTracks, each stereo file's channels described one by one, each channel a mono clipitem on a track of its
        own (sourcetrack and outputchannelindex 1 and 2), every clipitem of a clip and the clip it's linked to linked
        together, a masterclipid each and a timecode on every file. One stereo clipitem on a "Stereo" track, the first
        layout, imported into Premiere with no sound at all
      And each channel of a stereo file is panned to its side by an Audio Pan on its clip (-1 left, 1 right), as
        Resolve and FCP7 write it: without it both editors import the channels centred, and a stereo track plays as
        mono; Premiere's own track pan (PannerCurrentValue) was tried, and neither editor read it
      And footage with keys (LA4c) is keyframed: Basic Motion's scale and centre, its crop (what object-fit and its
        clip cut) and Opacity, each a value where it holds and a keyframe at each key where it changes, timed in the
        source's frames as FCP7 times a clip's filters; a key a straight line between its neighbours already gives
        (to a tenth of a pixel) is left out, so a steady move is its two ends
      And a clip raea plays at another speed is placed at its own (FCP7's speed isn't written) and said: as many source
        frames as it lasts, its keys timed at that speed
      And the crop is written as each editor writes it, read from an XML each exported of a clip it keyframed (11-17):
        Premiere's as Basic Motion's leftcrop, topcrop, rightcrop and bottomcrop; Resolve's as FCP7's Crop filter. One
        XML can't be both, so the folder holds NAME.premiere.xml and NAME.resolve.xml, the same but for that
      And a file stored turned (a phone's take: its container's display matrix stands it upright; layers.json's
        `rotation`) is cropped as Resolve cuts it: on 11-17 each editor rendered a timeline cropping one picture
        stored at 0, 90, -90 and 180 (top 25, then left 25). Premiere 26.2.2 cut the upright picture's named side at
        each; Resolve 21.2.0 cut it mirrored, top and bottom swapped at 90, left and right at -90, both at 180, so
        Resolve's sides are written swapped to match
      And footage's Scale is as each editor reads it: Premiere's of the media's own pixels, Resolve's of the media
        already fitted to the timeline (its default, "scale entire image to fit": a 4K take on a 1080 timeline is
        Premiere's 50, Resolve's 100), read from the XML Resolve exported on 11-17; the earlier tests' media were the
        timeline's size, where the two agree. Resolve's export says so when footage isn't the timeline's size: with
        another Input Scaling, that footage comes in at another size
      And footage's Center is as each editor reads it: Resolve's in the frame's width and height, Premiere's in the
        media's own pixels, upright and unscaled (read on 11-17 from the Position Premiere gave the Telluride close:
        Center vert -0.120370 at y 497.8 on a 1920-high sequence, 462.2 px up, 0.120370 of 3840); a 2160 x 3840 take
        risen 260 px on a 1080 x 1920 timeline is Resolve's -260 / 1920, Premiere's -260 / 3840
      And a layer's movie has its alpha from its first frame in one sample description (depth 32), even when it opens
        on a full-frame card: VideoToolbox encodes leading opaque frames without alpha and starts a second description
        where the alpha starts, which Premiere 26.2.2 won't import ("the importer reported a generic error") and
        Resolve 21.2.0 shows opaque (the Telluride graphics, 11-17)
      And a command of an editor's own (`raea export premiere`, `raea export resolve`) is added only where LA5b finds
        that editor reading the XML differently: none is, on 11-16

    Scenario: LA5b. Premiere and Resolve open it as raea rendered it
      Given the LA3a fixtures exported
      When the XML is imported (Premiere: File, Import; Resolve: File, Import, Timeline)
      Then every track and clip is where layers.json puts it, nothing is offline, and the frame at each clip's
        middle matches the reference render
      And which alpha codecs stay transparent in each is recorded here, with the version tested, and the export
        uses the first that does
      And on 11-16, on this Mac, the multi-clip test (three raea-rendered clips A, B and C labelled with their source
        time, cuts, a re-trim, a picture in picture on a card, four captions, graphics coming and going, footage sound
        linked and a muted clip, a raea-rendered stereo music track, low note left and high note right): Premiere Pro
        26.2.2 and DaVinci Resolve 21.2.0 each import it with every track, clip, source time, placement and caption as
        raea rendered it, nothing offline, ProRes 4444 transparent, the footage's sound linked (Unlink, Link Clips) and
        the music stereo, panned left and right on its clips

    Scenario: LA5c. CapCut opens what raea export capcut writes (docs/capcut.feature CC3a)
      Then on 11-16 CapCut 9.4.4589 listed both tests the export entered in its project list (markers from a draft it
        made itself) and played them as raea rendered them: the footage's main track, the picture in picture on its
        card, the overlay and captions tracks transparent (ProRes 4444), the sound and the music

  Rule: LA6. raea's timeline is laid out the same way (raea-feed, after LA1)
    Scenario: LA6a. A timeline read from the code puts captions on the Captions track
      Given a composition read as a timeline (TL0c)
      Then elements marked as captions are on the Captions track, and everything else where TL0c puts it, so the
        timeline and the exports agree on what a caption is
      And they stay what TL0c makes them: clips that edit the code, not movies

  Rule: LA7. The skill teaches it
    Scenario: LA7a. The delivery guide teaches layers
      Then sdk/guides/craft/motion/delivery.md says a cut an editor finishes goes out with raea export capcut or
        raea export xml (Premiere, Resolve), that captions arrive styled on their own track with an .srt beside them,
        and that words are fixed in raea and exported again, since the editors get them as pictures
      And it says to mark captions (LA1a) and to pass on what the export notes
