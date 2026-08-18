// The ```lgtm:tests block: what a test suite actually covers.
//
// Ported from `mockup/kinds.html`, which is the visual contract.
//
// Two things a test file has that a module doesn't:
//
//   · describes grouping tests, and assertions inside those. Each describe
//     carries a strip of one square per test, shaded by how much it asserts —
//     a pale square is a test that checks one thing, so thin coverage is
//     visible without reading a word.
//
//   · setup, which STACKS. A test starts from the module's setup_all, plus the
//     module's setup, plus its own describe's — blocks that can be a hundred
//     lines apart. So each describe shows the accumulated context its tests can
//     destructure, which is the question plain text answers worst.

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : "&quot;",
  );
}

export interface Setup {
  kind: string;
  line: number;
  endLine: number;
  named: string | null;
  /** Context keys provided; `null` means unknown, which is not "none". */
  provides: string[] | null;
}

export interface TestCase {
  name: string;
  line: number;
  endLine: number;
  asserts: number;
  tags: string[];
}

export interface Describe {
  name: string | null;
  line: number;
  endLine: number;
  setups: Setup[];
  tests: TestCase[];
}

export interface Suite {
  setups: Setup[];
  describes: Describe[];
}

/**
 *     setup : 4 :user
 *     describe "create_user/1" : 12
 *       setup : 13 runs :put_user
 *       creates a user : 16 3 @slow
 */
export function parseTests(body: string): Suite {
  const out: Suite = { setups: [], describes: [] };

  for (const raw of body.split("\n")) {
    if (!raw.trim() || raw.trim().startsWith("#")) continue;
    const colon = raw.lastIndexOf(" : ");
    if (colon < 1) continue;

    const label = raw.slice(0, colon).trim();
    const value = raw.slice(colon + 3).trim();
    const indent = raw.length - raw.trimStart().length;
    const [first, ...rest] = value.split(/\s+/);
    // `12` or `12-40` — a span, so selecting a row covers the whole block.
    const [startTxt, endTxt] = first.split("-");
    const line = parseInt(startTxt, 10) || 0;
    const endLine = parseInt(endTxt ?? startTxt, 10) || line;
    const tail = rest.join(" ");

    if (label === "setup" || label === "setup_all") {
      const named = tail.startsWith("runs :") ? tail.slice(6).trim() : null;
      const provides = named
        ? null
        : tail === "?"
          ? null
          : tail === "-" || !tail
            ? []
            : tail.split(/\s+/).map((k) => k.replace(/^:/, ""));
      const setup: Setup = { kind: label, line, endLine, named, provides };
      if (indent >= 4 && out.describes.length) {
        out.describes[out.describes.length - 1].setups.push(setup);
      } else {
        out.setups.push(setup);
      }
      continue;
    }

    if (label.startsWith("describe ") || label === "(no describe)") {
      const name = label === "(no describe)" ? null : label.replace(/^describe\s+"?|"$/g, "");
      out.describes.push({ name, line, endLine, setups: [], tests: [] });
      continue;
    }

    if (out.describes.length) {
      out.describes[out.describes.length - 1].tests.push({
        name: label,
        line,
        endLine,
        asserts: parseInt(rest[0] ?? "0", 10) || 0,
        tags: rest.filter((t) => t.startsWith("@")).map((t) => t.slice(1)),
      });
    }
  }

  return out;
}

/** Everything a describe's tests can destructure: module scope plus its own. */
function contextOf(suite: Suite, d: Describe) {
  const all = [...suite.setups, ...d.setups];
  return {
    keys: all.flatMap((s) => s.provides ?? []),
    // A named callback is opaque, so the set is marked incomplete rather than
    // silently short.
    partial: all.some((s) => s.provides === null),
    count: all.length,
  };
}

function setupHtml(s: Setup, scope: "mod" | "desc"): string {
  const what = s.named
    ? `<span class="named">:${esc(s.named)}</span>`
    : s.provides === null
      ? `<span class="unknown">unknown</span>`
      : s.provides.length
        ? s.provides.map((k) => `<span class="key">:${esc(k)}</span>`).join("")
        : `<span class="unknown">no context</span>`;

  return (
    `<div class="su ${scope}"${span(s.line, s.endLine)} data-sig="${esc(s.kind)} ${s.line}">` +
    `<span class="gear">⚙</span><span class="kind">${esc(s.kind)}</span>` +
    `<span class="lbl">${s.named ? "runs" : "provides"}</span>${what}` +
    `<span class="spacer"></span>` +
    (s.line ? `<span class="ln">${s.line}</span>` : "") +
    `</div>`
  );
}

const shade = (n: number) => (n <= 1 ? "a1" : n <= 2 ? "a2" : "a3");

/** `data-line` plus `data-end`, so a click can select the whole block. */
function span(line: number, endLine: number): string {
  if (!line) return "";
  return ` data-line="${line}" data-end="${Math.max(endLine, line)}"`;
}

export function renderTests(body: string, module: string): string {
  const suite = parseTests(body);
  if (!suite.describes.length && !suite.setups.length) {
    return `<div class="lgtm-tests empty">Empty tests block — re-seed this doc, or write <code>describe "create_user/1" : 12</code> style rows here.</div>`;
  }

  const all = suite.describes.flatMap((d) => d.tests);
  const named = suite.describes.filter((d) => d.name).length;

  const moduleScope = suite.setups.length
    ? `<div class="modsetup">` +
      `<div class="scopelbl">module scope — every test below inherits these</div>` +
      suite.setups.map((s) => setupHtml(s, "mod")).join("") +
      `</div>`
    : "";

  const groups = suite.describes
    .map((d) => {
      const asserts = d.tests.reduce((a, t) => a + t.asserts, 0);
      const ctx = contextOf(suite, d);

      const strip = d.tests
        .map((t) => {
          const skipped = t.tags.includes("skip");
          const cls = [shade(t.asserts), t.tags.length ? "tagged" : "", skipped ? "skipped" : ""]
            .filter(Boolean)
            .join(" ");
          return `<i class="${cls}" title="${esc(t.name)} — ${t.asserts} assertion${t.asserts === 1 ? "" : "s"}"></i>`;
        })
        .join("");

      const ctxHtml =
        ctx.keys.length || ctx.partial
          ? `<span class="ctx" title="what these tests start with">` +
            `<span class="gear">⚙</span>${ctx.count}` +
            ctx.keys.map((k) => `<span class="key">:${esc(k)}</span>`).join("") +
            (ctx.partial ? `<span class="unknown">+?</span>` : "") +
            `</span>`
          : "";

      const tests = d.tests
        .map((t) => {
          const skipped = t.tags.includes("skip");
          return (
            `<div class="t${skipped ? " skipped" : ""}"${span(t.line, t.endLine)} data-sig="${esc(t.name)}">` +
            `<span class="dot"></span><span class="name">${esc(t.name)}</span>` +
            t.tags.map((g) => `<span class="badge">@${esc(g)}</span>`).join("") +
            `<span class="spacer"></span>` +
            `<span class="as">${t.asserts}×</span>` +
            (t.line ? `<span class="ln">${t.line}</span>` : "") +
            `</div>`
          );
        })
        .join("");

      return (
        `<div class="desc">` +
        `<div class="head"${span(d.line, d.endLine)} data-sig="${esc(d.name ?? "no describe")}">` +
        (d.name
          ? `<span class="name">describe <span class="q">"</span>${esc(d.name)}<span class="q">"</span></span>`
          : `<span class="name loose">no describe</span>`) +
        ctxHtml +
        `<span class="strip">${strip}</span>` +
        `<span class="n">${d.tests.length} test${d.tests.length === 1 ? "" : "s"} · ${asserts} assertions</span>` +
        `</div>` +
        d.setups.map((s) => setupHtml(s, "desc")).join("") +
        `<div class="tests">${tests}</div>` +
        `</div>`
      );
    })
    .join("");

  return (
    `<div class="lgtm-tests">` +
    `<header><span class="tag">lgtm:tests</span><span>${esc(module)}</span>` +
    `<span class="count">${all.length} tests · ${named} describes</span></header>` +
    moduleScope +
    groups +
    `</div>`
  );
}
