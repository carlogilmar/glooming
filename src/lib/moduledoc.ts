// A module's `@moduledoc`, rendered for the Woop.
//
// The parser hands over the doc as clean markdown — the `"""` fencing and the
// common indent already stripped (`unquote` in `parse/elixir.rs`) — so this is a
// render and a table of contents, nothing more.
//
// A plain markdown-it, deliberately NOT the note's (`createMarkdownIt`). The
// note's turns inline code that names a function into a reference chip, which is
// right for prose you wrote and wrong here: a moduledoc's `Repo.insert/1` is the
// author's, about code they meant, and a chip in the Woop would invite a click
// that nothing on this side is wired to answer.
//
// Pure, so it is probed headlessly.

import MarkdownIt from "markdown-it";
import hljs from "highlight.js/lib/core";
import elixir from "highlight.js/lib/languages/elixir";

hljs.registerLanguage("elixir", elixir);

export interface DocHeading {
  /** The element id the navigation bar scrolls to. */
  id: string;
  text: string;
  level: number;
}

export interface Moduledoc {
  html: string;
  /** h1–h3, in order — what the navigation bar offers. */
  headings: DocHeading[];
  /**
   * `@moduledoc false`: the author hid the module from the docs on purpose.
   * Worth saying — it marks a module as internal — and never worth rendering as
   * the word "false".
   */
  hidden: boolean;
}

function escapeHtml(s: string): string {
  return s.replace(/[&<>"]/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : "&quot;",
  );
}

const slug = (s: string) =>
  s
    .toLowerCase()
    .replace(/<[^>]*>/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "") || "section";

const md = new MarkdownIt({
  html: false,
  linkify: true,
  typographer: false,
  highlight(str, lang) {
    // A moduledoc's examples are doctests — `iex>` in an indented block, which
    // carries no language at all. In an Elixir module, unlabelled code is Elixir.
    const l = !lang || lang === "iex" || lang === "ex" ? "elixir" : lang;
    if (hljs.getLanguage(l)) {
      try {
        return hljs.highlight(str, { language: l, ignoreIllegals: true }).value;
      } catch {
        /* fall through to plain */
      }
    }
    return escapeHtml(str);
  },
});

// Indented code blocks bypass `highlight` in markdown-it, and doctests are
// almost always indented. Route them through the fence renderer so they are
// highlighted like everything else.
md.renderer.rules.code_block = (tokens, idx, options) => {
  const body = options.highlight?.(tokens[idx].content, "", "") ?? escapeHtml(tokens[idx].content);
  return `<pre><code class="hljs">${body}</code></pre>\n`;
};

/**
 * Render a moduledoc. `key` prefixes the heading ids so two renders on one page
 * — a file with two modules, say — cannot collide.
 */
export function renderModuledoc(doc: string, key: string): Moduledoc {
  if (doc.trim() === "false") return { html: "", headings: [], hidden: true };

  const tokens = md.parse(doc, {});
  const headings: DocHeading[] = [];
  const seen = new Map<string, number>();
  for (let i = 0; i < tokens.length; i++) {
    const t = tokens[i];
    if (t.type !== "heading_open") continue;
    const level = Number(t.tag.slice(1));
    const text = tokens[i + 1]?.content ?? "";
    const base = `${key}-${slug(text)}`;
    const n = seen.get(base) ?? 0;
    seen.set(base, n + 1);
    const id = n ? `${base}-${n + 1}` : base;
    t.attrSet("id", id);
    if (level <= 3) headings.push({ id, text, level });
  }
  return { html: md.renderer.render(tokens, md.options, {}), headings, hidden: false };
}
