@spec
Feature: Realtime editing: a composition's controls change the live frame, then the render around it
  The founder, 11-16, on the realtime-editing plan (kept with his project notes, not in this repo): build it in full,
  validate it and optimise it, and include that timeline playback of plain pre-rendered videos is choppy and repeats
  itself. The plan, in short: a composition declares its controls (SDK `controls`); dragging one changes the engine's
  live stage at once (the stage is engine/page/stage, a raea_runtime_live::Live in the app's window), and on release a
  draft of the seconds around the playhead is rendered (the app's host/draft.ts: `edit={{ segments, scale }}`) and
  played in place of the live frame. Phases:
    P1 (0.1.6) controls with a live frame. Drag to the first changed pixel p50 <= 120 ms, p95 <= 200 ms for 2D and
       raster 3D at 1080p (<= 250 ms ray-traced at half size); release to a buffered window <= 3 s for 2D, <= 6 s
       ray-traced at half size; the stage against `raea render --frame N` at SSIM 1.000000 for 2D and raster,
       >= 0.9994 for HTML over 3D; no draft left running for a value that has passed.
    P2 (0.1.7) smart buffering. A buffered window <= 1.5 s for 2D, <= 3 s ray-traced; a trim costs no render; frames
       redrawn = frames that read the control; a structural edit shows in <= 1.3 s with no stall; no mismatch in a
       1,000-frame audit.
    P3 live playback at 90% of the frame rate, a converged ray-traced frame when paused, keyframed controls, live
       sound within 1 LU of the export, exports reusing cached segments (docs/rerender.feature RR4).
  The founder's answers to the plan's questions (11-16):
    1. A composition's knobs are "Controls". The workspace's controllers keep their name.
    2. A live frame that isn't exact is shown labelled while paused or dragging; during playback the last exact frame
       is held.
    3. Drafts may skip Content Credentials while they stay on the machine; anything that leaves it is signed, and that
       guard is tested.
    4. Agents declare controls in P1. "Make this a control" (promote a literal the person points at) goes in only if
       RT2 finds fewer than half of notes are value-only.
    5. Review links stay watch-and-comment.

  Status, 11-16: the plan's experiments 1, 2 and 3a are measured; nothing of P1 is built. On the realtime-bench lane:
  bench/realtime/stage.sh (a headless raea window driven over its own MCP port: a composition shown on the stage,
  paused, a control changed, the frame captured after each draw, against renders), bench/realtime/window.sh (a draft
  window rendered as the host renders one, cold and warm, then once with signing on) and their cases
  (bench/realtime/cases: a 2D card, a raster scene, the same under an HTML caption, a Solari room, a card reading
  Math.random and Date, and 60 s and 180 s films of RR0's six shots), plus render = play rows for the 2D, raster and
  caption cases in bench/render-play/rows.tsv. run.sh takes RAEA_RP_ROWS (rows from another file, absolute paths
  allowed), and stage.sh RT_ROWS and window.sh RT_WINDOW_ROWS the same way, so copies of private films are measured
  from outside the repo; the private rows aren't kept here. Every number was taken on the M5 Pro Mac with other
  sessions' renders and builds running (antsy board), so times are upper bounds and spread wide; SSIM is ffmpeg's over
  RGB24, as bench/render-play/run.sh takes it.

    measured                                         how                                                    result
    2D card: stage = render                          stage.sh title row, frame 75, hue 210 -> 30            1.000000 before and after the change
    raster scene: stage = render                     stage.sh raster row, colour changed                    1.000000
    HTML caption over 3D: stage = render             stage.sh caption row                                   0.999999 (bar 0.9994)
    props on a paused stage, no seek after           the same rows, stage.props alone                       not drawn: 0.697 2D, 0.986 raster and
                                                                                                            caption, against the new render
    props, then seek N (what the app sends)          the same rows                                          drawn at the first draw, = render
    props -> painted                                 MCP round trips to the window (an upper bound)         115-230 ms 2D, 117 ms raster, 398 ms
                                                                                                            caption
    props + seek -> painted                          the same                                               160 ms 2D, 171 ms raster, 541 ms caption
    stage.show -> first paint                        the same                                               0.8-1.1 s 2D; 4.8-20 s Solari
    Math.random + Date card: stage = render          stage.sh random row, the window's own seed             0.928677
    the same, the window on the render's seed        RAEA_SEED=seedOf(id), 340724834                        0.998346 (the rest: Date's start)
    render = play: 2D, raster, caption               run.sh rows, frames 15, 45, 75                         1.000000, 1.000000, 0.999998
    render = play: a 90 s raster film (a private     run.sh, RAEA_RP_ROWS, frames 15 and 45; the play's     0.945446, 0.945121; each side repeats
      copy, 2160x1080, the paint look)               seed replayed in the render (--input)                  itself at 1.000000 over two runs
    the same, the paint look's wind set to 0         the copy with wind={0}                                 0.997021, 0.992958 (the rest: willows,
                                                                                                            trunks, far edges)
    the same film on the stage against its still     stage.sh, RT_ROWS, frame 45, a gooseScale control      0.943171 at every draw, before and after
                                                                                                            (its change doesn't show at frame 45)
    render = play: a skinned 1080p animatic (copy)   run.sh                                                 not measured: it throws at frame 0 on
                                                                                                            main ("Model: graph has at most 16 nodes")
    Solari: stage after a seek, against the film's   stage.sh solari row, frame 75, 65 draws, against       0.952 (draw 0), 0.9926 (24), 0.9934
      own frame                                      `render --at 0..75`                                    (32), 0.9942 (64): never 0.995
    Solari: two renders of one film frame            render --at 0..75 twice                                0.996778 (the floor)
    Solari: a still against the film's frame         render --frame 75 against render --at 0..75            0.905443
    Solari: the stage against a still                the solari row against render --frame 75               0.975 (draw 0) falling to 0.909 (64)
    window render, 60 s film (raster, 1920x1080)     window.sh film-60: [900, 1050) at 0.5, 9 runs          10.2-15.2 s, median 12.6 s; 41-67 ms a
                                                                                                            frame
    window render, 180 s film                        window.sh film-180: the same window, 9 runs,            9.2-20.1 s, median 15.9 s; the 60 s
                                                     alternated with the 60 s film's                        film's x0.80-1.52 in the same pair,
                                                                                                            median x1.14, x0.86-0.91 when quietest
    warm startup (bundle cached) to the 3D frame     RAEA_STARTUP_TRACE marks, 6 runs                       2.05-3.71 s, median 2.5 s: ~1.0 s to a
                                                                                                            first 2D frame, a restart with 3D, ~1.0 s
                                                                                                            to its first frame
    the clipping check, a draft                      RAEA_TIMES                                             0.45-1.20 s
    window render, the 90 s raster film (copy)       window.sh, [1350, 1500) at 0.4444 (960 wide)           35.1 s cold, 25.7 and 22.6 s warm; 140-206
                                                                                                            ms a frame (exit 1 on its clip checks)
    a draft signed? the public raea                  window.sh, once with RAEA_SIGN unset                   never: it has the seam and no signer
    a draft signed? the signed build (this Mac's     RAEA_SIGNED_BIN, its manifest read                     yes: Trusted Content Credentials; 10.2 s
      raea)                                                                                                 against 7.0-7.5 s unsigned (one run)
    notes that are value-only                        the last 30 timestamped notes on composition films     8 of 30 (27%); at most 11 (37%)

  What each experiment does to the plan:
    RT0 (experiment 1). The bench's 2D and raster cases are 1.000000 on the stage and in render = play, and the
      Math.random/Date card misses only by its seed and start, which P1's "the render's seed and epoch on the stage"
      covers. But a real raster film misses for a cause other than the seed: the 90 s goose film plays at 0.945
      against its render, the play's seed replayed (the paint look's wind in the grass, then willows and far edges).
      So the branch is taken: live frames are labelled "Preview" everywhere, and the warm worker moves into P1.
      Solari needs more than 30 draws and never reaches 0.995 in 65, so ray-traced previews draw without Solari.
      And a paused stage doesn't draw props without a seek after them (P1's "drawn on a paused stage without a
      re-seek" is needed; the app's re-seek works meanwhile).
    RT1 (experiment 2). The window costs the window, not the film: alternated with the 60 s film's, the 180 s film's
      took a median 1.14 times as long (0.86-0.91 in the quietest pair; 1.52 once, under load), so that half of the
      branch isn't taken. Warm startup tops 2 s (median 2.5 s, of which ~1 s is starting again once the first frame
      meets 3D), so the warm worker moves into P1. A draft from the signed build is signed, so unsigned previews are
      a P1 decision, already made by answer 3. And a 5 s window at half size took 9-20 s (23-35 s for the goose film)
      against P1's 3 s for 2D and 6 s ray-traced: the warm worker is needed for the target, not only for startup.
    RT2 (experiment 3a). Fewer than half the notes are value-only (at most 37%), so background reopen moves into P1,
      and by answer 4 so does "make this a control".
    Experiment 3b (three agent runs on fresh briefs, paid) was not run.

  Background:
    Given the engine's stage is engine/page/stage (stage.show, play, seek, props, pointer, status), driven from a
      page by `call` (@raea/ui/wire), and a headless raea window answers MCP on its own port (never 7477)
    And a render for a pixel check has RAEA_SIGN=0, RAEA_MARK=0, RAEA_AUDIO_MUTE=1 and RAEA_READBACK=rgba
    And every scenario below is a task (docs/agents.md "Tasks: rask"); a status line under a rule says what is built

  Rule: RT0. The live frame is the render's frame
    # Bench (file | job)
    #   bench/realtime/stage.sh        | rows of stage.tsv: show, pause, seek N, props, a capture after each draw, SSIM
    #   bench/realtime/stage.mjs       | the MCP driver: stage calls through the page's rt.call action, captures
    #   bench/realtime/stage/page.tsx  | the window's page: one <img data-raea-stage>, the rt.call action
    #   bench/render-play/rows.tsv     | the cases' render = play rows; RAEA_RP_ROWS for private copies
    # Status: measured (the table above). Not built: props drawn on a paused frame without a seek, the render's seed
    # and epoch on the stage, the paint look's wind and the trees' motion on the frame's clock in play.

    Scenario: RT0a. 2D and raster on the stage equal the render
      Given a 2D composition and a raster-lit 3D one, each with a control in its defaultProps
      When it is shown on the stage, paused on frame N, and the control changed with stage.props
      Then the next draw equals `raea render --frame N` of a copy with the new value, at SSIM 1.000000
      And it does without a seek after the props (today one is needed: props alone leave the old frame up)
      And HTML over 3D does at >= 0.9994

    Scenario: RT0b. The stage takes the render's seed and start
      Given a composition that reads Math.random and Date
      When the stage shows it
      Then its frames equal the render's at 1.000000 (today 0.998346 on the render's seed, 0.928677 on its own)

    Scenario: RT0c. A film's play equals its render
      Given a raster film whose look moves on its own (wind in the grass, swaying trees)
      Then a play of it equals its render at every frame compared, at 1.000000 (today 0.945 for the goose film)
      And until it does, a live frame is labelled "Preview"

    Scenario: RT0d. A ray-traced frame on the stage
      Given a Solari scene shown on the stage and seeked to frame N
      Then within 30 draws it is within 0.995 of the film's own frame N, or the preview draws without Solari
      And today it isn't: 0.9934 at 32 draws, 0.9942 at 64, against a floor of 0.9968 between two renders

    Scenario: RT0e. A still of a ray-traced film is not its frame
      Given a Solari film
      Then `raea render --frame N` is not the film's frame N (0.905), since Solari's history is the frames before it
      And a ray-traced stage is compared against `render --at 0..N`, never a still

  Rule: RT1. A window render costs the window
    # Bench (file | job)
    #   bench/realtime/window.sh       | rows of window.tsv: a copy with edit={{ segments, scale }}, rendered as a draft
    #   bench/realtime/cases/films.tsx | film-60 and film-180: RR0's six shots round, 1920x1080 at 30 fps
    # Status: measured (the table above). Not built: the warm worker, one start for a draft that meets 3D, unsigned
    # local drafts.

    Scenario: RT1a. The same window of a longer film costs the same
      Given the 5 s window [900, 1050) of a 60 s and of a 180 s 3D film, at scale 0.5
      When each is rendered as host/draft.ts renders it (`raea render COPY -o OUT.mp4 --quality draft`), cold and warm
      Then the 180 s film's window costs no more than 1.5 times the 60 s film's (today a median 1.14)
      And a warm render reaches its first frame within 2 s (today 2.05-3.71 s)

    Scenario: RT1b. A draft starts once
      Given a draft of a window that shows 3D
      Then its render starts its engine once, with its 3D (today the first frame meets 3D and the render starts again)

    Scenario: RT1c. A draft that stays on the machine is unsigned; one that leaves is signed
      Given a draft rendered for the stage
      Then it carries no Content Credentials and makes no call to the signer
      And anything exported, posted or sent for review from it is signed, and a test holds that

  Rule: RT2. What notes ask for
    # Status: measured (the table above); the notes are read only, classified by hand, and not kept in this repo.

    Scenario: RT2a. Notes on composition films, by kind
      Given the last 30 timestamped notes on items whose film is a composition's render
      Then each is value-only (a property the live list holds, or a value on the 2D page) or structural
      And today 8 are value-only (27%), 20 structural and 2 about sound; background reopen and "make this a
        control" are in P1

  Rule: RT3. Playback of pre-rendered video
    # The founder, 11-16: timeline playback, even of plain pre-rendered videos, is choppy and repeats itself.
    # Bench (file | job)
    #   the app's e2e/playback.spec.ts   | 12 s films made with ffmpeg (testsrc2 and a tone, H.264 with B-frames, a
    #                                    | keyframe every 2 s: 1080p30, 1080p60, 4K30) posted as plain review items and
    #                                    | played in the Watch view under the timeline, in raea's window, headless.
    #                                    | Each frame the window shows is logged from the news it sends the player
    #                                    | (`time`, one per frame drawn), with each seek the page makes and the
    #                                    | playhead's place at each page frame. PB_SECONDS, PB_STRICT=1 (frames passed
    #                                    | over under 1 in 500), PB_PROFILE=1 (the page's JS cost)
    #   runtime/js/src/media_play.rs     | a_seek_while_playing_answers_first_and_plays_on_from_there
    #   runtime/js/src/media_sound.rs    | the sound clock between callbacks, a late callback, a start during one
    #   runtime/dom/media/src/lib.rs     | frames_decoded_before_a_seek_are_not_kept
    # Status, 11-16: RT3a-RT3e built (the app's timeline-playback branch, and the timeline-playback lane for the
    # window); RT3f, RT3g and RT3h not. Measured with RAEA_PAGE_FPS=120 and RAEA_AUDIO_MUTE=1 on the M5 Pro Mac with
    # other sessions' renders running (antsy board), so the page's frame times, and the frames it passes over, go with
    # the load. Before: the app at the branch's base with a window built 11-15. After: the branch with a window built
    # from this lane. Runs alternated back to back.
    #
    #   measured                                  before                                after
    #   plain play: steps back, frames twice      0 and 0 on all three films             0 and 0
    #   plain play: the playhead's moves a second 3-4 (the four timeupdates)             23-58 (each frame the page drew)
    #   plain play: frames passed over            0.5-58% (page frame p50 9-35 ms)       0.5-62% (p50 9-54 ms): the load's
    #   a seek back and one on, while playing     no frame from before either; at most   the same (the old window's queue
    #                                             2 already drawn; there in 27-44 ms     of frames didn't show headless);
    #                                                                                    there in 48-55 ms, 190-280 ms
    #                                                                                    when the page drew at 18-54 ms
    #   an edited film across a split             frames 93-95, then 92 again: 1 step    no seek, no frame twice
    #                                             back, 2 shown twice (a seek back
    #                                             found up to 250 ms late)
    #   an edited film across a cut               not reached (fails at the split)       no frame the cut took out; after
    #                                                                                    a stall the seek comes early by
    #                                                                                    the page's frame time, so the
    #                                                                                    clip's last 4-13 frames go unshown
    #   played to its end                         went round: 359, then 0 and on         stopped on 359, paused
    #   the page's CPU a frame drawn              19-41 ms (old window)                  15-29 ms (branch, old window):
    #                                                                                    the playhead's move costs nothing
    #                                                                                    seen past the load's spread

    Scenario: RT3a. A plain film plays each frame once, in order
      Given a plain .mp4 on the timeline
      When it plays
      Then every frame shown is shown once, and never one before the frame before it
      # Status: built; e2e "plain play of a ... film shows each frame once, in order": 0 and 0 before and after (the
      # repeats the founder saw were RT3d's and RT3e's)

    Scenario: RT3b. The playhead moves with the picture
      Given a film playing under the timeline
      Then the playhead and the collapsed scrubber's played part move at each frame the page draws
      And the view still renders at the clock's four timeupdates a second, not at every frame
      # Status: built (the app's src/timeline/livePlayhead.ts on the player's time each page frame, clock.ts useLive);
      # e2e "plain play": more than min(10, 0.6 of what it could) moves a second, 3-4 before; a key at the playhead
      # while it plays (a step, a split, a mark) acts on the frame showing, not the last timeupdate

    Scenario: RT3c. A seek while playing plays on from there, with nothing from where it was
      Given a film playing
      When it is seeked back or on (the ruler, Home)
      Then the seek's frame is the first from its new place, and no frame decoded for the old place shows after it
      And play goes on from the frame after it, with no frame passed over for the seek's decode
      # Status: built in the window: the page drops the frames queued for the old place and keeps none that come while
      # the seek is on its way (runtime/dom/media keep_due), and the decoder sends the seek's frame before it starts
      # its clock (media_play.rs index_at). Unit tests above; e2e "a seek while it plays" (passes on both builds)

    Scenario: RT3d. An edited film plays across its edits once
      Given a film split on the timeline, and a stretch of it taken out
      When it plays across the split
      Then it plays on into the next clip with no seek, and no frame is shown twice
      When it plays across the cut
      Then no frame the cut took out is shown, while the page draws a frame at least every three of the film's
      # Status: built (the app's src/timeline/playcut.ts onward): a clip's end is heard at every page frame, not at the
      # timeupdates; into a clip that goes on from the same place in the file the player plays on; anywhere else the
      # seek is made on the clip's last frame, early by the page's frame time (clock.ts Live.gap). e2e "an edited film
      # plays across a split ... and across a cut"; unit tests in the app's src/timeline/playcut.test.ts

    Scenario: RT3e. Played to its end, the film stops there
      Given a film playing under the timeline
      When it reaches its last frame
      Then it stops on it, paused, and play starts it again from the start
      # Status: built (the player's loop is off under the timeline; the old strip's player still goes round), and a
      # page's media element played again from its end is at 0 from the call on, as a browser's is (sdk/ui
      # dom/media.ts play; test/dom.test.tsx). e2e "played to its end"

    Scenario: RT3f. Frames passed over under 1 in 500 on a loaded machine
      Given the films of RT3a played on a 120 Hz page with other sessions' renders and builds running
      Then fewer than 1 in 500 of their frames are passed over
      # Status: not built. Passed over goes with the page's frame time (0.5% at 9 ms p50, 58% at 35 ms), the same
      # before and after. A sample of the window playing 1080p60: a third of the main thread waits on the render
      # sub-app, Metal's command buffer submission is the biggest single cost (the GPU shared with other renders), the
      # page's JS step about a tenth, the DOM host's layout out about 8%, Blitz's paint 4%; the page uses 15-40 ms of
      # CPU over its threads for each frame it draws. Left: the frame's cost (a video frame that changes only its
      # texture should not lay out or paint the page again), and the IOSurface path for decoded frames.

    Scenario: RT3g. The window plays an edited film's clips itself
      Given an edited film's main track as a list of file ranges
      When the page hands the list to the player
      Then the window shows each range's frames and goes on to the next range's first frame, decoded ahead, with no
        hold at the cut and no frame of the file between ranges, however late the page draws
      # Status: not built. Today the page seeks at each cut (RT3d): the last frame holds for the seek's decode (27-280
      # ms headless), and a page held up for more than three frames at the cut shows the one frame it draws there

    Scenario: RT3h. The sound clock on screen
      Given a film playing with its sound on, on screen
      Then the picture follows a clock that moves at every page frame and never goes back
      # Status: built, not measured: the clock runs on between the device's callbacks from the last one's position,
      # never before a time already read, and a start during a callback stands (media_sound.rs Shared::time_at, its
      # unit tests). The headless bench mutes sound (RAEA_AUDIO_MUTE=1), whose clock is the wall clock
