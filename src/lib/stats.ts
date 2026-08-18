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

/**
 * Labels for the keys the seeder writes. A file kind decides which of these
 * appear — a config has no `public`, a test suite has no `settings` — so the
 * renderer formats whatever it finds rather than expecting a fixed set. An
 * unknown key still renders, with the key itself as the label, because the
 * block is yours to edit.
 */
const LABELS: Record<string, string> = {
  lines: "lines",
  code: "code",
  public: "public fns",
  private: "private fns",
  // config
  apps: "apps",
  groups: "groups",
  settings: "settings",
  fromenv: "from env",
  literal: "literal",
  imports: "imports",
  // tests
  tests: "tests",
  describes: "describes",
  assertions: "assertions",
  setups: "setups",
  async: "async",
  case: "case",
  tagged: "tagged",
  // git
  commits: "commits",
  authors: "authors",
  created: "created",
  updated: "last touched",
};

/** Keys worth colouring: something to act on rather than just to know. */
const WARN = new Set(["fromenv", "tagged"]);

/** `1 (1 required)` → value `1`, sub `1 required`. */
function split(raw: string): { value: string; sub: string | null } {
  const m = /^(.*?)\s*\(([^)]*)\)\s*$/.exec(raw);
  return m ? { value: m[1].trim(), sub: m[2].trim() } : { value: raw, sub: null };
}

function tile(value: string, label: string, sub?: string | null, warn = false): string {
  return (
    `<div class="stat${warn ? " warn" : ""}">` +
    `<b>${esc(value)}</b>` +
    `<span class="lbl">${esc(label)}</span>` +
    (sub ? `<span class="sub">${esc(sub)}</span>` : "") +
    `</div>`
  );
}

export function renderStats(body: string): string {
  const s = parseStats(body);
  const keys = Object.keys(s);
  if (!keys.length) {
    return `<div class="lgtm-stats empty">Empty stats block — re-seed this doc, or write <code>lines: 120</code> style lines here.</div>`;
  }

  const tiles: string[] = [];
  const seen = new Set<string>();

  for (const key of keys) {
    // `code` and `authors` ride along with another tile rather than taking one
    // of their own.
    if (key === "code" || key === "authors" || seen.has(key)) continue;

    const { value, sub } = split(s[key]);
    const label = LABELS[key] ?? key;

    if (key === "lines") {
      tiles.push(tile(value, label, s.code ? `${s.code} non-blank` : sub));
      continue;
    }
    if (key === "commits") {
      // The busiest committer is who you'd actually ask about this file.
      const authors = (s.authors ?? "").split(",").map((a) => a.trim()).filter(Boolean);
      if (authors.length) {
        tiles.push(
          tile(String(authors.length), authors.length === 1 ? "author" : "authors", authors[0]),
        );
      }
      tiles.push(tile(value, label, sub));
      continue;
    }
    if (key === "created" || key === "updated") {
      tiles.push(tile(shortDate(value) ?? value, label, key === "updated" ? ago(value) : sub));
      continue;
    }
    tiles.push(tile(value, label, sub, WARN.has(key)));
    seen.add(key);
  }

  const authors = (s.authors ?? "").split(",").map((a) => a.trim()).filter(Boolean);
  const who =
    authors.length > 1
      ? `<div class="who">Touched by ${authors.map(esc).join(", ")}</div>`
      : "";

  return `<div class="lgtm-stats"><div class="grid">${tiles.join("")}</div>${who}</div>`;
}
