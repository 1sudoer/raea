Feature: Drawn characters: a drawing becomes a raea character that looks like the drawing
  Set by the user 2026-11-13, over a day of tests in ~/Studio/internal/projects/sheet3d: a character drawn as a
  sheet (the style of a flat-shaded, chunky, painted figure) should become a rigged, animated raea character, and
  it has to look like the drawing: "i do not want that if it doesnt look like the actual original image. the goal
  is to only use the makehuman as the api layer for the body and that we are genuinely rigging the and fixing up
  the rest either by hand or procedurally"; "we basically only need to retarget to the makehuman mesh we dont need
  to completely conform it". On seeing the drawing's own Gaussian splat posed by raea's skeleton: "oh this looks
  infinitely better". And: "lets patch this then for 0.1.5 which will be the character sdk update aftyer
  performance".

  What the tests settled (each cost a round; the memory note sheet3d-character-workflow has the detail):
  - One image is enough for Idol (speridlabs/idol, Apache-2.0, local through mlx-serve, 8-bit) when it is a
    three-quarter view of an unlit, 3D-looking figure drawn over a pose guide. Its splat IS the drawing.
  - A mesh carved from the splat with vertex colours never looked like the drawing, whatever was done to it. The
    splat does. So the splat is what is seen, and a mesh carved from it is the hidden driver: it carries the
    skeleton's weights, the cloth and the face's units, and is never drawn.
  - MakeHuman's game_engine rig is the API layer only: the same joints, each off its parent in the rig's own rest
    direction, at the DRAWN figure's distances. Reshaping MakeHuman's body to the figure and drawing that was
    rejected twice.
  - The yardstick is an overlay, not an opinion: the character posed as drawn, through the pose guide's camera,
    laid over the drawing (outline overlap). Pieces assembled from layer drawings scored 0.73; the final drawing's
    own model 0.81, the rest being the drawing's perspective.

  Rule: 0.1.5, the character SDK update (after 0.1.4, the performance release)

    Scenario: DC1. A splat that follows a skeleton
      # raea's <Splat> is still: reveal, assemble, morph and the like move it, a skeleton cannot. The test moved one
      # million Gaussians by raea's rig outside the engine (sheet3d tools/splat_walk.py) and it held: each Gaussian
      # has four joints and weights, its centre goes by linear blend skinning, its turn by the nearest rotation of
      # the blended turn, its size is kept.
      Given a splat and, beside it, its binding (four joints and weights a Gaussian, the joints by name)
      When it is drawn with a skeleton's pose
      Then every Gaussian is where its joints take it and turned with them, in the shader, with no copy of the
        splat made on the CPU
      And the drawing order is the posed splat's (sorted from where the Gaussians are now, not where they rest)
      And a splat with no binding draws exactly as it does today
      And the plan:
        | crate               | file            | lines | job                                                                |
        | raea-sys-splat-skin | src/lib.rs      | 260   | the binding read and uploaded, the joints' matrices each frame     |
        | raea-sys-splat-skin | src/skin.wesl   | 110   | a Gaussian's centre and turn from its joints                       |
        | raea-sys-splat      | src/splat.wesl  | 490   | the skin applied before the splat's own animation                  |
        | raea-sys-splat      | src/order.rs    | 320   | the order from posed centres                                       |
      # (a crate of its own: raea-sys-splat is at 1,591 lines of its 2,000)

    @needs:DC1
    Scenario: DC2. Character draws a splat body
      Given a character folder whose body is a splat with its binding (and the hidden driver mesh beside it)
      When `<Character body pose>` draws it
      Then the splat is drawn on the character's skeleton by DC1, the driver mesh is not drawn, and every clip of
        the motion library plays on it as on any body of the rig
      And the character's shadow is the driver mesh's (a splat casts none)
      And the plan:
        | crate | file                                | lines | job                                          |
        | sdk   | sdk/character/src/character/splat.tsx | 180 | the splat body and its driver under Character |

    @needs:DC2
    Scenario: DC3. The face's controls on a splat
      # MakeHuman's 34 expression units were carried onto a drawn head once already (the fitted MakeHuman face laid
      # on the character's, each point taking the unit's movement at its place): they moved a mesh. A splat needs
      # the same movement a Gaussian, and a mouth that opens onto something.
      Given a splat body whose head carries the expression units as movements a Gaussian
      When `face={{ smile: 1 }}` (any unit or preset) is set
      Then the Gaussians of the face move by it, on top of the pose
      And an open mouth shows teeth, a tongue and a dark inside, a closed eye a lid, none of them stretched paint

    @needs:DC2
    Scenario: DC4. Clothes and hair that move on a splat
      # raea's cloth solver bakes a garment's mesh. Here the garment that is seen is Gaussians.
      Given the driver mesh's pieces (the jacket, the shorts, the hair) baked by the cloth solver as today
      When the character moves
      Then the Gaussians of each piece follow that piece's baked surface (bound to the surface, not only to bones)
      And nothing under a piece shows where it lifts (what lies under it is there, from the layer drawings)

    Scenario: DC5. From a written character to its folder, in raea
      # Today: sheet3d's tools (Python: draw.sh, sheet.sh, build_step.sh, segment.py, retarget.py, splat_to_rig.py,
      # splat_walk.py, match.py). They are the reference; they are not shipped.
      Given a character described in words (and, for layers, each step of its outfit)
      When `raea character NAME` runs
      Then the drawings are made over the pose guide in the unlit recipe, each becomes a splat locally (Idol), the
        final drawing's splat is fitted to the rig (the skeleton at the drawn lengths, the driver mesh carved and
        weighted, the binding written), and the character's folder is written
      And the match with the drawing is measured and said (the overlay's overlap), and the run fails under 0.75
      And open: which of the steps are raea's own code (Rust or TypeScript) and which stay a tool run through raea

    @needs:DC5
    Scenario: DC6. Detail from close-ups: the head and the hands
      # Idol shrinks whatever it is given into one small cube: a whole figure's face and hands are a few thousand
      # Gaussians. A close-up of the head carries the real face and hair.
      Given a close-up drawing of the head (and of a hand)
      When the character is built
      Then the close-up's splat replaces the figure's own head (and hands), at the size and place the figure's own
        have (fitted to the figure, never to MakeHuman's head: that shrank it to 0.88 once)

    @needs:DC5
    Scenario: DC7. Any humanoid, shown on a second character
      Given a second design that shares nothing with the first (chars/mara: a woman in a long coat, a bob)
      When DC5 runs on it with no step changed by hand
      Then it matches its drawing as well as the first does, and walks, turns and speaks

    Scenario: DC8. A mesh rigged on a Mixamo skeleton plays in Character
      # The user, 11-13: "what if we made the character sdk use mixomo skelton and then that any mesh thing that
      # uses mixamo skeleon". raea's rig stays the SDK's own (the motion library, cloth and the face are on it); a
      # Mixamo-rigged model is read onto it.
      Given a model skinned to Mixamo's joints (mixamorig:Hips ...), in its T-pose
      When it is given to Character
      Then its joints are read as the rig's by name, its rest pose is lined up with the rig's, and the library's
        clips play on it
      And a joint Mixamo has and the rig has not (or the other way) is said, not guessed
