// The ```lgtm:treemap block: every function in the module as a rectangle, sized
// by how many lines it occupies.
//
// This is the one view that answers a question the function table cannot: *is
// anything in here disproportionate?* A module of twelve tidy functions and one
// 90-line monster looks fine as a list and obvious as a treemap.
//
// The sizes live in the markdown, one `sig : lines visibility` row per
// function, written when the doc is seeded — the same shape as Alexandria's
// `label: value flags` treemap syntax. The renderer only draws what the text
// says, so the doc is the data and stays hand-editable. The live outline is
// consulted for one thing only: the line number a tile jumps to.
//
// The block is chrome-free on purpose. Headers and legends were eating the
// space the chart needed, and the chart is the information — every label it
// can't fit is available on hover instead.
//
// Squarified layout via d3-hierarchy, rendered to an SVG string during the
// markdown pass, because md.render is synchronous.

import { hierarchy, treemap, treemapSquarify } from "d3-hierarchy";
import type { ModuleInfo } from "$lib/ipc";

const W = 1000;
const H = 700;
/** How many of the biggest functions get a label and the breathing animation. */
const TOP = 3;

export interface Cell {
  /** As written in the block: `create_user/1` or `search/1..2`. */
  sig: string;
  lines: number;
  visibility: "public" | "private";
  /** Where to jump, filled in from the outline. Undefined = not clickable. */
  line: number | undefined;
  /** Rank by size, 0 = biggest. The top three are the ones worth looking at. */
  rank: number;
}

/**
 * `create_user/1 : 6 public` — one row per function. Everything after the
 * first colon is `<lines> <visibility>`; a missing visibility just means the
 * cell is drawn in the public colour.
 */
export function parseCells(body: string): Cell[] {
  const cells: Cell[] = [];

  for (const raw of body.split("\n")) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const colon = line.indexOf(":");
    if (colon < 1) continue;

    const sig = line.slice(0, colon).trim().replace(/^-\s*/, "");
    const rest = line.slice(colon + 1).trim();
    const lines = parseInt(rest, 10);
    if (!sig || !Number.isFinite(lines) || lines <= 0) continue;

    cells.push({
      sig,
      lines,
      visibility: /\bprivate\b/i.test(rest) ? "private" : "public",
      line: undefined,
      rank: 0,
    });
  }

  // Rank by size so the renderer can single out the top three: those are the
  // ones that decide whether a module is balanced or lopsided.
  [...cells]
    .sort((a, b) => b.lines - a.lines)
    .forEach((c, i) => {
      c.rank = i;
    });
  return cells;
}

/**
 * Attach the line each function starts on, so a tile can jump to it. Cells
 * with no match stay unclickable rather than jumping somewhere wrong.
 */
export function withOutline(cells: Cell[], module: ModuleInfo | null): Cell[] {
  if (!module) return cells;
  const byKey = new Map<string, number>();
  for (const f of module.functions) {
    byKey.set(`${f.name}/${f.arity}`, f.line);
    if (f.minArity < f.arity) byKey.set(`${f.name}/${f.minArity}..${f.arity}`, f.line);
  }
  return cells.map((c) => {
    const bare = c.sig.replace(/~~/g, "");
    const slash = bare.lastIndexOf("/");
    const name = slash === -1 ? bare : bare.slice(0, slash);
    const top = bare.slice(slash + 1).split("..").pop() ?? "";
    return { ...c, line: byKey.get(c.sig) ?? byKey.get(`${name}/${top}`) };
  });
}

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : "&quot;",
  );
}

export function renderTreemap(body: string, module: ModuleInfo | null): string {
  const cells = withOutline(parseCells(body), module);
  if (!cells.length) {
    return `<div class="lgtm-treemap empty">Empty treemap block — re-seed this doc, or write <code>my_fn/1 : 12 public</code> style rows here.</div>`;
  }

  const total = cells.reduce((n, c) => n + c.lines, 0);
  const root = hierarchy<{ children?: Cell[]; value?: number }>({ children: cells } as never)
    .sum((d) => (d as unknown as Cell).lines ?? 0)
    .sort((a, b) => (b.value ?? 0) - (a.value ?? 0));

  const laid = treemap<{ children?: Cell[] }>()
    .tile(treemapSquarify)
    .size([W, H])
    .paddingInner(5)
    .round(true)(root as never);

  const parts: string[] = [];

  for (const leaf of laid.leaves()) {
    const d = leaf.data as unknown as Cell;
    const x = leaf.x0;
    const y = leaf.y0;
    const bw = leaf.x1 - leaf.x0;
    const bh = leaf.y1 - leaf.y0;
    if (bw <= 0 || bh <= 0) continue;

    const cls = [
      "tm-cell",
      d.visibility === "private" ? "tm-private" : "tm-public",
      d.rank < TOP ? "tm-top" : "",
    ]
      .filter(Boolean)
      .join(" ");

    const pct = Math.round((d.lines / total) * 100);

    // Corner radius scales with the cell. A fixed rx turns the smallest
    // rectangles into lozenges — they stop reading as squares at all.
    const r = Math.max(1, Math.min(5, Math.min(bw, bh) * 0.12));

    parts.push(
      // The tooltip is rendered by the doc pane, not by the browser: a native
      // <title> is slow to appear and can't be styled. data-tip is just the
      // function name — the number is already on the tile or unimportant.
      `<g class="tm-tile" data-sig="${esc(d.sig)}"${d.line !== undefined ? ` data-line="${d.line}"` : ""} data-tip="${esc(d.sig)}" data-lines="${d.lines}" data-pct="${pct}" role="button" tabindex="0">`,
      `<rect class="${cls}" x="${x}" y="${y}" width="${bw}" height="${bh}" rx="${r.toFixed(1)}" ry="${r.toFixed(1)}"/>`,
    );

    // Only the top three are labelled. Numbers on every square turned the
    // chart into a wall of digits; the shapes carry the comparison, and the
    // tooltip carries the rest.
    if (d.rank < TOP) {
      const num = String(d.lines);
      const numFs = Math.min(52, bh * 0.34, (bw - 16) / (num.length * 0.62));

      if (numFs >= 10 && bw > 40 && bh > 30) {
        const cx = x + bw / 2;
        const nameFs = Math.min(16, numFs * 0.44);
        const maxChars = Math.floor((bw - 14) / (nameFs * 0.6));
        const roomForName = bh > numFs + nameFs + 16 && maxChars >= 6;

        if (roomForName) {
          const label =
            d.sig.length > maxChars ? d.sig.slice(0, Math.max(1, maxChars - 1)) + "…" : d.sig;
          const cy = y + bh / 2;
          parts.push(
            `<text class="tm-num" x="${cx}" y="${(cy - nameFs * 0.35).toFixed(1)}" text-anchor="middle" font-size="${numFs.toFixed(1)}">${num}</text>`,
            `<text class="tm-name" x="${cx}" y="${(cy + numFs * 0.6).toFixed(1)}" text-anchor="middle" font-size="${nameFs.toFixed(1)}">${esc(label)}</text>`,
          );
        } else {
          parts.push(
            `<text class="tm-num" x="${cx}" y="${(y + bh / 2).toFixed(1)}" text-anchor="middle" dominant-baseline="central" font-size="${numFs.toFixed(1)}">${num}</text>`,
          );
        }
      }
    }

    parts.push(`</g>`);
  }

  return (
    `<div class="lgtm-treemap">` +
    `<svg viewBox="0 0 ${W} ${H}" role="img" aria-label="Function sizes" preserveAspectRatio="xMidYMid meet">${parts.join("")}</svg>` +
    `</div>`
  );
}
