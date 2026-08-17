// Typed wrappers over the Rust commands. Every shape here mirrors a serde
// struct in src-tauri; keep them in step.

import { invoke } from "@tauri-apps/api/core";

export type Visibility = "public" | "private";

/** Inclusive 1-based line span. */
export interface Range {
  start: number;
  end: number;
}

export interface FnInfo {
  name: string;
  arity: number;
  /** Lower bound when the definition has default args (`f/1..2`). */
  minArity: number;
  visibility: Visibility;
  line: number;
  endLine: number;
  clauses: number;
  /** Every clause, in source order — selecting the row highlights all of them. */
  clauseRanges: Range[];
  doc: string | null;
  docRange: Range | null;
  specRange: Range | null;
}

export type DepKind = "app" | "lib" | "std";

export interface RemoteFn {
  name: string;
  callers: string[];
}

export interface Dep {
  module: string;
  kind: DepKind;
  functions: RemoteFn[];
}

export interface ModuleInfo {
  name: string;
  line: number;
  doc: string | null;
  docRange: Range | null;
  functions: FnInfo[];
  deps: Dep[];
}

export interface Outline {
  lang: string;
  modules: ModuleInfo[];
}

export interface DocSummary {
  id: number;
  path: string;
  filename: string;
  lang: string;
  title: string;
  branch: string | null;
  label: string | null;
  sourceSha: string;
  createdAt: string;
  updatedAt: string;
}

export interface Doc extends DocSummary {
  markdown: string;
  source: string;
}

export interface OpenedFile {
  path: string;
  filename: string;
  source: string;
  sourceSha: string;
  lang: string | null;
  outline: Outline | null;
  branch: string | null;
  hasGit: boolean;
  existing: DocSummary[];
}

export interface BlameLine {
  line: number;
  author: string;
  when: string;
  sha: string;
}

// ---- files ---------------------------------------------------------------

export const openFile = (path: string) =>
  invoke<OpenedFile>("open_file", { path });

export const reparse = (path: string) =>
  invoke<OpenedFile>("reparse", { path });

export const blameFile = (path: string) =>
  invoke<BlameLine[]>("blame_file", { path });


// ---- docs ----------------------------------------------------------------

export const seedDoc = (path: string, outline: Outline, source: string) =>
  invoke<string>("seed_doc", { path, outline, source });

export const createDoc = (args: {
  path: string;
  lang: string;
  title: string;
  branch: string | null;
  markdown: string;
  source: string;
}) => invoke<Doc>("create_doc", args);

export const saveDoc = (args: {
  id: number;
  markdown?: string;
  title?: string;
  branch?: string;
  label?: string;
}) => invoke<Doc>("save_doc", args);

export const loadDoc = (id: number) => invoke<Doc>("load_doc", { id });

export const listDocs = (query?: string, limit = 100) =>
  invoke<DocSummary[]>("list_docs", { query: query ?? null, limit });

export const deleteDoc = (id: number) => invoke<void>("delete_doc", { id });

export const reconcileDoc = (id: number, outline: Outline, source: string) =>
  invoke<Doc>("reconcile_doc", { id, outline, source });
