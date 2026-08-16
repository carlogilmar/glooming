// The single markdown-it instance for the doc pane, with lgtm's extended
// syntax. Same approach as Alexandria's markdownit.ts: intercept the `fence`
// rule, build the block's HTML by hand, let everything else render normally.

import MarkdownIt from "markdown-it";
import hljs from "highlight.js/lib/core";
import elixir from "highlight.js/lib/languages/elixir";
import { parseBlock, withOutline, type FnEntry } from "$lib/lgtmBlock";
import type { Outline } from "$lib/ipc";

hljs.registerLanguage("elixir", elixir);

const BLOCK_TAG = "lgtm:functions";

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

  const slash = e.sig.replace(/~~/g, "").indexOf("/");
  const bare = e.sig.replace(/~~/g, "");
  const name = slash === -1 ? bare : bare.slice(0, slash);
  const arity = slash === -1 ? "" : bare.slice(slash);

  const sig = e.removed
    ? `<s>${escapeHtml(name)}<span class="ar">${escapeHtml(arity)}</span></s>`
    : `${escapeHtml(name)}<span class="ar">${escapeHtml(arity)}</span>`;

  // Multi-clause functions get a quiet badge — the row jumps to the first one.
  const clauses = e.clauses && e.clauses > 1 ? `<span class="clauses">·${e.clauses}</span>` : "";

  const why = e.prose
    ? `<span class="why">${md.renderInline(e.prose)}</span>`
    : `<span class="why empty"></span>`;

  return `<div ${attrs}><span class="sig">${sig}${clauses}</span>${why}</div>`;
}
