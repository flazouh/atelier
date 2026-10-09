# Bot face player: spike result (v1)

Status: result of ENG-402. It answers open point 1 of `faces-v1.md`: write our own player, or use a tool such as Rive.

**Answer: write our own small player.** It is built. The crate is `crates/bot-face`. The gallery page is the story named Bots.

## What was built

- `atelier-bot-face`: reads `faces.v1.json`, parses the SVG shapes, runs the moods and habits, and draws with GPUI paths. 23 tests pass.
- The "Bots" story in the gallery: many bots at once, six mood buttons, a click reaction, eyes that follow the pointer, and a line that shows the cost of a frame.
- A benchmark with no window: `cargo test --release -p atelier-bot-face frame_cost -- --ignored --nocapture`.

The first run looked like the prototype. Same shapes, same colours, same habits.

## What it costs

Measured on the HP: Intel i5-10500T, 2.3 GHz, release build, CPU only, 400 frames a run. This chip is slower than a recent Mac, so the numbers are on the safe side.

| Bots | Shapes a frame | Trace and tessellate (avg) | p99 |
|---|---|---|---|
| 7 | 68 | 0.7 ms | 1.6 ms |
| 30 (idle) | 295 | 3.1 ms | 6.7 ms |
| 30 (working) | 265 | 2.9 ms | 7.6 ms |
| 30 (done) | 265 | 3.0 ms | 6.9 ms |
| 60 (working) | 526 | 5.3 ms | 9.2 ms |
| 120 (working) | 1047 | 10.3 ms | 14.4 ms |

- Moving the bots (moods, habits, blends) costs 0.004 to 0.05 ms. It is not a problem.
- Turning shapes into triangles costs about 10 µs a shape, and it grows in a straight line with the shape count. This is nearly all of the cost.
- A frame at 60 fps has 16.7 ms. 30 bots use about 18% of it on average. At 120 fps (8.3 ms) the p99 of 30 bots is close to the whole frame.

## What was not measured

- **The GPU side.** The HP has no GPU. The gallery ran on a software Vulkan driver at 5 to 12 fps, which says nothing about a Mac. The benchmark has no window, so it does not include `paint_path` or the draw itself.
- **A Mac.** Nothing was timed on a Mac. A run there is the next number to take.
- **Motion that you see.** The motion was written from the prototype and checked by tests (hop height, blink, reaction, blend). Nobody watched it in the app yet.

## Why not Rive

- Speed is not the reason to switch. Our own player is fast enough, and the cost is in tessellation, which Rive would also pay on GPUI.
- The data is plain shapes in a JSON file that we read in the repo. A Rive file is a binary made in a separate editor. Bots made in the builder (body, colour, tool) are data, which fits our format and not a Rive file.
- It would add a dependency for a feature that is about 800 lines of Rust.

Rive would still help if an artist makes complex hand-drawn animation. The faces are not that.

## Ways to cut the cost (ideas, none built or measured)

1. **Slower updates for small bots.** A bot at 18 or 30 px can update at 20 to 30 fps. This should cut the cost of a long list by half or more.
2. **Skip bots that are off screen,** and bots at rest in a scrolled list.
3. **Fewer shapes at small sizes.** At 18 px, skip the cheeks and the small tools.
4. **Reuse a tessellated path** when a part has not moved. GPUI's `Path` cannot be moved after it is built, so this only works for a part that stays still on screen.

## What the player needs from the app

- One `FaceSet` per app, loaded once from the data.
- One `BotRuntime` for each bot on screen. Call `tick` each frame with the mood and the pointer, then `paint_bot`.
- Ask for a new frame (`request_animation_frame`) only while a bot on screen is moving.
- "Reduce motion": built. `BotRuntime::still` returns the pose of the mood with no movement: the mood shows at once, no blink, no grow-in, no click reaction. The Bots story uses it when the system asks for reduced motion, and it stops asking for new frames.

## Found on the way

`faces.v1.json` had two parts (Olive's spring and Dot's legs) that wrote `stroke-width` twice in one element. A browser keeps the first value and hides the error. A strict parser refuses it. The data is fixed. The value is the first one, which is what the prototype showed.

## Next steps

1. A rule for small sizes (ideas 1 to 3 above), with a test for each.
2. One timing run on a Mac, to put a real GPU number beside this one.
3. The four new faces: Quill, Ink, Mimi and Gus.
4. The bot library view, then Sessions with the bots in it.
