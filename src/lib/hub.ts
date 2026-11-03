// The Hub's reading of a gloom: what it walks, whether it still matches the code,
// and how it starts. `mockup/hub.html` is the contract.
//
// Everything here is derived from the prose **by rendering it**, with the same
// markdown-it and the same resolver the note uses. A second parser for "which
// references are broken" would be a second opinion, and the day the two
// disagreed the Hub would say a gloom is healthy while the note strikes half of
// it through. Rendering is cheap at the size of a knowledge base, and one
// opinion cannot disagree with itself.
//
// Pure, so it is probed headlessly rather than clicked at.

import type MarkdownIt from "markdown-it";
import type { Doc, IndexedModule, ReadingFile, Topic } from "$lib/ipc";
import { namedPaths } from "$lib/fileset";
import { parseTags } from "$lib/tags";

export interface GloomFacts {
  /** References that resolve or could — every chip the note would draw. */
  refs: number;
  /** Struck through: the code moved out from under the prose. */
  broken: string[];
  /** Names more than one module; the note offers a choice. */
  ambiguous: string[];
  /** The modules it walks, in the order the prose first reaches them, as shown. */
  modules: string[];
  /** The first paragraph you wrote, rendered — or "" while it is only the invitation. */
  lead: string;
  /**
   * Some file the prose names is not loaded yet, so a reference that looks
   * fine may still be broken. Said as "checking…" rather than as healthy —
   * "not read yet" is not "fine", the same rule the note follows.
   */
  pending: boolean;
}

const unescape = (s: string) =>
  s.replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&amp;/g, "&");

/**
 * What the Hub says about one gloom. `md` must be the note's own renderer over
 * the same `files` (see `createMarkdownIt`), built once per change of files and
 * shared across every gloom.
 */
export function factsOf(
  markdown: string,
  md: MarkdownIt,
  index: IndexedModule[],
  files: ReadingFile[],
  labels: Map<string, string>,
): GloomFacts {
  const html = md.render(markdown);

  const broken = [...html.matchAll(/<code class="ref broken"[^>]*>([^<]*)<\/code>/g)].map((m) => unescape(m[1]));
  const ambiguous = [...html.matchAll(/<code class="ref ambiguous"[^>]*data-text="([^"]*)"/g)].map((m) => unescape(m[1]));
  const refs = (html.match(/<code class="ref[ "]/g) ?? []).length;

  // The walk: each resolved chip's file, in order, named by its module. A file
  // can define several modules; the first one indexed is the one it is for.
  const moduleOfPath = new Map<string, string>();
  for (const m of index) if (!moduleOfPath.has(m.path)) moduleOfPath.set(m.path, m.module);
  const modules: string[] = [];
  for (const [, path] of html.matchAll(/data-path="([^"]*)"/g)) {
    const mod = moduleOfPath.get(unescape(path));
    const shown = mod ? (labels.get(mod) ?? mod) : null;
    if (shown && !modules.includes(shown)) modules.push(shown);
  }

  // The lead: the first paragraph that is yours. The invitation a new gloom
  // starts with is a paragraph of nothing but emphasis — the same structural test
  // the arrival nudge uses — so it is skipped, and an untouched gloom has none.
  let lead = "";
  for (const [p, inner] of html.matchAll(/<p>([\s\S]*?)<\/p>/g)) {
    if (/^<em>[\s\S]*<\/em>$/.test(inner.trim())) continue;
    lead = p;
    break;
  }

  return { refs, broken, ambiguous, modules, lead, pending: namedPaths(markdown, index, files).length > 0 };
}

/** Every file any of these glooms names that is not loaded yet. */
export function pathsToCheck(glooms: Doc[], index: IndexedModule[], files: ReadingFile[]): string[] {
  const want = new Set<string>();
  for (const g of glooms) for (const p of namedPaths(g.markdown, index, files)) want.add(p);
  return [...want];
}

/**
 * Does a gloom match the filter? The name, the tags and the prose — a gloom is
 * usually remembered by what it says, which is why the library searched prose
 * and the Hub, which replaced it, still does.
 */
export function matches(g: Doc, query: string, tag: string | null): boolean {
  if (tag && !parseTags(g.label).includes(tag)) return false;
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return (
    (g.title || "unnamed gloom").toLowerCase().includes(q) ||
    (g.label ?? "").toLowerCase().includes(q) ||
    g.markdown.toLowerCase().includes(q)
  );
}

export interface Group {
  /** Null is Unsorted. */
  topic: Topic | null;
  ids: number[];
}

/**
 * The glooms grouped as the Hub draws them: topics in their order, Unsorted
 * last, each group in its glooms' order. A gloom whose topic has gone (it
 * cannot, but a row can outlive a bug) is Unsorted rather than lost.
 */
export function groupsOf(topics: Topic[], glooms: Doc[]): Group[] {
  const groups: Group[] = [...topics]
    .sort((a, b) => a.position - b.position || a.id - b.id)
    .map((t) => ({ topic: t, ids: [] }));
  const unsorted: Group = { topic: null, ids: [] };
  const byTopic = new Map(groups.map((g) => [g.topic!.id, g]));
  const ordered = [...glooms].sort(
    (a, b) => a.position - b.position || b.updatedAt.localeCompare(a.updatedAt),
  );
  for (const g of ordered) (byTopic.get(g.topicId ?? -1) ?? unsorted).ids.push(g.id);
  return [...groups, unsorted];
}

/** Newest first — the order you wrote in. Filing a gloom does not move it here. */
export function byRecent(glooms: Doc[]): Doc[] {
  return [...glooms].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
}

/**
 * Move `id` to sit before `before` (or at the end of `topic` when `before` is
 * null). Returns new groups; the arrangement the Hub then saves in one call.
 */
export function moveGloom(groups: Group[], id: number, topic: number | null, before: number | null): Group[] {
  const out = groups.map((g) => ({ topic: g.topic, ids: g.ids.filter((x) => x !== id) }));
  const target = out.find((g) => (g.topic?.id ?? null) === topic);
  if (!target) return groups;
  const at = before === null ? -1 : target.ids.indexOf(before);
  if (at === -1) target.ids.push(id);
  else target.ids.splice(at, 0, id);
  return out;
}

/** Move topic `id` to sit before topic `before`. Unsorted never moves: it is always last. */
export function moveTopic(groups: Group[], id: number, before: number): Group[] {
  const moving = groups.find((g) => g.topic?.id === id);
  if (!moving || id === before) return groups;
  const rest = groups.filter((g) => g !== moving);
  const at = rest.findIndex((g) => g.topic?.id === before);
  if (at === -1) return groups;
  rest.splice(at, 0, moving);
  return rest;
}
