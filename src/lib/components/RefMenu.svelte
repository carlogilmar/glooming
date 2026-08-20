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

  let {
    entries = [],
    query = "",
    x = 0,
    y = 0,
    onpick,
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
    query: string;
    x: number;
    y: number;
    onpick: (text: string) => void;
    onclose: () => void;
  } = $props();

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
   * Matched, then ranked local-first.
   *
   * Writing about the file you are looking at is the common case, and its
   * functions insert bare — so they belong at the top even when a function in
   * another file scores a slightly better substring hit. Beyond that the score
   * decides, and the module name is part of what you can match against, so
   * `billing.to_c` narrows the way you'd expect.
   */
  const hits = $derived.by(() =>
    entries
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
          Number(b.e.local) - Number(a.e.local) ||
          a.s - b.s ||
          a.e.sig.localeCompare(b.e.sig),
      )
      .slice(0, 9),
  );

  /** Rows in order, with a header wherever the module changes. */
  const rows = $derived.by(() => {
    const out: { hit: (typeof hits)[number]; i: number; head: string | null }[] = [];
    let seen: string | null = null;
    hits.forEach((hit, i) => {
      const key = hit.e.module;
      out.push({ hit, i, head: key === seen ? null : key });
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
      cursor = Math.min(cursor + 1, hits.length - 1);
      keepVisible();
      return true;
    }
    if (e.key === "ArrowUp") {
      cursor = Math.max(cursor - 1, 0);
      keepVisible();
      return true;
    }
    if (e.key === "Enter" || e.key === "Tab") {
      const hit = hits[cursor];
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

<div class="refmenu" style:left="{x}px" style:top="{y}px" bind:this={listEl}>
  {#if !hits.length}
    <p class="none">no function matches “{query}”</p>
  {:else}
    {#each rows as row (row.hit.e.path + row.hit.e.sig)}
      {#if row.head}
        <!-- Grouped by module rather than by file: a qualified reference names a
             module, so that is the label you are actually choosing between. -->
        <div class="grp">
          <span>{row.head}</span>
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
        {#if !row.hit.e.local}
          <span class="away" title="inserts {row.hit.e.insert}">qualified</span>
        {/if}
        <span class="ln">{row.hit.e.line}</span>
      </div>
    {/each}
  {/if}
  <footer><kbd>↑</kbd><kbd>↓</kbd> <kbd>↵</kbd> insert · <kbd>esc</kbd></footer>
</div>

<style>
  .refmenu {
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
  .away {
    flex: none;
    font-size: 9px;
    padding: 0 4px;
    border-radius: 3px;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
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
