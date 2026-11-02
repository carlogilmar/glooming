// markdown-it, plus the one thing a gloom adds to markdown: references.
//
// There are no `lgtm:*` blocks any more. They carried their values as text,
// which was right while a gloom held a snapshot of the code — the block and the
// code under it could never disagree — and wrong the moment the code became
// live: a `/deps` block written on Monday next to the live diagram drawn on
// Friday is two pictures of one module that disagree with no explanation. The
// views beside the note are the live ones; the note is prose and references.
//
// A fenced block is therefore just code, highlighted as Elixir when it says so.

import MarkdownIt from "markdown-it";
import hljs from "highlight.js/lib/core";
import elixir from "highlight.js/lib/languages/elixir";
import { looksLikeRef, refResolver } from "$lib/refs";
import type { ReadingFile } from "$lib/ipc";

hljs.registerLanguage("elixir", elixir);

function escapeHtml(s: string): string {
  return s.replace(/[&<>"]/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : "&quot;",
  );
}

/**
 * `files` are the loaded files — the only ones whose outlines can place a
 * reference. `known` is every module in the project, which is what decides
 * whether a name is a reference at all, and whether it is ambiguous.
 */
export function createMarkdownIt(
  files: ReadingFile[],
  currentPath: string | null,
  known: string[],
  labels: Map<string, string>,
): MarkdownIt {
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

  const resolver = refResolver(files, known, labels);

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
        `<code class="ref broken" title="not in the code any more">` +
        `${escapeHtml(text)}</code>`
      );
    }
    if (!hit) return defaultInline(tokens, idx, opts, env, self);

    // Underspecified, not wrong: the text names a suffix two modules share.
    // Clicking offers the candidates and rewrites the text to the full name.
    if ("ambiguous" in hit) {
      return (
        `<code class="ref ambiguous" role="button" tabindex="0"` +
        ` data-written="${escapeHtml(hit.written)}" data-text="${escapeHtml(text)}"` +
        ` data-candidates="${escapeHtml(hit.candidates.join(","))}"` +
        ` title="${escapeHtml(`${hit.written} is ${hit.candidates.length} modules — click to choose`)}">` +
        `${escapeHtml(text)}</code>`
      );
    }

    const away = hit.path !== currentPath;
    const spans = hit.ranges.map((r) => `${r.start}-${r.end}`).join(",");

    // The full text in the tooltip, because the chip shows the short form.
    const notes: string[] = [text];
    if (hit.ranges.length > 1) notes.push(`${hit.ranges.length} arities`);
    if (away) notes.push(`in ${hit.filename}`);

    return (
      `<code class="ref${away ? " away" : ""}${hit.module ? " mod" : ""}"` +
      ` data-sig="${escapeHtml(hit.sig)}"` +
      ` data-path="${escapeHtml(hit.path)}" data-ranges="${spans}"` +
      ` data-line="${hit.start}" data-end="${hit.end}" role="button" tabindex="0"` +
      ` title="${escapeHtml(notes.join(" · "))}"` +
      `>${escapeHtml(hit.label)}</code>`
    );
  };

  return md;
}
