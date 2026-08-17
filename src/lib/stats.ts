// The ```lgtm:stats block: the facts about a file you'd otherwise go and look
// up — how big it is, how much of it is exposed, and what git knows about who
// has been in here and for how long.
//
// The values live in the markdown as `key: value` lines, written once when the
// doc is seeded. The renderer only formats what the text says. That means the
// doc is readable as plain text, survives being pasted anywhere, and can be
// corrected by hand — the file is the data, not a cache of a computation.

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : "&quot;",
  );
}

export type Stats = Record<string, string>;

/** `lines: 142` per line. Unknown keys are kept — the block is yours to edit. */
export function parseStats(body: string): Stats {
  const out: Stats = {};
  for (const raw of body.split("\n")) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const colon = line.indexOf(":");
    if (colon < 1) continue;
    const key = line.slice(0, colon).trim().toLowerCase();
    const value = line.slice(colon + 1).trim();
    if (key && value) out[key] = value;
  }
  return out;
}

/** "14 Feb 2025" — short enough for a tile, unambiguous across locales. */
function shortDate(value: string | undefined): string | null {
  if (!value) return null;
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return value; // hand-edited to something else
  return d.toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
}

function ago(value: string | undefined): string | null {
  if (!value) return null;
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return null;
  const days = Math.floor((Date.now() - d.getTime()) / 86_400_000);
  if (days <= 0) return "today";
  if (days === 1) return "yesterday";
  if (days < 30) return `${days} days ago`;
  if (days < 365) return `${Math.floor(days / 30)} months ago`;
  return `${Math.floor(days / 365)} years ago`;
}

function tile(value: string, label: string, sub?: string | null): string {
  return (
    `<div class="stat">` +
    `<b>${esc(value)}</b>` +
    `<span class="lbl">${esc(label)}</span>` +
    (sub ? `<span class="sub">${esc(sub)}</span>` : "") +
    `</div>`
  );
}

function plural(value: string | undefined, one: string, many: string): string {
  return value === "1" ? one : many;
}

export function renderStats(body: string): string {
  const s = parseStats(body);
  if (!Object.keys(s).length) {
    return `<div class="lgtm-stats empty">Empty stats block — re-seed this doc, or write <code>lines: 120</code> style lines here.</div>`;
  }

  const tiles: string[] = [];
  if (s.lines) tiles.push(tile(s.lines, "lines", s.code ? `${s.code} non-blank` : null));
  if (s.public) tiles.push(tile(s.public, plural(s.public, "public fn", "public fns")));
  if (s.private) tiles.push(tile(s.private, plural(s.private, "private fn", "private fns")));

  const authors = s.authors ? s.authors.split(",").map((a) => a.trim()).filter(Boolean) : [];
  if (authors.length) {
    // The first name is the busiest committer — who you'd actually ask.
    tiles.push(
      tile(
        String(authors.length),
        authors.length === 1 ? "author" : "authors",
        authors[0] ?? null,
      ),
    );
  }
  if (s.commits) tiles.push(tile(s.commits, plural(s.commits, "commit", "commits")));
  if (s.created) tiles.push(tile(shortDate(s.created) ?? "—", "created"));
  if (s.updated) tiles.push(tile(shortDate(s.updated) ?? "—", "last touched", ago(s.updated)));

  const who =
    authors.length > 1
      ? `<div class="who">Touched by ${authors.map(esc).join(", ")}</div>`
      : "";

  return `<div class="lgtm-stats"><div class="grid">${tiles.join("")}</div>${who}</div>`;
}
