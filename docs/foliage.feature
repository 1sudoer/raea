@spec
Feature: raea/foliage, plants grown from their species and drawn through the year
  Shane, 2026-11-17: "full seasonal foliage, researching native tree/plant/flower types and try to build procedural
  foliage systems, wild, overgrowth, ivy (if applicable), residential gardens or parks ... hyper realistic city wide
  procedural generation". Every season and weather is drawn (docs/wear.feature WR9; docs/realism.feature D17, LT16),
  so a plant is grown once and the year is drawn over it: leaves come, colour and fall, flowers open, snow lies on
  the branches, all from uniforms, nothing grown again per season.

  raea's plants so far are a stylized meadow (systems/grass, <Grass>), a field-made tree (raea/render grow.tree, an SDF
  solid with no leaves) and props placed round a room (raea/room plants.tsx). None knows a species, a leaf or a
  season. The method here is the literature's:
    - Trees grow by space colonization (Runions, Lane and Prusinkiewicz, "Modeling Trees with a Space Colonization
      Algorithm", 2007): attraction points fill the species' crown, and branches grow toward the points near them and
      remove them, so branches compete for space as real ones do. Branch radii follow the pipe model (a parent's
      radius to the power n is the sum of its children's).
    - Climbers grow by Luft's ivy generator (2006): a stem steps along its last direction, pulled to the nearest
      surface (adhesion), up toward light, down by gravity when it lets go, with some chance in each step.
    - Seasons follow phenology: each species' dates of leaf unfolding, flowering, colouring and leaf fall, as the
      DWD's observers record them (CC BY 4.0), sourced per place and era by the project that uses them.
  Leaves and bark are made in code (a leaf's outline, teeth, veins and blotches from its shape's rules; bark's
  fissures from noise in the shader), never photographed, so a species is a table of numbers and a seed.

  Rule: 1. Plants

    Scenario: FO1. A tree grown from its species
      Given a request: a species (FO4), a height, a crown width, a clear trunk (the height of the first limb), and a
        seed
      When raea-sys-foliage grows it
      Then a trunk rises to the crown's base, the crown's envelope (the species' profile, its widest point and its
        squareness) is filled with attraction points (denser near the surface, where light reaches), and the tree is
        grown into it by space colonization with the species' step, reach, kill distance and tropism (up for an
        ascending lime, down at the tips for a sweeping chestnut)
      And short twigs sprout from the outer shoots, so a bare tree in December shows its fine twig haze
      And radii follow the pipe model from a twig's radius at every tip, and each branch is one tube from its parent's
        surface to its tip, its sides fewer as it thins
      And the same request gives the same bytes
      And the output is one .glb with material "bark" and eight leaf groups "leaf-0" to "leaf-7" (FO3), in the
        plant's local frame: y up, metres, the origin at the foot of the trunk

    Scenario: FO2. Leaves and bark made in code
      Then each leaf kind is drawn into an atlas by rules: a blade's width along its midrib (cordate, ovate,
        lanceolate, obovate), its teeth, its lobes (palmate, oak's rounded ones) or its leaflets (palmate compound as a
        chestnut's, pinnate as a black locust's), its veins and its blotches, with sixteen variants a species
      And the atlas's channels are what the season needs, not a colour: alpha is the blade, red the veins, green the
        blotches, blue the distance to the edge (autumn colours from the edge inward)
      And each leaf is a card on a shoot, turned toward the light and out from the crown, its normal bent outward so
        the crown shades as a volume; its vertex colour holds its own random number, how exposed it is and how far
        along its shoot it is
      And bark is shaded in a surface hook: fissures from noise along the branch, the species' pattern (a plane's
        flaking patches, a birch's white with dark bands, a locust's ropes), soot, wet and snow on what faces up

    Scenario: FO3. The year on one tree
      Given the day of the year, the species' phenology and the weather
      Then leaves unfold from bud in order along each shoot, grow to full size, darken through summer, colour from
        their edges inward in the species' autumn colours (lime yellow, maple orange to red, plane brown), and fall
        in the order their random numbers give
      And a leaf group whose leaves have all fallen, or not yet come out, is hidden through its material (its base
        colour's alpha 0 with alpha mask), so its shadow goes too; a surface hook's discard can't do this, as the
        prepass and shadows draw the standard material's (systems/shader: only the main pass runs a hook)
      And flowers (a chestnut's candles, a lime's yellow clusters) and fruit come and go by the same dates
      And snow lies on the tops of branches, bark darkens when wet, and a frost whitens the twigs

    Scenario: FO4. The species
      Then each species is a row of numbers: crown profile, growth, leaf kind and size, leaves a shoot, bark, height
        to width, autumn colours, and its sources
        | species                       | crown                         | leaf                                  |
        | lime (Tilia cordata, x europaea) | dense ovoid, ascending limbs | cordate, toothed, 6-10 cm          |
        | elm (Ulmus minor, laevis)     | broad, vase                   | ovate, lopsided base, toothed, 7-12 cm |
        | plane (Platanus x acerifolia) | broad dome, big limbs         | palmate, 3-5 lobes, 12-25 cm          |
        | Norway maple (Acer platanoides) | round, dense                | palmate, 5-7 pointed lobes, 10-15 cm  |
        | horse chestnut (Aesculus hippocastanum) | broad dome, sweeping | palmate compound, 5-7 leaflets       |
        | black locust (Robinia pseudoacacia) | open, irregular         | pinnate, 9-19 leaflets                 |
        | pedunculate oak (Quercus robur) | broad, crooked limbs        | rounded lobes, 7-14 cm                 |
        | silver birch (Betula pendula) | narrow, weeping twigs         | triangular, toothed, 3-7 cm           |
        | Lombardy poplar (Populus nigra 'Italica') | column            | rhombic, 5-10 cm                       |
        | white willow (Salix alba)     | broad, hanging twigs by water | lanceolate, 5-12 cm                   |
        | lilac, elder (shrubs)         | many stems                    | cordate; pinnate                       |

  Rule: 2. Climbers and the ground

    Scenario: FO5. Climbers grown over a model
      Given a model (a city tile or a house) and roots where a wall meets soil
      Then a climber grows over it as Luft's ivy does, clinging to walls and sills, turning at a cornice, thinning
        toward its top, and kept off the openings tenants cut it from (ASSUMED 0.3 m)
      And ivy keeps its leaves all year; wild vine colours red in autumn and is bare in winter, its stems showing

    Scenario: FO6. The ground's plants by their causes
      Then weeds grow in paving's joints and at walls' feet where feet rarely tread and it stays damp (the wear bake's
        touch, ground and sky), lawns and beds where the map says garden, vegetable rows and allotments where it says
        so, each as instanced tufts near the camera and a shaded ground beyond

    Scenario: FO7. Shrubs and wild growth
      Then shrubs grow from the same generator with many stems, and ground left alone fills by how long it was left:
        grasses and mugwort in the first year, elder and locust saplings after a few

  Rule: 3. A city

    Scenario: FO8. Wind
      Then branches and leaves sway by their order and distance from the trunk; this needs a vertex hook the surface
        shader doesn't have yet (an engine gap: docs/shaders.feature)

    Scenario: FO9. Levels of detail
      Then a tree near the camera has every leaf; farther, its leaves are cards of leaf clusters; farther still, an
        impostor; each level's triangles and bytes are measured against the frame budget (docs/realism.feature LT20)

    Scenario: FO10. `foliage.grow`, `raea foliage grow` and raea/foliage
      Then a composition grows a plant with a native call, a shell with the verb, and raea/foliage gives the leaf and
        bark hooks and a season's uniforms from a day, a species and the weather

    Scenario: FO11. A city planted from data
      Given a tree layer (points with crown widths), a register of trees with species and planting years, the streets
        and the gardens
      Then each tree takes its species from the register where an old tree stands near it, else from its street's or
        park's weights, and its size from its crown; climbers take walls by rule; the ground takes its plants by
        cause; and each tile gets a placement file of plants by kind

    Scenario: FO12. Proof
      Then one tree of each species is rendered through the year (bud, full leaf, autumn, bare, under snow) beside
        public-domain photographs of the species, and a street, a square and a court are rendered in four seasons,
        with each species' triangles, bytes and growth time printed
