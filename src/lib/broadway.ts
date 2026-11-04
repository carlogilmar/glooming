// The Broadway Woop: two views of a module that says `use Broadway`.
// `mockup/broadway.html` is the contract.
//
//   wiring     the shape of start_link's options — which producer, how many
//              processes at each stage (drawn as that many squares), how batches
//              are cut, and what every callback is handed.
//   lifecycle  Broadway's callbacks as a node graph, routes as edges, ending in
//              the only two outcomes a message has: acked, or failed.
//
// Hand-written SVG, like `deps.ts`: every node is ours to place, the layout is
// deterministic, and there is no library to bundle. Pure — data in, markup out —
// so it is probed headlessly.
//
// Every interactive element carries `data-lines` ("12-14" or "12-14,20-22"): the
// Woop tints those lines in the code on hover and selects them on click. Nodes
// carry `data-out` (the edges they lead to) for the hover trace. The outcomes —
// acked, failed — carry neither: they are where a message ends, not code.

import type { BroadwayInfo, BroadwayStage, FnInfo, Knob, KnobValue, Range } from "$lib/ipc";

const esc = (s: string) =>
  s.replace(/[&<>"]/g, (c) => (c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : "&quot;"));

const lines = (...rs: (Range | null | undefined)[]) =>
  rs.filter((r): r is Range => !!r).map((r) => `${r.start}-${r.end}`).join(",");

// ---- values ---------------------------------------------------------------

/** What a value shows, and where it came from. */
export interface Shown {
  text: string;
  /** `lit` | `env` | `def` | `unk` — the colour that says where it came from. */
  src: "lit" | "env" | "def" | "unk";
  /** The number, when there is one — what the squares count. */
  n: number | null;
  /** The source expression, for the tooltip. */
  title: string;
}

const numeric = (t: string | null | undefined) => {
  if (!t) return null;
  const n = Number(t.replace(/_/g, ""));
  return Number.isFinite(n) ? n : null;
};

/**
 * Broadway's own defaults, for an option the module leaves out. Shown as a
 * default — its own colour, and said in the tooltip — rather than as "+?": a
 * value the code does not set is still a known value. Processor concurrency is
 * the one that depends on the machine (`System.schedulers_online() * 2`), so it
 * has words and no number — drawing sixteen squares would describe a laptop, not
 * the code.
 */
export const DEFAULTS = {
  producerConcurrency: { text: "1", n: 1 },
  processorConcurrency: { text: "2 × cores", n: null },
  maxDemand: { text: "10", n: 10 },
  batcherConcurrency: { text: "1", n: 1 },
  batchSize: { text: "100", n: 100 },
  batchTimeout: { text: "1000", n: 1000 },
} as const;

type Default = { text: string; n: number | null };

/**
 * `MyConfig.batch_partition_count(config)` → `batch_partition_count(…)`;
 * `config.weather_processing_batch_size` → `weather_processing_batch_size`.
 * What computes a runtime value, short enough for a label — far more than "+?".
 */
export function brief(expr: string): string {
  const call = expr.indexOf("(");
  const head = call === -1 ? expr : expr.slice(0, call);
  const name = head.split(".").pop() || head;
  return call === -1 ? name : `${name}(…)`;
}

/** A literal, an app-env read (its default shown), Broadway's default, or what computes it at runtime. */
export function shown(v: KnobValue | null | undefined, dflt?: Default): Shown {
  if (!v) {
    return dflt
      ? { text: dflt.text, src: "def", n: dflt.n, title: `not set — Broadway's default, ${dflt.text}` }
      : { text: "—", src: "def", n: null, title: "not set" };
  }
  if (v.kind === "literal") {
    const n = numeric(v.text);
    return { text: n !== null ? String(n) : v.text, src: "lit", n, title: v.text };
  }
  if (v.kind === "env") {
    const n = numeric(v.default);
    const title = `${v.required ? "required " : ""}env ${v.key}${v.default ? `, default ${v.default}` : ""}`;
    return { text: v.default ? (n !== null ? String(n) : v.default) : v.key, src: "env", n, title };
  }
  return { text: brief(v.text), src: "unk", n: null, title: `${v.text} — only known at runtime` };
}

/** 1000 → "1s", 1500 → "1.5s"; a timeout reads in seconds. */
export function seconds(s: Shown): string {
  return s.n === null ? s.text : `${s.n / 1000}s`;
}

// ---- the wiring -------------------------------------------------------------

/** `n` process squares from (x, y), in rows of `cols`. Capped: past 100 the count says it. `def` squares are Broadway's default, drawn lighter. */
function procs(x: number, y: number, n: number, cols = 10, sz = 7, gap = 2, def = false): [string, number, number] {
  const shownN = Math.min(n, 100);
  const c = Math.min(shownN, cols);
  const r = Math.ceil(shownN / cols);
  let out = "";
  for (let i = 0; i < shownN; i++) {
    // --i orders the hover cascade; capped so a hundred light in under half a second.
    out += `<rect class="proc${def ? " def" : ""}" style="--i:${Math.min(i, 30)}" x="${x + (i % cols) * (sz + gap)}" y="${y + Math.floor(i / cols) * (sz + gap)}" width="${sz}" height="${sz}" rx="1.5"/>`;
  }
  return [out, Math.max(0, c * (sz + gap) - gap), Math.max(0, r * (sz + gap) - gap)];
}

/** A value and its label, lighting its own lines. */
function knob(x: number, y: number, k: Knob | null | undefined, label: string, anchor = "start", fmt = (s: Shown) => s.text, dflt?: Default) {
  const s = shown(k?.value, dflt);
  // A runtime value's name says what it is; the label would only crowd it out.
  const word = s.src === "unk" ? "" : label;
  return `<text x="${x}" y="${y}" text-anchor="${anchor}" class="knob"${k ? ` data-lines="${lines(k.range)}"` : ""}><title>${esc(s.title)}</title><tspan class="t-mono v-${s.src}" font-weight="600">${esc(fmt(s))}</tspan><tspan class="t-dim"> ${esc(word)}</tspan></text>`;
}

const plural = (s: Shown, one: string, many: string) => (s.n === 1 ? one : many);

/**
 * Cut a label to `n` characters; the tooltip has the rest. A call keeps its
 * start (`batch_partition_count(…)`); a field keeps its END, because fields share
 * prefixes — `weather_processing_producer_concurrency` and
 * `weather_processing_processor_concurrency` both cut from the end read
 * `weather_process…`, which tells them apart not at all.
 */
const trunc = (t: string, n: number) => {
  if (t.length <= n) return t;
  return t.endsWith("(…)") ? `${t.slice(0, n - 4)}…(…)` : `…${t.slice(-(n - 1))}`;
};

/** What start_link's options say, drawn. Null when there are none to read. */
export function wiring(info: BroadwayInfo): string {
  const o = info.options;
  if (!o) {
    return `<div class="bw-unknown"${info.call ? ` data-lines="${lines(info.call)}"` : ""}>
      The options come from <code>${esc(info.runtimeOptions ?? "elsewhere")}</code> — built at runtime, so
      the producer, the concurrency and the batch sizes are only known at runtime — not guessed here.</div>`;
  }
  const X1 = 14, X2 = 236, X3 = 470;
  const p = o.producer;
  const pc = shown(p?.concurrency?.value, DEFAULTS.producerConcurrency);
  const [pq, pw] = procs(X1 + 14, 92, pc.n ?? 0, 10, 7, 2, pc.src === "def");

  const proc: BroadwayStage | undefined = o.processors[0];
  const rc = shown(proc?.concurrency?.value, DEFAULTS.processorConcurrency);
  const [rq, , rh0] = procs(X2 + 14, 74, rc.n ?? 0, 10, 7, 2, rc.src === "def");
  const rh = Math.max(rh0, 7);
  const procH = 74 + rh + 44 - 30;

  const BH = 88, BG = 14;
  const batchers = o.batchers;
  const bTop = (i: number) => 30 + i * (BH + BG);
  const height = Math.max(30 + procH, 142, batchers.length ? bTop(batchers.length - 1) + BH : 0) + 14;

  const wires = [
    `<g class="wg" data-edge="w-pp"><path class="wire" d="M178 104 L${X2} 104"/><path class="tr" d="M178 104 L${X2} 104" pathLength="1"/></g>`,
    ...batchers.map((_, i) => {
      const y = bTop(i) + 40;
      const d = `M${X2 + 198} 104 C ${X2 + 216} 104, ${X3 - 18} ${y}, ${X3} ${y}`;
      return `<g class="wg" data-edge="w-b${i}"><path class="wire" d="${d}"/><path class="tr" d="${d}" pathLength="1"/></g>`;
    }),
  ].join("");

  const batcherNodes = batchers
    .map((b, i) => {
      const y = bTop(i), x = X3;
      const c = shown(b.concurrency?.value, DEFAULTS.batcherConcurrency);
      const [sq] = procs(x + 14, y + 36, c.n ?? 0, 6, 7, 2, c.src === "def");
      const size = b.batchSize;
      const timeout = b.batchTimeout;
      return `
      <g class="wstage" data-out="w-b${i}" data-lines="${lines(b.range)}">
        <rect class="node" x="${x}" y="${y}" width="206" height="${BH}" rx="9"/>
        <text x="${x + 14}" y="${y + 24}"><tspan class="t-mono" fill="var(--gloom-deep)" font-weight="600">:${esc(b.name)}</tspan><tspan class="t-dim"> batcher</tspan></text>
        ${sq}
        ${
          // A runtime value has no squares and little room beside the bucket:
          // its name alone, cut short, with the whole expression in the tooltip.
          c.src === "unk"
            ? knob(x + 14, y + 72, b.concurrency, "", "start", (v) => trunc(v.text, 15))
            : knob(x + 14, y + 72, b.concurrency, plural(c, "process", "processes"), "start", undefined, DEFAULTS.batcherConcurrency)
        }
        <path class="bucketwall" d="M${x + 120} ${y + 18} L${x + 124} ${y + 64} L${x + 178} ${y + 64} L${x + 182} ${y + 18}"/>
        <rect class="bucketfill" x="${x + 124}" y="${y + 34}" width="54" height="30"/>
        <text class="t-dim" x="${x + 151}" y="${y + 30}" text-anchor="middle">size</text>
        ${knob(x + 151, y + 54, size, "", "middle", (v) => trunc(v.text, 9), DEFAULTS.batchSize)}
        ${knob(x + 151, y + 80, timeout, "", "middle", (v) => `⏱ ${trunc(seconds(v), 8)}`, DEFAULTS.batchTimeout)}
      </g>`;
    })
    .join("");

  const noBatchers = batchers.length
    ? ""
    : `<text class="t-dim" x="${X3}" y="60">no batchers — acked after</text><text class="t-dim" x="${X3}" y="74">handle_message</text>`;

  return `
  <svg class="bw" viewBox="0 0 690 ${height}" role="img" aria-label="How start_link wires the pipeline">
    <text class="colhead" x="${X1}" y="16">producer</text>
    <text class="colhead" x="${X2}" y="16">processors</text>
    <text class="colhead" x="${X3}" y="16">batchers</text>
    ${wires}

    <g class="wstage" data-out="w-pp" data-lines="${lines(p?.range)}">
      <rect class="node src" x="${X1}" y="30" width="164" height="112" rx="9"/>
      <text class="t-mono" x="${X1 + 14}" y="54" font-weight="700"${p?.moduleRange ? ` data-lines="${lines(p.moduleRange)}"` : ""}>${esc(p?.module ?? "producer")}</text>
      ${p?.options.length ? `<text class="t-dim knob" x="${X1 + 14}" y="72" data-lines="${lines(p.optionsRange)}"><title>${esc(p.options.join(", "))}</title>${p.options.length} option${p.options.length === 1 ? "" : "s"} ›</text>` : ""}
      ${pq}
      ${knob(X1 + 14 + (pw ? pw + 8 : 0), 99, p?.concurrency, plural(pc, "producer", "producers"), "start", (v) => trunc(v.text, pw ? 12 : 22), DEFAULTS.producerConcurrency)}
    </g>

    <g class="wstage" data-out="${batchers.map((_, i) => `w-b${i}`).join(" ")}" data-lines="${lines(proc?.range)}">
      <rect class="node" x="${X2}" y="30" width="198" height="${procH}" rx="9"/>
      <text x="${X2 + 14}" y="54"><tspan class="t-mono" fill="var(--gloom-deep)" font-weight="600">:${esc(proc?.name ?? "default")}</tspan><tspan class="t-dim"> processors</tspan></text>
      ${rq}
      ${knob(X2 + 14, 74 + rh + 18, proc?.concurrency, plural(rc, "process", "processes"), "start", (v) => trunc(v.text, 26), DEFAULTS.processorConcurrency)}
      ${proc ? knob(X2 + 184, 74 + rh + 32, proc.maxDemand, "demand", "end", (v) => trunc(v.text, 26), DEFAULTS.maxDemand) : ""}
    </g>

    ${batcherNodes}${noBatchers}
  </svg>`;
}

/** The context, as the map it is — a chip per entry — and the registered name. */
export function contextRow(info: BroadwayInfo): string {
  const o = info.options;
  if (!o) return "";
  const chip = (k: string, v: KnobValue, r: Range, cls = "") => {
    const s = shown(v);
    const text = v.kind === "env" ? `env ${v.key}` : s.text;
    return `<span class="kv ${cls}" data-lines="${lines(r)}" title="${esc(s.title)}"><span class="k">${esc(k)}</span><span class="v v-${s.src}">${esc(text)}</span></span>`;
  };
  const c = o.context;
  const entries = c
    ? c.runtime
      ? chip("built", { kind: "runtime", text: c.runtime }, c.range)
      : c.entries.map((e) => chip(e.key, e.value, e.range)).join("")
    : `<span class="t-dim">none</span>`;
  const name = o.name
    ? `<span class="kv name" data-lines="${lines(o.name.range)}"><span class="k">name</span><span class="v">${esc(shown(o.name.value).text)}</span></span>`
    : "";
  return `<div class="ctxrow"><span class="ctxlabel"${c ? ` data-lines="${lines(c.range)}"` : ""}>context</span>${entries}<span class="sp"></span>${name}</div>`;
}

// ---- the lifecycle ----------------------------------------------------------

const H = 46;

function gnode(o: { cx: number; cy: number; w?: number; fn: string; sub: string; lines?: string; sig?: string; out?: string; optional?: boolean; cls?: string; ghost?: boolean }) {
  const w = o.w ?? 170;
  const attrs = o.ghost ? "" : ` data-lines="${o.lines ?? ""}"${o.sig ? ` data-sig="${o.sig}"` : ""} data-out="${o.out ?? ""}"`;
  return `
    <g class="gn${o.cls ? ` ${o.cls}` : ""}${o.ghost ? " ghost" : ""}"${attrs}>
      <rect class="node cb" x="${o.cx - w / 2}" y="${o.cy - H / 2}" width="${w}" height="${H}" rx="9"/>
      <text class="t-mono" x="${o.cx}" y="${o.cy - 3}" text-anchor="middle" font-weight="600" fill="var(--syn-fn)">${o.fn}</text>
      <text class="t-dim" x="${o.cx}" y="${o.cy + 13}" text-anchor="middle">${o.ghost ? "not defined" : o.sub}${o.optional && !o.ghost ? " · optional" : ""}</text>
    </g>`;
}

/** An edge; vertical by default, or along `path`. `.tr` is the copy the hover trace draws. */
function gedge(x1: number, y1: number, x2: number, y2: number, id: string, o: { label?: string; fail?: boolean; lx?: number; ly?: number; path?: string; side?: boolean } = {}) {
  const my = (y1 + y2) / 2;
  const d =
    o.path ??
    (o.side
      ? `M${x1} ${y1} C ${x1 + 40} ${y1}, ${x2 - 40} ${y2}, ${x2 - 6} ${y2}`
      : `M${x1} ${y1} C ${x1} ${my}, ${x2} ${my}, ${x2} ${y2 - 6}`);
  return `
    <g class="ge${o.fail ? " fail" : ""}" data-edge="${id}">
      <path class="ln-main" d="${d}" marker-end="url(#bw-${o.fail ? "hf" : "h"})"/>
      <path class="tr" d="${d}" pathLength="1"/>
      ${o.label ? `<text class="elabel" x="${o.lx ?? (x1 + x2) / 2 + 6}" y="${o.ly ?? my}">${o.label}</text>` : ""}
    </g>`;
}

/** Every clause of a callback, as one `data-lines`. */
const fnLines = (fns: FnInfo[], name: string) =>
  fns
    .filter((f) => f.name === name)
    .flatMap((f) => (f.clauseRanges?.length ? f.clauseRanges : [{ start: f.line, end: f.endLine }]))
    .map((r) => `${r.start}-${r.end}`)
    .join(",");

export function lifecycle(info: BroadwayInfo, fns: FnInfo[]): string {
  const has = (n: string) => fns.some((f) => f.name === n);
  const o = info.options;
  const batchers = o?.batchers ?? [];
  const n = batchers.length;

  const C = 250, F = 575;
  // Batchers spread across the left of the graph; acked sits under them.
  const left = 40, right = 470;
  const span = (right - left) / Math.max(n, 1);
  const bw = Math.min(190, span - 16);
  const bx = (i: number) => left + span * (i + 0.5);
  const BY = 330;
  const ackX = n ? (bx(0) + bx(n - 1)) / 2 : C;
  const ackY = n ? 500 : 330;
  const height = ackY + 60;

  const prep = has("prepare_messages");
  const failed = has("handle_failed");

  const batchNodes = batchers
    .map((b, i) => {
      // The clause for this batcher, or the catch-all; none at all would be a
      // pipeline that crashes on its first batch — drawn as not defined.
      const clause = info.batchClauses.find((c) => c.batcher === b.name) ?? info.batchClauses.find((c) => c.batcher === null);
      const size = shown(b.batchSize?.value, DEFAULTS.batchSize);
      return gnode({
        cx: bx(i), cy: BY, w: bw, fn: "handle_batch/4",
        sub: `:${esc(b.name)}${size.n !== null ? ` · ≤${size.n}` : ""}`,
        lines: clause ? lines(clause.range) : "",
        out: `e-ack-${i} e-fail-batch`,
        ghost: !clause,
      });
    })
    .join("");

  const gx1 = n ? bx(0) - bw / 2 - 12 : 0;
  const gx2 = n ? bx(n - 1) + bw / 2 + 12 : 0;

  return `
  <svg class="bw" viewBox="0 0 690 ${height}" role="img" aria-label="The Broadway lifecycle of this module">
    <defs>
      <!-- In user units, so a head keeps its size when a lit line thickens; a lit
           one of its own, or the line turns blue and its head stays grey. -->
      <marker id="bw-h" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="9" markerHeight="9" markerUnits="userSpaceOnUse" orient="auto"><path d="M0 0 L8 4 L0 8 z" fill="var(--fg-faint)"/></marker>
      <marker id="bw-hl" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="9" markerHeight="9" markerUnits="userSpaceOnUse" orient="auto"><path d="M0 0 L8 4 L0 8 z" fill="var(--accent)"/></marker>
      <marker id="bw-hf" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="9" markerHeight="9" markerUnits="userSpaceOnUse" orient="auto"><path d="M0 0 L8 4 L0 8 z" fill="var(--priv)"/></marker>
    </defs>

    <g class="gn" data-lines="${lines(o?.producer?.moduleRange ?? o?.producer?.range)}" data-out="e-prep">
      <rect class="node src" x="${C - 95}" y="10" width="190" height="40" rx="9"/>
      <text class="t-mono" x="${C}" y="34" text-anchor="middle" font-weight="700">${esc(o?.producer?.module ?? "producer")}</text>
    </g>

    <g class="stage" data-lines="${lines(o?.processors[0]?.range)}">
      <rect class="pgroup" x="${C - 120}" y="78" width="240" height="176" rx="12"/>
      <text class="t-dim" x="${C - 108}" y="96">processors</text>
    </g>
    ${gnode({ cx: C, cy: 132, fn: "prepare_messages/2", sub: "messages", lines: fnLines(fns, "prepare_messages"), sig: "prepare_messages/2", out: "e-handle e-fail-msg", optional: true, ghost: !prep })}
    ${gnode({ cx: C, cy: 216, fn: "handle_message/3", sub: "1 message", lines: fnLines(fns, "handle_message"), sig: "handle_message/3", out: `${batchers.map((_, i) => `e-b${i}`).join(" ")} ${n ? "" : "e-ack-direct"} e-fail-msg` })}

    ${n ? `<g class="stage" data-lines="${lines(...batchers.map((b) => b.range))}">
      <rect class="pgroup" x="${gx1}" y="280" width="${gx2 - gx1}" height="88" rx="12"/>
      <text class="t-dim" x="${gx1 + 12}" y="296">batchers</text>
    </g>` : ""}
    ${batchNodes}

    ${gnode({ cx: F, cy: 410, w: 160, fn: "handle_failed/2", sub: "failed messages", lines: fnLines(fns, "handle_failed"), sig: "handle_failed/2", out: "e-failed", optional: true, cls: "failnode", ghost: !failed })}

    <!-- Outcomes, not code: nothing to light. -->
    <g class="outcomeg">
      <rect class="outcome ok" x="${ackX - 70}" y="${ackY}" width="140" height="40" rx="20"/>
      <text x="${ackX}" y="${ackY + 25}" text-anchor="middle" class="olabel ok">✓ acked</text>
    </g>
    <g class="outcomeg">
      <rect class="outcome bad" x="${F - 70}" y="500" width="140" height="40" rx="20"/>
      <text x="${F}" y="525" text-anchor="middle" class="olabel bad">✗ failed</text>
    </g>

    ${gedge(C, 50, C, 109, "e-prep")}
    ${gedge(C, 155, C, 193, "e-handle")}
    ${batchers.map((_, i) => gedge(C, 239, bx(i), BY - H / 2, `e-b${i}`)).join("")}
    ${n ? "" : gedge(C, 239, C, ackY, "e-ack-direct", { path: `M${C} 239 L${C} ${ackY - 6}` })}
    ${gedge(C + 120, 166, F, 387, "e-fail-msg", { label: "failed", fail: true, lx: F - 40, ly: 160, path: `M${C + 120} 166 C ${F} 166, ${F} 166, ${F} 381` })}
    ${n ? gedge(gx2, BY - 6, F - 80, 410, "e-fail-batch", { label: "failed", fail: true, lx: gx2 + 9, ly: 384, side: true }) : ""}
    ${batchers.map((_, i) => gedge(bx(i), BY + H / 2, ackX + (i - (n - 1) / 2) * 24, ackY, `e-ack-${i}`)).join("")}
    ${gedge(F, 433, F, 500, "e-failed", { fail: true })}
  </svg>`;
}
