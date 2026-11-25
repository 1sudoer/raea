Feature: Creatures: a four-legged body in the character SDK
  Set by the user 2026-11-14, with the trashpanda project (a stylized raccoon that walks, sneaks, crawls, stands and
  walks on two legs, climbs a tree, dangles from a wire, vaults, kicks off a wall, holds things, and goes limp): "if
  you feel that a four legged creature has merit in the character sdk then also make your PRs alongside the other
  work to include them and what you find".

  It has merit. The character SDK is one skeleton (MakeHuman's game_engine: 22 body joints and 30 finger joints) and
  one way of moving it (captures retargeted joint by joint, a motion matcher that picks frames, IK only to hold a
  contact). None of that reaches an animal: there is no capture library for a raccoon, a dog or a cat on our rigs,
  and retargeting a person's clip to four legs is not a thing. An earlier game's dog and cat were posed by a solver
  written inside that project, and called stiff. So an animal needs the other half of what docs/hands.feature
  began: motion made from what the body is doing, with no capture of it. This file is that for the whole body.

  What was found, building the raccoon:
  - Skeleton first holds for a built model as it does for a drawn one. The joints are set down by measure; the body
    is then laid round them as solids, so every limb is where its bone is and the weights have something true to
    start from.
  - Where a limb joins the trunk decides whether the skin tears, more than the weights do. With the knee and elbow
    touching the belly at rest, a voxel union fuses skin across them, and no weighting saves an edge whose ends ride
    on joints that move 10 cm apart: 0.4% of edges torn at a walk. Moving the knees and elbows clear, so a leg joins
    the trunk only at its hip or shoulder, took the walk to 0.05% with the same weights. Smoothing weights harder
    made it worse (0.2% to 0.4%): it spreads a far-moving joint's pull into the trunk.
  - What ended the tearing at the hip and shoulder was to stop joining them: each leg its own closed piece of
    skin set into the trunk, the join hidden by the fur. A furred animal does not need one watertight skin.
  - Bone heat gives the belly to the elbows and knees (they sit just inside it, the spine far above). The trunk's
    skin has to be given to the spine outright: each point to the two spine joints it lies between.
  - Feet kept in the world, not in the gait's cycle, are what stop sliding: a planted foot is a point that does not
    move until its leg's turn comes. Measured slide is 0 mm in every move, turning and stopping included. A cycle
    played under a moving body (that earlier solver) slides whenever speed or heading changes.
  - One rigging (the hips' and shoulders' frames, four limb ends, the head's turn, the tail) is enough to pose every
    move, and blending riggings rather than poses lets one move ease into the next with the limbs still solved.
  - A small animal's legs are short for its stride: at standing height a foreleg reaches 8 to 11 cm ahead of its
    shoulder. Strides have to come from the leg's length (a walk's stance travels about one leg length), or the
    elbow locks every step.
  - Fur as shells (16 copies of the skin pushed out along its normals, each cut by one height map at its own height)
    draws on a skinned model in the raster renderer at no visible cost: 232k triangles, 11 ms a frame at 1280x720.
  - A limp body made of points must not gain speed from being pushed out of a solid: the point's last place moves
    with the push. Without that, a body that flops half over a box leaves at 16 m/s.

  Rule: Built

    @done
    Scenario: CR0. Where it lives
      # sdk/body/src/creature, engine-free TypeScript beside the person's body layer, exported as
      # `creature` from raea/body. The moves that need a world (CR7 on) are in the trashpanda project until they
      # have settled (its game/ folder).
      Then the plan:
        | crate | file                           | lines | job                                                                  |
        | sdk   | sdk/body/src/creature/q.ts     | 40    | the body layer's maths under short names; a bend from places; a turn from axes |
        | sdk   | sdk/body/src/creature/body.ts  | 45    | a rig's measures: sockets, bone lengths, where each foot belongs, reach |
        | sdk   | sdk/body/src/creature/loco.ts  | 185   | the gait by speed and posture; feet planted in the world and stepped   |
        | sdk   | sdk/body/src/creature/pose.ts  | 225   | the rigging, the solver, the blend between riggings, the tail on springs |
        | sdk   | sdk/body/test/creature.test.ts | 60    | slide, gait, every joint posed, feet squared up on a stop             |

    @done
    Scenario: CR1. A creature's rig is read from its joints
      # A rig is joints by name with a parent and a place at rest, every joint resting unturned (the engine places
      # each by its own transform), and `marks` for the points that end a bone and carry no skin (nose, tail's tip,
      # toes). Names: root, spine1..3, chest, neck1, neck2, head, jaw, ear_l/r, lid_l/r, tail1..N; fore legs fl/fr
      # _scap, _upper, _lower, _wrist, _hand, digits _d1a/_d1b.._d5a/_d5b; hind legs hl/hr _upper, _lower, _hock, _paw.
      Given a rig.json with those joints
      When bodyOf reads it
      Then it has each leg's socket, bone lengths, reach and where its foot belongs under the body

    @done
    Scenario: CR2. A planted foot does not move
      # Each foot is a point in the world. It stays until its leg's turn in the gait, then swings to where the body
      # will need it half a stance after it lands, aimed again every frame of the swing (a turn mid-step still
      # lands right). A foot left more than 15 cm (fore) or 19 cm (hind) from its place goes at once.
      Given a body walking, trotting, galloping, sneaking, crawling and walking on two legs, turning all the while
      Then no planted foot moves between two frames
      # measured 2026-11-14: 0.0 mm in all six (sdk/body/test/creature.test.ts; tools/locotest.ts in the project)

    @done
    Scenario: CR3. The gait is the speed's
      # Stride, the share of a stride each foot stands, each leg's place in the stride and the lift are blended by
      # speed: a walk (lateral sequence, 0.66 standing) to 0.8 m/s, a trot (diagonal pairs, 0.5) to 2.1, a bounding
      # gallop (hind feet together, 0.36) above. Lowered, steps shorten and stand longer; on two legs the hind feet
      # alternate.
      Then stride grows and standing share falls from walk to trot to gallop

    @done
    Scenario: CR4. One rigging poses every move
      # The solver takes the hips' and shoulders' frames, four limb ends (a place, a turn, how far the wrist or heel
      # is rolled over it, how far the fingers are closed, where the elbow or knee points), the head's turn and the
      # tail's joints, and gives every joint's place and turn: the back laid link by link between the two frames,
      # the shoulder blade sliding with its leg, elbow and knee bent in a plane, the neck sharing the head's turn.
      Given the body on the ground, lowered, and up on its hind legs
      Then every joint is a number in every frame, and at a walk and a trot every leg reaches its foot

    @done
    Scenario: CR5. Standing still, it squares up
      Given a body that galloped and stopped
      Then within three seconds each foot is within 3 cm of where it belongs, brought there one at a time

    @done
    Scenario: CR6. The tail is late
      # Its joints are points pulled toward where a stiff tail would be, stiffest at the root, kept their lengths and
      # off the ground; how it is carried (up at a gallop, low sneaking, down behind on two legs) is the posture's.
      Then the tail follows the hips' turns after them and never passes through the ground

  Rule: Built in the project, to come into the SDK

    Scenario: CR7. Moves off the ground share the rigging
      # In the project (game/player.ts): in the air (nose along its path, hands reaching for the ground coming down),
      # a vault (hands down on the thing's top, hind legs tucked through), a pull-up onto a ledge (hands on top, hind
      # feet scrabbling on the face), a kick off a wall, climbing a trunk (diagonal pairs, hands that stay where
      # they grip), hanging from a wire by the hands (a pendulum the player pumps, hand over hand along it), and
      # gone limp. Each builds a rigging; a move eases into the next over 6 to 34 frames by blending riggings.
      # A tour of all of them runs headless with every frame posed (tools/playtest.ts in the project).
      # To do here: the world they ask of (ground height, what is ahead, a ledge, a trunk, a wire) as an interface.
      Given a level that answers those questions
      Then a creature jumps, vaults, pulls up, kicks off, climbs, hangs and lands on it

    Scenario: CR8. Gone limp, and up again
      # In the project: nine points on springs (hips and shoulders left and right, the head, four feet), each foot
      # kept within its leg's reach of its socket; pushed out of solids without gaining speed; when it lies still
      # the body gets up by blending from the limp rigging to the standing one.
      Then a fall from 2 m tumbles, comes to rest in under 2 s and stands up where it lies

    Scenario: CR9. It takes things
      # In the project: on four legs a thing is taken in the mouth; up on its hind legs both hands close on it and
      # turn it over; thrown from either. To do: docs/hands.feature's holds and finger curl on a creature's hand
      # (five digits of two joints), so the fingers lie on the thing rather than at a fixed curl.
      Then a thing held is in the mouth or between the hands in every frame, and the fingers touch it

  Rule: To do

    Scenario: CR10. The skin holds in every move
      # Measured by tools/rigcheck.ts in the project: edges past 0.4x or 1.6x their length by more than 8 mm, as a
      # share of all edges. With one skin over the whole body (11-15 early) 9 of 13 moves passed, and the gallop
      # (0.76%), the crawl (0.48%) and two legs (0.9%) tore at the hip and shoulder, whatever the weights.
      # 11-15: each leg is now its own closed piece of skin, set into the trunk under the fur, and follows only its
      # own bones; the trunk follows none of a leg's. No skin bridges a leg and the trunk, so none tears there: 10 of
      # 13 pass (crawl 0.007%), and the fur hides the join. Left: gallop 0.11% (the back's flexing), two legs 0.22%
      # and 0.20% (the neck's fold and the tail's root).
      Then no move tears more than 0.1% of the skin's edges

    Scenario: CR11. A gallop with a back
      # 11-15: every leg now reaches its foot at a gallop (a stance of 0.29 of the stride, the body 14 to 16% lower
      # at speed; it was 87 of 1,200 leg-frames short). Still rough: the back's arch following the legs, the hind
      # legs' reach past the fore, and a real moment with all four feet off the ground.
      Then at a gallop every leg reaches its foot and the back's arch follows the legs

    Scenario: CR12. Other animals on the same solver
      # 11-15: done on a dragon (four legs, two wings, four neck joints, eight tail joints, horns, scales): the same
      # solver, sized by the legs' reach beside a raccoon's (k: every stride, speed and length in the solver is a
      # raccoon's times k; 1.74 for the dragon). Feet slide 0 mm in every move; the skin holds in 13 of 13. What it
      # needed that a raccoon did not: any number of neck joints sharing the head's turn, optional ears, lids, digits
      # and belly, wings (CR17), a body on nearly straight legs carried a little lower so it has slack to step with,
      # and how it carries itself as four numbers in its rig (head low, heel down, tail's hang, wings' fold).
      # 11-15 later: a dog and a cat made by another project (the monk game's animals, on the same names) walk, trot
      # and gallop on it too (sdk/body test/creature.test.ts, private repo 3de8813). They needed two things the
      # raccoon and the dragon hid: a fore leg may end in a paw instead of a hand (the body names each leg's planted
      # joint), and a body whose legs have less room behind their feet than a raccoon's (straighter legs: measured
      # from the rest pose at the height it is carried) steps at a smaller size, so shorter, quicker steps at the
      # same speed. Before that, setting off left a hind foot planted up to 31 mm out of its leg's reach while the
      # other three swung. The raccoon's steps are unchanged; the dragon's are 9% shorter.
      Given a dragon's rig
      Then it passes CR2 to CR5 and CR10 on the raccoon's solver with no line of the solver naming it

    Scenario: CR15. A creature is a spec, and one flow makes it
      # In the project (blender/creatures.py, tools/creature.sh). A creature is data: its joints by measure on the
      # names' convention (root, spine1..N, chest, neck1..N, head, jaw, ear_/lid_, tail1..N, fl/fr/hl/hr legs, wl/wr
      # wings, belly), the marks that end a bone, how thick its flesh is round each joint, its webs and horns, what
      # its skin is, the words it is drawn from, and its carriage. The flow:
      #   mannequin  the skeleton clothed in grey flesh (tubes by the radii, sheets for webs, cones for horns)
      #   art        the image model draws the animal on the mannequin, same pose and view (flux_2_pro, 3 cents)
      #   idol       the drawing made 3D on this Mac (Idol, about 3.5 minutes), carved to one closed mesh
      #   build      the mesh laid on the skeleton by its feet and nose (its long axis found), thinned, painted from
      #              its own colour; the joints refitted to it (back, a standing neck by the middle of its flesh, legs,
      #              the tail along its own line, ear and horn tips, wing tips and wrists); weights by bone heat on a
      #              stand-in, the trunk given to the back by place; each leg and wing cut into its own closed piece
      #   check      the reel posed by the solver and the skin measured (CR10)
      Given a spec for a raccoon, the same skeleton as a vinyl toy, and a dragon
      Then each command makes each of the three, and all three pass CR10 with no change between them

    Scenario: CR16. The coat is the spec's
      # fur: the generated surface is the coat's outside; the skin is set in by the fur's length for each part and 16
      # shells grow back out to it (CR0's finding). scales, plain: the surface as it is, no shells, the picture opaque.
      # A second style is a second spec with other words and another skin: the toy raccoon passed on its first run.
      # Not done: a fur length read from the drawing (clipping the coat in the drawing keeps the outline, so the gap
      # measures nothing), lids that close over painted eyes, a mouth with an inside.
      Then a creature's skin is fur shells or the bare surface by its spec, and a restyle changes no code

    Scenario: CR17. Wings
      # A wing is a limb (wl/wr: shoulder, elbow, wrist, the tip a mark) and its own piece of skin. Its membrane is
      # too thin for the 3D model to keep (Idol gave the arm and fingers and dropped the sheet), so the build makes it:
      # a sheet between the refitted joints, each point following the joints at its corners. The solver raises a wing
      # about the body's line and folds it in its own plane joint by joint (which way round each joint folds is read
      # from the rest pose). Rough: the membrane is one flat colour and does not follow the finger bones; no flight.
      Then a winged creature folds its wings as it walks, opens and beats them as it runs, and no web point is unbound

    Scenario: CR18. Mouths and eyelids
      # 11-15, in the project's build, the same on all three creatures. The mouth: the drawn mouth is paint on a closed
      # head, so a turning jaw only stretched the lips. The head is cut along the lips' line (a plane through the
      # muzzle's front a third of the way up it, rising a little toward the corners), what is under the cut follows the
      # jaw, what is over it the head, and a dark inside (a roof on the head, a floor on the jaw, meeting at the hinge)
      # fills what opens. The skin still holds in 13 of 13 moves. No teeth or tongue.
      # The lids: each is a small cap of its own, resting under the brow, that turns about an axis lying along the head
      # (the rig's lid_axis; a turn about the head's side-to-side axis does nothing for an eye that faces sideways) and
      # takes the colour of the skin round the eye. Where the eye is comes from its pixel in the drawing (eye_px in the
      # spec). The model was made from the drawing, so seen from the drawing's side it is the drawing: that side is
      # searched for (view, size and shift, scored by how much of the model lies inside the drawing's outline, since a
      # drawn wing's membrane is not on the model), the pixel is cast along it onto the raw model, and the point it
      # meets is the same point on the model once laid on the skeleton. A strongly coloured eye is then moved onto
      # the few points nearby whose colour leans toward the pixel's. Casting from the mannequin's camera instead was
      # centimetres off (the model is not laid exactly where the mannequin stood).
      # Rough: the lid shuts over the eye on the toy raccoon and about on it on the raccoon; on the dragon it lands
      # about an eye and a half away (outlines agree 0.77 against 0.84 and 0.92, and its eye is a handful of points).
      Then a creature's jaw opens a mouth with an inside, and its lids shut over its eyes

    Scenario: CR13. In the engine
      # As docs/hands.feature moved the hands to Rust (raea-sys-body), a creature's solver belongs beside it once
      # the TypeScript has settled, with <Creature> in raea/character drawing a rig from a rigging.
      Then <Creature rig pose /> draws a four-legged body, and character.creature answers a rigging's pose
