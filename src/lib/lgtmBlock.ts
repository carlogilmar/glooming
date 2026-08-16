// Parsing the body of a ```lgtm:functions block.
//
// Deliberately forgiving: you type this by hand, and a malformed block must
// degrade to a plain code fence rather than lose your writing. The Rust side
// has an equivalent parser for reconciliation (src-tauri/src/reconcile.rs) —
// the two must agree on the grammar, which is why it is this simple.

export interface FnEntry {
  /** As written: `create_user/1`, `search/1..2`, `~~gone/0~~`. */
  sig: string;
  name: string;
  arity: number;
  visibility: "public" | "private";
  prose: string;
  removed: boolean;
  /** Filled in from the live outline, so the row can jump to the code. */
  line?: number;
  clauses?: number;
}

export interface FunctionsBlock {
  module: string | null;
  entries: FnEntry[];
}

/** `lgtm:functions module=MyApp.Accounts` → { module: "MyApp.Accounts" } */
export function parseInfo(info: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const part of info.trim().split(/\s+/).slice(1)) {
    const eq = part.indexOf("=");
    if (eq > 0) out[part.slice(0, eq)] = part.slice(eq + 1);
  }
  return out;
}

export function parseBlock(info: string, body: string): FunctionsBlock {
  const entries: FnEntry[] = [];
  let visibility: "public" | "private" = "public";

  for (const raw of body.split("\n")) {
    const line = raw.trim();
    if (!line) continue;
    if (line === "public:") {
      visibility = "public";
      continue;
    }
    if (line === "private:") {
      visibility = "private";
      continue;
    }

    const row = line.startsWith("- ") ? line.slice(2) : line.startsWith("-") ? line.slice(1) : null;
    if (row === null) continue;

    // Everything after the FIRST colon is prose, so explanations may contain
    // colons, backticks and inline code freely.
    const colon = row.indexOf(":");
    const sig = (colon === -1 ? row : row.slice(0, colon)).trim();
    const prose = colon === -1 ? "" : row.slice(colon + 1).trim();
    if (!sig) continue;

    const removed = sig.startsWith("~~");
    const bare = sig.replace(/~~/g, "");
    const slash = bare.lastIndexOf("/");
    const name = slash === -1 ? bare : bare.slice(0, slash);
    // `1..2` → the top arity is the identity.
    const arityText = slash === -1 ? "0" : bare.slice(slash + 1);
    const arity = parseInt(arityText.split("..").pop() ?? "0", 10) || 0;

    entries.push({ sig, name, arity, visibility, prose, removed });
  }

  return { module: parseInfo(info).module ?? null, entries };
}

/** `name/arity` — the key prose is stored against. */
export function keyOf(e: { name: string; arity: number }): string {
  return `${e.name}/${e.arity}`;
}

/**
 * Attach line numbers and clause counts from the live outline, so clicking a
 * row can focus the code. Entries with no match (removed functions) keep
 * `line` undefined and render as non-clickable.
 */
export function withOutline(
  block: FunctionsBlock,
  functions: { name: string; arity: number; line: number; clauses: number }[],
): FunctionsBlock {
  const byKey = new Map(functions.map((f) => [keyOf(f), f]));
  return {
    ...block,
    entries: block.entries.map((e) => {
      const hit = byKey.get(keyOf(e));
      return hit ? { ...e, line: hit.line, clauses: hit.clauses } : e;
    }),
  };
}
