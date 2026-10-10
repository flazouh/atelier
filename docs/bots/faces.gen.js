#!/usr/bin/env node
// Makes docs/bots/faces.v1.json: the bots as plain shapes for the Rust player (crates/bot-face).
// Run: node docs/bots/faces.gen.js > docs/bots/faces.v1.json
// Each bot is a list of parts in draw order. A part is some SVG shapes (rect, ellipse, circle, polygon, path with
// M L H V Q C Z), a pivot, and maybe a habit. Colours are tokens ({body}, {eye}) or plain hex. No gradients and no
// filters: a glow is a few circles that fade. A shape that turns says so with its own transform="rotate(a cx cy)".
'use strict';
const INK = '#141413', GREY = '#3d3d42', STEEL = '#a9a7b0', WHITE = '#ffffff';
const n = (v) => +v.toFixed(2);

// ---- shape helpers (each returns one SVG element as text) ----
const attrs = (o) => Object.entries(o).filter(([, v]) => v !== undefined && v !== null).map(([k, v]) => `${k}="${typeof v === 'number' ? n(v) : v}"`).join(' ');
const el = (tag, o) => `<${tag} ${attrs(o)}/>`;
const outline = (w = 3.2) => ({ stroke: INK, 'stroke-width': w, 'stroke-linejoin': 'round', 'stroke-linecap': 'round' });
const rect = (x, y, w, h, r, fill, sw = 3.2, extra = {}) => el('rect', { x, y, width: w, height: h, rx: r, fill, ...(sw ? outline(sw) : {}), ...extra });
const circle = (cx, cy, r, fill, sw = 3.2, extra = {}) => el('circle', { cx, cy, r, fill, ...(sw ? outline(sw) : {}), ...extra });
const ellipse = (cx, cy, rx, ry, fill, sw = 3.2, extra = {}) => el('ellipse', { cx, cy, rx, ry, fill, ...(sw ? outline(sw) : {}), ...extra });
const path = (d, fill, sw = 3.2, extra = {}) => el('path', { d, fill: fill || 'none', ...(sw ? outline(sw) : {}), ...extra });
const line = (d, stroke, w, extra = {}) => el('path', { d, fill: 'none', stroke, 'stroke-width': w, 'stroke-linecap': 'round', 'stroke-linejoin': 'round', ...extra });
const rot = (a, cx, cy) => ({ transform: `rotate(${a} ${cx} ${cy})` });
const shine = (x, y, rx, ry, a = -25) => ellipse(x, y, rx, ry, WHITE, 0, { opacity: 0.32, ...rot(a, x, y) });
// a bar with a dark outline and a lighter inside, for a leg or an antenna
const bar = (d, w = 7, inner = 3) => line(d, INK, w) + line(d, STEEL, inner);
const glow = (cx, cy, r, c) => circle(cx, cy, r * 1.9, c, 0, { opacity: 0.16 }) + circle(cx, cy, r * 1.4, c, 0, { opacity: 0.3 });
// the reactor: a dark housing, a glowing core. Kinds: core, ring, spark.
function reactor(cx, cy, r, c, kind = 'core') {
  const housing = circle(cx, cy, r + 3, INK, 0) + circle(cx, cy, r + 0.8, GREY, 0);
  if (kind === 'spark') {
    const k = r * 0.78;
    const d = `M${cx} ${cy - k}L${cx + k * .3} ${cy - k * .3}L${cx + k} ${cy}L${cx + k * .3} ${cy + k * .3}L${cx} ${cy + k}L${cx - k * .3} ${cy + k * .3}L${cx - k} ${cy}L${cx - k * .3} ${cy - k * .3}Z`;
    return housing + path(d, WHITE, 0);
  }
  if (kind === 'ring') {
    return housing + circle(cx, cy, r - 1.6, 'none', 3, { stroke: c, opacity: 0.45 }) + circle(cx, cy, r - 1.6, 'none', 0, { stroke: WHITE, 'stroke-width': 1.6 }) + circle(cx, cy, r * 0.3, c, 0);
  }
  return housing + glow(cx, cy, r - 2.6, c) + circle(cx, cy, r - 2.2, c, 0) + circle(cx - .8, cy - .8, Math.max(1, (r - 2.2) * .32), WHITE, 0);
}
const wheel = (cx, cy, r) => circle(cx, cy, r, GREY, 3.2) + circle(cx, cy, r * .5, STEEL, 2) + line(`M${cx} ${cy - r * .5}V${cy + r * .5}M${cx - r * .5} ${cy}H${cx + r * .5}`, INK, 1.4);
const cog = (cx, cy, r) => [0, 45, 90, 135].map((a) => rect(cx - 3.2, cy - r - 3.2, 6.4, 2 * r + 6.4, 1.6, GREY, 2.2, rot(a, cx, cy))).join('') + circle(cx, cy, r, GREY, 2.6) + circle(cx, cy, r * .4, STEEL, 0);
const part = (layer, svg, pivot = [60, 100], habit = null) => ({ layer, svg, pivot, habit });

// ---- eyes and mouth for each mood ----
function eyeSets(cy, dx, mouth, er = 6.4) {
  const L = 60 - dx, R = 60 + dx;
  const ball = (x) => circle(x, cy, er, '{eye}', 0) + circle(x + er * .36, cy - er * .375, er * .34, WHITE, 0) + circle(x - er * .31, cy + er * .34, er * .16, WHITE, 0, { opacity: 0.8 });
  const smile = (depth = 6.5, w = 6) => line(`M${60 - w} ${mouth}Q60 ${mouth + depth} ${60 + w} ${mouth}`, '{eye}', 2.6);
  const arcs = (x) => line(`M${x - 6} ${cy + 3}Q${x} ${cy - 6} ${x + 6} ${cy + 3}`, '{eye}', 3.6);
  const cross = (x) => line(`M${x - 4} ${cy - 4}L${x + 4} ${cy + 4}M${x + 4} ${cy - 4}L${x - 4} ${cy + 4}`, '{eye}', 3);
  return {
    idle: ball(L) + ball(R) + smile(),
    thinking: [L - 2, 60, R + 2].map((x, i) => circle(x, cy, 3.4, '{eye}', 0, { opacity: [1, .6, .3][i] })).join('') + line(`M55 ${mouth}H65`, '{eye}', 2.6),
    working: rect(42, cy - 2, 36, 4, 2, '{eye}', 0),
    done: arcs(L) + arcs(R) + smile(8.5, 7),
    needs: rect(L - 4, cy - 10, 8, 20, 4, '{eye}', 0) + rect(R - 4, cy - 10, 8, 20, 4, '{eye}', 0) + circle(60, mouth + 2, 2.6, '{eye}', 0),
    stuck: cross(L) + cross(R) + line(`M54 ${mouth + 1}Q57 ${mouth - 2} 60 ${mouth + 1}Q63 ${mouth + 4} 66 ${mouth + 1}`, '{eye}', 2.4),
  };
}
const cheeks = (a, b, cy) => [a, b].map((x) => [x - 6.5, cy - 4.4, 13, 8.8]);

// ---- habits (the player reads them; see docs/bots/faces-v1.md) ----
const H = {
  swing: (amp_deg, hz, extra = {}) => ({ kind: 'swing', amp_deg, hz, ...extra }),
  spin: (deg_per_s) => ({ kind: 'spin', deg_per_s }),
  sweep: (dx, rot_deg, hz) => ({ kind: 'sweep', dx, rot_deg, hz }),
  spring: (squash, hz) => ({ kind: 'spring', squash, hz }),
  flicker: (amp) => ({ kind: 'flicker', amp }),
  bob: (dy, rot_deg, hz) => ({ kind: 'bob', dy, rot_deg, hz }),
  scuttle: (rot_deg, dy, hz) => ({ kind: 'scuttle', rot_deg, dy, hz }),
  wave: (amp_deg, stretch, hz) => ({ kind: 'wave', amp_deg, stretch, hz }),
  rotor: (amp, hz) => ({ kind: 'rotor', amp, hz }),
  pulse: (amp, hz) => ({ kind: 'pulse', amp, hz }),
};

// ---- the bots ----
const BODY = '{body}';
const bots = [];
const add = (id, name, role, colour, eye, parts) => bots.push({ id, name, role, colour, eye_y: eye.cy, cheeks: eye.cheeks, parts, eyes: eyeSets(eye.cy, eye.dx, eye.mouth, eye.er) });

// Bolt, Builder: a tank chain and a hard hat
add('bolt', 'Bolt', 'Builder', '#6CCBFA', { cy: 60, dx: 12, mouth: 68, cheeks: cheeks(40, 80, 70) }, [
  part('back', rect(18, 88, 84, 20, 10, GREY)),
  part('back', line([...Array(8)].map((_, i) => `M${n(29 + i * 9)} 91v4M${n(29 + i * 9)} 101v4`).join(''), '#8d8b95', 2)),
  ...[28, 60, 92].map((x) => part('back', circle(x, 98, x === 60 ? 5 : 5.5, STEEL, 2) + circle(x, 98, 1.8, INK, 0), [x, 98], H.spin(150))),
  part('body', ellipse(60, 62, 37, 33, BODY) + shine(44, 44, 11, 6)),
  part('body', reactor(60, 86, 6.5, '#7DF3FF'), [60, 86], H.pulse(0.1, 2.2)),
  part('front', rect(31, 40, 58, 9, 4.5, '#F5C842') + path('M35 41Q35 21 60 21Q85 21 85 41Z', '#FBD73C'), [60, 42], H.bob(1.2, 2, 1.6)),
]);

// Pip, Prover: one wheel and a scanner lens
add('pip', 'Pip', 'Prover', '#66D9A0', { cy: 64, dx: 10, mouth: 72, cheeks: cheeks(41, 79, 74) }, [
  part('back', wheel(60, 98, 13), [60, 98], H.spin(130)),
  part('body', path('M60 12C70 34 94 46 94 70C94 90 79 100 60 100C41 100 26 90 26 70C26 46 50 34 60 12Z', BODY) + shine(46, 58, 8, 15, 20)),
  part('body', reactor(60, 88, 5.6, '#9DFFF0'), [60, 88], H.pulse(0.1, 2.2)),
  part('front', bar('M95 104L108 117', 7, 3) + circle(88, 94, 14, '#CFF7FF') + circle(88, 94, 8.5, '#9DFFF0', 0, { opacity: 0.55 }) + line('M80 90Q84 84 91 85', WHITE, 2.2), [95, 100], H.sweep(3, 8, 1.6)),
]);

// Olive, Reviewer: a spring and goggles
add('olive', 'Olive', 'Reviewer', '#8F5CF2', { cy: 45, dx: 11, mouth: 66, er: 5.2, cheeks: cheeks(42, 78, 64) }, [
  part('back', bar('M60 92L68 94.5L52 99.5L68 104.5L52 109.5L60 112', 6.5, 2.6) + rect(50, 111, 20, 6, 3, GREY, 2.4), [60, 92], H.spring(0.08, 3)),
  part('body', rect(34, 14, 52, 82, 26, BODY) + shine(48, 34, 8, 14, 10)),
  part('body', [49, 71].map((x) => circle(x, 44, 12.5, '#EDE8FF') + circle(x, 44, 10, 'none', 0, { stroke: '#4ACFFF', 'stroke-width': 1.6, opacity: 0.9 })).join('') + line('M61.5 44h-3', INK, 3)),
  part('body', reactor(60, 82, 6.5, '#4ACFFF', 'ring'), [60, 82], H.pulse(0.1, 2.2)),
]);

// Skip, Shipper: jets and a paper plane
const jet = (x) => [
  part('back', rect(x - 7, 96, 14, 9, 3, GREY, 2.6)),
  part('back', ellipse(x, 112, 5.5, 9, '#FFB347', 0, { opacity: 0.45 }) + path(`M${x - 4.5} 105Q${x} 126 ${x + 4.5} 105Z`, '#FFE08A', 1.6), [x, 105], H.flicker(0.35)),
];
add('skip', 'Skip', 'Shipper', '#F29A4B', { cy: 62, dx: 12, mouth: 70, cheeks: cheeks(38, 82, 72) }, [
  part('back', path('M28 74L10 90L28 94Z', '#E4672E')),
  part('back', path('M92 74L110 90L92 94Z', '#E4672E')),
  ...jet(42), ...jet(78),
  part('body', circle(60, 66, 35, BODY) + shine(44, 48, 10, 6)),
  part('body', reactor(60, 87, 6.5, '#FFE08A', 'spark'), [60, 87], H.pulse(0.1, 2.2)),
  part('front', path('M60 14L84 8L74 30L66 26Z', '#F6F4EF') + line('M66 26L84 8', INK, 1.8), [68, 28], H.bob(1.4, 3, 2)),
]);

// Dot, Debugger: six legs and two antennae
const leg = (a, b, c, m) => { const p = [a, b, c].map(([x, y]) => [m ? 120 - x : x, y]); return bar(`M${p[0]}L${p[1]}L${p[2]}`, 7, 3) + circle(p[1][0], p[1][1], 3.2, GREY, 1.8); };
const legSet = [[[34, 82], [18, 92], [16, 106]], [[42, 86], [32, 98], [30, 110]], [[52, 88], [46, 100], [46, 112]]];
add('dot', 'Dot', 'Debugger', '#EE82EE', { cy: 62, dx: 11, mouth: 70, cheeks: cheeks(40, 80, 72) }, [
  part('back', legSet.map((p) => leg(...p, false)).join('') + legSet.map((p) => leg(...p, true)).join(''), [60, 86], H.scuttle(2, 1.4, 7)),
  part('back', bar('M46 38L38 18', 5.5, 2.2) + circle(38, 18, 4.6, '#FBD73C', 2) + circle(37, 17, 1.2, WHITE, 0), [46, 38], H.swing(10, 2.2)),
  part('back', bar('M74 38L82 18', 5.5, 2.2) + circle(82, 18, 4.6, '#7DF3FF', 2) + circle(81, 17, 1.2, WHITE, 0), [74, 38], H.swing(10, 2.2, { phase: 1.6 })),
  part('body', path('M26 88V62C26 43.2 41.2 28 60 28C78.8 28 94 43.2 94 62V88Z', BODY) + shine(44, 44, 10, 6)),
  part('body', reactor(60, 81, 6.5, '#FBD73C'), [60, 81], H.pulse(0.1, 2.2)),
]);

// Nimbus, Planner: a cloud on hover pads with a dish
const cloudShapes = (fill, sw) => [circle(38, 72, 19, fill, sw), circle(82, 72, 19, fill, sw), circle(60, 58, 25, fill, sw), rect(26, 70, 68, 26, 13, fill, sw)];
const pad = (x, y, glowC) => ellipse(x, y + 5, 10, 3.6, glowC, 0, { opacity: 0.5 }) + rect(x - 9, y - 5, 18, 8, 4, GREY, 2.6);
add('nimbus', 'Nimbus', 'Planner', '#F6D95C', { cy: 62, dx: 10, mouth: 70, cheeks: cheeks(40, 80, 72) }, [
  part('back', pad(43, 103, '#7DF3FF') + pad(77, 103, '#7DF3FF'), [60, 103], H.bob(1.2, 0, 1.4)),
  part('body', cloudShapes(INK, 6.4).join('') + cloudShapes(BODY, 0).join('') + shine(46, 48, 10, 6)),
  part('body', reactor(60, 86, 5.8, '#7DF3FF'), [60, 86], H.pulse(0.1, 2.2)),
  part('front', rect(58, 26, 4, 12, 0, STEEL, 1.6) + path('M46 24Q60 6 74 24Z', STEEL, 2.4) + circle(60, 19, 2.2, '#7DF3FF', 0), [60, 38], H.swing(8, 1.4)),
]);

// Keyla, SSO Auditor (a company example): an armoured shield on hover pads, with a keyhole reactor
add('keyla', 'Keyla', 'SSO Auditor', '#EF6B64', { cy: 48, dx: 12, mouth: 56, cheeks: cheeks(40, 80, 58) }, [
  part('back', pad(43, 110, '#7DF3FF') + pad(77, 110, '#7DF3FF'), [60, 110], H.bob(1.2, 0, 1.4)),
  part('body', path('M30 22H90V60Q90 88 60 102Q30 88 30 60Z', BODY) + shine(44, 38, 8, 14, 10)),
  part('body', circle(60, 82, 11, INK, 0) + circle(60, 82, 9.4, GREY, 0) + glow(60, 82, 6, '#FBD73C') + circle(60, 82, 7.4, '#FBD73C', 0) + circle(60, 80, 2.6, INK, 0) + path('M58.4 81.5h3.2l1 5h-5.2z', INK, 0), [60, 82], H.pulse(0.08, 2.2)),
]);

// Quill, Researcher: lunar-lander legs and a telescope
const strut = (x1, y1, x2, y2) => bar(`M${x1} ${y1}L${x2} ${y2}`, 8, 3.4) + rect(x2 - 8, y2 - 1, 16, 6, 3, GREY, 2.4);
add('quill', 'Quill', 'Researcher', '#58B4F4', { cy: 56, dx: 12, mouth: 64, cheeks: cheeks(39, 81, 66) }, [
  part('back', strut(40, 84, 24, 106) + strut(80, 84, 96, 106)),
  part('back', ellipse(60, 103, 6, 4, '#7DF3FF', 0, { opacity: 0.55 }) + rect(53, 88, 14, 11, 4, GREY, 2.6), [60, 100], H.flicker(0.25)),
  part('body', rect(32, 34, 56, 56, 24, BODY) + shine(46, 46, 9, 6)),
  part('body', reactor(60, 80, 6.6, '#7DF3FF', 'ring'), [60, 80], H.pulse(0.1, 2.2)),
  part('front', rect(22, 12, 34, 15, 5, GREY, 3.2, rot(-24, 50, 34)) + rect(14, 9, 12, 21, 4, '#CFF7FF', 3.2, rot(-24, 50, 34)) + rect(16, 13, 3, 13, 1.5, WHITE, 0, { opacity: 0.9, ...rot(-24, 50, 34) }), [50, 34], H.swing(10, 1.1)),
]);

// Ink, Writer: two casters and a feather
add('ink', 'Ink', 'Writer', '#A074F5', { cy: 56, dx: 12, mouth: 64, cheeks: cheeks(39, 81, 66) }, [
  part('back', wheel(40, 102, 9), [40, 102], H.spin(130)),
  part('back', wheel(80, 102, 9), [80, 102], H.spin(130)),
  part('body', rect(30, 30, 60, 68, 26, BODY) + shine(46, 46, 9, 6)),
  part('body', reactor(60, 82, 6.8, '#FFE08A', 'spark'), [60, 82], H.pulse(0.1, 2.2)),
  part('front', path('M60 30Q44 20 52 6Q64 8 66 18Q68 26 60 30Z', '#F6F4EF') + line('M60 30Q56 20 54 9', INK, 1.6) + line('M58 22Q62 18 64 14', INK, 1.2, { opacity: 0.6 }), [60, 30], H.wave(7, 0.03, 1.8)),
]);

// Mimi, Designer: a propeller and a brush, on a small thruster
add('mimi', 'Mimi', 'Designer', '#F27BEF', { cy: 62, dx: 11, mouth: 70, cheeks: cheeks(40, 80, 72) }, [
  part('back', ellipse(60, 104, 14, 4.5, '#7DF3FF', 0, { opacity: 0.5 }) + rect(50, 97, 20, 8, 3, GREY, 2.6), [60, 104], H.flicker(0.2)),
  part('back', rect(58, 26, 4, 12, 0, STEEL, 1.6)),
  part('body', circle(60, 66, 33, BODY) + shine(44, 50, 10, 6)),
  part('body', reactor(60, 86, 5.8, '#FFE08A'), [60, 86], H.pulse(0.1, 2.2)),
  part('front', ellipse(60, 24, 38, 4.5, WHITE, 0, { opacity: 0.28 }) + ellipse(60, 24, 27, 4.2, STEEL, 2.4) + circle(60, 24, 3.6, GREY, 1.8), [60, 24], H.rotor(0.7, 7)),
  part('front', rect(82, 64, 14, 9, 4.5, BODY)),
  part('front', rect(96, 40, 7, 28, 3.5, '#FBD73C', 3.2, rot(32, 100, 66)) + rect(95, 36, 9, 6, 0, STEEL, 1.8, rot(32, 100, 66)) + path('M95 36Q99.5 24 104 36Z', '#7DF3FF', 2, rot(32, 100, 66)) + circle(99.5, 26, 5, '#7DF3FF', 0, { opacity: 0.5, ...rot(32, 100, 66) }), [97, 68], H.swing(10, 2.2)),
]);

// Gus, Operator: two cogs and a wrench
add('gus', 'Gus', 'Operator', '#3FC38B', { cy: 60, dx: 12, mouth: 68, cheeks: cheeks(39, 81, 70) }, [
  part('back', cog(34, 98, 11), [34, 98], H.spin(80)),
  part('back', cog(86, 98, 11), [86, 98], H.spin(-80)),
  part('back', rect(57, 20, 6, 24, 2, STEEL, 2.4)),
  part('body', rect(24, 40, 72, 56, 18, BODY) + shine(40, 52, 10, 6)),
  part('body', reactor(60, 84, 6.8, '#9DFFB0'), [60, 84], H.pulse(0.1, 2.2)),
  part('front', line('M51 22V14Q51 6 60 6Q69 6 69 14V22', INK, 10) + line('M51 22V14Q51 6 60 6Q69 6 69 14V22', STEEL, 5), [60, 44], H.swing(6, 1.2)),
]);

const doc = {
  version: 1,
  view_box: [-4, -2, 128, 128],
  ground_y: 104,
  grey: '#74716A',
  colour_mix_with_grey: 0,
  tokens: { body: 'the bot colour', eye: 'the ink colour of the eyes and the mouth' },
  states: {
    idle: { speed: 1, amount: 1 },
    thinking: { speed: 0.7, amount: 0.8 },
    working: { speed: 3.2, amount: 1 },
    done: { speed: 2, amount: 1.2 },
    needs: { speed: 2.5, amount: 1.4 },
    stuck: { speed: 0.5, amount: 0.25 },
  },
  bots,
};
process.stdout.write(JSON.stringify(doc, null, 1) + '\n');
