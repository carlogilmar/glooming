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

/**
 * What an assertion checks — three kinds, because three is what the call name
 * says for certain. `assert {:error, cs} = …` stays `assert`: the pattern is in
 * the arguments, and a guess drawn confidently is worse than a plain mark.
 */
export type AssertKind = "assert" | "error" | "message";

export interface Assertion {
  line: number;
  kind: AssertKind;
}

export interface TestCase {
  name: string;
  line: number;
  endLine: number;
  /** `assertions.length`, kept so nothing reading the count breaks. */
  asserts: number;
  assertions: Assertion[];
  /** The `@tag`s above it — its own span, the way a function's `@spec` is. */
  tagRange: Range | null;
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
  /** `@moduletag :skip` — applies to every test in the file. */
  moduleTags: string[];
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

/** A gloom, as the sidebar and the library list it — without the prose. */
export interface DocSummary {
  id: number;
  projectId: number;
  /** Empty until you name it. */
  title: string;
  /** Tags, comma-separated. */
  label: string | null;
  createdAt: string;
  updatedAt: string;
}

/** A gloom: prose and nothing else. No snapshot, no file list. */
export interface Doc extends DocSummary {
  markdown: string;
}

/** A file as it is on disk right now. */
export interface OpenedFile {
  path: string;
  filename: string;
  source: string;
  lang: string | null;
  outline: Outline | null;
  branch: string | null;
  hasGit: boolean;
}

/**
 * A file loaded this session — on screen, walked through, or named by the prose.
 *
 * The name is kept from when a gloom was a *reading* of a set of files; it is
 * now a cache of live reads, and nothing about it is stored. Same shape as
 * `OpenedFile`, so one becomes the other without conversion.
 */
export type ReadingFile = OpenedFile;

/** One module in the project, and the file it is defined in. */
export interface IndexedModule {
  module: string;
  path: string;
  rel: string;
  test: boolean;
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

/** The branch a path is on right now — asked for, never remembered. */
export const branchOf = (path: string) => invoke<string | null>("branch_of", { path });

export const blameFile = (path: string) =>
  invoke<BlameLine[]>("blame_file", { path });


// ---- docs ----------------------------------------------------------------

/** Start an unnamed gloom in a project. */
export const createDoc = (projectId: number) => invoke<Doc>("create_doc", { projectId });

export const saveDoc = (args: {
  id: number;
  markdown?: string;
  title?: string;
  label?: string;
}) => invoke<Doc>("save_doc", args);

export const loadDoc = (id: number) => invoke<Doc>("load_doc", { id });

export const listDocs = (projectId: number, query?: string, limit = 200) =>
  invoke<DocSummary[]>("list_docs", { projectId, query: query ?? null, limit });

export const deleteDoc = (id: number) => invoke<void>("delete_doc", { id });

/** Write the note to a path the save dialog chose. The only write outside the DB. */
export const exportNote = (path: string, markdown: string) =>
  invoke<string>("export_note", { path, markdown });

// ---- projects --------------------------------------------------------------

/** A codebase you explore, and how many glooms belong to it. */
export interface Project {
  id: number;
  path: string;
  name: string;
  openedAt: string;
  gloomCount: number;
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

/** Every module in the project and where it lives. Derived on every call. */
export const projectIndex = (path: string) =>
  invoke<IndexedModule[]>("project_index", { path });
