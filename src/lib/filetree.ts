// The project as a file tree, the way an editor shows it.
//
// Every file, not only Elixir: reading a codebase means `mix.exs`, the config,
// the migrations and the README as much as the modules, and a tree of only what
// lgtm can parse is a partial map that looks complete. Folders first, then files,
// each sorted case-insensitively — what every editor does, so nobody has to
// learn it. Pure, so it is probed headlessly rather than clicked at.

import type { ProjectFile } from "$lib/ipc";

export interface TreeNode {
  /** The relative path — a folder's or a file's. */
  key: string;
  name: string;
  depth: number;
  /** Null for a folder. */
  file: ProjectFile | null;
}

/**
 * The visible rows, given which folders are open.
 *
 * `opened` is the whole truth about folding. Revealing the file on screen is
 * `reveal`'s job, done once when you arrive at a file — it used to happen here,
 * on every render, which meant a folder on the way to the open file could never
 * be folded: the click removed it and the next render put it straight back.
 */
export function fileTree(files: ProjectFile[], opened: Set<string>): TreeNode[] {
  const dirs = new Map<string, Set<string>>();
  const leaves = new Map<string, ProjectFile[]>();
  for (const f of files) {
    const parts = f.rel.split("/");
    for (let i = 0; i < parts.length - 1; i++) {
      const parent = parts.slice(0, i).join("/");
      const key = parts.slice(0, i + 1).join("/");
      if (!dirs.has(parent)) dirs.set(parent, new Set());
      dirs.get(parent)!.add(key);
    }
    const parent = parts.slice(0, -1).join("/");
    if (!leaves.has(parent)) leaves.set(parent, []);
    leaves.get(parent)!.push(f);
  }

  const byName = (a: string, b: string) => a.toLowerCase().localeCompare(b.toLowerCase());
  const leaf = (k: string) => k.split("/").pop() ?? k;
  const rows: TreeNode[] = [];
  const walk = (parent: string, depth: number) => {
    for (const key of [...(dirs.get(parent) ?? [])].sort((a, b) => byName(leaf(a), leaf(b)))) {
      rows.push({ key, name: leaf(key), depth, file: null });
      if (opened.has(key)) walk(key, depth + 1);
    }
    for (const f of [...(leaves.get(parent) ?? [])].sort((a, b) => byName(a.name, b.name))) {
      rows.push({ key: f.rel, name: f.name, depth, file: f });
    }
  };
  walk("", 0);
  return rows;
}

/**
 * `opened` plus every folder on the way to `rel` — arriving at a file from a
 * reference or the reach diagram should show you where it lives. Returns the
 * same set when nothing needs opening, so a no-op does not re-render the tree.
 */
export function reveal(opened: Set<string>, rel: string | null): Set<string> {
  if (!rel) return opened;
  const parts = rel.split("/");
  const missing: string[] = [];
  for (let i = 1; i < parts.length; i++) {
    const k = parts.slice(0, i).join("/");
    if (!opened.has(k)) missing.push(k);
  }
  return missing.length ? new Set([...opened, ...missing]) : opened;
}

/**
 * What the open gloom's prose names: the files themselves, and every folder they
 * sit in — so a reference is findable in a collapsed tree too. What the file set
 * used to be, now derived from what you wrote.
 */
export function mentionedIn(
  files: ProjectFile[],
  referenced: Set<string>,
): { files: Set<string>; dirs: Set<string> } {
  const out = { files: new Set<string>(), dirs: new Set<string>() };
  for (const f of files) {
    if (!referenced.has(f.path)) continue;
    out.files.add(f.rel);
    const parts = f.rel.split("/");
    for (let i = 1; i < parts.length; i++) out.dirs.add(parts.slice(0, i).join("/"));
  }
  return out;
}
