Feature: First person: his own body's hands, what they do with the world, and cutscenes, at AA quality
  Set by the user 2026-11-17, on the Gambit trailer's previz: "the ones in the previz look hella broken we will for
  sure want super relaistic hands and fps system for procedural hand animaitons + whatver e can do in kimodo. or
  sequences liek iopening a swere grate and climbging in, or steppign through a drill hol in the wall, or pulling up
  a map to view etc", then: "we need like AA level quality hands and fps interaction and cutscene sytem id honeslty
  work on that before the trailer". So the trailer waits on this.

  What raea already has, and this builds on rather than redoes:
  - raea/character's hands (docs/hands.feature, HG0 to HG19): the arm's reach that leans, fingers closed on a thing's
    distance field in a grip chosen for its shape (graspThing), take and fetch (a thing taken from where the body
    stands, or walked up to first), interact/plan/hold (beats of reach, grab, lift, carry, put, throw), hand shapes
    from SOMA-X's prior, and HG14's measured bar (tools/hands-audit) that the hands are held to.
  - raea motion: Kimodo on MLX on this Mac (a PyTorch backend for other machines is being built), whole-body motion
    from words with keys (full-body poses, hand and foot targets, a root path).
  - Games: useInput and useGameState, a pure step per frame, played live by raea play.
  Where the code lives: in the Gambit project's components/fp/ while it is made (so no uncommitted file of this
  sits in sdk/character and holds up the character lane's landings), moved into raea/character as `raea/character/fp`
  once it passes FP7 and commits are agreed.

  What's missing: a first-person view of that body; the layers a first-person game puts on its hands; hands on the
  world's fixed things (a grate, a ladder, a door, a hole's rim) rather than things carried; and sequences that take
  the camera from play into a scripted move and back.

  What "AA" means here, as qualities to measure (the user's bar; no other game's code or art is used): hands that
  read as real hands in a close-up (skin, nails, creases, gloves that crease), that touch what they touch (no skin
  in a thing, pads on it), that never pop, that lag and settle as a body's do when the view turns, walks or fires,
  and moves into and out of scripted sequences with no seam a player sees.

  Rule: The rig

    Scenario: FP1. True first person: the camera is his body's eyes
      # A first-person body, not floating arms: the dressed body posed by its stance, a clip or the walk, its eyes at
      # the camera, its head turned the way the camera looks. Gambit's probe: components/trailer/fp-body.tsx.
      Given a body posed standing, sitting, kneeling or from a clip
      Then the camera is at its eyes' landmark within 1 mm and looks where its head faces
      And the head's motion reaches the camera stabilised: roll within 1 degree, and no shake above 4 Hz larger than
        0.3 degrees (a person's eyes hold the world still as the head bobs)
      And no skin or cloth of his own is nearer the camera than its near plane, in any frame of FP7's set
      And his body casts its shadow and is seen when he looks down (feet, knees, chest)

    Scenario: FP2. His hands are framed
      # True first person at a film's lens puts the hands low or out of frame; a game's hands are framed. Hands are
      # pulled in toward the view's centre by the least that frames them, along the arm (shoulder and elbow follow),
      # never moved off what they hold.
      Then in every beat of FP7 that needs them the hands are within the frame's safe area
      And his sleeves and elbows never fill more than a quarter of the frame
      And a hand on a thing stays on it (FP5) while it is pulled in

    Scenario: FP3. The layers on the hands and the view, as functions of time
      # Frame-pure: each layer is read from the camera's path, the body's clip and the game's state, never integrated
      # frame by frame in a composition. A game feeds the same layer from useGameState.
      Then these are layers with a weight each, off at 0:
        | layer   | what moves                                                                          |
        | breath  | chest, shoulders and the held thing rise and fall, 12 to 20 a minute by exertion   |
        | idle    | a slow drift of the hands and view, under 5 mm and 0.3 degrees                      |
        | sway    | the hands lag the view's turn and settle (a critically damped spring on its path)   |
        | bob     | the view and hands follow the walk's foot contacts, not a sine                      |
        | kick    | a shot's recoil back and up, and its recovery, by the weapon's weight               |
        | tremor  | the hands' shake under stress or cold, by a heart rate                              |
      And each is measured: its amplitude and frequency in a test, and a still frame of each is unchanged at weight 0

  Rule: Hands, AA

    Scenario: FP4. The surface holds up in a close-up
      # Owner: the character lane (HG18, a surface: skin, clothes, hair). Shown at 4K, hands filling the frame.
      Then the hand is SOMA's at full detail with nails, knuckle creases and skin with light under it
      And period gloves are garments on the hand's joints that crease at the knuckles and leave no skin through them
      And a sleeve's cuff covers the wrist through the arm's whole range, with no gap and no cuff inside the forearm
      And a men's civilian winter coat of 1917 Berlin exists for him (the probe tints the British jacket: a stand-in)

    Scenario: FP5. Contact, in first person
      Given every interaction in FP7, measured by tools/hands-audit's measures on the body's own skin
      Then no skin of the hand is in a thing beyond a pad's give (2 mm), none in the scene or his own body
      And every finger meant to hold has its pad on the thing
      And no joint's speed changes by more than 300 degrees a second in a frame, fingers included
      And no hand pops between beats: a goal that changes is blended by inertialization, not cut

  Rule: Interactions with the world

    Scenario: FP6. An interactable is data
      Then an interactable names:
        | field    | what                                                                                       |
        | solid    | its shape, as a distance field and a mesh                                                 |
        | guides   | where hands go: bars, rungs, edges, handles, rims, each a raea/character guide              |
        | parts    | what moves: a lid that lifts and slides, a door on its hinge, a lever, with mass            |
        | beats    | approach, align, reach, grip, act (along a path or about an axis, with its weight), release |
        | camera   | free look within limits, guided toward a point, or locked to the body                     |
        | body     | optionally a whole-body move: Kimodo with keys taken from the guides and a traversal path  |
      And running it gives, for any time, his pose, the camera and every part's place

    Scenario: FP7. The set, each in first person, in motion, passing FP1 to FP5
      Then these are made and shown:
        | id         | what he does                                                                        |
        | grate      | lifts a sewer lid with a hook, slides it aside, sits on the rim, climbs down the rungs  |
        | hole       | steps through a hole drilled in a wall: a hand on the rim, ducks, one leg then the other |
        | map        | takes a folded map from his coat, unfolds it and holds it up to read                    |
        | ledge      | lights a cigarette with a match at an open window, then sits up on the ledge            |
        | rifle      | shoulders a bolt-action rifle, aims, fires, works the bolt, loads a charger clip        |
        | door       | opens a door by its handle and goes through                                             |
        | lantern    | picks up a lantern by its bail and carries it, lighting the way                         |
      And each passes FP1 to FP5, with the numbers printed

  Rule: Cutscenes

    Scenario: FP8. A cutscene is tracks of time
      Then a cutscene has cameras (shots, cuts, blends), actors (clips captured, from Kimodo or procedural, with
        their contacts held by IK), props (parts moving), and events (sounds, effects, the score's cues)
      And the same file plays in a game (raea play) and renders (a trailer's shot)

    Scenario: FP9. Into and out of play with no seam
      Then play goes into a cutscene by a camera blend from his eyes, or a match on motion, and comes back to his
        eyes where the cutscene leaves his body
      And his pose is inertialized at each seam (FP5's 300 degrees a second holds across it)
      And where the cutscene allows, the player keeps look within limits (an interactive cutscene)

    Scenario: FP10. A playable test level
      Then a Berlin courtyard (a manhole, a door, a window's ledge) and a cellar with a drilled hole are played live
        with keyboard and mouse at 60 frames a second on this Mac
      And FP7's moves are rendered from the same files as trailer shots

  Rule: Order, and who
    Scenario: FP11. The order
      Then:
        | step | what                                                                      | who                      |
        | 1    | FP1 to FP3 on the probe (bakery counter, manhole, map)                     | the trailer overseer     |
        | 2    | FP6's data and runner, and grate, hole and map from FP7                    | the trailer overseer     |
        | 3    | FP4, the surface, and the coat                                             | the character lane       |
        | 4    | FP5's measures on FP7 (tools/hands-audit in first person)                 | both                     |
        | 5    | FP8 and FP9                                                                | the trailer overseer     |
        | 6    | FP10, with the interiors and map lanes' sets                               | the trailer overseer     |
      And the trailer resumes when FP7's set passes and Shane has watched it in motion
