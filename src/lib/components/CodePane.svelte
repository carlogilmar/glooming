<script lang="ts">
  import hljs from "highlight.js/lib/core";
  import elixir from "highlight.js/lib/languages/elixir";
  import { blameFile, type BlameLine, type Outline } from "$lib/ipc";
  import { focus } from "$lib/stores/focus.svelte";

  hljs.registerLanguage("elixir", elixir);

  let {
    source = "",
    lang = null,
    filename = "",
    path = "",
    hasGit = false,
    outline = null,
  }: {
    source: string;
    lang: string | null;
    filename: string;
    path: string;
    hasGit: boolean;
    outline: Outline | null;
  } = $props();

  let body = $state<HTMLDivElement | null>(null);
  let blame = $state<BlameLine[]>([]);
  let showBlame = $state(false);
  let blaming = $state(false);

  // ---- font size ----------------------------------------------------------
  const FONT_KEY = "codeFontSize";
  const MIN = 10;
  const MAX = 22;
  const DEFAULT = 12.5;

  let fontSize = $state(DEFAULT);

  $effect(() => {
    const stored = parseFloat(localStorage.getItem(FONT_KEY) ?? "");
    if (stored >= MIN && stored <= MAX) fontSize = stored;
  });

  function setFont(next: number) {
    fontSize = Math.min(MAX, Math.max(MIN, Math.round(next * 2) / 2));
    localStorage.setItem(FONT_KEY, String(fontSize));
  }

  // ---- soft wrap ----------------------------------------------------------
  // On by default: reading a file shouldn't require scrolling sideways to
  // finish a line. Off is there for the rare case where column alignment
  // matters more than seeing the whole line.
  const WRAP_KEY = "codeWrap";
  let wrap = $state(true);

  $effect(() => {
    wrap = localStorage.getItem(WRAP_KEY) !== "0";
  });

  function toggleWrap() {
    wrap = !wrap;
    localStorage.setItem(WRAP_KEY, wrap ? "1" : "0");
  }

  const lines = $derived(source.length ? source.split("\n") : []);

  // ---- highlighting -------------------------------------------------------

  /**
   * Give module aliases their own color. highlight.js's Elixir grammar doesn't
   * distinguish `Repo` from a local call, so this is a second pass over the
   * highlighted HTML: split into tags and text, and only rewrite the text.
   * Segments already inside a string or comment span are skipped — a module
   * name mentioned in prose is not a module reference.
   */
  function colorModules(html: string): string {
    const parts = html.split(/(<[^>]+>)/);
    const open: string[] = [];
    let out = "";

    for (const part of parts) {
      if (part.startsWith("<")) {
        if (part.startsWith("</")) open.pop();
        else if (!part.endsWith("/>")) open.push(part);
        out += part;
        continue;
      }
      const inProse = open.some((t) => /hljs-(string|comment|doctag|meta)/.test(t));
      out += inProse
        ? part
        : part.replace(/\b[A-Z][A-Za-z0-9_]*(?:\.[A-Z][A-Za-z0-9_]*)*\b/g, (m) => `<span class="mod">${m}</span>`);
    }
    return out;
  }

  // Highlight the whole file once, then split — hljs needs full context to get
  // multi-line constructs (heredocs, block comments) right.
  const highlighted = $derived.by(() => {
    if (!source) return [];
    let html: string;
    if (lang && hljs.getLanguage(lang)) {
      try {
        html = hljs.highlight(source, { language: lang, ignoreIllegals: true }).value;
      } catch {
        html = escapeAll(source);
      }
    } else {
      html = escapeAll(source);
    }
    return colorModules(html).split("\n");
  });

  function escapeAll(s: string): string {
    return s.replace(/[&<>]/g, (c) => (c === "&" ? "&amp;" : c === "<" ? "&lt;" : "&gt;"));
  }

  /**
   * Lines occupied by `@moduledoc` / `@doc`, so documentation reads as prose
   * rather than as code. Comes from the parser, so heredoc continuation lines
   * are included correctly.
   */
  const docLines = $derived.by(() => {
    const set = new Set<number>();
    for (const m of outline?.modules ?? []) {
      if (m.docRange) for (let i = m.docRange.start; i <= m.docRange.end; i++) set.add(i);
      for (const f of m.functions) {
        if (f.docRange) for (let i = f.docRange.start; i <= f.docRange.end; i++) set.add(i);
      }
    }
    return set;
  });

  // ---- blame --------------------------------------------------------------

  async function toggleBlame() {
    if (showBlame) {
      showBlame = false;
      return;
    }
    if (!blame.length && path) {
      blaming = true;
      try {
        blame = await blameFile(path);
      } catch {
        blame = [];
      } finally {
        blaming = false;
      }
    }
    showBlame = true;
  }

  const blameRows = $derived.by(() =>
    blame.map((b, i) => ({
      ...b,
      show: i === 0 || blame[i - 1]?.author !== b.author,
    })),
  );

  function authorTone(author: string): string {
    let h = 0;
    for (const c of author) h = (h * 31 + c.charCodeAt(0)) >>> 0;
    return `var(--who-${(h % 3) + 1})`;
  }

  // Scroll the focused definition into view. scrollIntoView clamps at the end
  // of the document, so no padding is needed to make centering work.
  $effect(() => {
    const line = focus.line;
    if (!line || !body) return;
    const row = body.querySelector<HTMLElement>(`[data-line="${line}"]`);
    row?.scrollIntoView({ behavior: "smooth", block: "center" });
  });

  function clickBackground(e: MouseEvent) {
    // A drag that selects text ends in a click; don't treat that as "clear".
    if (window.getSelection()?.toString()) return;
    if (!(e.target as HTMLElement).closest(".row")) focus.clear();
  }
</script>

<div class="pane">
  <div class="panehead">
    <span>{filename || "no file"}</span>
    <span class="spacer"></span>

    <div class="fontsize" title="Code font size">
      <button onclick={() => setFont(fontSize - 0.5)} disabled={fontSize <= MIN} aria-label="Smaller">
        A<small>−</small>
      </button>
      <button class="val" onclick={() => setFont(DEFAULT)} title="Reset to default">{fontSize}</button>
      <button onclick={() => setFont(fontSize + 0.5)} disabled={fontSize >= MAX} aria-label="Larger">
        A<small>+</small>
      </button>
    </div>

    <button class="btn icon" class:primary={wrap} onclick={toggleWrap} title="Soft wrap long lines">
      ↵ Wrap
    </button>

    {#if hasGit}
      <button class="btn icon" class:primary={showBlame} onclick={toggleBlame} disabled={blaming}>
        {blaming ? "…" : "◫ Blame"}
      </button>
    {/if}
    <span class="meta">{lang ?? "text"} · {lines.length} lines</span>
  </div>

  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="panebody" bind:this={body} onclick={clickBackground}>
    <div class="code" class:focusing={focus.active} class:wrap style:font-size="{fontSize}px">
      {#each highlighted as html, i}
        {@const n = i + 1}
        <div
          class="row"
          class:hit={focus.contains(n)}
          class:head={focus.isHead(n)}
          class:tail={focus.isTail(n)}
          class:related={focus.isRelated(n)}
          class:spec={focus.isSpec(n)}
          class:docline={docLines.has(n)}
          data-line={n}
        >
          {#if showBlame}
            {@const b = blameRows[i]}
            <span class="bl">
              {#if b}
                <i style:background={authorTone(b.author)}></i>
                <b>{b.show ? b.author : ""}</b>
                <em>{b.show ? b.when : ""}</em>
              {/if}
            </span>
          {/if}
          <span class="ln">{n}</span>
          <span class="src">{@html html || "&nbsp;"}</span>
        </div>
      {/each}
    </div>
  </div>

  {#if focus.active}
    <button class="focushint" onclick={() => focus.clear()}>
      <span>Reading <b>{focus.sig}</b></span>
      <span class="span">
        {focus.lineCount} lines{focus.clauseCount > 1 ? ` · ${focus.clauseCount} clauses` : ""}
      </span>
      <kbd>esc</kbd>
      <span>to exit</span>
    </button>
  {/if}
</div>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    height: 100%;
    position: relative;
  }
  .panebody {
    flex: 1;
    overflow: auto;
    background: var(--code-bg);
  }
  .meta {
    white-space: nowrap;
  }

  .fontsize {
    display: flex;
    align-items: stretch;
    border: 1px solid var(--line);
    border-radius: 5px;
    overflow: hidden;
  }
  .fontsize button {
    font: inherit;
    font-size: 11px;
    background: transparent;
    color: var(--fg-dim);
    border: 0;
    padding: 2px 7px;
    cursor: pointer;
    line-height: 1.6;
  }
  .fontsize button small {
    font-size: 9px;
    vertical-align: super;
  }
  .fontsize button:hover:not(:disabled) {
    background: var(--bg-inset);
    color: var(--fg);
  }
  .fontsize button:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .fontsize .val {
    font-family: var(--mono);
    font-size: 10px;
    min-width: 30px;
    border-left: 1px solid var(--line);
    border-right: 1px solid var(--line);
    color: var(--fg-faint);
  }

  .code {
    font-family: var(--mono);
    line-height: 1.65;
    padding: 10px 0 24px;
  }
  .row {
    display: flex;
    padding-right: 12px;
    transition:
      opacity 0.18s ease,
      background 0.18s ease;
  }
  .row .ln {
    width: 46px;
    flex: none;
    align-self: flex-start; /* stay level with the FIRST visual line when wrapped */
    text-align: right;
    padding-right: 14px;
    color: var(--fg-faint);
    opacity: 0.6;
    user-select: none; /* keep line numbers out of a copied selection */
  }
  .row .src {
    flex: 1 1 auto;
    min-width: 0;
    white-space: pre;
    user-select: text;
    cursor: text;
  }

  /* Soft wrap. `anywhere` rather than `break-word` because a single long
     string or URL would otherwise still force the pane sideways — and the
     whole point is that there is nothing to scroll to. The hanging indent
     makes a continuation visibly a continuation, not a new statement. */
  .code.wrap .row .src {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    padding-left: 2ch;
    text-indent: -2ch;
  }

  /* Mouse selection, made unmistakable. The webview's default selection tint
     is nearly invisible against the code background. */
  .code :global(::selection) {
    background: color-mix(in srgb, var(--accent) 28%, transparent);
    color: inherit;
  }

  /* ---- blame gutter ---- */
  .row .bl {
    display: flex;
    flex: none;
    width: 128px;
    overflow: hidden;
    align-items: baseline;
    gap: 6px;
    padding: 0 10px 0 8px;
    margin-right: 4px;
    border-right: 1px solid var(--line-soft);
    font-size: 10.5px;
    white-space: nowrap;
    user-select: none;
    align-self: flex-start;
  }
  .row .bl i {
    width: 3px;
    height: 11px;
    border-radius: 2px;
    flex: none;
    align-self: center;
    opacity: 0.8;
  }
  .row .bl b {
    font-weight: 500;
    color: var(--fg-faint);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .row .bl em {
    font-style: normal;
    color: var(--fg-faint);
    opacity: 0.6;
    margin-left: auto;
  }
  .row:hover .bl b,
  .row:hover .bl em {
    color: var(--fg-dim);
    opacity: 1;
  }

  /* ---- documentation reads as commentary ----
     Colour only: @moduledoc / @doc go grey like a comment, so they recede
     without a background band drawing attention to them. The override has to
     reach the hljs spans inside, hence the descendant selector. */
  .row.docline .src,
  .row.docline .src :global(*) {
    color: var(--syn-doc);
  }

  /* ---- module aliases ---- */
  .row .src :global(.mod) {
    color: var(--syn-mod);
  }

  /* ---- selection: whole body, all clauses, spec alongside ---- */
  .row.hit {
    background: var(--sel);
    box-shadow: inset 2px 0 0 color-mix(in srgb, var(--accent) 30%, transparent);
  }
  .row.hit .ln {
    opacity: 1;
    color: var(--accent);
  }
  /* Same function name at a different arity: present, but clearly secondary. */
  .row.related {
    background: color-mix(in srgb, var(--accent) 7%, transparent);
    box-shadow: inset 2px 0 0 color-mix(in srgb, var(--accent) 18%, transparent);
  }
  /* The @spec gets its own color — it's the contract, not the body. */
  .row.spec {
    background: color-mix(in srgb, var(--mark) 12%, transparent);
    box-shadow: inset 2px 0 0 var(--mark);
  }
  .row.spec .ln {
    opacity: 1;
    color: var(--mark);
  }
  .code.focusing .row:not(.hit):not(.related):not(.spec) {
    opacity: 0.32;
  }
  .row.hit.head {
    animation: rowPulse 2.1s ease-in-out infinite;
  }
  .row.hit.tail {
    box-shadow:
      inset 2px 0 0 color-mix(in srgb, var(--accent) 30%, transparent),
      inset 0 -1px 0 color-mix(in srgb, var(--accent) 25%, transparent);
  }
  @keyframes rowPulse {
    0%,
    100% {
      box-shadow: inset 2px 0 0 color-mix(in srgb, var(--accent) 35%, transparent);
    }
    50% {
      box-shadow: inset 3px 0 0 var(--accent);
    }
  }

  /* ---- "esc to exit" pill ---- */
  .focushint {
    position: absolute;
    left: 50%;
    bottom: 16px;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 5px 11px;
    border-radius: 999px;
    background: var(--bg-raised);
    border: 1px solid var(--line);
    box-shadow: 0 4px 16px rgba(16, 24, 40, 0.12);
    font: inherit;
    font-size: 11px;
    color: var(--fg-dim);
    white-space: nowrap;
    cursor: pointer;
    animation: hintIn 0.16s ease-out;
  }
  .focushint:hover {
    color: var(--fg);
    border-color: var(--fg-faint);
  }
  .focushint .span {
    font-family: var(--mono);
    color: var(--fg-faint);
  }
  .focushint kbd {
    font-family: var(--mono);
    font-size: 10px;
    border: 1px solid var(--line);
    border-bottom-width: 2px;
    border-radius: 4px;
    padding: 1px 5px;
    color: var(--fg);
  }
  @keyframes hintIn {
    from {
      opacity: 0;
      transform: translate(-50%, 6px);
    }
    to {
      opacity: 1;
      transform: translate(-50%, 0);
    }
  }
</style>
