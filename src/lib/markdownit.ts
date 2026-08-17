// The single markdown-it instance for the doc pane, with lgtm's extended
// syntax. Same approach as Alexandria's markdownit.ts: intercept the `fence`
// rule, build the block's HTML by hand, let everything else render normally.

import MarkdownIt from "markdown-it";
import hljs from "highlight.js/lib/core";
import elixir from "highlight.js/lib/languages/elixir";
import { parseBlock, withOutline, type FnEntry } from "$lib/lgtmBlock";
import { renderTreemap } from "$lib/treemap";
import { renderStats } from "$lib/stats";
import { renderDeps } from "$lib/deps";
import { renderSurface } from "$lib/surface";
import type { Outline } from "$lib/ipc";

hljs.registerLanguage("elixir", elixir);

const BLOCK_TAG = "lgtm:functions";
const TREEMAP_TAG = "lgtm:treemap";
const STATS_TAG = "lgtm:stats";
const DEPS_TAG = "lgtm:deps";
const SURFACE_TAG = "lgtm:surface";

function escapeHtml(s: string): string {
  return s.replace(/[&<>"]/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : "&quot;",
  );
}

/**
 * `outline` is passed in so rendered rows carry the line numbers they jump to.
 * It changes whenever the file is re-parsed, so the instance is rebuilt rather
 * than mutated — markdown rendering is cheap and this keeps it stateless.
 */
/**
 * `outline` is the ONLY live input: blocks carry their own data as text, and
 * the outline supplies just the line numbers rows and tiles jump to.
 */
export function createMarkdownIt(outline: Outline | null): MarkdownIt {
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
        return renderStats(token.content);
      } catch {
        return defaultFence(tokens, idx, opts, env, self);
      }
    }

    // ```lgtm:surface — the directory: public and private, sorted by name.
    if (info.startsWith(SURFACE_TAG)) {
      try {
        return renderSurface(token.content, outline?.modules?.[0]?.name ?? "");
      } catch {
        return defaultFence(tokens, idx, opts, env, self);
      }
    }

    // ```lgtm:deps — what the module reaches, drawn from the edges in the text.
    if (info.startsWith(DEPS_TAG)) {
      try {
        return renderDeps(token.content, outline?.modules?.[0] ?? null);
      } catch {
        return defaultFence(tokens, idx, opts, env, self);
      }
    }

    // ```lgtm:treemap — function sizes, drawn from the live outline. The block
    // body is ignored; the shape of the code is not something you hand-edit.
    if (info.startsWith(TREEMAP_TAG)) {
      try {
        return renderTreemap(token.content, outline?.modules?.[0] ?? null);
      } catch {
        return defaultFence(tokens, idx, opts, env, self);
      }
    }

    if (!info.startsWith(BLOCK_TAG)) {
      return defaultFence(tokens, idx, opts, env, self);
    }

    try {
      let block = parseBlock(info, token.content);
      const module = outline?.modules?.[0];
      if (module) {
        block = withOutline(block, module.functions);
      }
      return renderFunctionsBlock(block.module ?? module?.name ?? "", block.entries, md);
    } catch {
      // A malformed block must never take the doc down with it.
      return defaultFence(tokens, idx, opts, env, self);
    }
  };

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
