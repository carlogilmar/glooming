// Recognising references in prose.
//
// There is deliberately no new syntax: inline code that happens to name
// something lgtm knows becomes a reference, and everything else stays ordinary
// inline code. That keeps the markdown portable — paste a reading into a PR
// comment and it still reads correctly, which would not survive inventing
// `{{create_user/1}}`.
//
// Four forms are recognised:
//
//   `create_user/1`               a function in the reading (also `search/1..2`)
//   `MyApp.Billing.charge/2`      module-qualified, for a reading of several
//                                 files — which is how you would write it in
//                                 prose anyway, so it costs nothing to read
//   `L30-34`                      a plain line range, for the bits that aren't
//                                 definitions — case-insensitive, and `L30..34`
//                                 works too, since `..` is already the range
//                                 separator in `search/1..2`
//   `billing.ex:30-34`            the same, said out loud about another file
//
// A name that *looks* like a reference but isn't in the reading is **dangling**.
// It renders struck through rather than silently falling back to plain text: the
// code moving out from under your explanation — or the file leaving the reading —
// is exactly the thing you want to be told about.
//
// ---- why the resolver is stateful -------------------------------------------
//
// A reading covers several files, so "which file" has to be answered for every
// reference, and half of them do not say. An unqualified name means the file the
// prose is currently about, so references resolve **in document order**, each one
// threading the file forward:
//
//     Then `MyApp.Billing.charge/2` builds the invoice.  → billing.ex
//     `L25-29` is where it rounds.                       → still billing.ex
//
// That is how the prose already reads, and — this is the part that matters — it
// depends only on document order, never on which tab happens to be open. Keying
// off the current tab would make read mode walk a different path depending on
// where you were standing when you started scrolling.

import type { ReadingFile } from "$lib/ipc";
import { byPath, findByFilename, findFn, findQualified, origin } from "$lib/fileset";

export interface Ref {
  /** Bare `name/arity` — the focus identity, shared with rows and tiles. */
  sig: string;
  /** Which file of the reading this points into. */
  path: string;
  filename: string;
  start: number;
  end: number;
}

/** `create_user/1`, `search/1..2`, `valid?/1` — shaped like a signature. */
const SIG = /^[a-z_][A-Za-z0-9_]*[!?]?\/\d+(?:\.\.\d+)?$/;
/** `MyApp.Billing.charge/2` — a signature that names its own module. */
const QUALIFIED =
  /^([A-Z][A-Za-z0-9_]*(?:\.[A-Z][A-Za-z0-9_]*)*)\.([a-z_][A-Za-z0-9_]*[!?]?)\/(\d+(?:\.\.\d+)?)$/;
/**
 * `L30`, `L30-34`, `l30..34`.
 *
 * Case-insensitive because nobody reaches for the shift key mid-sentence, and
 * `..` is accepted alongside `-` because it is already how an arity range is
 * written in this same grammar.
 */
const LINES = /^[Ll](\d+)(?:\s*(?:-|\.\.)\s*(\d+))?$/;
/** `billing.ex:30-34`, `billing.ex:L30`. The `L` is optional after a filename. */
const FILE_LINES =
  /^([A-Za-z0-9_.\-]+\.exs?):[Ll]?(\d+)(?:\s*(?:-|\.\.)\s*(\d+))?$/;

/** Does this inline code even claim to be a reference? */
export function looksLikeRef(text: string): boolean {
  return SIG.test(text) || QUALIFIED.test(text) || LINES.test(text) || FILE_LINES.test(text);
}

/** `search/1..2` → the top arity is the identity. */
function arityOf(spec: string): number {
  return parseInt(spec.split("..").pop() ?? "0", 10);
}

/** The span a function reference selects: every clause, plus its @spec and @doc. */
function spanOf(fn: {
  line: number;
  endLine: number;
  clauseRanges?: { start: number; end: number }[] | null;
  specRange?: { start: number; end: number } | null;
  docRange?: { start: number; end: number } | null;
}): { start: number; end: number } {
  const ranges = fn.clauseRanges?.length ? fn.clauseRanges : [{ start: fn.line, end: fn.endLine }];
  return {
    start: Math.min(
      ...ranges.map((r) => r.start),
      fn.specRange?.start ?? Infinity,
      fn.docRange?.start ?? Infinity,
    ),
    end: Math.max(...ranges.map((r) => r.end)),
  };
}

function lineCountOf(file: ReadingFile): number {
  return file.source.split("\n").length;
}

function lineRef(
  file: ReadingFile,
  text: string,
  a: number,
  b: number,
): Ref | "dangling" {
  // `L15-9` is obvious enough to just honour rather than reject.
  const start = Math.min(a, b);
  const end = Math.max(a, b);
  const lines = lineCountOf(file);
  // Past the end of the file is the same failure as naming a deleted function:
  // the code moved under the explanation. Say so, rather than resolving to lines
  // that don't exist and silently doing nothing.
  if (start < 1) return "dangling";
  if (lines > 0 && start > lines) return "dangling";
  return {
    sig: text,
    path: file.path,
    filename: file.filename,
    start,
    end: lines > 0 ? Math.min(end, lines) : end,
  };
}

export interface Resolver {
  /** Resolve one reference, threading the current file forward. */
  resolve(text: string): Ref | "dangling" | null;
  /** Back to the reading's origin. Call once per render pass. */
  reset(): void;
  /** The file the prose is currently about — exposed for tests. */
  readonly current: string | null;
}

/**
 * A resolver over one reading. Stateful by design (see the note at the top);
 * `reset()` puts the thread back to the origin at the start of a render.
 */
export function refResolver(files: ReadingFile[]): Resolver {
  const home = () => origin(files)?.path ?? null;
  let current: string | null = home();

  return {
    get current() {
      return current;
    },

    reset() {
      current = home();
    },

    resolve(text: string): Ref | "dangling" | null {
      if (!files.length) return null;

      // `billing.ex:30-34` — names its file, so it also moves the thread.
      const fileLines = FILE_LINES.exec(text);
      if (fileLines) {
        const file = findByFilename(files, fileLines[1]);
        if (!file) return "dangling";
        current = file.path;
        const a = parseInt(fileLines[2], 10);
        return lineRef(file, text, a, fileLines[3] ? parseInt(fileLines[3], 10) : a);
      }

      // `MyApp.Billing.charge/2` — names its module, so it also moves the thread.
      const qual = QUALIFIED.exec(text);
      if (qual) {
        const hit = findQualified(files, qual[1], qual[2], arityOf(qual[3]));
        if (!hit) return "dangling";
        current = hit.file.path;
        const span = spanOf(hit.fn);
        return {
          sig: `${qual[2]}/${qual[3]}`,
          path: hit.file.path,
          filename: hit.file.filename,
          ...span,
        };
      }

      // `L30-34` — inherits whichever file the prose is currently about.
      const lines = LINES.exec(text);
      if (lines) {
        const file = byPath(files, current) ?? origin(files);
        if (!file) return "dangling";
        const a = parseInt(lines[1], 10);
        return lineRef(file, text, a, lines[2] ? parseInt(lines[2], 10) : a);
      }

      // `charge/2` — the current file first, then the origin, then the rest.
      if (!SIG.test(text)) return null;
      const slash = text.lastIndexOf("/");
      const hit = findFn(files, current, text.slice(0, slash), arityOf(text.slice(slash + 1)));
      if (!hit) return "dangling";
      current = hit.file.path;
      return {
        sig: text,
        path: hit.file.path,
        filename: hit.file.filename,
        ...spanOf(hit.fn),
      };
    },
  };
}
