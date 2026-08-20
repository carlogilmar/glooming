// The ```lgtm:deps block: what this module reaches outside itself.
//
// Ported from `mockup/deps.html`, which is the visual contract — if this drifts
// from that file, that file is right.
//
// The concept is a boundary, not a graph. A node-link diagram of one file's
// dependencies is always a star: one hub, N leaves, identical topology every
// time, so the picture carries no information. Instead the module is drawn as a
// closed shape with its functions inside in source order, and the only lines
// drawn are the ones that pierce the boundary. A self-contained module reads as
// calm and closed; an entangled one reads as a sea urchin. Functions that reach
// nothing stay silent, and that silence is information too.
//
// Layout is deterministic — outside anchors are ordered by the barycentre of
// their callers, the classic crossing-reduction heuristic — so the same doc
// always renders the same picture. No force simulation, no jitter.

import type { ModuleInfo } from "$lib/ipc";
import { shortModule } from "$lib/fileset";
import { displaySig } from "$lib/select";

export type DepKind = "app" | "lib" | "std";

export interface RemoteFn {
  name: string;
  callers: string[];
  y?: number;
}

export interface Dep {
  module: string;
  kind: DepKind;
  functions: RemoteFn[];
  bary?: number;
  y?: number;
}

const KIND_LABEL: Record<DepKind, string> = {
  app: "your app",
  lib: "library",
  std: "stdlib",
};

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : "&quot;",
  );
}

/**
 * Two levels, by indent:
 *
 *     MyApp.Repo : app
 *       insert/1 : create_user/1
 *       get/2    : get_user/1, get_user!/1
 */
export function parseDeps(body: string): Dep[] {
  const deps: Dep[] = [];

  for (const raw of body.split("\n")) {
    if (!raw.trim() || raw.trim().startsWith("#")) continue;
    const colon = raw.indexOf(":");
    if (colon < 1) continue;

    const label = raw.slice(0, colon).trim();
    const value = raw.slice(colon + 1).trim();
    if (!label) continue;

    // Indent decides the level: four spaces or more is a remote function.
    const indent = raw.length - raw.trimStart().length;
    if (indent >= 4 && deps.length) {
      deps[deps.length - 1].functions.push({
        name: label,
        callers: value ? value.split(",").map((c) => c.trim()).filter(Boolean) : [],
      });
    } else {
      const kind = (["app", "lib", "std"] as const).find((k) => value.startsWith(k)) ?? "lib";
      deps.push({ module: label, kind, functions: [] });
    }
  }

  return deps.filter((d) => d.functions.length);
}

const W = 1000;
const BOX = { x: 40, y: 44, w: 360 };
const OUT_X = 620;
const DOT_X = OUT_X - 18;
/** Vertical pitch of one local function row. */
const ROW = 62;

export function renderDeps(body: string, module: ModuleInfo | null): string {
  const deps = parseDeps(body);
  if (!deps.length) {
    return `<div class="lgtm-deps empty">Empty reach block — re-seed this doc, or write <code>MyApp.Repo : app</code> rows here.</div>`;
  }

  // The left column is the module's own functions in source order. It comes
  // from the outline rather than the block: the block records the edges, and
  // listing every local function a fourth time would be noise.
  const locals = (module?.functions ?? [])
    .slice()
    .sort((a, b) => a.line - b.line)
    .map((f) => ({ sig: displaySig(f), line: f.line, y: 0, pure: true }));

  if (!locals.length) {
    return `<div class="lgtm-deps empty">No outline for this file, so there is nothing to draw the reach against.</div>`;
  }

  const index = new Map(locals.map((f, i) => [f.sig, i]));

  // Order the outside by the average position of its callers, so lines stay
  // roughly parallel instead of crossing.
  for (const d of deps) {
    const idx = d.functions.flatMap((fn) => fn.callers.map((c) => index.get(c) ?? 0));
    d.bary = idx.length ? idx.reduce((a, b) => a + b, 0) / idx.length : Number.MAX_SAFE_INTEGER;
  }
  deps.sort((a, b) => (a.bary ?? 0) - (b.bary ?? 0));

  // **Both** columns decide the height. Sizing from the local functions alone
  // meant a small file with many aliases produced a stack taller than the
  // viewBox, and centring it then pushed the top and the bottom outside — where
  // SVG simply clips them, with nothing to say it had happened.
  const localsH = ROW * Math.max(locals.length - 1, 1) + 68;
  const stackH = deps.reduce((h, d) => h + 22 + d.functions.length * 19 + 16, 0) - 16;
  const H = Math.max(320, 88 + Math.max(localsH, stackH));
  const boxH = H - 88;

  // Each column keeps its natural spacing and is centred in whatever height the
  // other one forced — so a short list never gets stretched thin across a tall
  // diagram just because the opposite column is long.
  const localsTop = BOX.y + (boxH - (localsH - 68)) / 2;
  locals.forEach((f, i) => {
    f.y = localsTop + i * ROW;
  });

  let y = BOX.y + (boxH - stackH) / 2 + 8;
  for (const d of deps) {
    d.y = y;
    y += 22;
    for (const fn of d.functions) {
      fn.y = y;
      y += 19;
    }
    y += 16;
  }

  const edges: { from: string; to: string; kind: DepKind; y1: number; y2: number }[] = [];
  for (const d of deps) {
    for (const fn of d.functions) {
      for (const c of fn.callers) {
        const local = locals[index.get(c) ?? -1];
        if (!local) continue;
        local.pure = false;
        edges.push({ from: c, to: `${d.module}.${fn.name}`, kind: d.kind, y1: local.y, y2: fn.y ?? 0 });
      }
    }
  }

  const parts: string[] = [];

  parts.push(
    `<rect class="bound" x="${BOX.x}" y="${BOX.y}" width="${BOX.w}" height="${boxH}" rx="10"/>`,
    // The module's own name. A nested one is mostly the path to the file, which
    // the diagram has no room for and which says nothing you don't know.
    `<text class="bound-label" x="${BOX.x + 14}" y="${BOX.y - 10}">` +
      `<title>${esc(module?.name ?? "")}</title>${esc(shortModule(module?.name ?? ""))}</text>`,
  );

  // Edges first, so labels sit on top of them.
  for (const e of edges) {
    const x1 = BOX.x + BOX.w;
    const c = (DOT_X - x1) * 0.55;
    parts.push(
      `<path class="edge ${e.kind}" data-from="${esc(e.from)}" data-to="${esc(e.to)}" stroke-width="2"` +
        ` d="M${x1},${e.y1} C${x1 + c},${e.y1} ${DOT_X - c},${e.y2} ${DOT_X},${e.y2}"/>`,
    );
  }

  for (const f of locals) {
    parts.push(
      `<g class="fn${f.pure ? " pure" : ""}" data-fn="${esc(f.sig)}" data-sig="${esc(f.sig)}" data-line="${f.line}" role="button" tabindex="0">`,
      `<rect class="fn-hit" x="${BOX.x + 6}" y="${f.y - 13}" width="${BOX.w - 12}" height="26" rx="5"/>`,
      `<text class="fn-name" x="${BOX.x + 18}" y="${f.y + 4}">${esc(f.sig)}</text>`,
      `<text class="fn-line" x="${BOX.x + BOX.w - 18}" y="${f.y + 4}" text-anchor="end">${f.line}</text>`,
      `</g>`,
    );
    // The puncture, only where something actually leaves.
    if (!f.pure) {
      parts.push(`<circle class="pierce" cx="${BOX.x + BOX.w}" cy="${f.y}" r="3.5"/>`);
    }
  }

  for (const d of deps) {
    // Shortened for the same reason as the boundary — and the kind label is
    // offset from the *drawn* width, not the full name's, or it lands in the
    // middle of nowhere.
    const shown = shortModule(d.module);
    parts.push(
      `<text class="mod-name ${d.kind}" x="${OUT_X}" y="${(d.y ?? 0) + 4}">` +
        `<title>${esc(d.module)}</title>${esc(shown)}</text>`,
      `<text class="mod-kind" x="${OUT_X + shown.length * 7.3 + 12}" y="${(d.y ?? 0) + 4}">${KIND_LABEL[d.kind]}</text>`,
    );
    for (const fn of d.functions) {
      parts.push(
        `<g class="rfn" data-to="${esc(`${d.module}.${fn.name}`)}" data-callers="${esc(fn.callers.join("|"))}" role="button" tabindex="0">`,
        `<rect class="rfn-hit" x="${DOT_X - 8}" y="${(fn.y ?? 0) - 11}" width="330" height="22" rx="5"/>`,
        `<circle class="dot ${d.kind}" cx="${DOT_X}" cy="${fn.y ?? 0}" r="3"/>`,
        `<text class="rfn-name" x="${DOT_X + 14}" y="${(fn.y ?? 0) + 4}">${esc(fn.name)}</text>`,
        `</g>`,
      );
    }
  }

  const reaching = new Set(edges.map((e) => e.from)).size;
  const rest =
    `${reaching} of ${locals.length} ${locals.length === 1 ? "function reaches" : "functions reach"} outside · ` +
    `${deps.length} ${deps.length === 1 ? "module" : "modules"} · ` +
    `${edges.length} ${edges.length === 1 ? "call site" : "call sites"}`;

  return (
    `<div class="lgtm-deps" data-rest="${esc(rest)}">` +
    `<svg viewBox="0 0 ${W} ${H}" preserveAspectRatio="xMidYMid meet" role="img"` +
    ` aria-label="What ${esc(module?.name ?? "this module")} reaches outside itself">${parts.join("")}</svg>` +
    `<div class="readout"><span class="muted">${esc(rest)}</span></div>` +
    `</div>`
  );
}
