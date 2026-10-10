# Bot faces and motion, v1 (draft for review)

Status: draft. The look was chosen from the storybook mockups in `prototype/crew-storybook.html`. This file turns that look into data and rules, and `crates/bot-face` runs them.

## 1. The look

A storybook robot with hardware. Round, friendly, and a little futuristic.

- **Outline.** Every body and every part has a dark outline, 3.2 units wide, with round corners.
- **A face.** Two round dark eyes with two white highlights, a small smile, and two soft cheeks (idle, thinking and done). Each mood has its own eyes and mouth (section 3).
- **Depth, not detail.** A soft shine on each body. Dark grey and steel for hardware. A glow is a few circles that fade out.
- **The reactor.** Each bot has one, on its chest, in a dark housing. It is a glowing core, a glowing ring, or a white spark. The glow colour is a part of the bot's identity.
- **One of each.** A body, one reactor, one way to move (tracks, one wheel, a spring, jets, legs, hover pads, lander legs, casters, cogs, a propeller) and one tool (a hard hat, a lens, goggles, a plane, antennae, a dish, a telescope, a feather, a brush, a wrench, a keyhole). This is a rule, not a taste. No rivets, bolts, extra lights, trails or loose objects.
- **Nothing around the bot.** No ring, badge or halo. State shows on the bot itself, by its eyes and its motion.
- **Colour.** The seven brand colours. Quill, Ink, Mimi and Gus share a colour with an older bot, and their shape tells them apart.

## 2. The data

`faces.v1.json` holds 11 bots: the ten starters and Keyla, the company example. A player needs nothing else.

**Do not edit the file by hand.** Run `node docs/bots/faces.gen.js > docs/bots/faces.v1.json`. The generator holds the drawing in code, so a change to the look is a change to one script.

- `view_box`: `[-4, -2, 128, 128]`. `ground_y`: 104, the line where bots stand.
- Each bot has `parts` in draw order. A part has a `layer` (`back`, `body` or `front`), `svg`, a `pivot` and an optional `habit`.
- `svg` holds flat shapes: `rect`, `circle`, `ellipse`, `polygon` and `path`. A path may use M L H V Q C and Z, in both cases. A shape may use `fill`, `stroke`, `stroke-width`, `stroke-linecap`, `stroke-linejoin`, `opacity` and `transform="rotate(a cx cy)"`.
- Colours are `{body}` (the bot's colour), `{eye}` (the ink of the face), `{ink}` (the outline, which the theme sets: dark on a light page, light on a dark one), `none`, or a plain hex colour. There are no gradients, no filters and no groups, so any renderer can draw them.
- Each bot has `eyes`: one drawing for each mood, with the mouth. `cheeks` lists the blush shapes. `eye_y` is the height of the eyes.
- `states`: for each mood, `speed` and `amount`. They scale every habit.

## 3. The moods

| Mood | Eyes | Whole-body pose | Speed | Amount |
|---|---|---|---|---|
| Idle | two pills, blink every 2 to 5.5 s | slow breath: up 1.8 px, squash 1% | 1 | 1 |
| Thinking | three dots | sway ±3° at 1.4 rad/s | 0.7 | 0.8 |
| Working | one line that scans ±8 px | tiny shake 0.9 px at 28 rad/s | 3.2 | 1 |
| Done | happy arcs | hop 10 px every 1.1 s, with squash | 2 | 1.2 |
| Needs you | tall pills | hop 7 px every 0.6 s | 2.5 | 1.4 |
| Stuck | crossed eyes | tilt −8°, shake | 0.5 | 0.25 |

The moods map to session states: `Idle` (waiting), `Thinking` (planning), `Working` (running a tool), `Done` (finished, not yet seen), `Needs you` (approval or a question), `Stuck` (failed).

## 4. Habits

A habit moves one part around its pivot. `speed` and `amount` from the mood scale it. Rates are the number inside `sin()` at speed 1, in radians per second.

| Kind | What it does |
|---|---|
| `swing` | Turns back and forth by `amp_deg`. With `every_s` it taps three times, then rests. |
| `spin` | Turns round at `deg_per_s`, only when `speed` is above 1.2. |
| `sweep` | Slides by `dx` and tilts by `rot_deg`. |
| `spring` | Squashes by `squash` and stretches the other way. |
| `flicker` | A flame: the part grows and shrinks in height, out of step with its twin. |
| `bob` | Moves up and down by `dy` and tilts by `rot_deg`. |
| `scuttle` | Legs: a small tilt and lift, at full strength only when busy. |
| `wave` | Turns by `amp_deg` and stretches by `stretch`. |
| `rotor` | Blades: the part gets narrower across and wide again, fast. |
| `pulse` | A reactor: the part grows and shrinks all round. |

| Bot | Habits |
|---|---|
| Bolt | hat bobs; the chain wheels spin; the reactor pulses |
| Pip | lens sweeps; the wheel spins |
| Olive | the spring squashes |
| Skip | both flames flicker; the plane bobs |
| Dot | the legs scuttle; the two antennae swing out of step |
| Nimbus | the dish swings; the hover pads bob |
| Keyla | the hover pads bob; the keyhole reactor pulses |
| Quill | the telescope swings; the lander jet flickers |
| Ink | the casters spin; the feather waves |
| Mimi | the propeller beats as a rotor; the brush swings; the thruster flickers |
| Gus | the cogs spin in turn; the wrench swings |

Every bot also pulses its reactor.

## 5. Motion rules

These make the bots feel alive. They apply to every bot.

1. **Blend, never snap.** Each mood has a weight. The weight moves toward 1 for the current mood by 12% each frame. Pose, eyes and habit are a weighted sum. A change of mood takes about a third of a second.
2. **Squash and stretch.** A hop stretches in the air by 7% and squashes 14% on landing, then settles.
3. **Blink.** Idle and Needs-you eyes close to 8% height for 160 ms. The gap is random, 2 to 5.5 s.
4. **Look.** Eyes move up to 3.2 px sideways and 2.2 px up or down toward the pointer. The body leans up to 1.6°. Stuck bots do not look.
5. **Appear.** A bot grows from nothing with a small overshoot over 0.8 s. Bots in a list appear 0.12 s apart.
6. **Click.** A click makes one reaction: crouch 20% for 0.15 s, jump 18 px, land with a squash. The eyes turn happy for the whole reaction.
7. **Hover.** The bot grows 6% with a soft spring.
8. **Out of step.** Each bot starts at a random phase, so a row of bots does not move together.

## 6. Sizes

- 150 px: the library and the profile.
- 44 px: chat messages.
- 30 px: sidebar rows and Sessions rows.
- 18 px: the status bar. At this size the player shows the pose and the eyes only. It skips the cheeks and the small habits.

Check each new bot at 18 and 30 px before it ships. A bot must still read by its silhouette and its colour.

## 7. What the player must do

- Draw basic shapes with a transform per part (move, rotate and scale around a pivot), at 60 fps.
- Cross-fade the six eye drawings by weight.
- Keep many bots on screen at low cost. A Sessions view may show 30. A bot that is off screen or at rest may drop to 15 fps.
- Respect "reduce motion": show the pose of the mood with no movement.
- Not depend on a web view or CSS. Atelier draws its screens in Rust.

## 8. The prototype

`prototype/crew-storybook.html` is the design reference: the ten bots, light and dark, with a before and after of the clutter cut. It is a still page, not the player. The player is `crates/bot-face`, and the gallery story named Bots shows it.

If the page and this file differ, the page was approved first. Fix this file.

## 9. Open points

1. **Player.** Decided: we write our own small Rust player (`crates/bot-face`). The numbers and the reasons are in `player-spike-v1.md`.
2. **Dark theme.** Decided: the outline is `{ink}`, a colour the theme sets. It is dark on a light page and light on a dark one, so the bots keep their storybook edge. The eyes and the mouth stay `{eye}`, dark on the coloured body.
3. **Making a bot.** The builder will pick from a parts kit: reactors, ways to move and tools (the mockup frame "parts kit"). The data now holds ten complete bots, not parts that mix. Splitting them into a kit is the builder step.
4. **Sound.** None in v1.
