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
//                     — case-insensitive, and `L30..34` works too, since `..` is
//                     already the range separator in `search/1..2`
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
/**
 * `L30`, `L30-34`, `l30..34`.
 *
 * Case-insensitive because nobody reaches for the shift key mid-sentence, and
 * `..` is accepted alongside `-` because it is already how an arity range is
 * written in this same grammar.
 */
const LINES = /^[Ll](\d+)(?:\s*(?:-|\.\.)\s*(\d+))?$/;

/** Does this inline code even claim to be a reference? */
export function looksLikeRef(text: string): boolean {
  return SIG.test(text) || LINES.test(text);
}

/**
 * Resolve a reference against the module. Returns null when the text isn't a
 * reference at all; a `Ref` when it resolves; and `"dangling"` when it looks
 * like one but names nothing in the file.
 */
export function resolveRef(
  text: string,
  module: ModuleInfo | null,
  lineCount = 0,
): Ref | "dangling" | null {
  const lines = LINES.exec(text);
  if (lines) {
    const a = parseInt(lines[1], 10);
    const b = lines[2] ? parseInt(lines[2], 10) : a;
    // `L15-9` is obvious enough to just honour rather than reject.
    const start = Math.min(a, b);
    const end = Math.max(a, b);
    // Past the end of the file is the same failure as naming a deleted
    // function: the code moved under the explanation. Say so, rather than
    // resolving to lines that don't exist and silently doing nothing.
    if (start < 1) return "dangling";
    if (lineCount > 0 && start > lineCount) return "dangling";
    return { sig: text, start, end: lineCount > 0 ? Math.min(end, lineCount) : end };
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
