// Recognising references in prose.
//
// There is deliberately no new syntax: inline code that happens to name
// something lgtm knows becomes a reference, and everything else stays ordinary
// inline code. That keeps the markdown portable — paste a reading into a PR
// comment and it still reads correctly, which would not survive inventing
// `{{create_user/1}}`.
//
// The forms recognised, and **the arity is optional**:
//
//   `MyApp.Billing.to_cents`      module-qualified — the whole family, every
//                                 arity. This is what the `/` menu inserts, and
//                                 it is how you would write it in prose anyway
//   `MyApp.Billing.to_cents/1`    the same, narrowed to exactly one arity
//   `to_cents`                    the module the prose is currently about
//   `to_cents/1`                  the same, narrowed
//   `L30-34`                      a plain line range, for the bits that aren't
//                                 definitions — case-insensitive, and `L30..34`
//                                 works too
//   `billing.ex:30-34`            the same, said out loud about another file
//
// **A reference without an arity means every arity**, because that is already how
// selection thinks: the focus store tints sibling arities as "related" on the
// grounds that they are one function to a reader, even though the BEAM treats
// them as separate. So `get_user` selects `get_user/1` and `get_user/2` together,
// and writing `get_user/1` is the narrowing. It also retires `search/1..2`, which
// was never a readable thing to have in the middle of a sentence.
//
// ---- what dangles, and what quietly stays prose -----------------------------
//
// A name that *looks* like a reference but isn't in the reading is **dangling**:
// it renders struck through rather than falling back to plain text, because the
// code moving out from under your explanation is exactly the thing you want to be
// told about.
//
// A **qualified name whose module is not in the reading** is not a reference at
// all — `String.trim` is prose about the standard library, not a broken link —
// and a **bare name with no arity** is the other exception. Prose
// about Elixir is full of lowercase words in backticks that are not functions —
// `attrs`, `opts`, `conn`, `config`, `path` — so treating an unresolved one as a
// broken reference would strike through half of what you write. A bare name
// therefore resolves or stays ordinary inline code, and cannot warn you that the
// code moved. The three explicit forms (qualified, or carrying an arity) are
// unmistakably meant as references, so they all still dangle visibly.
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
import {
  byPath,
  findByFilename,
  findByName,
  findFn,
  findQualified,
  findQualifiedByName,
  knowsModule,
  origin,
} from "$lib/fileset";

export interface Ref {
  /** Bare `name/arity` — the focus identity, shared with rows and tiles. */
  sig: string;
  /** Which file of the reading this points into. */
  path: string;
  filename: string;
  /**
   * Every span this reference selects — one per arity, each covering that
   * arity's clauses plus its @spec and @doc.
   *
   * Several spans rather than one enclosing range, because `get_user/1` and
   * `get_user/2` can sit either side of an unrelated function, and a single
   * min..max range would highlight that function too.
   */
  ranges: { start: number; end: number }[];
  /** First line of the first span — the scroll target. */
  start: number;
  /** Last line of the last span, for the "reading N lines" pill. */
  end: number;
}

/** `create_user`, `valid?` — a function name on its own. */
const NAME = /^[a-z_][A-Za-z0-9_]*[!?]?$/;
/** `create_user/1`, `search/1..2`, `valid?/1` — a name with an arity. */
const SIG = /^([a-z_][A-Za-z0-9_]*[!?]?)\/(\d+(?:\.\.\d+)?)$/;
/**
 * `MyApp.Billing.charge` and `MyApp.Billing.charge/2` — a name that carries its
 * own module, with the arity optional.
 */
const QUALIFIED =
  /^([A-Z][A-Za-z0-9_]*(?:\.[A-Z][A-Za-z0-9_]*)*)\.([a-z_][A-Za-z0-9_]*[!?]?)(?:\/(\d+(?:\.\.\d+)?))?$/;
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

/**
 * Does this inline code even claim to be a reference?
 *
 * `NAME` is in here so a bare `to_cents` gets *offered* to the resolver — but a
 * bare name that resolves to nothing comes back as `null`, not `"dangling"`, so
 * `attrs` and `opts` stay ordinary inline code. See the note at the top.
 */
export function looksLikeRef(text: string): boolean {
  return (
    NAME.test(text) ||
    SIG.test(text) ||
    QUALIFIED.test(text) ||
    LINES.test(text) ||
    FILE_LINES.test(text)
  );
}

/** `search/1..2` → the top arity is the identity. */
function arityOf(spec: string): number {
  return parseInt(spec.split("..").pop() ?? "0", 10);
}

interface FnLike {
  line: number;
  endLine: number;
  clauseRanges?: { start: number; end: number }[] | null;
  specRange?: { start: number; end: number } | null;
  docRange?: { start: number; end: number } | null;
}

/** The span one arity selects: every clause, plus its @spec and @doc. */
function spanOf(fn: FnLike): { start: number; end: number } {
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

/** One reference, several arities: a span each, in source order. */
function spansOf(fns: FnLike[]): Pick<Ref, "ranges" | "start" | "end"> {
  const ranges = fns.map(spanOf).sort((a, b) => a.start - b.start);
  return {
    ranges,
    start: ranges[0].start,
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
  const to = lines > 0 ? Math.min(end, lines) : end;
  return {
    sig: text,
    path: file.path,
    filename: file.filename,
    ranges: [{ start, end: to }],
    start,
    end: to,
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

      // `MyApp.Billing.charge` — names its module, so it also moves the thread.
      // The arity is optional; without one it means every arity of that name.
      const qual = QUALIFIED.exec(text);
      if (qual) {
        const [, mod, name, arity] = qual;
        // `String.trim`, `Enum.map`, `GenServer.call` — prose about code outside
        // the reading, not a broken reference to code inside it. Only a module
        // that is actually one of your files may render as struck through.
        //
        // The trade: remove a file from the reading and its references go quiet
        // rather than breaking loudly. That case has its own signals — the ×
        // spells out what it does, and the strip shows the set — whereas a
        // struck-through `String.trim` has none and just looks like a bug.
        if (!knowsModule(files, mod)) return null;
        if (arity === undefined) {
          const hit = findQualifiedByName(files, mod, name);
          if (!hit) return "dangling";
          current = hit.file.path;
          return {
            // The identity is bare `name/arity` so a chip and a table row can
            // still recognise each other. With several arities the top one wins,
            // which is the same identity rule the outline uses.
            sig: `${name}/${Math.max(...hit.fns.map((f) => f.arity))}`,
            path: hit.file.path,
            filename: hit.file.filename,
            ...spansOf(hit.fns),
          };
        }
        const hit = findQualified(files, mod, name, arityOf(arity));
        if (!hit) return "dangling";
        current = hit.file.path;
        return {
          sig: `${name}/${arity}`,
          path: hit.file.path,
          filename: hit.file.filename,
          ...spansOf([hit.fn]),
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
      const sig = SIG.exec(text);
      if (sig) {
        const hit = findFn(files, current, sig[1], arityOf(sig[2]));
        if (!hit) return "dangling";
        current = hit.file.path;
        return {
          sig: text,
          path: hit.file.path,
          filename: hit.file.filename,
          ...spansOf([hit.fn]),
        };
      }

      // `charge` — every arity, in the module the prose is currently about.
      //
      // Returns **null** rather than "dangling" when it finds nothing, and that
      // asymmetry is deliberate: this is the one form that overlaps with ordinary
      // prose, so an unresolved one has to stay ordinary prose. `attrs` and
      // `opts` must not strike through.
      if (!NAME.test(text)) return null;
      const family = findByName(files, current, text);
      if (!family) return null;
      current = family.file.path;
      return {
        sig: `${text}/${Math.max(...family.fns.map((f) => f.arity))}`,
        path: family.file.path,
        filename: family.file.filename,
        ...spansOf(family.fns),
      };
    },
  };
}
