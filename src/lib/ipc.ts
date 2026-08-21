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

export type FileKind = "module" | "config" | "test" | "plain";

export type ValueSource =
  | { kind: "literal"; value: string }
  | { kind: "env"; var: string; required: boolean }
  | { kind: "secret" };

export interface Setting {
  key: string;
  line: number;
  endLine: number;
  source: ValueSource;
}

export interface ConfigGroup {
  app: string;
  target: string | null;
  line: number;
  endLine: number;
  settings: Setting[];
}

export interface ConfigInfo {
  groups: ConfigGroup[];
  imports: string[];
}

export interface SetupInfo {
  kind: string;
  line: number;
  endLine: number;
  named: string | null;
  /** `null` means unknown — a named callback defined elsewhere. */
  provides: string[] | null;
}

export interface TestCase {
  name: string;
  line: number;
  endLine: number;
  asserts: number;
  tags: string[];
  skipped: boolean;
}

export interface Describe {
  name: string | null;
  line: number;
  endLine: number;
  setups: SetupInfo[];
  tests: TestCase[];
}

export interface TestInfo {
  module: string;
  caseTemplate: string | null;
  isAsync: boolean;
  setups: SetupInfo[];
  describes: Describe[];
}

export interface Outline {
  lang: string;
  /** Which blocks this file's doc gets — a config has no functions to size. */
  kind: FileKind;
  modules: ModuleInfo[];
  config: ConfigInfo | null;
  tests: TestInfo | null;
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
  /** Files this reading covers. 1 for a doc about a single file. */
  fileCount: number;
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

/**
 * One file of a reading, as the UI needs it.
 *
 * `source` is what to show: disk when the file is readable, the snapshot when it
 * is not — which is what lets a reading survive one of its files being deleted.
 */
export interface ReadingFile {
  path: string;
  filename: string;
  lang: string | null;
  source: string;
  sourceSha: string;
  /** sha of the snapshot taken when this file joined the reading. */
  snapshotSha: string;
  /** This file has changed on disk since it was read. Per-file, deliberately. */
  stale: boolean;
  /** Not on disk any more; `source` is the snapshot. */
  missing: boolean;
  outline: Outline | null;
  hasGit: boolean;
  branch: string | null;
  /** The file the doc was seeded from. Cannot be removed from the reading. */
  origin: boolean;
}

/** A whole reading: the note, and every file it covers. */
export interface Reading {
  doc: Doc;
  files: ReadingFile[];
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

// ---- the files a reading covers --------------------------------------------
// Every one of these returns the whole reading, so the UI replaces its state
// rather than patching it — the same one-payload habit as `open_file`.

export const openReading = (id: number) => invoke<Reading>("open_reading", { id });

/**
 * Generate one `lgtm:*` block for an already-parsed file.
 *
 * Round-trips to Rust even though the outline is already here, because the sort
 * order must live in exactly one place — see `block_for`.
 */
export const blockFor = (
  kind: "stats" | "surface" | "deps" | "treemap",
  path: string,
  outline: Outline,
) => invoke<string>("block_for", { kind, path, outline });

/** Adding a file seeds nothing. It contributes source, and vocabulary. */
export const addDocFile = (id: number, path: string) =>
  invoke<Reading>("add_doc_file", { id, path });

export const removeDocFile = (id: number, path: string) =>
  invoke<Reading>("remove_doc_file", { id, path });

export const resnapshotDocFile = (id: number, path: string) =>
  invoke<Reading>("resnapshot_doc_file", { id, path });

// ---- projects --------------------------------------------------------------

export interface Project {
  id: number;
  path: string;
  name: string;
  openedAt: string;
}

export interface ProjectFile {
  path: string;
  /** Relative to the project root — what you search and read. */
  rel: string;
  name: string;
}

export const openProject = (path: string) => invoke<Project>("open_project", { path });

export const recentProjects = () => invoke<Project[]>("recent_projects");

export const forgetProject = (id: number) => invoke<void>("forget_project", { id });

export const projectFiles = (path: string) => invoke<ProjectFile[]>("project_files", { path });
