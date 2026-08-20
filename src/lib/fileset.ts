// The set of files one reading covers.
//
// A reading used to be one file, so "the outline" was unambiguous. Now there are
// several, and three questions need answering in one place rather than being
// re-derived in every component:
//
//   which file owns this module?      — `lgtm:functions module=X` and a
//                                       qualified reference both ask this
//   which file owns this function?     — a bare `charge/2` in prose
//   what can I reference?              — the `/` menu's vocabulary
//
// None of this is stateful and none of it is cached: a reading has a handful of
// files, and walking them costs nothing next to the parse that produced them.

import type { FnInfo, ModuleInfo, Outline, ReadingFile } from "$lib/ipc";
import { displaySig } from "$lib/select";

/** The origin is the file the doc was seeded from — the reading's anchor. */
export function origin(files: ReadingFile[]): ReadingFile | null {
  return files.find((f) => f.origin) ?? files[0] ?? null;
}

export function byPath(files: ReadingFile[], path: string | null): ReadingFile | null {
  if (!path) return null;
  return files.find((f) => f.path === path) ?? null;
}

/** The first module of a file, which is the one its doc blocks are about. */
export function moduleOf(file: ReadingFile | null): ModuleInfo | null {
  return file?.outline?.modules?.[0] ?? null;
}

/**
 * The file whose module is named `module`.
 *
 * This is what makes `lgtm:functions module=MyApp.Accounts` keep working in a
 * multi-file reading: the block already carries the module it is about, so the
 * right outline can be found rather than assumed to be the current tab's.
 */
export function fileForModule(files: ReadingFile[], module: string): ReadingFile | null {
  if (!module) return null;
  return files.find((f) => f.outline?.modules?.some((m) => m.name === module)) ?? null;
}

/** `ImpactPipeline.Shared.AlertImpact.SingleTarget` → `SingleTarget`. */
export function shortModule(name: string): string {
  const i = name.lastIndexOf(".");
  return i === -1 ? name : name.slice(i + 1);
}

/**
 * How each module should be *written* in this reading.
 *
 * The last segment is the module's name; everything before it is where the file
 * lives. `ImpactPipeline.Shared.AlertImpact.SingleTarget.foo` in the middle of a
 * sentence is unreadable, and the prefix is the same for every module in the
 * reading anyway — so it carries no information exactly where it costs the most.
 *
 * **Shortened only where the short form is unique.** Two files whose modules both
 * end in `.Worker` would give `Worker.run` two meanings, so those keep their full
 * names. Ambiguity is rare; silently resolving it the wrong way would not be.
 */
export function moduleLabels(files: ReadingFile[]): Map<string, string> {
  const all = modulesIn(files);
  const taken = new Map<string, number>();
  for (const m of all) {
    const short = shortModule(m);
    taken.set(short, (taken.get(short) ?? 0) + 1);
  }
  const out = new Map<string, string>();
  for (const m of all) {
    const short = shortModule(m);
    out.set(m, taken.get(short) === 1 ? short : m);
  }
  return out;
}

/**
 * Find a module by whatever the prose called it.
 *
 * Exact name first, then any dot-boundary suffix — so `SingleTarget`,
 * `AlertImpact.SingleTarget` and the full path all reach the same module, and
 * writing as much of it as you feel like is enough. The dot in the suffix test is
 * what stops `Target` matching `SingleTarget`.
 */
export function findModule(
  files: ReadingFile[],
  given: string,
): { file: ReadingFile; module: ModuleInfo } | null {
  for (const file of files) {
    for (const m of file.outline?.modules ?? []) {
      if (m.name === given) return { file, module: m };
    }
  }
  for (const file of files) {
    for (const m of file.outline?.modules ?? []) {
      if (m.name.endsWith(`.${given}`)) return { file, module: m };
    }
  }
  return null;
}

/**
 * Is this module one of the reading's own?
 *
 * The gate on whether a qualified reference may render as broken. `String.trim`
 * and `Enum.map` are things you write in prose about Elixir, and once the arity
 * became optional they started matching the qualified form — so without this,
 * every mention of the standard library struck itself through.
 */
export function knowsModule(files: ReadingFile[], module: string): boolean {
  return !!findModule(files, module);
}

export function moduleForName(files: ReadingFile[], module: string): ModuleInfo | null {
  for (const f of files) {
    const hit = f.outline?.modules?.find((m) => m.name === module);
    if (hit) return hit;
  }
  return null;
}

/**
 * Search order for an *unqualified* name.
 *
 * `from` first, then the origin, then everything else in strip order. That
 * ordering is not arbitrary: prose about billing.ex saying `to_cents/1` means
 * billing's, and prose that has not named a file yet means the file the reading
 * started from. Everything after that is a fallback so a bare name reaches a
 * function that exists somewhere rather than dangling.
 *
 * Crucially it never depends on **which tab happens to be open**, or read mode
 * would walk a different path depending on where you were standing when you
 * started scrolling.
 */
export function searchOrder(files: ReadingFile[], from: string | null): ReadingFile[] {
  const out: ReadingFile[] = [];
  const push = (f: ReadingFile | null) => {
    if (f && !out.includes(f)) out.push(f);
  };
  push(byPath(files, from));
  push(origin(files));
  for (const f of files) push(f);
  return out;
}

export interface Hit {
  file: ReadingFile;
  module: ModuleInfo;
  fn: FnInfo;
}

/** Find `name/arity` in a specific module. */
function inModule(module: ModuleInfo, name: string, arity: number): FnInfo | null {
  return module.functions.find((f) => f.name === name && f.arity === arity) ?? null;
}

/**
 * Every arity of `name` in a module.
 *
 * A reference without an arity means the whole family, which is how the focus
 * store has always thought about it: sibling arities are one function to a
 * reader, even though the BEAM treats them as separate. So `get_user` selects
 * both `get_user/1` and `get_user/2`, and `get_user/1` is the narrowing.
 */
function allInModule(module: ModuleInfo, name: string): FnInfo[] {
  return module.functions.filter((f) => f.name === name);
}

/** Find `name/arity` anywhere in the reading, in `searchOrder`. */
export function findFn(
  files: ReadingFile[],
  from: string | null,
  name: string,
  arity: number,
): Hit | null {
  for (const file of searchOrder(files, from)) {
    for (const module of file.outline?.modules ?? []) {
      const fn = inModule(module, name, arity);
      if (fn) return { file, module, fn };
    }
  }
  return null;
}

/** Find `Module.name/arity`, which names its file itself. */
export function findQualified(
  files: ReadingFile[],
  module: string,
  name: string,
  arity: number,
): Hit | null {
  const at = findModule(files, module);
  if (!at) return null;
  const fn = inModule(at.module, name, arity);
  return fn ? { file: at.file, module: at.module, fn } : null;
}

/** Every arity of a bare `name`, searched in `searchOrder`. */
export function findByName(
  files: ReadingFile[],
  from: string | null,
  name: string,
): { file: ReadingFile; module: ModuleInfo; fns: FnInfo[] } | null {
  for (const file of searchOrder(files, from)) {
    for (const module of file.outline?.modules ?? []) {
      const fns = allInModule(module, name);
      if (fns.length) return { file, module, fns };
    }
  }
  return null;
}

/** Every arity of `Module.name`, which names its own file. */
export function findQualifiedByName(
  files: ReadingFile[],
  module: string,
  name: string,
): { file: ReadingFile; module: ModuleInfo; fns: FnInfo[] } | null {
  const at = findModule(files, module);
  if (!at) return null;
  const fns = allInModule(at.module, name);
  return fns.length ? { file: at.file, module: at.module, fns } : null;
}

/** A file named by its basename, for `billing.ex:30-34`. */
export function findByFilename(files: ReadingFile[], name: string): ReadingFile | null {
  const want = name.toLowerCase();
  return (
    files.find((f) => f.filename.toLowerCase() === want) ??
    files.find((f) => f.filename.toLowerCase().startsWith(want)) ??
    null
  );
}

// ---- the `/` menu's vocabulary ---------------------------------------------

export interface VocabEntry {
  /** What gets inserted: bare for the home module, qualified for any other. */
  insert: string;
  /** `create_user/1` — what you scan for. */
  sig: string;
  module: string;
  /** How the module is written in prose here — usually just its own name. */
  label: string;
  path: string;
  filename: string;
  line: number;
  visibility: "public" | "private";
  /**
   * This function's module is the reading's home module, so a bare name is
   * unambiguous. Decided by module, never by which tab is open.
   */
  home: boolean;
  /**
   * This function is in the file currently on screen. Used only to rank the
   * menu — you are usually writing about what you are looking at — and never to
   * decide what gets inserted.
   */
  nearby: boolean;
}

/**
 * Everything referenceable in the reading, grouped by module in strip order.
 *
 * This is the *only* thing adding a file changes. There is no seeding, no new
 * block and no edit to your prose — an added file simply widens what `/` can
 * offer you, which is exactly what a review needs and no more.
 *
 * **Once a reading covers more than one module, every reference names its module.**
 * Not just the ones from other files — all of them. A note that says
 * `MyApp.Accounts.create_user` in one paragraph and a bare `charge` in the next
 * makes you work out which file each belongs to, and the whole point of the
 * qualified form is not having to. Uniform reads better than minimal here, and it
 * means every reference in a multi-file note is unambiguous standing on its own —
 * which matters most where the markdown ends up pasted into a PR comment, with no
 * file strip next to it to explain itself.
 *
 * A reading of **one** module stays bare. There is nothing to disambiguate, the
 * doc's title already is the module name, and repeating it down twenty paragraphs
 * is noise.
 *
 * The discriminator is the *module count*, never the tab that happens to be open.
 * An earlier version keyed off the open tab, so looking at billing.ex and picking
 * `charge/2` inserted a bare name: the same keystroke produced different text
 * depending on where you were standing, and it emitted references whose meaning
 * depended on prose order you cannot see while typing.
 *
 * What gets inserted carries **no arity**: `MyApp.Billing.to_cents`, not
 * `MyApp.Billing.to_cents/1`. An arity is a narrowing to one member of a family
 * you usually mean all of, and `search/1..2` was never a readable thing to have
 * in a sentence.
 */
export function vocabulary(files: ReadingFile[], currentPath: string | null): VocabEntry[] {
  const home = moduleOf(origin(files))?.name ?? null;
  // More than one module in the reading means a bare name could belong to any of
  // them, so from that point on every reference says which.
  const qualifyAll = modulesIn(files).length > 1;
  const labels = moduleLabels(files);
  const out: VocabEntry[] = [];
  for (const file of files) {
    for (const module of file.outline?.modules ?? []) {
      const isHome = !!home && module.name === home;
      // The module's own name, not the path to it — `SingleTarget.foo`.
      const label = labels.get(module.name) ?? module.name;
      for (const fn of module.functions) {
        const sig = displaySig(fn);
        out.push({
          sig,
          // Name-only, because an arity is a narrowing you rarely want: `/1..2`
          // was never readable in prose, and sibling arities are one function to
          // a reader anyway. Type the arity by hand when you mean exactly one.
          insert: isHome && !qualifyAll ? fn.name : `${label}.${fn.name}`,
          module: module.name,
          label,
          path: file.path,
          filename: file.filename,
          line: fn.line,
          visibility: fn.visibility,
          home: isHome,
          nearby: file.path === currentPath,
        });
      }
    }
  }
  return out;
}

/** Every distinct module in the reading, in strip order. */
export function modulesIn(files: ReadingFile[]): string[] {
  const seen: string[] = [];
  for (const f of files) {
    for (const m of f.outline?.modules ?? []) {
      if (!seen.includes(m.name)) seen.push(m.name);
    }
  }
  return seen;
}

/** Is there anything at all to reference? Gates read mode and the `/` menu. */
export function hasModules(files: ReadingFile[]): boolean {
  return files.some((f) => (f.outline?.modules?.length ?? 0) > 0);
}

export type { Outline };
