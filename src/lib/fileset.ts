// Which file owns what, across a whole project.
//
// A gloom no longer has a set of files. The project is what you explore, and two
// things stand in for the set the old reading kept in rows:
//
//   the index    every module in the project and the file it is defined in —
//                names only, from Rust's line scan, rebuilt on open and focus
//   the loaded   the files actually read this session, with their outlines: the
//                one on screen, the ones you walked through, and the ones your
//                prose names. A cache of live reads, never a record of anything
//
// The index answers "which module does `Pipeline` mean" — across the codebase,
// which is the only place that question has an honest answer. The loaded files
// answer "where is `run/1` in it". A reference needs both.

import type { FnInfo, IndexedModule, ModuleInfo, Outline, ReadingFile } from "$lib/ipc";
import { displaySig } from "$lib/select";

export function byPath(files: ReadingFile[], path: string | null): ReadingFile | null {
  if (!path) return null;
  return files.find((f) => f.path === path) ?? null;
}

/** The first module of a file, which is the one its views are about. */
export function moduleOf(file: ReadingFile | null): ModuleInfo | null {
  return file?.outline?.modules?.[0] ?? null;
}

/** The loaded file whose module is named exactly `module`. */
export function fileForModule(files: ReadingFile[], module: string): ReadingFile | null {
  if (!module) return null;
  return files.find((f) => f.outline?.modules?.some((m) => m.name === module)) ?? null;
}

export function moduleForName(files: ReadingFile[], module: string): ModuleInfo | null {
  for (const f of files) {
    const hit = f.outline?.modules?.find((m) => m.name === module);
    if (hit) return hit;
  }
  return null;
}

/** `ImpactPipeline.Shared.AlertImpact.SingleTarget` → `SingleTarget`. */
export function shortModule(name: string): string {
  const i = name.lastIndexOf(".");
  return i === -1 ? name : name.slice(i + 1);
}

/**
 * Every module name known: the project's index, plus anything loaded from
 * outside it (a file opened with ⌘⇧O has modules the index never saw).
 */
export function knownModules(index: IndexedModule[], files: ReadingFile[]): string[] {
  const out = new Set(index.map((m) => m.module));
  for (const f of files) for (const m of f.outline?.modules ?? []) out.add(m.name);
  return [...out];
}

/**
 * Which modules a name written in prose could mean.
 *
 * An exact name wins outright. Otherwise every module whose name ends in it at a
 * **dot boundary** — so `Pipeline` matches `Weather.Pipeline` and `Ai.Pipeline`
 * but never `ProcessingPipeline`, and `Weather.Pipeline` matches only the one.
 * More than one answer is *ambiguous*, which the prose shows rather than
 * guesses: across a whole codebase, `Pipeline` is the normal case, not an edge.
 */
export function matchModules(known: string[], given: string): string[] {
  if (known.includes(given)) return [given];
  return known.filter((m) => m.endsWith(`.${given}`));
}

/**
 * How each module is *shown*: the shortest dot-suffix that names it uniquely in
 * the project.
 *
 * `MyApp.Billing` shows as `Billing`; with both `Weather.Pipeline` and
 * `Ai.Pipeline` in the codebase, each shows as itself, because `Pipeline` alone
 * would be two things. The text always stores the full name — this is display
 * only, so a note stays exact on disk and short on screen.
 */
export function moduleLabels(known: string[]): Map<string, string> {
  // How many modules each dot-suffix could mean. Counted once rather than asking
  // `matchModules` per candidate, which is quadratic in a project of two thousand
  // modules. A suffix only one module carries is exactly one `matchModules` would
  // resolve to that module: if it were another module's full name, that module
  // would carry it too.
  const count = new Map<string, number>();
  const suffixes = (m: string) => {
    const parts = m.split(".");
    return parts.map((_, i) => parts.slice(i).join("."));
  };
  for (const m of known) for (const s of suffixes(m)) count.set(s, (count.get(s) ?? 0) + 1);

  const out = new Map<string, string>();
  for (const m of known) {
    // Shortest first; the full name is the fallback and is always unique.
    const label = suffixes(m).reverse().find((s) => count.get(s) === 1) ?? m;
    out.set(m, label);
  }
  return out;
}

/**
 * The files the prose names, by qualified reference or module reference, that
 * are not loaded yet.
 *
 * A reference to `Weather.Station.read/1` cannot resolve until Weather.Station's
 * outline is here, and nobody may have opened it this session. So the shell asks
 * this after every edit and loads what is missing; the note re-renders when they
 * arrive. Bare names never need it — they live in a module a qualified reference
 * already named.
 */
export function namedPaths(markdown: string, index: IndexedModule[], files: ReadingFile[]): string[] {
  const known = index.map((m) => m.module);
  const loaded = new Set(files.map((f) => f.path));
  const want = new Set<string>();
  for (const [, inner] of markdown.matchAll(/`([^`\n]+)`/g)) {
    const text = inner.trim();
    const m = /^([A-Z][A-Za-z0-9_]*(?:\.[A-Z][A-Za-z0-9_]*)*)(?:\.[a-z_][A-Za-z0-9_]*[!?]?(?:\/\d+)?(?::\d+(?:-\d+)?)?)?$/.exec(text);
    if (!m) continue;
    const hits = matchModules(known, m[1]);
    if (hits.length !== 1) continue;
    const path = index.find((e) => e.module === hits[0])?.path;
    if (path && !loaded.has(path)) want.add(path);
  }
  return [...want];
}

// ---- the `/` menu's vocabulary ---------------------------------------------

export interface VocabEntry {
  /** What gets inserted — always the full module name, never a short form. */
  insert: string;
  /** `create_user/1` — what you scan for. */
  sig: string;
  module: string;
  /** How the module is shown — the shortest unique name in the project. */
  label: string;
  path: string;
  filename: string;
  line: number;
  visibility: "public" | "private";
  /**
   * This function is in the file currently on screen. Used only to rank the
   * menu — you are usually writing about what you are looking at — and never to
   * decide what gets inserted.
   */
  nearby: boolean;
}

/**
 * Everything the `/` menu can insert: every function in every file loaded this
 * session — the one on screen, the ones you walked through, the ones your prose
 * already names. You write about what you have read, so that is the set.
 *
 * **The insert is the full module name. Always.** `MyApp.Billing.to_cents`, never
 * `Billing.to_cents`: a short form that is unique today becomes ambiguous the day
 * someone adds another `Billing`, and a note is meant to last longer than that.
 * The chip shows the short form anyway — see `moduleLabels` — so the prose is
 * exact in the text and readable on screen.
 *
 * It carries **no arity**: an arity is a narrowing to one member of a family you
 * usually mean all of. Type it by hand when you mean exactly one.
 */
export function vocabulary(
  files: ReadingFile[],
  currentPath: string | null,
  labels: Map<string, string>,
): VocabEntry[] {
  const out: VocabEntry[] = [];
  for (const file of files) {
    for (const module of file.outline?.modules ?? []) {
      const label = labels.get(module.name) ?? module.name;
      for (const fn of module.functions) {
        out.push({
          sig: displaySig(fn),
          insert: `${module.name}.${fn.name}`,
          module: module.name,
          label,
          path: file.path,
          filename: file.filename,
          line: fn.line,
          visibility: fn.visibility,
          nearby: file.path === currentPath,
        });
      }
    }
  }
  return out;
}

/** Is there anything at all to reference? Gates read mode and the `/` menu. */
export function hasModules(files: ReadingFile[]): boolean {
  return files.some((f) => (f.outline?.modules?.length ?? 0) > 0);
}

export interface Hit {
  file: ReadingFile;
  module: ModuleInfo;
  fns: FnInfo[];
}

/** Every arity of `name` in a loaded module, or exactly `arity` when given. */
export function inModule(
  files: ReadingFile[],
  module: string,
  name: string,
  arity: number | null,
): Hit | null {
  const file = fileForModule(files, module);
  const mod = file ? moduleForName([file], module) : null;
  if (!file || !mod) return null;
  const fns = mod.functions.filter((f) => f.name === name && (arity === null || f.arity === arity));
  return fns.length ? { file, module: mod, fns } : null;
}

export type { Outline };
