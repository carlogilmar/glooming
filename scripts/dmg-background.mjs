#!/usr/bin/env node
// The DMG's backdrop, drawn in code rather than in an editor.
//
// `pnpm tauri build` opens a Finder window with the app on the left and an alias
// to /Applications on the right, and that window has a background picture. Ours
// is generated here because the alternative is a binary nobody can diff: this
// script is 120 lines of arithmetic, and changing the palette is changing a hex
// string rather than reopening a design file.
//
// No dependencies — macOS ships neither ImageMagick nor PIL — so it writes the
// PNG itself: raw RGBA rows, `zlib.deflateSync`, and the four chunks a viewer
// needs. Node's zlib is doing the only hard part.
//
//     node scripts/dmg-background.mjs        → src-tauri/dmg-background.png
//
// Geometry matches `bundle.macOS.dmg` in tauri.conf.json: a 620×420 window, the
// app at x=165 and Applications at x=455, both at y=210. If those move, move the
// marks here, or the arrow will point at nothing.

import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";

const W = 620;
const H = 420;

/** The gloom palette, as the app defines it. */
const INK = [0x12, 0x36, 0x34];       // --gloom-bg
const INK_DEEP = [0x0b, 0x24, 0x22];  // a shade under it, for the floor
const TEAL = [0x5f, 0xd6, 0xc4];      // --gloom
const PAPER = [0xf6, 0xfa, 0xf9];     // --gloom-ink

const px = new Float64Array(W * H * 3);

const clamp = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);
const lerp = (a, b, t) => a + (b - a) * t;

/** Everything is painted with alpha onto what is already there. */
function paint(x, y, [r, g, b], a = 1) {
  if (a <= 0 || x < 0 || y < 0 || x >= W || y >= H) return;
  const i = (y * W + x) * 3;
  px[i] = lerp(px[i], r, a);
  px[i + 1] = lerp(px[i + 1], g, a);
  px[i + 2] = lerp(px[i + 2], b, a);
}

// ---- the ground: a vertical wash, ink at the top, deeper at the floor -------
for (let y = 0; y < H; y++) {
  const t = y / (H - 1);
  const c = [0, 1, 2].map((k) => lerp(INK[k], INK_DEEP[k], t * t));
  for (let x = 0; x < W; x++) paint(x, y, c, 1);
}

// ---- strata: the app's own picture, faint, behind everything ---------------
// Thickness ∝ √lines, exactly as `shape.ts` computes it — the backdrop is the
// same drawing the app makes, at 8% ink.
const LINES = [186, 342, 97, 34, 203, 118];
const weight = (l) => Math.sqrt(l);
const total = LINES.reduce((n, l) => n + weight(l), 0);
const GAP = 7;
const bandsTop = 96;
const bandsH = H - bandsTop - 74;
let y0 = bandsTop;
for (const l of LINES) {
  const h = (weight(l) / total) * (bandsH - GAP * (LINES.length - 1));
  for (let y = Math.round(y0); y < Math.round(y0 + h); y++) {
    for (let x = 54; x < W - 54; x++) {
      // Fade the bands out towards the middle, so the two icons sit on quiet
      // ground and the strata read as texture rather than as content.
      const mid = 1 - Math.abs(x - W / 2) / (W / 2);
      paint(x, y, TEAL, 0.07 * (1 - mid * 0.75));
    }
  }
  y0 += h + GAP;
}

// ---- the arrow: from the app to the folder, at icon height -----------------
const Y = 210;
const FROM = 165 + 48;   // clear of the app icon
const TO = 455 - 48;     // clear of the Applications alias
for (let x = FROM; x <= TO; x++) {
  // Dashed, and fading in from both ends so it has no hard start or stop.
  const t = (x - FROM) / (TO - FROM);
  const edge = Math.min(t, 1 - t) * 4;
  const on = Math.floor((x - FROM) / 7) % 2 === 0;
  if (!on) continue;
  for (let d = -1; d <= 1; d++) paint(x, Y + d, TEAL, 0.55 * clamp(edge) * (d === 0 ? 1 : 0.5));
}
// the head
for (let k = 0; k < 14; k++) {
  const x = TO - k;
  const spread = Math.round(k * 0.62);
  for (let d = -spread; d <= spread; d++) {
    const a = 0.62 * (1 - Math.abs(d) / (spread + 1));
    paint(x, Y + d, TEAL, a);
  }
}

// ---- two hairline plinths, so the icons sit on something -------------------
for (const cx of [165, 455]) {
  for (let x = cx - 62; x <= cx + 62; x++) {
    const t = 1 - Math.abs(x - cx) / 62;
    paint(x, Y + 74, PAPER, 0.16 * t);
    paint(x, Y + 75, PAPER, 0.08 * t);
  }
}

// ---- write the PNG ---------------------------------------------------------
function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body) >>> 0);
  return Buffer.concat([len, body, crc]);
}

let TABLE = null;
function crc32(buf) {
  if (!TABLE) {
    TABLE = new Int32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      TABLE[n] = c;
    }
  }
  let c = -1;
  for (const b of buf) c = TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return c ^ -1;
}

const raw = Buffer.alloc(H * (W * 3 + 1));
for (let y = 0; y < H; y++) {
  raw[y * (W * 3 + 1)] = 0; // filter: none. The image is smooth; paeth buys little.
  for (let x = 0; x < W; x++) {
    const i = (y * W + x) * 3;
    const o = y * (W * 3 + 1) + 1 + x * 3;
    raw[o] = Math.round(clamp(px[i] / 255) * 255);
    raw[o + 1] = Math.round(clamp(px[i + 1] / 255) * 255);
    raw[o + 2] = Math.round(clamp(px[i + 2] / 255) * 255);
  }
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(W, 0);
ihdr.writeUInt32BE(H, 4);
ihdr[8] = 8;  // bit depth
ihdr[9] = 2;  // colour type: truecolour
const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw, { level: 9 })),
  chunk("IEND", Buffer.alloc(0)),
]);

const out = new URL("../src-tauri/dmg-background.png", import.meta.url);
writeFileSync(out, png);
console.log(`wrote ${out.pathname} — ${W}×${H}, ${(png.length / 1024).toFixed(1)}KB`);
