// Recognising references in prose.
//
// There is deliberately no new syntax: inline code that happens to name
// something in the project becomes a reference, and everything else stays
// ordinary inline code. That keeps the markdown portable — paste a gloom into a
// PR comment and it still reads correctly, which would not survive inventing
// `{{create_user/1}}`.
//
// **Every reference is looked up by name, every render.** A gloom holds no copy
// of the code, so a reference is a question asked of the code as it is now —
// which is what lets it follow a function that moved, and say so when it is
// gone. That is the whole reason there are no plain line ranges any more: `L25`
// cannot follow anything and cannot tell you it broke.
//
//   `MyApp.Billing.to_cents`        every arity — what the `/` menu inserts
//   `MyApp.Billing.to_cents/1`      the same, narrowed to exactly one arity
//   `MyApp.Billing.charge/2:3-6`    lines 3–6 of that function, counted from its
//                                   first line — an edit elsewhere in the file
//                                   cannot move it
//   `Weather.Pipeline`              the module itself
//
// **A reference always names its module.** A bare `to_cents/1` is ordinary
// inline code. It used to resolve against the module the prose was "threading"
// — the last one a reference had named — which read well and carried no
// information: `enabled?/2` alone does not say which file it is in, moving a
// paragraph silently re-pointed it, and across a codebase `changeset/1` exists in
// thirty modules. The full name is the only form that says where it goes and
// keeps saying it.
//
// The module part may be any dot-suffix — `Billing.charge` is fine while
// `Billing` is unique in the project. When it is not, the reference is
// **ambiguous**, which is a third state beside resolved and broken: it is not
// wrong, it is underspecified, and the reader is offered the candidates rather
// than a guess.
//
// ---- what breaks, and what quietly stays prose ------------------------------
//
// A **qualified name whose module is not in the project** is not a reference at
// all — `String.trim` is prose about the standard library, not a broken link.
// A missing function in a module the project *has*, or lines past the end of the
// function, renders struck through, because the code moving out from under your
// explanation is exactly what you want to be told.
//
// Resolution depends on nothing but the reference's own text — no order, no
// thread, no file on screen — so read mode walks the same path wherever you stand.

import type { ReadingFile } from "$lib/ipc";
import { fileForModule, inModule, matchModules, moduleForName } from "$lib/fileset";

export interface Ref {
  /** Bare `name/arity` — the focus identity, shared with rows. */
  sig: string;
  /** Which loaded file this points into. */
  path: string;
  filename: string;
  /** What the chip shows: the module at its shortest unique name. */
  label: string;
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
  /** A reference to the module itself rather than to something in it. */
  module?: boolean;
}

export interface Ambiguous {
  ambiguous: true;
  /** Every module the written name could mean, full names. */
  candidates: string[];
  /** The module part as written, so a choice can replace exactly that. */
  written: string;
}

export type Resolution = Ref | Ambiguous | "dangling" | null;

const NAME = "[a-z_][A-Za-z0-9_]*[!?]?";
const MOD = "[A-Z][A-Za-z0-9_]*(?:\\.[A-Z][A-Za-z0-9_]*)*";
/** `:3` or `:3-6` — lines inside the function. */
const LINES = "(?::(\\d+)(?:-(\\d+))?)?";

/** `MyApp.Billing.charge`, with the arity and the lines optional. */
const QUALIFIED = new RegExp(`^(${MOD})\\.(${NAME})(?:\\/(\\d+))?${LINES}$`);
/** `Weather.Pipeline`. */
const MODULE = new RegExp(`^${MOD}$`);

/** Does this inline code even claim to be a reference? */
export function looksLikeRef(text: string): boolean {
  return QUALIFIED.test(text) || MODULE.test(text);
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

/**
 * Lines `a`–`b` of a function, counted from its first line.
 *
 * Counted from the `def`, not from the file, so an edit anywhere else cannot move
 * them. Past the function's last line is the one way they can still break — the
 * function got shorter under your explanation — and that dangles rather than
 * quietly highlighting whatever comes next.
 */
function linesIn(fns: FnLike[], a: number, b: number): Pick<Ref, "ranges" | "start" | "end"> | null {
  const first = Math.min(...fns.map((f) => f.line));
  const last = Math.max(...fns.map((f) => spanOf(f).end));
  const lo = Math.min(a, b);
  const hi = Math.max(a, b);
  const start = first + lo - 1;
  const end = first + hi - 1;
  if (lo < 1 || end > last) return null;
  return { ranges: [{ start, end }], start, end };
}

export interface Resolver {
  /** Resolve one reference. Depends on nothing but its text. */
  resolve(text: string): Resolution;
}

/**
 * A resolver over the project. `known` is every module name in the project;
 * `files` the ones loaded, whose outlines say where things are; `labels` how each
 * module is shown.
 */
export function refResolver(
  files: ReadingFile[],
  known: string[],
  labels: Map<string, string>,
): Resolver {
  const label = (m: string) => labels.get(m) ?? m;

  /** A qualified name's module part, or the reason it is not one. */
  function moduleFor(written: string): string | Ambiguous | null {
    const hits = matchModules(known, written);
    if (hits.length > 1) return { ambiguous: true, candidates: hits, written };
    return hits[0] ?? null;
  }

  function fnRef(
    module: string,
    name: string,
    arity: string | undefined,
    a: string | undefined,
    b: string | undefined,
  ): Resolution {
    // In the project but not loaded yet: the shell is fetching it, and the note
    // re-renders when it lands. Plain until then, never struck through — "not
    // read yet" is not "gone".
    if (!fileForModule(files, module)) return null;
    const hit = inModule(files, module, name, arity === undefined ? null : parseInt(arity, 10));
    if (!hit) return "dangling";

    const top = Math.max(...hit.fns.map((f) => f.arity));
    const sig = arity === undefined ? `${name}/${top}` : `${name}/${arity}`;
    const lines = a ? `:${a}${b ? `-${b}` : ""}` : "";
    const shown = `${label(module)}.${name}${arity === undefined ? "" : `/${arity}`}${lines}`;

    const spans = a ? linesIn(hit.fns, parseInt(a, 10), parseInt(b ?? a, 10)) : spansOf(hit.fns);
    if (!spans) return "dangling";
    return { sig, path: hit.file.path, filename: hit.file.filename, label: shown, ...spans };
  }

  return {
    resolve(text: string): Resolution {
      const qual = QUALIFIED.exec(text);
      if (qual) {
        const [, written, name, arity, a, b] = qual;
        const module = moduleFor(written);
        // `String.trim`, `Enum.map` — prose about code outside the project.
        if (module === null) return null;
        if (typeof module !== "string") return module;
        return fnRef(module, name, arity, a, b);
      }

      if (MODULE.test(text)) {
        const module = moduleFor(text);
        if (module === null) return null;
        if (typeof module !== "string") return module;
        const file = fileForModule(files, module);
        const info = file ? moduleForName([file], module) : null;
        if (!file || !info) return null;
        return {
          sig: module,
          path: file.path,
          filename: file.filename,
          label: label(module),
          ranges: [{ start: info.line, end: info.line }],
          start: info.line,
          end: info.line,
          module: true,
        };
      }

      // A bare name — `enabled?/2`, `opts` — is ordinary inline code.
      return null;
    },
  };
}
