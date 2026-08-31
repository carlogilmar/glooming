// The shape of a change: a gloom's files as layers, thickness ∝ size.
//
// `mockup/stack.html` is the contract. The claim the picture makes is exactly
// one — **thickness is size** — which is why nothing else got in: no threads out
// to the modules it reaches (a portrait is not a dependency list), no reading
// path, no line-count captions, no per-setting marks.
//
// Layout only. Pure functions over the outline and the source, so the arithmetic
// can be probed headlessly (`esbuild` + node) rather than clicked at: the two
// things that went wrong while drawing it were both sums — marks running past
// the plate they belong to, and a floor that ate the proportion at twelve files.

import type { FileKind, ReadingFile } from "$lib/ipc";

export interface Mark {
  x: number;
  y: number;
  w: number;
  h: number;
  /** Source order across the whole layer — what the hover wave travels along. */
  k: number;
  vis: "public" | "private" | "describe";
  /** Lines, for the readout. A describe reports how many tests it holds. */
  len: number;
}

export interface Layer {
  path: string;
  /** The module's last segment, or the filename when there is no module. */
  name: string;
  kind: FileKind;
  lines: number;
  publicCount: number;
  privateCount: number;
  /** Describes for a suite, settings for a config — what the layer is made of. */
  extra: number;
  /** Nothing to draw inside: a config, a plain file, an unparsed one. */
  bare: boolean;
  stale: boolean;
  missing: boolean;
  y: number;
  h: number;
  marks: Mark[];
}

/** Geometry, shared with the mockup so the two cannot drift silently. */
export const GEO = {
  /**
   * Room for the names — measured, not fixed.
   *
   * 138px was fine for `AllTargets` and clipped
   * `ImpactPipelineTelemetryReporter`. The column is sized from the longest name
   * in the gloom, clamped so one absurd module cannot take half the drawing.
   */
  labelMin: 110,
  labelMax: 300,
  /** ~7.1px per character at 11px in the mono stack, measured in the browser. */
  charW: 7.1,
  gap: 8,
  top: 6,
  /** Every band stays clickable: this is the file picker, not only a picture. */
  floor: 18,
  /**
   * Proportional room *per file*, rather than a fixed height shared out.
   *
   * Measured: sharing a fixed 300px between twelve files put every band within
   * 4px of the floor — the drawing still worked as a list and had stopped saying
   * anything about size. At 40px a file the tallest stays ~2.3× the shortest,
   * against a √ ratio of 3.6×, and the modal body scrolls for the rest.
   */
  poolPerFile: 40,
  pad: 10,
  markGap: 4,
};

const lastSegment = (s: string) => s.split(".").pop() ?? s;

/** How wide the name column has to be for the longest name to fit whole. */
export function labelWidth(names: string[]): number {
  const longest = names.reduce((n, s) => Math.max(n, s.length), 0);
  return Math.min(GEO.labelMax, Math.max(GEO.labelMin, longest * GEO.charW + 26));
}

/** √lines: linear would draw a 26-line config at a fifth of a 342-line module. */
const weight = (lines: number) => Math.sqrt(Math.max(lines, 1));

function describe(f: ReadingFile) {
  const kind: FileKind = f.outline?.kind ?? "plain";
  const module = f.outline?.modules?.[0];
  const name = module ? lastSegment(module.name) : (f.outline?.tests?.module ?? f.filename);
  const fns = module?.functions ?? [];
  return {
    kind,
    name: kind === "test" ? lastSegment(f.outline?.tests?.module ?? f.filename) : name,
    lines: f.source ? f.source.split("\n").length : 0,
    pub: fns.filter((x) => x.visibility !== "private").map((x) => x.endLine - x.line + 1),
    priv: fns.filter((x) => x.visibility === "private").map((x) => x.endLine - x.line + 1),
    describes: f.outline?.tests?.describes ?? [],
    settings: (f.outline?.config?.groups ?? []).reduce((n, g) => n + g.settings.length, 0),
  };
}

/**
 * Marks for one run of functions.
 *
 * The gaps and the padding come off the width **before** it is shared out. The
 * first version divided the full width by length and then added the gaps, so a
 * module with seven private helpers drew its last mark past the plate's right
 * edge — an amber block floating outside the layer it described.
 */
function run(lens: number[], up: boolean, from: number, x: number, width: number, half: number, h: number): Mark[] {
  if (!lens.length) return [];
  const room = width - GEO.pad * 2 - GEO.markGap * (lens.length - 1);
  const total = lens.reduce((a, v) => a + v, 0) || 1;
  const mh = Math.max(2.5, h / 2 - 6);
  let cx = x + GEO.pad;
  return lens.map((len, k) => {
    const w = Math.max(3, (len / total) * room);
    const mark: Mark = {
      x: cx,
      y: up ? half - mh - 1.5 : half + 1.5,
      w,
      h: mh,
      k: from + k,
      vis: up ? "public" : "private",
      len,
    };
    cx += w + GEO.markGap;
    return mark;
  });
}

/** The whole drawing: one layer per file, in the order the gloom collected them. */
export function layout(files: ReadingFile[], viewWidth: number): {
  layers: Layer[];
  height: number;
  label: number;
} {
  const seen = files.map(describe);
  const label = labelWidth(seen.map((s) => s.name));
  const x = label + 6;
  const width = viewWidth - x - 6;
  const total = seen.reduce((n, s) => n + weight(s.lines), 0) || 1;
  const pool = GEO.poolPerFile * files.length;

  let y = GEO.top;
  const layers = files.map((f, i) => {
    const s = seen[i];
    const h = GEO.floor + (weight(s.lines) / total) * pool;
    const half = y + h / 2;
    const bare = s.pub.length + s.priv.length === 0;

    let marks: Mark[] = [];
    if (!bare) {
      marks = [
        ...run(s.pub, true, 0, x, width, half, h),
        ...run(s.priv, false, s.pub.length, x, width, half, h),
      ];
    } else if (s.kind === "test" && s.describes.length) {
      // A suite has describes, not functions: one bar each, evenly, because a
      // describe's *length* says nothing — how many tests it holds does.
      const n = s.describes.length;
      const room = width - GEO.pad * 2 - GEO.markGap * (n - 1);
      const w = room / n;
      marks = s.describes.map((d, k) => ({
        x: x + GEO.pad + k * (w + GEO.markGap),
        y: half - 4,
        w,
        h: 8,
        k,
        vis: "describe" as const,
        len: d.tests.length,
      }));
    }

    const layer: Layer = {
      path: f.path,
      name: s.name,
      kind: s.kind,
      lines: s.lines,
      publicCount: s.pub.length,
      privateCount: s.priv.length,
      extra: s.kind === "test" ? s.describes.length : s.settings,
      bare: marks.length === 0,
      stale: f.stale,
      missing: f.missing,
      y,
      h,
      marks,
    };
    y += h + GEO.gap;
    return layer;
  });

  return { layers, height: y - GEO.gap + GEO.top, label };
}

/** Where the plates start and how wide they are, given the measured label column. */
export function plate(viewWidth: number, label: number): { x: number; width: number } {
  const x = label + 6;
  return { x, width: viewWidth - x - 6 };
}

/**
 * A path, trimmed from the LEFT to fit.
 *
 * The end identifies a file, so that is the end that survives. Done here rather
 * than with `text-overflow` because the footer mixes directions — a right-to-left
 * ellipsis in CSS needs `direction: rtl`, which reorders the segments of a path
 * that has any punctuation in it.
 */
export function tailPath(path: string, max = 68): string {
  if (path.length <= max) return path;
  const cut = path.length - max;
  const at = path.indexOf("/", cut);
  return "…/" + path.slice(at === -1 ? cut : at + 1);
}
