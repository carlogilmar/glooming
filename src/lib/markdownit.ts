// The single markdown-it instance for the doc pane, with lgtm's extended
// syntax. Same approach as Alexandria's markdownit.ts: intercept the `fence`
// rule, build the block's HTML by hand, let everything else render normally.

import MarkdownIt from "markdown-it";
import hljs from "highlight.js/lib/core";
import elixir from "highlight.js/lib/languages/elixir";
import { parseBlock, parseInfo, withOutline, type FnEntry } from "$lib/lgtmBlock";
import { renderTreemap } from "$lib/treemap";
import { renderStats } from "$lib/stats";
import { renderDeps } from "$lib/deps";
import { renderSurface } from "$lib/surface";
import { renderSettings } from "$lib/settings";
import { renderTests } from "$lib/tests";
import { looksLikeRef, refResolver } from "$lib/refs";
import { fileForModule, moduleOf, origin } from "$lib/fileset";
import type { ReadingFile } from "$lib/ipc";

hljs.registerLanguage("elixir", elixir);

const BLOCK_TAG = "lgtm:functions";
const TREEMAP_TAG = "lgtm:treemap";
const STATS_TAG = "lgtm:stats";
const DEPS_TAG = "lgtm:deps";
const SURFACE_TAG = "lgtm:surface";
const SETTINGS_TAG = "lgtm:settings";
const TESTS_TAG = "lgtm:tests";

function escapeHtml(s: string): string {
  return s.replace(/[&<>"]/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : "&quot;",
  );
}

/**
 * Tag every block with the file it is about.
 *
 * A row in a block belonging to billing.ex has to switch the code pane to
 * billing.ex when you click it, and the renderers are shared with the
 * single-file case — so rather than threading a path through six signatures,
 * the attribute is added to the one element each of them returns. Every block
 * renderer's output starts with `<div`, which is what makes this safe.
 */
function owned(html: string, path: string | null): string {
  if (!path || !html.startsWith("<div")) return html;
  return html.replace("<div", `<div data-path="${escapeHtml(path)}"`);
}

/**
 * The file set is the ONLY live input: blocks carry their own data as text, and
 * the outlines supply just the line numbers rows and tiles jump to.
 *
 * Blocks belong to the file the doc was seeded from — except when the block
 * names its module, which `lgtm:functions`, `lgtm:surface` and `lgtm:deps` all
 * do. That attribute was already in the markdown for readability; in a
 * multi-file reading it becomes the thing that finds the right outline, instead
 * of the current tab's being assumed.
 */
export function createMarkdownIt(
  files: ReadingFile[],
  currentPath: string | null = null,
): MarkdownIt {
  const home = origin(files);
  const outline = home?.outline ?? null;
  const filename = home?.filename ?? "";

  /** The file a block is about: whatever its `module=` names, else the origin. */
  const blockFile = (info: string) => {
    const named = parseInfo(info).module;
    return (named ? fileForModule(files, named) : null) ?? home;
  };
  const md = new MarkdownIt({
    html: false,
    linkify: true,
    breaks: false,
    typographer: false,
    highlight(str, lang) {
      if (lang && hljs.getLanguage(lang)) {
        try {
          return hljs.highlight(str, { language: lang, ignoreIllegals: true }).value;
        } catch {
          /* fall through to default escaping */
        }
      }
      return "";
    },
  });

  const defaultFence =
    md.renderer.rules.fence ??
    ((tokens, idx, opts, _env, self) => self.renderToken(tokens, idx, opts));

  md.renderer.rules.fence = (tokens, idx, opts, env, self) => {
    const token = tokens[idx];
    const info = (token.info || "").trim();

    // ```lgtm:stats — file-level facts, drawn from the outline plus git.
    if (info.startsWith(STATS_TAG)) {
      try {
        return owned(renderStats(token.content), home?.path ?? null);
      } catch {
        return defaultFence(tokens, idx, opts, env, self);
      }
    }

    // ```lgtm:settings — a config script's tree, and where each value comes from.
    if (info.startsWith(SETTINGS_TAG)) {
      try {
        return owned(renderSettings(token.content, filename), home?.path ?? null);
      } catch {
        return defaultFence(tokens, idx, opts, env, self);
      }
    }

    // ```lgtm:tests — describes, setups and tests.
    if (info.startsWith(TESTS_TAG)) {
      try {
        return owned(
          renderTests(token.content, outline?.tests?.module ?? ""),
          home?.path ?? null,
        );
      } catch {
        return defaultFence(tokens, idx, opts, env, self);
      }
    }

    // ```lgtm:surface — the directory: public and private, sorted by name.
    if (info.startsWith(SURFACE_TAG)) {
      try {
        const f = blockFile(info);
        return owned(
          renderSurface(token.content, moduleOf(f)?.name ?? ""),
          f?.path ?? null,
        );
      } catch {
        return defaultFence(tokens, idx, opts, env, self);
      }
    }

    // ```lgtm:deps — what the module reaches, drawn from the edges in the text.
    if (info.startsWith(DEPS_TAG)) {
      try {
        const f = blockFile(info);
        return owned(renderDeps(token.content, moduleOf(f)), f?.path ?? null);
      } catch {
        return defaultFence(tokens, idx, opts, env, self);
      }
    }

    // ```lgtm:treemap — function sizes, drawn from the live outline. The block
    // body is ignored; the shape of the code is not something you hand-edit.
    if (info.startsWith(TREEMAP_TAG)) {
      try {
        const f = blockFile(info);
        return owned(renderTreemap(token.content, moduleOf(f)), f?.path ?? null);
      } catch {
        return defaultFence(tokens, idx, opts, env, self);
      }
    }

    if (!info.startsWith(BLOCK_TAG)) {
      return defaultFence(tokens, idx, opts, env, self);
    }

    try {
      const f = blockFile(info);
      let block = parseBlock(info, token.content);
      const module = moduleOf(f);
      if (module) {
        block = withOutline(block, module.functions);
      }
      return owned(
        renderFunctionsBlock(block.module ?? module?.name ?? "", block.entries, md),
        f?.path ?? null,
      );
    } catch {
      // A malformed block must never take the doc down with it.
      return defaultFence(tokens, idx, opts, env, self);
    }
  };

  // Inline code that names something in the reading becomes a reference: a click
  // target, and the anchor a scroll-driven reading steps through. Only when
  // there is a module somewhere in the set — a config or a test suite is a
  // directory, not a narrative, so there is nothing to walk.
  const walkable = files.some((f) => f.outline?.kind === "module");
  if (walkable) {
    const resolver = refResolver(files);

    // The resolver threads a "current file" through the prose in document
    // order, so it has to start clean on every pass. `render` is the only entry
    // point the doc pane uses, which makes this the one honest place to reset —
    // and doing it here rather than per-reference is what keeps an unqualified
    // name meaning "the file this paragraph is about".
    const baseRender = md.render.bind(md);
    md.render = (src: string, env?: unknown) => {
      resolver.reset();
      return baseRender(src, env as never);
    };

    const defaultInline =
      md.renderer.rules.code_inline ??
      ((tokens, idx, opts, _env, self) => self.renderToken(tokens, idx, opts));

    md.renderer.rules.code_inline = (tokens, idx, opts, env, self) => {
      const text = tokens[idx].content.trim();
      if (!looksLikeRef(text)) return defaultInline(tokens, idx, opts, env, self);

      const hit = resolver.resolve(text);
      if (hit === "dangling") {
        return (
          `<code class="ref broken" title="not in this reading any more">` +
          `${escapeHtml(text)}</code>`
        );
      }
      if (!hit) return defaultInline(tokens, idx, opts, env, self);

      // `data-path` is what lets a reference point into a file other than the
      // one on screen: clicking it, or scrolling onto it in read mode, switches
      // the code pane first and then selects.
      const away = hit.path !== currentPath;
      return (
        `<code class="ref${away ? " away" : ""}" data-sig="${escapeHtml(hit.sig)}"` +
        ` data-path="${escapeHtml(hit.path)}"` +
        ` data-line="${hit.start}" data-end="${hit.end}" role="button" tabindex="0"` +
        (away ? ` title="in ${escapeHtml(hit.filename)}"` : "") +
        `>${escapeHtml(text)}</code>`
      );
    };
  }

  return md;
}

function renderFunctionsBlock(module: string, entries: FnEntry[], md: MarkdownIt): string {
  const groups: ["public" | "private", FnEntry[]][] = [
    ["public", entries.filter((e) => e.visibility === "public")],
    ["private", entries.filter((e) => e.visibility === "private")],
  ];

  const counts = groups
    .filter(([, list]) => list.length)
    .map(([name, list]) => `${list.length} ${name}`)
    .join(" · ");

  let html = `<div class="lgtm-block">`;
  html += `<header><span class="tag">${BLOCK_TAG}</span>`;
  html += `<span>${escapeHtml(module)}</span>`;
  html += `<span class="count">${counts}</span></header>`;

  for (const [name, list] of groups) {
    if (!list.length) continue;
    html += `<div class="grp ${name}">`;
    html += `<div class="label"><span class="bar"></span>${name}</div>`;
    for (const e of list) html += renderRow(e, md);
    html += `</div>`;
  }

  return html + `</div>`;
}

/**
 * One row per function, stacked rather than columned: the signature on its own
 * line, the explanation beneath it. A two-column layout squeezed long names
 * into a narrow gutter and wrapped them badly — and real modules are full of
 * long names.
 */
function renderRow(e: FnEntry, md: MarkdownIt): string {
  const clickable = e.line !== undefined && !e.removed;
  const attrs = [
    `class="fnrow${e.removed ? " removed" : ""}${clickable ? "" : " static"}"`,
    clickable ? `data-line="${e.line}"` : "",
    clickable ? `data-sig="${escapeHtml(e.sig)}"` : "",
    clickable ? `role="button" tabindex="0"` : "",
  ]
    .filter(Boolean)
    .join(" ");

  const bare = e.sig.replace(/~~/g, "");
  const slash = bare.indexOf("/");
  const name = slash === -1 ? bare : bare.slice(0, slash);
  const arity = slash === -1 ? "" : bare.slice(slash);

  const sig = e.removed
    ? `<s>${escapeHtml(name)}<span class="ar">${escapeHtml(arity)}</span></s>`
    : `${escapeHtml(name)}<span class="ar">${escapeHtml(arity)}</span>`;

  // Badges spell themselves out. `create_user/1 ·2` read as nonsense; the
  // arity range gets a word too, since `/1..2` is not self-explanatory.
  const badges: string[] = [];
  if (arity.includes("..")) badges.push(`<span class="badge">default args</span>`);
  if (e.clauses && e.clauses > 1) badges.push(`<span class="badge">${e.clauses} clauses</span>`);
  if (e.removed) badges.push(`<span class="badge gone">removed</span>`);

  const why = e.prose
    ? `<div class="why">${md.renderInline(e.prose)}</div>`
    : `<div class="why empty"></div>`;

  return `<div ${attrs}><div class="sigline"><span class="sig">${sig}</span>${badges.join("")}</div>${why}</div>`;
}
