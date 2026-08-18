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
  import { displaySig } from "$lib/select";
  import type { FnInfo, ModuleInfo } from "$lib/ipc";

  let {
    module = null,
    query = "",
    x = 0,
    y = 0,
    onpick,
    onclose,
  }: {
    module: ModuleInfo | null;
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

  const hits = $derived.by(() => {
    const fns = module?.functions ?? [];
    return fns
      .map((f: FnInfo) => ({ f, sig: displaySig(f), s: score(displaySig(f), query) }))
      .filter((h) => h.s !== null)
      .sort((a, b) => a.s! - b.s! || a.sig.localeCompare(b.sig))
      .slice(0, 8);
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
      if (hit) onpick(`\`${hit.sig}\``);
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
    {#each hits as hit, i (hit.sig)}
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div
        class="hit"
        class:on={i === cursor}
        onmousedown={(e) => {
          // mousedown, not click: the textarea must not lose its caret first.
          e.preventDefault();
          onpick(`\`${hit.sig}\``);
        }}
        onmouseenter={() => (cursor = i)}
      >
        <span class="dot" class:priv={hit.f.visibility === "private"}></span>
        <span class="sig">{hit.sig}</span>
        <span class="spacer"></span>
        <span class="ln">{hit.f.line}</span>
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
