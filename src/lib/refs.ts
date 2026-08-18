// Recognising references in prose.
//
// There is deliberately no new syntax: inline code that happens to name
// something lgtm knows becomes a reference, and everything else stays ordinary
// inline code. That keeps the markdown portable — paste a reading into a PR
// comment and it still reads correctly, which would not survive inventing
// `{{create_user/1}}`.
//
// Two forms are recognised:
//
//   `create_user/1`   a function in this module (also `search/1..2`)
//   `L30-34`          a plain line range, for the bits that aren't definitions
//
// A name that *looks* like a signature but isn't in the file is a **dangling**
// reference. It renders struck through rather than silently falling back to
// plain text: the code moving out from under your explanation is exactly the
// thing you want to be told about.

import type { ModuleInfo } from "$lib/ipc";

export interface Ref {
  sig: string;
  start: number;
  end: number;
}

/** `create_user/1`, `search/1..2`, `valid?/1` — shaped like a signature. */
const SIG = /^[a-z_][A-Za-z0-9_]*[!?]?\/\d+(?:\.\.\d+)?$/;
/** `L30` or `L30-34`. */
const LINES = /^L(\d+)(?:-(\d+))?$/;

/** Does this inline code even claim to be a reference? */
export function looksLikeRef(text: string): boolean {
  return SIG.test(text) || LINES.test(text);
}

/**
 * Resolve a reference against the module. Returns null when the text isn't a
 * reference at all; a `Ref` when it resolves; and `"dangling"` when it looks
 * like one but names nothing in the file.
 */
export function resolveRef(text: string, module: ModuleInfo | null): Ref | "dangling" | null {
  const lines = LINES.exec(text);
  if (lines) {
    const start = parseInt(lines[1], 10);
    const end = lines[2] ? parseInt(lines[2], 10) : start;
    return start > 0 ? { sig: text, start, end: Math.max(end, start) } : "dangling";
  }

  if (!SIG.test(text)) return null;
  if (!module) return "dangling";

  const slash = text.lastIndexOf("/");
  const name = text.slice(0, slash);
  // `search/1..2` is one function; the top arity is its identity.
  const arity = parseInt(text.slice(slash + 1).split("..").pop() ?? "0", 10);

  const hit = module.functions.find((f) => f.name === name && f.arity === arity);
  if (!hit) return "dangling";

  // Every clause, plus the @doc and @spec above it — the same unit clicking a
  // function name selects.
  const ranges = hit.clauseRanges?.length
    ? hit.clauseRanges
    : [{ start: hit.line, end: hit.endLine }];
  const start = Math.min(
    ...ranges.map((r) => r.start),
    hit.specRange?.start ?? Infinity,
    hit.docRange?.start ?? Infinity,
  );
  const end = Math.max(...ranges.map((r) => r.end));
  return { sig: text, start, end };
}
