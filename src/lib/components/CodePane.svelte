<script lang="ts">
  import hljs from "highlight.js/lib/core";
  import elixir from "highlight.js/lib/languages/elixir";
  import { blameFile, type BlameLine, type Outline } from "$lib/ipc";
  import { focus } from "$lib/stores/focus.svelte";
  import { displaySig, locate } from "$lib/select";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import FontStepper from "$lib/components/FontStepper.svelte";
  import { fontSize } from "$lib/stores/fontSize.svelte";

  hljs.registerLanguage("elixir", elixir);

  let {
    source = "",
    lang = null,
    filename = "",
    path = "",
    hasGit = false,
    outline = null,
    keysEnabled = true,
  }: {
    source: string;
    lang: string | null;
    filename: string;
    path: string;
    hasGit: boolean;
    outline: Outline | null;
    /** False while a modal is up — motions must not fire behind a dialog. */
    keysEnabled: boolean;
  } = $props();

  let body = $state<HTMLDivElement | null>(null);
  let blame = $state<BlameLine[]>([]);
  let showBlame = $state(false);
  let blaming = $state(false);
  /** Brief confirmation for copies — the path, or a yanked line. */
  let toast = $state("");
  let toastTimer: ReturnType<typeof setTimeout> | null = null;

  function flash(message: string) {
    toast = message;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = ""), 1600);
  }

  /**
   * The filename is a copy button: the path is what you need to open the file
   * in your editor, and retyping it from the screen is the tax this removes.
   */
  async function copyPath() {
    if (!path) return;
    try {
      await writeText(path);
      flash("path copied ✓");
    } catch {
      /* no clipboard in a plain browser — nothing worth interrupting for */
    }
  }

  // ---- font size ----------------------------------------------------------
  const font = fontSize("codeFontSize", 12.5);
  $effect(() => font.load());

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
  // multi-line constructs (heredocs, block comments) right. Kept separate from
  // the search pass below so typing a query doesn't re-run any of this.
  const syntaxLines = $derived.by(() => {
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
    return colorModules(html)
      .split("\n")
      .map((lineHtml, i) => {
        const n = i + 1;
        const def = defLines.get(n);
        if (def) return markWord(lineHtml, def.name, `data-sig="${def.sig}"`);

        const block = blockLines.get(n);
        if (block) {
          const label = block.label.replace(/"/g, "&quot;");
          return markWord(
            lineHtml,
            block.word,
            `data-start="${block.start}" data-end="${block.end}" data-label="${label}"`,
          );
        }
        return lineHtml;
      });
  });

  /** Syntax + search marks. Only this pass re-runs while you type a query. */
  const highlighted = $derived.by(() => {
    if (!query) return syntaxLines;
    let n = 0;
    return syntaxLines.map((lineHtml) => {
      const [html, used] = markMatches(lineHtml, query, caseSensitive, n);
      n += used;
      return html;
    });
  });

  // ---- search ------------------------------------------------------------
  //
  // Every occurrence is marked, not just the one you jumped to — seeing where a
  // name appears across the file is most of why you searched for it.

  let searching = $state(false);
  let query = $state("");
  let queryInput = $state<HTMLInputElement | null>(null);
  let matchIdx = $state(0);

  /** smartcase, as vim does it: a lowercase query ignores case, any capital doesn't. */
  const caseSensitive = $derived(/[A-Z]/.test(query));

  /** One entry per occurrence, in file order. */
  const matches = $derived.by(() => {
    if (!query) return [] as { line: number; col: number }[];
    const out: { line: number; col: number }[] = [];
    const needle = caseSensitive ? query : query.toLowerCase();
    lines.forEach((raw, i) => {
      const hay = caseSensitive ? raw : raw.toLowerCase();
      let from = 0;
      for (;;) {
        const at = hay.indexOf(needle, from);
        if (at === -1) break;
        out.push({ line: i + 1, col: at });
        from = at + Math.max(needle.length, 1);
      }
    });
    return out;
  });

  function decodeEntities(t: string): string {
    return t
      .replace(/&lt;/g, "<")
      .replace(/&gt;/g, ">")
      .replace(/&quot;/g, '"')
      .replace(/&#x27;|&#39;/g, "'")
      .replace(/&amp;/g, "&");
  }

  /**
   * Wrap every occurrence in one line of highlighted HTML.
   *
   * Tag-aware like colorModules, but it also decodes entities before matching
   * and re-escapes after: hljs writes `|&gt;`, so a search for `|>` would never
   * match the raw HTML. Returns the line plus how many marks it added, so the
   * caller can keep a file-wide match index.
   */
  function markMatches(
    html: string,
    needle: string,
    cased: boolean,
    startIndex: number,
  ): [string, number] {
    const parts = html.split(/(<[^>]+>)/);
    const find = cased ? needle : needle.toLowerCase();
    let used = 0;

    const out = parts.map((part) => {
      if (part.startsWith("<") || !part) return part;
      const text = decodeEntities(part);
      const hay = cased ? text : text.toLowerCase();

      let built = "";
      let from = 0;
      for (;;) {
        const at = hay.indexOf(find, from);
        if (at === -1) break;
        built +=
          escapeAll(text.slice(from, at)) +
          `<mark class="sm" data-m="${startIndex + used}">` +
          escapeAll(text.slice(at, at + needle.length)) +
          `</mark>`;
        used++;
        from = at + Math.max(needle.length, 1);
      }
      return from === 0 ? part : built + escapeAll(text.slice(from));
    });

    return [out.join(""), used];
  }

  function openSearch() {
    searching = true;
    // If the bar is already on screen (confirmed a search, then pressed `/`
    // again) the element exists and can be focused now. On a fresh open it
    // doesn't exist yet — Svelte hasn't rendered it — so the effect below
    // catches that case once the binding lands.
    queryInput?.focus();
    queryInput?.select();
  }

  // Focus the query field the moment it exists, so `/` puts you straight into
  // typing the way vim does. Depends only on `searching` and the binding, so
  // confirming a search (which blurs) never steals focus back.
  $effect(() => {
    if (searching && queryInput) {
      queryInput.focus();
      queryInput.select();
    }
  });

  function closeSearch() {
    searching = false;
    query = "";
    matchIdx = 0;
  }

  /** Land on the first match at or after wherever the reader is. */
  function confirmSearch() {
    if (!matches.length) return;
    const from = focus.cursorLine ?? 1;
    const at = matches.findIndex((m) => m.line >= from);
    matchIdx = at === -1 ? 0 : at;
    gotoMatch(matchIdx);
    queryInput?.blur();
  }

  function stepMatch(dir: 1 | -1) {
    if (!matches.length) return;
    matchIdx = (matchIdx + dir + matches.length) % matches.length;
    gotoMatch(matchIdx);
  }

  function gotoMatch(i: number) {
    const m = matches[i];
    if (!m) return;
    focus.gotoLine(m.line, lines.length);
  }

  // Mark the current occurrence by class rather than by rebuilding the HTML —
  // stepping with n/N shouldn't re-render the whole file.
  $effect(() => {
    const i = matchIdx;
    const _ = query;
    if (!body) return;
    for (const el of body.querySelectorAll(".sm")) {
      el.classList.toggle("on", (el as HTMLElement).dataset.m === String(i));
    }
  });

  function escapeAll(s: string): string {
    return s.replace(/[&<>]/g, (c) => (c === "&" ? "&amp;" : c === "<" ? "&lt;" : "&gt;"));
  }

  /**
   * Every line that starts a clause, and the function it belongs to. Used to
   * turn the name in `def foo(x) do` into a click target.
   */
  const defLines = $derived.by(() => {
    const map = new Map<number, { name: string; sig: string }>();
    for (const f of outline?.modules?.[0]?.functions ?? []) {
      const entry = { name: f.name, sig: displaySig(f) };
      for (const r of f.clauseRanges ?? []) map.set(r.start, entry);
      map.set(f.line, entry);
    }
    return map;
  });

  /**
   * Wrap the first occurrence of a word on a line in a click target.
   *
   * Tag-aware, like colorModules: split on tags and only rewrite text, so the
   * wrapper nests inside whatever span hljs already put the word in rather than
   * breaking it. Only the first occurrence is wrapped — on a `def` line that is
   * always the name being defined, and on a `test` line it is always the
   * keyword opening the block.
   */
  function markWord(html: string, word: string, attrs: string): string {
    const parts = html.split(/(<[^>]+>)/);
    let done = false;

    return parts
      .map((part) => {
        if (done || part.startsWith("<")) return part;
        // `(?![\w!?])` stops `get_user` matching inside `get_user!`, and
        // `setup` matching inside `setup_all`.
        const re = new RegExp(`(^|[^\\w])(${word.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})(?![\\w!?])`);
        const m = re.exec(part);
        if (!m) return part;
        done = true;
        return (
          part.slice(0, m.index) +
          m[1] +
          `<span class="fname" ${attrs} role="button" tabindex="0">${m[2]}</span>` +
          part.slice(m.index + m[0].length)
        );
      })
      .join("");
  }

  /**
   * The other kinds of block a line can open — `test`, `describe`, `setup`,
   * `config` — keyed by the line the keyword sits on. A test file has no
   * functions, so without this its code pane would have nothing to click.
   */
  const blockLines = $derived.by(() => {
    const map = new Map<number, { word: string; label: string; start: number; end: number }>();

    for (const s of outline?.tests?.setups ?? []) {
      map.set(s.line, { word: s.kind, label: s.kind, start: s.line, end: s.endLine });
    }
    for (const d of outline?.tests?.describes ?? []) {
      if (d.name) {
        map.set(d.line, { word: "describe", label: d.name, start: d.line, end: d.endLine });
      }
      for (const s of d.setups) {
        map.set(s.line, { word: s.kind, label: s.kind, start: s.line, end: s.endLine });
      }
      for (const t of d.tests) {
        map.set(t.line, { word: "test", label: t.name, start: t.line, end: t.endLine });
      }
    }
    for (const g of outline?.config?.groups ?? []) {
      const label = g.target ? `${g.app} ${g.target}` : g.app;
      map.set(g.line, { word: "config", label, start: g.line, end: g.endLine });
    }
    return map;
  });

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

  /**
   * A hue per author, handed out in order of first appearance rather than
   * hashed from the name: hashing produces near-identical colours for unlucky
   * pairs of names, and the point is telling people apart. Evenly spread, so
   * the first eight authors in a file are always clearly distinct.
   */
  const HUES = [212, 150, 35, 280, 340, 190, 95, 18];

  const authorHues = $derived.by(() => {
    const map = new Map<string, number>();
    for (const b of blame) {
      if (b.author && !map.has(b.author)) map.set(b.author, HUES[map.size % HUES.length]);
    }
    return map;
  });

  const blameRows = $derived.by(() =>
    blame.map((b, i) => ({
      ...b,
      hue: authorHues.get(b.author) ?? 212,
      show: i === 0 || blame[i - 1]?.author !== b.author,
      /** Last line of a run by this author — closes the stripe. */
      last: i === blame.length - 1 || blame[i + 1]?.author !== b.author,
    })),
  );

  // ---- sticky function header -------------------------------------------
  // Deep inside a long function you lose track of which one you're in. This
  // pins the enclosing signature to the top of the pane.

  /** First line currently visible at the top of the scroll container. */
  let topLine = $state(1);
  let rafPending = false;

  /**
   * The line whose row occupies a given scroll offset.
   *
   * Binary search by `offsetTop` rather than dividing by a line height: with
   * soft wrap on, rows have different heights and the arithmetic lies.
   */
  function lineAtOffset(offset: number): number {
    if (!body) return 1;
    const rows = body.querySelectorAll<HTMLElement>(".row");
    if (!rows.length) return 1;
    let lo = 0;
    let hi = rows.length - 1;
    let hit = 0;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (rows[mid].offsetTop <= offset) {
        hit = mid;
        lo = mid + 1;
      } else {
        hi = mid - 1;
      }
    }
    return hit + 1;
  }

  /** First and last line currently on screen — what H, M, L and ⌃d work from. */
  function visibleRange(): { top: number; bottom: number } {
    if (!body) return { top: 1, bottom: 1 };
    return {
      top: lineAtOffset(body.scrollTop),
      bottom: lineAtOffset(body.scrollTop + body.clientHeight - 1),
    };
  }

  function onScroll() {
    if (rafPending || !body) return;
    rafPending = true;
    requestAnimationFrame(() => {
      rafPending = false;
      topLine = lineAtOffset(body?.scrollTop ?? 0);
    });
  }

  // Resizing rewraps every line, so row heights change and the sticky header's
  // answer goes stale until the next scroll. Recompute it directly.
  $effect(() => {
    const onResize = () => onScroll();
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  });

  /**
   * The function enclosing the top visible line — but only once its own `def`
   * line has scrolled out of sight. While the definition is on screen the
   * header would just be repeating it.
   */
  const sticky = $derived.by(() => {
    const n = topLine;
    for (const f of outline?.modules?.[0]?.functions ?? []) {
      for (const r of f.clauseRanges ?? []) {
        if (n > r.start && n <= r.end) {
          return { sig: displaySig(f), visibility: f.visibility, line: r.start };
        }
      }
    }
    return null;
  });

  // Scroll the focused definition into view. scrollIntoView clamps at the end
  // of the document, so no padding is needed to make centering work.
  $effect(() => {
    const line = focus.line;
    if (!line || !body) return;
    const row = body.querySelector<HTMLElement>(`[data-line="${line}"]`);
    row?.scrollIntoView({ behavior: "smooth", block: "center" });
  });

  // Keep the review cursor visible as it steps, without yanking the view when
  // it is already on screen.
  $effect(() => {
    const n = focus.cursorLine;
    if (!n || !body) return;
    body.querySelector<HTMLElement>(`[data-line="${n}"]`)?.scrollIntoView({ block: "nearest" });
  });

  // ---- vim motions -------------------------------------------------------
  //
  // Only motions, because the pane is read-only: there is no insert mode, no
  // operators, no registers, nothing to undo. What's left is the part of vim
  // that is actually about reading.
  //
  // Two deliberate deviations from vim: `?` opens help rather than searching
  // backwards (it is the web convention, and lgtm had it first), and `[`/`]`
  // step functions with one keypress instead of `[[`/`]]`.

  /** Buffer for two-key sequences: gg, zz, yy. */
  let pending = $state("");
  /** Digits typed before a motion — 5j, 42G. */
  let count = $state("");
  let seqTimer: ReturnType<typeof setTimeout> | null = null;

  function resetSeq() {
    pending = "";
    count = "";
    if (seqTimer) clearTimeout(seqTimer);
    seqTimer = null;
  }

  /** A half-finished sequence expires, the way vim's timeoutlen does. */
  function armSeq() {
    if (seqTimer) clearTimeout(seqTimer);
    seqTimer = setTimeout(resetSeq, 700);
  }

  function takeCount(fallback = 1): number {
    const n = parseInt(count, 10);
    return Number.isFinite(n) && n > 0 ? n : fallback;
  }

  const total = $derived(lines.length);

  function centreCursor() {
    const n = focus.cursorLine;
    if (!n || !body) return;
    body.querySelector<HTMLElement>(`[data-line="${n}"]`)?.scrollIntoView({ block: "center" });
  }

  /**
   * `}` — forward to the end of this blank-line-separated block. In Elixir that
   * lands on function and pipeline boundaries without any parsing, which is why
   * paragraph motion is more useful here than it looks.
   */
  function paragraph(dir: 1 | -1) {
    const blank = (n: number) => (lines[n - 1] ?? "").trim() === "";
    let i = focus.cursorLine ?? (dir === 1 ? 0 : total + 1);

    i += dir;
    while (i >= 1 && i <= total && blank(i)) i += dir;
    while (i >= 1 && i <= total && !blank(i)) i += dir;
    focus.gotoLine(Math.min(Math.max(i, 1), total), total);
  }

  /** `[` / `]` — previous / next definition, selected whole. */
  function jumpFunction(dir: 1 | -1) {
    const fns = [...(outline?.modules?.[0]?.functions ?? [])].sort((a, b) => a.line - b.line);
    if (!fns.length) return;
    const from = focus.cursorLine ?? focus.ranges[0]?.start ?? 0;
    const next = dir === 1 ? fns.find((f) => f.line > from) : [...fns].reverse().find((f) => f.line < from);
    if (!next) return; // stay put at either end rather than wrapping
    const sig = displaySig(next);
    const at = locate(sig, outline?.modules?.[0] ?? null);
    if (at) focus.select(sig, at.ranges, at.related, at.spec, at.doc);
  }

  async function yankLine() {
    const n = focus.cursorLine;
    if (!n) return;
    try {
      await writeText(lines[n - 1] ?? "");
      flash(`line ${n} copied ✓`);
    } catch {
      /* no clipboard outside the app shell */
    }
  }

  function onVimKey(e: KeyboardEvent) {
    if (!keysEnabled || e.metaKey || e.altKey) return;
    const el = e.target as HTMLElement | null;
    if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable)) return;

    // ⌃d / ⌃u — half a screen, the way vim measures it: from what's on screen.
    if (e.ctrlKey) {
      if (e.key !== "d" && e.key !== "u") return;
      e.preventDefault();
      const { top, bottom } = visibleRange();
      const half = Math.max(1, Math.floor((bottom - top) / 2));
      focus.moveCursor(e.key === "d" ? half : -half, total);
      resetSeq();
      return;
    }

    // Counts. A leading 0 is a column motion in vim and meaningless here, so it
    // only counts as a digit once a count is under way.
    if (/^[0-9]$/.test(e.key) && !(e.key === "0" && count === "")) {
      count += e.key;
      armSeq();
      e.preventDefault();
      return;
    }

    // Two-key sequences.
    if (pending) {
      const seq = pending + e.key;
      const n = takeCount(0);
      resetSeq();
      if (seq === "gg") {
        e.preventDefault();
        focus.gotoLine(n || 1, total);
        return;
      }
      if (seq === "zz") {
        e.preventDefault();
        centreCursor();
        return;
      }
      if (seq === "yy") {
        e.preventDefault();
        yankLine();
        return;
      }
      // Not a sequence we know — fall through and treat the key on its own.
    }

    if (e.key === "g" || e.key === "z" || e.key === "y") {
      pending = e.key;
      armSeq();
      e.preventDefault();
      return;
    }

    if (e.key === "Escape" && query) {
      closeSearch();
      // The page's Escape also clears the selection — "back to plain code" is
      // one keypress, not two.
      return;
    }
    if (e.key === "/") {
      e.preventDefault();
      resetSeq();
      openSearch();
      return;
    }
    if (matches.length && (e.key === "n" || e.key === "N")) {
      e.preventDefault();
      resetSeq();
      stepMatch(e.key === "n" ? 1 : -1);
      return;
    }

    const n = takeCount();
    const { top, bottom } = visibleRange();
    let handled = true;

    switch (e.key) {
      case "j":
      case "ArrowDown":
        focus.moveCursor(n, total);
        break;
      case "k":
      case "ArrowUp":
        focus.moveCursor(-n, total);
        break;
      case "G":
        focus.gotoLine(count ? n : total, total);
        break;
      case "H":
        focus.gotoLine(top, total);
        break;
      case "M":
        focus.gotoLine(Math.floor((top + bottom) / 2), total);
        break;
      case "L":
        focus.gotoLine(bottom, total);
        break;
      case "}":
        paragraph(1);
        break;
      case "{":
        paragraph(-1);
        break;
      case "]":
        jumpFunction(1);
        break;
      case "[":
        jumpFunction(-1);
        break;
      default:
        handled = false;
    }

    if (handled) e.preventDefault();
    resetSeq();
  }

  /**
   * Whether to dim the rest of the file.
   *
   * Dimming answers "show me this one function". Blame and search ask the
   * opposite kind of question — who wrote all of this, where does this name
   * appear — and dimming 68% of the file hides precisely the answer. So while
   * either is active the selection keeps its own highlight (you don't lose your
   * place, and the pill still says where you are) but stops suppressing
   * everything else.
   */
  const dimming = $derived(focus.active && !showBlame && !query);
  /** In a guided reading the file should recede further — you are being led. */
  const guided = $derived(focus.reading && dimming);

  function onCodeClick(e: MouseEvent) {
    // A drag that selects text ends in a click; don't treat that as a click.
    if (window.getSelection()?.toString()) return;

    // A function name selects the whole unit — every clause, its @spec and its
    // @doc — the same thing clicking its row in the explanation does.
    const name = (e.target as HTMLElement).closest<HTMLElement>(".fname[data-sig]");
    if (name) {
      const sig = name.dataset.sig ?? "";
      const at = locate(sig, outline?.modules?.[0] ?? null);
      if (at) {
        focus.set(sig, at.ranges, at.related, at.spec, at.doc);
        return;
      }
    }

    // `test`, `describe`, `setup`, `config` — not functions, but still blocks,
    // and selecting one covers the whole thing.
    const block = (e.target as HTMLElement).closest<HTMLElement>(".fname[data-start]");
    if (block) {
      const start = parseInt(block.dataset.start ?? "0", 10);
      const end = parseInt(block.dataset.end ?? "0", 10) || start;
      if (start > 0) {
        focus.set(block.dataset.label ?? `line ${start}`, [{ start, end }]);
        return;
      }
    }

    const row = (e.target as HTMLElement).closest<HTMLElement>(".row[data-line]");
    if (!row) {
      focus.clear();
      return;
    }
    // Anywhere else on a line puts the review cursor on it — one line at a
    // time, a different question from "show me this whole function".
    focus.setCursor(parseInt(row.dataset.line ?? "0", 10));
  }

  function onCodeKey(e: KeyboardEvent) {
    if (e.key !== "Enter" && e.key !== " ") return;
    const el = (e.target as HTMLElement).closest<HTMLElement>(".fname");
    if (!el) return;
    e.preventDefault();

    if (el.dataset.sig) {
      const at = locate(el.dataset.sig, outline?.modules?.[0] ?? null);
      if (at) focus.set(el.dataset.sig, at.ranges, at.related, at.spec, at.doc);
      return;
    }
    const start = parseInt(el.dataset.start ?? "0", 10);
    const end = parseInt(el.dataset.end ?? "0", 10) || start;
    if (start > 0) focus.set(el.dataset.label ?? `line ${start}`, [{ start, end }]);
  }
</script>

<svelte:window onkeydown={onVimKey} />

<div class="pane">
  <div class="panehead">
    {#if filename}
      <button class="name" onclick={copyPath} title="Click to copy the full path&#10;{path}">
        {filename}
      </button>
      {#if toast}<span class="copied">{toast}</span>{/if}
    {:else}
      <span>no file</span>
    {/if}
    <span class="spacer"></span>

    <FontStepper {font} label="code" />

  </div>

  <!-- The stage holds only the code and the things that float over it. The
       search bar and footer live outside it, so an overlay anchored to the
       stage's bottom can never collide with them. -->
  <div class="stage">
    {#if sticky}
    <button
      class="sticky"
      class:priv={sticky.visibility === "private"}
      onclick={() => {
        const at = locate(sticky.sig, outline?.modules?.[0] ?? null);
        if (at) focus.set(sticky.sig, at.ranges, at.related, at.spec, at.doc);
      }}
      title="Jump to the definition"
    >
      <span class="bar"></span>
      <span class="sig">{sticky.sig}</span>
      <span class="at">line {sticky.line}</span>
    </button>
    {/if}

    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="panebody" bind:this={body} onclick={onCodeClick} onkeydown={onCodeKey} onscroll={onScroll}>
      <div class="code" class:focusing={dimming} class:guided style:font-size="{font.value}px">
        {#each highlighted as html, i}
          {@const n = i + 1}
          <div
            class="row"
            class:hit={focus.contains(n)}
            class:head={focus.isHead(n)}
            class:tail={focus.isTail(n)}
            class:related={focus.isRelated(n)}
            class:spec={focus.isSpec(n)}
            class:docsel={focus.isDoc(n)}
          class:leaving={focus.isLeaving(n)}
            class:docline={docLines.has(n)}
            class:cursor={focus.cursorLine === n}
            class:authored={showBlame && !!blameRows[i]}
            style:--who-h={showBlame ? (blameRows[i]?.hue ?? 212) : undefined}
            data-line={n}
          >
            {#if showBlame}
              {@const b = blameRows[i]}
              <span class="bl" class:runstart={b?.show} class:runend={b?.last}>
                {#if b}
                  <i></i>
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

    {#if focus.anything}
      <button class="focushint" onclick={() => focus.clear()}>
        {#if focus.active}
          <span>Reading <b>{focus.sig}</b></span>
          <span class="span">
            {focus.lineCount} lines{focus.clauseCount > 1 ? ` · ${focus.clauseCount} clauses` : ""}
          </span>
        {/if}
        {#if focus.cursorLine !== null}
          <span class="span">line {focus.cursorLine}</span>
          <kbd>↑</kbd><kbd>↓</kbd>
        {/if}
        <kbd>esc</kbd>
        <span>to exit</span>
      </button>
    {/if}
  </div>

  {#if searching || query}
    <div class="searchbar">
      <span class="slash">/</span>
      <input
        bind:this={queryInput}
        bind:value={query}
        placeholder="find a name, a param, anything…"
        spellcheck="false"
        autocapitalize="off"
        onkeydown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            confirmSearch();
          } else if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            closeSearch();
          }
        }}
      />
      <span class="count" class:none={query && !matches.length}>
        {#if !query}
          &nbsp;
        {:else if matches.length}
          {matchIdx + 1}/{matches.length}
          {matches.length === 1 ? "match" : "matches"}
        {:else}
          no matches
        {/if}
      </span>
      {#if caseSensitive}<span class="flag" title="A capital letter makes the search case-sensitive">Aa</span>{/if}
      <button class="btn icon" onclick={() => stepMatch(-1)} disabled={!matches.length} title="Previous (N)">↑</button>
      <button class="btn icon" onclick={() => stepMatch(1)} disabled={!matches.length} title="Next (n)">↓</button>
      <button class="btn icon" onclick={closeSearch} title="Clear (Esc)">×</button>
    </div>
  {/if}

  <div class="panefoot">
    <span class="meta">{lang ?? "text"} · {lines.length} lines</span>
    <span class="spacer"></span>
    {#if hasGit}
      <button
        class="btn icon"
        class:primary={showBlame}
        onclick={toggleBlame}
        disabled={blaming}
        title="Who last touched each line (runs git blame)"
      >
        {blaming ? "…" : "◫ Blame"}
      </button>
    {/if}
  </div>
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
  .stage {
    flex: 1;
    min-height: 0;
    position: relative;
    display: flex;
    flex-direction: column;
  }
  .panebody {
    flex: 1;
    overflow: auto;
    background: var(--code-bg);
  }
  .meta {
    white-space: nowrap;
  }

  /* The filename is a button, but must not read as one until you reach for it. */
  .name {
    font: inherit;
    font-size: inherit;
    letter-spacing: inherit;
    text-transform: inherit;
    color: var(--fg-dim);
    background: none;
    border: 0;
    padding: 0;
    cursor: pointer;
  }
  .name:hover {
    color: var(--fg);
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .copied {
    color: var(--pub);
    text-transform: none;
    letter-spacing: 0;
    font-size: 10.5px;
    animation: hintIn var(--fast) var(--ease-out);
  }

  /* Floats over the code rather than sitting in the flow, so appearing and
     disappearing never shifts the lines you are reading. */
  .sticky {
    position: absolute;
    top: 0; /* the stage already starts below the pane header */
    left: 0;
    right: 0;
    z-index: 3;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 12px;
    border: 0;
    border-bottom: 1px solid var(--line);
    background: color-mix(in srgb, var(--bg-raised) 94%, transparent);
    backdrop-filter: blur(6px);
    font: inherit;
    font-size: 11.5px;
    text-align: left;
    cursor: pointer;
    animation: hintIn var(--fast) var(--ease-out);
  }
  .sticky:hover {
    background: var(--bg-inset);
  }
  .sticky .bar {
    width: 3px;
    height: 12px;
    border-radius: 2px;
    background: var(--pub);
    flex: none;
  }
  .sticky.priv .bar {
    background: var(--priv);
  }
  .sticky .sig {
    font-family: var(--mono);
    color: var(--fg);
  }
  .sticky .at {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg-faint);
    margin-left: auto;
  }

  /* Search sits at the bottom, where vim puts it, and stays visible while a
     search is live so the match count and n/N are never invisible state. */
  .searchbar {
    flex: none;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 5px 10px;
    border-top: 1px solid var(--line);
    background: var(--bg-raised);
  }
  .searchbar .slash {
    font-family: var(--mono);
    font-size: 13px;
    color: var(--accent);
  }
  .searchbar input {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-family: var(--mono);
    font-size: 12px;
    padding: 3px 6px;
    border: 1px solid transparent;
    border-radius: 5px;
    background: var(--bg);
    color: var(--fg);
    outline: none;
  }
  .searchbar input:focus {
    border-color: var(--accent);
  }
  .searchbar .count {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg-faint);
    white-space: nowrap;
  }
  .searchbar .count.none {
    color: var(--priv);
  }
  .searchbar .flag {
    font-family: var(--mono);
    font-size: 10px;
    color: var(--fg-dim);
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 4px;
  }

  /* Every occurrence is marked; the one you are on is marked harder. */
  .code :global(mark.sm) {
    background: color-mix(in srgb, var(--syn-atom) 34%, transparent);
    color: inherit;
    border-radius: 2px;
    padding: 0 1px;
  }
  .code :global(mark.sm.on) {
    background: var(--syn-atom);
    color: #fff;
    box-shadow: 0 0 0 1px var(--syn-atom);
  }

  /* The pane's own footer: what this file is, and the one control that reads
     history. Both belong at the bottom, out of the way of the code. */
  .panefoot {
    height: 26px;
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    border-top: 1px solid var(--line-soft);
    background: var(--bg-raised);
    font-size: 11px;
    color: var(--fg-faint);
  }
  .panefoot .spacer {
    flex: 1;
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
      background 0.18s ease,
      box-shadow 0.18s ease;
  }
  .code.guided .row {
    transition:
      opacity 0.35s ease,
      background 0.35s ease,
      box-shadow 0.35s ease;
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
    user-select: text;
    cursor: text;
  }

  /* Always soft-wrapped: reading a file should never mean scrolling sideways
     to finish a line, and there is no case where that trade is worth it.
     `anywhere` rather than `break-word` because a single long string or URL has
     no break opportunity and would still force the pane wide. The hanging
     indent makes a continuation visibly a continuation, not a new statement. */
  .code .row .src {
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
  /* One continuous stripe per author rather than a dash per line, so a run of
     lines by the same person reads as a single block. */
  .row .bl i {
    width: 3px;
    flex: none;
    align-self: stretch;
    background: hsl(var(--who-h) 62% 48%);
  }
  .row .bl.runstart i {
    border-top-left-radius: 2px;
    border-top-right-radius: 2px;
  }
  .row .bl.runend i {
    border-bottom-left-radius: 2px;
    border-bottom-right-radius: 2px;
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

  /* Each author's lines tinted with their hue. Deliberately faint — it sits
     under code and must not compete with it.
     ONE rule at two classes' specificity, with the per-theme strength coming
     from tokens rather than a `html.dark` override: that override would carry
     an extra element selector and quietly outrank every selection state below,
     so in dark mode the blame tint would beat focus, spec, doc and cursor. */
  .row.authored {
    background: hsl(var(--who-h) 70% var(--who-l) / var(--who-a));
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

  /* ---- a defined name is a click target ----
     Underlined only on hover: a permanent underline on every def line would
     turn the file into a page of links. */
  .row .src :global(.fname) {
    cursor: pointer;
    border-radius: 3px;
  }
  .row .src :global(.fname:hover) {
    text-decoration: underline;
    text-decoration-thickness: 1.5px;
    text-underline-offset: 2px;
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .row .src :global(.fname:focus-visible) {
    outline: 1px solid var(--accent);
    outline-offset: 1px;
  }

  /* The @doc of a selected function: present, but the faintest of the three
     weights — dashed marker for the prose, violet for the contract, solid
     accent for the body. */
  .code .row.docsel {
    background: color-mix(in srgb, var(--accent) 7%, transparent);
    box-shadow: inset 2px 0 0 color-mix(in srgb, var(--accent) 22%, transparent);
  }

  /* ---- selection: whole body, all clauses, spec alongside ---- */
  .code .row.hit {
    background: var(--sel);
    box-shadow: inset 2px 0 0 color-mix(in srgb, var(--accent) 30%, transparent);
  }
  .code .row.hit .ln {
    opacity: 1;
    color: var(--accent);
  }
  /* Same function name at a different arity: present, but clearly secondary. */
  .code .row.related {
    background: color-mix(in srgb, var(--accent) 7%, transparent);
    box-shadow: inset 2px 0 0 color-mix(in srgb, var(--accent) 18%, transparent);
  }
  /* The @spec gets its own color — it's the contract, not the body. */
  .code .row.spec {
    background: color-mix(in srgb, var(--mark) 12%, transparent);
    box-shadow: inset 2px 0 0 var(--mark);
  }
  .code .row.spec .ln {
    opacity: 1;
    color: var(--mark);
  }
  .code.focusing .row:not(.hit):not(.related):not(.spec):not(.docsel):not(.leaving) {
    opacity: 0.32;
  }
  /* Deeper while a reading is driving: 32% is right for "show me this one
     function", but a guided reading should push the rest further back. */
  .code.guided .row:not(.hit):not(.related):not(.spec):not(.docsel):not(.leaving) {
    opacity: 0.16;
  }
  /* The crossfade: the range you are leaving lingers at a lower weight while
     the new one arrives, so for a beat you can see both. That overlap is what
     makes a jump legible instead of a cut. */
  .code .row.leaving {
    background: color-mix(in srgb, var(--accent) 7%, transparent);
    box-shadow: inset 2px 0 0 color-mix(in srgb, var(--accent) 30%, transparent);
    opacity: 1;
  }
  .code .row.leaving .ln {
    opacity: 0.8;
    color: var(--accent);
  }

  .code .row.hit.head {
    animation: rowPulse var(--slow) ease-in-out infinite;
  }
  /* The rule for every reduced-motion block in this app: the motion stops, the
     MEANING survives. This bar is how you find the top of a selection, so it
     keeps the bright end of its own pulse rather than disappearing. Honouring
     the setting by removing the signal would be worse than ignoring it. */
  @media (prefers-reduced-motion: reduce) {
    .code .row.hit.head {
      animation: none;
      box-shadow: inset 3px 0 0 var(--accent);
    }
    /* The pills and the sticky header: they translate as they fade in, and a
       translate is the part of "arrival" that reduced motion is actually asking
       about. They still appear — just without travelling to get there. */
    .copied,
    .sticky,
    .focushint {
      animation: none;
    }
  }
  .code .row.hit.tail {
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

  /* The review cursor: one line, marked hard, always full opacity even when the
     rest of the file is dimmed — it is the line you are on. Declared last, and
     it cancels the head pulse: on a shared line the cursor is the stronger
     signal, and an animation would otherwise override this box-shadow. */
  .code .row.cursor {
    background: color-mix(in srgb, var(--accent) 20%, transparent);
    box-shadow:
      inset 3px 0 0 var(--accent),
      inset 0 1px 0 color-mix(in srgb, var(--accent) 32%, transparent),
      inset 0 -1px 0 color-mix(in srgb, var(--accent) 32%, transparent);
    opacity: 1;
    animation: none;
  }
  .code .row.cursor .ln {
    opacity: 1;
    color: var(--accent);
    font-weight: 600;
  }

  /* ---- "esc to exit" pill ---- */
  .focushint {
    position: absolute;
    left: 50%;
    /* Anchored to the stage, so the search bar and footer can come and go
       without the pill ever landing on top of them. */
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
    animation: hintIn var(--fast) var(--ease-out);
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
