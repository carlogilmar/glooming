<script lang="ts">
  // `/` in the editor — insert a reference without leaving the keyboard.
  //
  // The list is free: the outline is already parsed by the time you can type, so
  // this is a filter over data we have rather than any new analysis.
  //
  // Anchored to the caret via a mirror element. Measuring a textarea caret is
  // the one fiddly part: there is no API for it, so a hidden div copies the
  // textarea's own metrics, is filled with the text up to the caret, and a span
  // at the end reports where that lands.
  import type { VocabEntry } from "$lib/fileset";
  import { COMMANDS, parseSlash, type BlockKind, type FileOption } from "$lib/slash";

  let {
    entries = [],
    currentFile = "",
    targets = [],
    query = "",
    x = 0,
    y = 0,
    flip = false,
    onpick,
    oncommand,
    onclose,
  }: {
    /**
     * Everything referenceable in the reading, in strip order.
     *
     * This is the one thing adding a file to a reading changes — no seeding, no
     * new block, no edit to your prose, just a wider vocabulary. Which means the
     * menu has to say *where* each function lives, and insert a qualified name
     * when it is somewhere else.
     */
    entries: VocabEntry[];
    /** Name of the file on screen — what a bare block command works on. */
    currentFile: string;
    /** Files a block can be generated for, offered once a command takes an argument. */
    targets: FileOption[];
    query: string;
    x: number;
    /**
     * Vertical anchor. Measured from the top of the pane normally, and from the
     * *bottom* when `flip` is set — which is how the menu opens upward near the
     * end of a note without its height ever being measured.
     */
    y: number;
    flip: boolean;
    onpick: (text: string) => void;
    /**
     * A block command was chosen. `path` is null for the file on screen, which is
     * what a bare `/surface` means.
     */
    oncommand?: (kind: BlockKind, path: string | null) => void;
    onclose: () => void;
  } = $props();

  /**
   * What the query currently means.
   *
   * `/su` is a command on the file you are looking at; `/su impact_stage.ex` is
   * the same command aimed somewhere else. The grammar lives in `$lib/slash` so
   * the pane deciding whether to stay open and this menu deciding what to show
   * cannot disagree about it.
   */
  const slash = $derived(parseSlash(query));

  /**
   * Set one frame after mount so there is a state to transition *from*.
   *
   * The menu is created already open — Svelte renders it when `slashAt` is set —
   * so without a first frame at `scale(.96)` the browser has nothing to animate
   * between and the entrance is skipped entirely.
   */
  let mounted = $state(false);
  $effect(() => {
    const id = requestAnimationFrame(() => (mounted = true));
    return () => cancelAnimationFrame(id);
  });

  let cursor = $state(0);
  let listEl = $state<HTMLDivElement | null>(null);

  /** Substring first, then subsequence — same rule as ⌘P, so they agree. */
  function score(sig: string, q: string): number | null {
    if (!q) return 0;
    const hay = sig.toLowerCase();
    const needle = q.toLowerCase();
    const at = hay.indexOf(needle);
    if (at !== -1) return at;
    let i = 0;
    for (const ch of needle) {
      i = hay.indexOf(ch, i);
      if (i === -1) return null;
      i++;
    }
    return 1000;
  }

  /**
   * Matched, then ranked by the file you are looking at.
   *
   * You are usually writing about what is on screen, so its functions come first
   * even when one elsewhere scores a slightly better substring hit. That is the
   * *only* thing the open tab decides here — what gets **inserted** is settled by
   * module, so the same keystroke always produces the same text. Beyond that the
   * score decides, and the module name is part of what you can match against, so
   * `billing.to_c` narrows the way you'd expect.
   */
  /** Command rows: only while the argument has not been started. */
  const cmds = $derived.by(() => {
    if (slash.arg !== null) return [];
    const q = slash.token.toLowerCase();
    if (q.length < 2) return [];
    return COMMANDS.filter((c) => c.name.startsWith(q));
  });

  /**
   * File rows, once a command has an argument.
   *
   * The directory is shown only when it is doing work — three pipelines each with
   * a `config.ex` is the normal case, and the basename alone cannot tell them
   * apart. This is also the one place lgtm lets you choose between them.
   */
  const files = $derived.by(() => {
    if (!slash.command || slash.arg === null) return [];
    const q = (slash.arg ?? "").trim();
    const dupes = new Set(
      targets.filter((t, i) => targets.findIndex((o) => o.filename === t.filename) !== i)
        .map((t) => t.filename),
    );
    return targets
      .map((t) => ({ t, s: score(t.filename, q) ?? score(t.path, q), ambiguous: dupes.has(t.filename) }))
      .filter((h) => h.s !== null)
      .sort((a, b) => a.s! - b.s! || a.t.filename.localeCompare(b.t.filename))
      .slice(0, 9);
  });

  const hits = $derived.by(() =>
    // Naming a file replaces the function list entirely — you are choosing a
    // target, not a reference, and mixing the two would be two menus in one.
    slash.arg !== null
      ? []
      : entries
      .map((e) => ({
        e,
        s: Math.min(
          score(e.sig, query) ?? Infinity,
          score(`${e.module}.${e.sig}`, query) ?? Infinity,
        ),
      }))
      .filter((h) => h.s !== Infinity)
      .sort(
        (a, b) =>
          Number(b.e.nearby) - Number(a.e.nearby) ||
          a.s - b.s ||
          a.e.sig.localeCompare(b.e.sig),
      )
      .slice(0, 9),
  );

  /** Commands come first, so a matched one is what `↵` takes. */
  const total = $derived(cmds.length + files.length + hits.length);

  /** Rows in order, with a header wherever the module changes. */
  const rows = $derived.by(() => {
    const out: { hit: (typeof hits)[number]; i: number; head: string | null }[] = [];
    let seen: string | null = null;
    hits.forEach((hit, i) => {
      const key = hit.e.module;
      out.push({ hit, i: i + cmds.length, head: key === seen ? null : key });
      seen = key;
    });
    return out;
  });

  $effect(() => {
    query;
    cursor = 0;
  });

  /** Owned here rather than in the textarea, so the two can't disagree. */
  export function handleKey(e: KeyboardEvent): boolean {
    if (e.key === "ArrowDown") {
      cursor = Math.min(cursor + 1, total - 1);
      keepVisible();
      return true;
    }
    if (e.key === "ArrowUp") {
      cursor = Math.max(cursor - 1, 0);
      keepVisible();
      return true;
    }
    if (e.key === "Enter" || e.key === "Tab") {
      if (cursor < cmds.length) {
        oncommand?.(cmds[cursor].kind, null);
        return true;
      }
      if (files.length && slash.command) {
        const f = files[cursor - cmds.length];
        if (f) oncommand?.(slash.command.kind, f.t.path);
        else onclose();
        return true;
      }
      const hit = hits[cursor - cmds.length];
      if (hit) onpick(`\`${hit.e.insert}\``);
      else onclose();
      return true;
    }
    if (e.key === "Escape") {
      onclose();
      return true;
    }
    return false;
  }

  function keepVisible() {
    queueMicrotask(() => {
      listEl?.querySelector<HTMLElement>(".hit.on")?.scrollIntoView({ block: "nearest" });
    });
  }
</script>

<!-- `transform-origin` is the corner nearest the caret, so the menu grows *out
     of the place you are typing*. Centre would say it came from the middle of
     itself, which is nowhere. When the menu flips above the caret the origin
     flips with it, or it would grow away from the thing that spawned it. -->
<div
  class="refmenu"
  class:mounted
  style:left="{x}px"
  style:top={flip ? "auto" : `${y}px`}
  style:bottom={flip ? `${y}px` : "auto"}
  style:transform-origin={flip ? "bottom left" : "top left"}
  bind:this={listEl}
>
  {#each cmds as c, i (c.kind)}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div
      class="hit cmd"
      class:on={i === cursor}
      onmousedown={(e) => {
        e.preventDefault();
        oncommand?.(c.kind, null);
      }}
      onmouseenter={() => (cursor = i)}
    >
      <span class="slash">/</span>
      <span class="sig">{c.name}</span>
      <span class="spacer"></span>
      <span class="where">{c.what} · {currentFile}</span>
    </div>
  {/each}

  {#if files.length}
    <div class="grp">
      <span>/{slash.command?.name}</span>
      <span class="spacer"></span>
      <span class="in">pick a file</span>
    </div>
  {/if}
  {#each files as f, i (f.t.path)}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div
      class="hit file"
      class:on={i + cmds.length === cursor}
      onmousedown={(e) => {
        e.preventDefault();
        if (slash.command) oncommand?.(slash.command.kind, f.t.path);
      }}
      onmouseenter={() => (cursor = i + cmds.length)}
    >
      <span class="dot"></span>
      <span class="sig">{f.t.filename}</span>
      <span class="spacer"></span>
      <!-- The directory earns its place only when two files share a name. -->
      <span class="where">{f.ambiguous ? f.t.dir : f.t.module}</span>
    </div>
  {/each}

  {#if slash.arg !== null && !files.length}
    <p class="none">no file matches “{slash.arg.trim()}”</p>
  {:else if !hits.length && !cmds.length && !files.length}
    <p class="none">no function matches “{query}”</p>
  {:else if hits.length}
    {#each rows as row (row.hit.e.path + row.hit.e.sig)}
      {#if row.head}
        <!-- Grouped by module rather than by file: a qualified reference names a
             module, so that is the label you are actually choosing between. -->
        <div class="grp" title={row.hit.e.module}>
          <span>{row.hit.e.label}</span>
          <span class="spacer"></span>
          <span class="in">{row.hit.e.filename}</span>
        </div>
      {/if}
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div
        class="hit"
        class:on={row.i === cursor}
        onmousedown={(e) => {
          // mousedown, not click: the textarea must not lose its caret first.
          e.preventDefault();
          onpick(`\`${row.hit.e.insert}\``);
        }}
        onmouseenter={() => (cursor = row.i)}
      >
        <span class="dot" class:priv={row.hit.e.visibility === "private"}></span>
        <span class="sig">{row.hit.e.sig}</span>
        <span class="spacer"></span>
        <span class="ln">{row.hit.e.line}</span>
      </div>
    {/each}
  {/if}
  <!-- Spells out the text you are about to get. The row shows a bare signature
       for scanning, but what lands in your prose is module-qualified and carries
       no arity — different enough that leaving you to infer it was the bug. -->
  <footer>
    <kbd>↑</kbd><kbd>↓</kbd>
    {#if cursor < cmds.length && cmds[cursor]}
      <kbd>↵</kbd> inserts <code>lgtm:{cmds[cursor].kind}</code> for
      <code>{currentFile}</code> · <kbd>space</kbd> for another file
    {:else if files.length && files[cursor - cmds.length]}
      <kbd>↵</kbd> inserts <code>lgtm:{slash.command?.kind}</code> for
      <code>{files[cursor - cmds.length].t.filename}</code>
    {:else if hits[cursor - cmds.length]}
      <kbd>↵</kbd> inserts <code>`{hits[cursor - cmds.length].e.insert}`</code>
    {:else}
      <kbd>↵</kbd> insert
    {/if}
    · <kbd>esc</kbd>
  </footer>
</div>

<style>
  .refmenu {
    /* 140ms — inside the 125–200ms budget for a small popover. Anything slower
       is in the way of typing, which is what you were doing when it opened. */
    opacity: 0;
    transform: scale(0.96);
    transition:
      opacity var(--fast) var(--ease-out),
      transform var(--fast) var(--ease-out);
    position: absolute;
    z-index: 30;
    width: 268px;
    max-height: 264px;
    overflow: auto;
    background: var(--bg-raised);
    border: 1px solid var(--line);
    border-radius: 8px;
    box-shadow: 0 12px 32px rgba(10, 12, 16, 0.24);
  }
  /* Sticky, so you never lose track of which module you are scrolling through. */
  .grp {
    position: sticky;
    top: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 11px 3px;
    background: var(--bg-inset);
    border-bottom: 1px solid var(--line-soft);
    font-family: var(--mono);
    font-size: 9.5px;
    letter-spacing: 0.02em;
    color: var(--fg-dim);
  }
  .grp .spacer {
    flex: 1;
  }
  .grp .in {
    color: var(--fg-faint);
  }
  footer code {
    font-family: var(--mono);
    font-size: 10px;
    color: var(--accent);
  }
  /* A command is not a function, so it wears a slash rather than the
     public/private dot, and sits above the list it is not part of. */
  .hit.cmd {
    border-bottom: 1px solid var(--line-soft);
  }
  .hit.cmd .slash {
    flex: none;
    width: 6px;
    text-align: center;
    font-family: var(--mono);
    font-size: 12px;
    font-weight: 700;
    color: var(--accent);
  }
  .hit.cmd .sig {
    color: var(--accent);
  }
  /* A file row is a target, not a function — square dot, no visibility colour. */
  .hit.file .dot {
    border-radius: 2px;
    background: var(--fg-faint);
  }
  .hit.file .sig {
    color: var(--fg);
  }
  .hit .where {
    flex: none;
    font-size: 9.5px;
    color: var(--fg-faint);
  }
  .refmenu.mounted {
    opacity: 1;
    transform: scale(1);
  }
  /* Movement goes, the menu still arrives. Reduced motion means gentler, not
     nothing — an element that pops in with no transition at all is harder to
     follow than one that fades. */
  @media (prefers-reduced-motion: reduce) {
    .refmenu {
      transform: none;
      transition: opacity 0.1s var(--ease-out);
    }
    .refmenu.mounted {
      transform: none;
    }
  }

  .none {
    margin: 0;
    padding: 12px 12px;
    font-size: 11.5px;
    color: var(--fg-faint);
  }
  .hit {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 11px;
    cursor: pointer;
    border-left: 2px solid transparent;
  }
  .hit.on {
    background: var(--sel);
    border-left-color: var(--accent);
  }
  .hit .dot {
    width: 6px;
    height: 6px;
    border-radius: 2px;
    background: var(--pub);
    flex: none;
  }
  .hit .dot.priv {
    background: var(--priv);
  }
  .hit .sig {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hit .spacer {
    flex: 1;
  }
  .hit .ln {
    font-family: var(--mono);
    font-size: 10px;
    color: var(--fg-faint);
    flex: none;
  }
  footer {
    position: sticky;
    bottom: 0;
    padding: 5px 11px;
    border-top: 1px solid var(--line-soft);
    background: var(--bg-inset);
    font-size: 10px;
    color: var(--fg-faint);
  }
  kbd {
    font-family: var(--mono);
    font-size: 9.5px;
    border: 1px solid var(--line);
    border-bottom-width: 2px;
    border-radius: 3px;
    padding: 0 3px;
    margin-right: 2px;
    color: var(--fg-dim);
  }
</style>
