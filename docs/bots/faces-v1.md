# Bot faces and motion, v1 (draft for review)

Status: draft. Docs only. The look was chosen by looking at the live prototype in `prototype/crew-live.html`. This file turns that look into data and rules, so a Rust player can run it.

## 1. The look

- **Flat colour.** Each bot is one colour. Shade and light are the same colour with black or white laid over it at fixed opacity. No gradients, no glow, no outline.
- **Soft body.** Big head, small body, large corner radius. No circles as the body shape.
- **Nothing around the bot.** No ring, badge or halo. State shows on the bot itself, by its eyes and its motion.
- **Eyes are the face.** Two dark pills on the body. Idle, done and thinking also show two faint cheeks.
- **Colour.** The seven brand colours, each mixed 10% toward a warm grey (`#74716A`) so they sit calm on a dark and on a light page. The eye colour is `#141413` on both.
- **A way to move and a tool.** Every bot has one way to move (tracks, wheels, one spring, jets, legs, feet, a diamond base) and one tool. These make the shapes differ, so colour is not the only clue.

## 2. The data

`faces.v1.json` holds the 7 starter bots. A player needs nothing else.

- `view_box`: always `[0, 0, 120, 120]`. `ground_y`: 104, the line where bots stand.
- `tokens`: `{body}`, `{shade}`, `{light}`, `{eye}`. The player replaces them with colours.
- Each bot has `parts` in draw order. A part has a `layer` (`back`, `body` or `front`), `svg` (basic shapes: rect, path, polygon, circle), a `pivot` and an optional `habit`.
- Each bot has `eyes`: one drawing for each mood, centred on `eye_y`.
- `states`: for each mood, `speed` and `amount`. They scale every habit.

Only basic shapes are allowed, so any renderer can draw them. A bot with a new body adds a `parts` list. It does not need new code.

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

| Bot | Part | Habit |
|---|---|---|
| Bolt | hammer arm | swings 30° and taps three times every 3.2 s |
| Pip | lens | slides 5 px and tilts 6° at 1.6 rad/s |
| Pip | wheels | spin 130°/s, only when `speed` is above 1.2 |
| Olive | periscope | turns ±14° at 1.2 rad/s |
| Olive | spring | squashes 7% at 3 rad/s |
| Skip | jets | flames flicker (scale 1 ± 0.35) |
| Skip | parcel | bobs 1.4 px and tilts 2° |
| Dot | legs | scuttle: tilt 2° and lift 1.4 px at 7 rad/s, full strength only when busy |
| Dot | antennae | swing ±10° at 2.2 rad/s, out of step |
| Nimbus | flag | waves ±6° and stretches 5% |
| Keyla | key | swings ±10° at 2.4 rad/s |

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

`prototype/crew-live.html` is the reference. Open it in a browser. It shows every rule above and the three real sizes. If the prototype and this file differ, the prototype was approved first. Fix this file.

## 9. Open points

1. **Player.** Decided: we write our own small Rust player (`crates/bot-face`). The numbers and the reasons are in `player-spike-v1.md`.
2. **Making a bot.** The builder picks a body, a colour and a tool. How many bodies and tools do we offer at the start? The prototype has 7 and 7.
3. **Faces for the four new roles** are part of v1, because all ten roles ship: Quill (Researcher, blue), Ink (Writer, purple), Mimi (Designer, pink) and Gus (Operator, green). Each needs its own way to move and its own tool.
4. **Sound.** None in v1.
