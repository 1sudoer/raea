@spec
Feature: Callouts: an agent's labels on what it made, beside it, pointing in when looked at
  Shane, 2026-11-18: "whenever it makes a image or video, it will like combine a bunch of frames and say the left is
  this, the middle is this, the last is this ... label the frames within an image or within a video ... it can just
  look like little tags if they were really short, but ... opening it up to have like a paragraph of information if
  ... the agent wants to. So instead of just like putting it in the title of the image of left this, bottom this, right
  this, whatever, now it can have ... these labels show up like in the UI, not overlapping any of the imagery. Like we
  would want it in the like periphery, like in the canvas area ... not an edge, like we don't want to use the edge
  components, but like some sort of little like armature thing that just comes over and animates when you hover, just
  to be sure that we're pointing at the right thing ... we could even utilize that same aspect for specific call out
  to the elements within scenes ... link these little armature comments to like the 3D scene elements that can be ...
  hovered on."

  What there was (read 11-18): an agent that put several frames in one still (a contact sheet from `raea media sheet`,
  a side by side, a before and after, a grid of props) said which was which in the item's title or in its message:
  "left: v1, middle: v2, right: v3". The person's own notes have pins and marks on the picture (docs/fingerprint.feature
  FP11a) and in a scene's space (docs/scene-feed.feature SF3), but those are comments, asking for something. Nothing let
  the agent say what a part of its own picture is.

  The callout:
    - A label is the agent's: a tag of a few words, and a paragraph more when there is more to say. It points at a
      place in what was made: a box or a point on a picture, a still of a storyboard, a moment or a span of a film, a
      part of a 3D scene or a point in its space.
    - It is never drawn over the picture. The tags stand in the canvas around it, each on the side nearest the place it
      names, in that place's order, apart from each other.
    - Pointing is on hover: the tag's armature, a thin line with one bend, draws out from the tag to the place, and the
      place's corners are marked. Moving away, it draws back. Nothing is drawn on the picture until then.
    - It isn't a comment: it asks nothing and goes to no agent. It is part of the item, as its title is.

  Background:
    Given the app's code is apps/app (raea-feed): src/model/labels.ts, src/detail/blocks/Callouts.tsx, the views that
      draw them (src/detail/views Look.tsx, Watch.tsx, Scene.tsx), and how they come in (src/acp/post.ts, host/show.ts,
      src/acp/protocol.ts feed_post)
    And raea's own side: `raea show --label` (engine/cli/src/show.ts) and the sheet's labels (apps/media picture.rs)

  Rule: CO1. A label, and what it can point at
    # Status 11-18: in 0.1.8. The feed draws them (raea-feed labels: a still, a film, a scene's parts live, the card's
    # count); raea show takes --label, --detail and --labels; raea media sheet writes OUT.labels.json; the raea skill says
    # when to label (CO5a).

    Scenario: CO1a. A tag and a paragraph
      Given a label
      Then its tag is its text, a few words (up to 60 characters; longer is cut, with the rest put first in its
        paragraph)
      And its detail, when it has one, is a paragraph (up to 1200 characters) the tag opens

    Scenario: CO1b. Where it points
      Then a label names one place: a box (x0, y0, x1, y1) or a point (x, y) in the picture's own 0-1 coordinates
      And with frame N, the storyboard's still N (from 0, as a still's pin is) instead of the first
      And with t (and to), a film's moment (or span) in seconds: it is drawn while the playhead is within half a second
        of the moment, or in the span
      And with part, a 3D scene's part by its source key or its name; with space, a point in the scene's space; with
        entry, which of a board's scenes
      And a label that names no place it can be drawn at is kept off the stage and listed under the picture instead

  Rule: CO2. How labels come in
    Scenario: CO2a. feed_post takes labels
      When an agent posts with feed_post and labels: [{ text, detail?, box? | point? | part? | space?, frame?, t?, to? }]
      Then the item has them, cleaned: up to 24, numbers held to 0-1, an empty tag dropped, a box's corners put in order
      And a label it can't read is said back in the post's answer, not dropped quietly

    Scenario: CO2b. raea show takes labels
      When an agent runs raea show FILE --label "PLACE TAG" [--detail "PARAGRAPH"] ...
      Then PLACE is x0,y0,x1,y1 or x,y, then #N for the Nth file shown (from 1), @T or @T-TO for a film's seconds, or
        part=KEY for a scene's part, joined without spaces (0,0,.5,1#2@3-5); the tag is the rest
      And a --detail is the paragraph of the --label before it
      And --labels FILE.json gives them as feed_post does

    Scenario: CO2c. A contact sheet comes labelled
      When raea media sheet writes OUT.png
      Then it writes OUT.labels.json beside it: each cell's box and its time ("0:03.2"), or its file's name for a sheet of
        several files
      And raea show OUT.png takes OUT.labels.json along when it is there, so the sheet's cells say what they are with
        nothing more done; an agent that wants to say more edits the JSON's tags and paragraphs first

  Rule: CO3. Drawn beside the picture, never on it
    Scenario: CO3a. In the canvas around the picture
      Given a still, a film or a scene with labels, opened
      Then each label's tag stands in the canvas beside the picture, on the left or right, whichever the place it names
        is nearer, or below when neither side has room
      And tags on a side are in the order of their places from the top, and never overlap each other or the picture
      And as the picture is zoomed or panned, or the stage resized, they keep to its sides

    Scenario: CO3b. On a film, in time
      Given a film's labels with moments or spans
      Then a label stands while the playhead is in its time and fades out of it; one with no time stands throughout

  Rule: CO4. Pointing, on hover
    Scenario: CO4a. The armature
      When the pointer is on a tag (or on the place it names, for a box)
      Then the tag's armature draws out from the tag to the nearest edge of its place (a box) or to its point: along
        the side's margin, one bend at the picture's edge, then in
      And the place's corners are marked while it's out
      And moving off, it draws back
      And with reduced motion it appears and goes without drawing

    Scenario: CO4b. The paragraph
      When a tag with a paragraph is clicked
      Then the paragraph opens under the tag, in the margin, and the armature stays out while it's open
      And Esc or a click elsewhere closes it

    Scenario: CO4c. A scene's part
      Given a label on a scene's part
      When its tag is hovered
      Then the armature goes to the part as the stage draws it now, and the part is outlined as when the pointer is on it
      And it follows the camera and the scene's time as they move

  Rule: CO5. The agent's skill says when to label
    Scenario: CO5a. Label, don't title
      Then the raea skill says: when one picture holds several things (a sheet, a side by side, before and after, a grid
        of props or characters), label each instead of naming them by position in the title or the message, with a
        paragraph only where a tag isn't enough
      And it shows raea show sheet.png --label "0,0,.5,1 v1, hook first" --label ".5,0,1,1 v2, product first"
        --detail "Opens on the product..."

  Rule: CO6. On the feed's card
    Scenario: CO6a. Labels on the feed's card
      Then the card shows how many labels the item has, and not the tags themselves, until a card has room for them
