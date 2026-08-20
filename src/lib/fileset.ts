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
  for (const file of files) {
    for (const m of file.outline?.modules ?? []) {
      if (m.name !== module) continue;
      const fn = inModule(m, name, arity);
      if (fn) return { file, module: m, fn };
    }
  }
  return null;
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
  /** What gets inserted: bare inside its own file, qualified across files. */
  insert: string;
  /** `create_user/1` — what you scan for. */
  sig: string;
  module: string;
  path: string;
  filename: string;
  line: number;
  visibility: "public" | "private";
  /** True when this function lives in the file you are looking at. */
  local: boolean;
}

/**
 * Everything referenceable in the reading, grouped by module in strip order.
 *
 * This is the *only* thing adding a file changes. There is no seeding, no new
 * block and no edit to your prose — an added file simply widens what `/` can
 * offer you, which is exactly what a review needs and no more.
 *
 * Insertion is qualified only when it has to be: prose about the file you are in
 * reads better as `to_cents/1` than `MyApp.Billing.to_cents/1`, and a single-file
 * reading should look exactly as it always did.
 */
export function vocabulary(files: ReadingFile[], currentPath: string | null): VocabEntry[] {
  const out: VocabEntry[] = [];
  for (const file of files) {
    const local = file.path === currentPath;
    for (const module of file.outline?.modules ?? []) {
      for (const fn of module.functions) {
        const sig = displaySig(fn);
        out.push({
          sig,
          insert: local ? sig : `${module.name}.${sig}`,
          module: module.name,
          path: file.path,
          filename: file.filename,
          line: fn.line,
          visibility: fn.visibility,
          local,
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

export type { Outline };
