// The ```lgtm:surface block: the module's surface as a directory.
//
// Ported from `mockup/surface.html`, which is the visual contract.
//
// Two columns — public left, private right — each sorted by name and scrolling
// on its own. This is deliberately *not* `lgtm:functions`: that block is where
// you write, and its rows carry your prose. This one is for getting somewhere.
//
// Sorting by name is the whole point and also the trade: it makes the block a
// directory you look things up in, and it throws away source order completely.
// That is why every row carries its line number — once the list is alphabetical
// the line is the only remaining hint of where a row will take you.
//
// The sort happens ONCE, in `seed.rs`. This renders the order the text gives it.
//
// Plain HTML rather than SVG, unlike the treemap and the reach block: this is a
// list with independent scrolling, and that is what the DOM is good at.

export interface SurfaceFn {
  sig: string;
  line: number;
  flags: string[];
}

export interface Surface {
  public: SurfaceFn[];
  private: SurfaceFn[];
}

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : "&quot;",
  );
}

/**
 * ```
 * public:
 *   create_user/1     : 12
 *   search_users/1..2 : 46 default args
 * private:
 *   normalize/1       : 112 3 clauses
 * ```
 *
 * Everything after the first colon is the value: the line, then any flags.
 */
export function parseSurface(body: string): Surface {
  const out: Surface = { public: [], private: [] };
  let group: "public" | "private" = "public";

  for (const raw of body.split("\n")) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    if (line === "public:") {
      group = "public";
      continue;
    }
    if (line === "private:") {
      group = "private";
      continue;
    }

    const colon = line.indexOf(":");
    if (colon < 1) continue;
    const sig = line.slice(0, colon).trim().replace(/^-\s*/, "");
    if (!sig) continue;

    const rest = line.slice(colon + 1).trim().split(/\s+/);
    const lineNo = parseInt(rest[0] ?? "", 10);
    out[group].push({
      sig,
      line: Number.isFinite(lineNo) ? lineNo : 0,
      // `default args`, `3 clauses` — whatever the seeder wrote, kept as text.
      flags: rest.slice(1).join(" ").trim() ? [rest.slice(1).join(" ").trim()] : [],
    });
  }

  return out;
}

function rowHtml(f: SurfaceFn): string {
  const slash = f.sig.indexOf("/");
  const name = slash === -1 ? f.sig : f.sig.slice(0, slash);
  const arity = slash === -1 ? "" : f.sig.slice(slash);
  const badges = f.flags.map((b) => `<span class="badge">${esc(b)}</span>`).join("");

  return (
    `<div class="row" data-sig="${esc(f.sig)}"${f.line ? ` data-line="${f.line}"` : ""} role="button" tabindex="0">` +
    `<span class="sig">${esc(name)}<span class="ar">${esc(arity)}</span></span>` +
    badges +
    `<span class="spacer"></span>` +
    (f.line ? `<span class="ln">${f.line}</span>` : "") +
    `</div>`
  );
}

function column(kind: "public" | "private", fns: SurfaceFn[], empty: string): string {
  const rows = fns.length ? fns.map(rowHtml).join("") : `<p class="none">${empty}</p>`;
  return (
    `<div class="col ${kind}">` +
    `<div class="label"><span class="bar"></span>${kind}<span class="n">${fns.length}</span></div>` +
    `<div class="list">${rows}</div>` +
    `</div>`
  );
}

export function renderSurface(body: string, module: string): string {
  const s = parseSurface(body);
  if (!s.public.length && !s.private.length) {
    return `<div class="lgtm-surface empty">Empty surface block — re-seed this doc, or write <code>create_user/1 : 12</code> rows under <code>public:</code> here.</div>`;
  }

  // Deliberately NOT re-sorted. The seeder already ordered these, and sorting
  // again here would mean two sorters that have to agree — they didn't:
  // Rust orders by (name, arity), giving `get_user/1, get_user!/1`, while
  // JS `localeCompare` on the full signature reorders punctuation and gave
  // `get_user_by_email/1, get_user!/1, get_user/1`. The text and the picture
  // would have disagreed. Rendering the order in the text also means a row you
  // move by hand stays where you put it.
  return (
    `<div class="lgtm-surface">` +
    `<header><span class="tag">lgtm:surface</span><span>${esc(module)}</span>` +
    `<span class="count">${s.public.length} public · ${s.private.length} private</span></header>` +
    `<div class="cols">` +
    column("public", s.public, "no public functions") +
    column("private", s.private, "nothing private") +
    `</div>` +
    `</div>`
  );
}
