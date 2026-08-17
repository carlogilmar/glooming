<script lang="ts">
  // ⌘P — jump to a function by name.
  //
  // The functions table is already a directory, but it's unreachable when the
  // doc is scrolled elsewhere or in edit mode. This is the keyboard route to
  // the same place, and it never leaves the home row.
  import { displaySig } from "$lib/select";
  import type { FnInfo, ModuleInfo } from "$lib/ipc";

  let {
    module = null,
    onpick,
    onclose,
  }: {
    module: ModuleInfo | null;
    onpick: (f: FnInfo) => void;
    onclose: () => void;
  } = $props();

  let query = $state("");
  let cursor = $state(0);
  let listEl = $state<HTMLDivElement | null>(null);

  /**
   * Substring matches first, then subsequence ("cu" finds create_user), so
   * typing a prefix behaves predictably and abbreviations still work.
   */
  function score(sig: string, q: string): number | null {
    const hay = sig.toLowerCase();
    const needle = q.toLowerCase();
    if (!needle) return 0;

    const at = hay.indexOf(needle);
    if (at !== -1) return at; // 0..n — earlier is better

    let i = 0;
    for (const ch of needle) {
      i = hay.indexOf(ch, i);
      if (i === -1) return null;
      i++;
    }
    return 1000; // matched, but only loosely
  }

  const hits = $derived.by(() => {
    const fns = module?.functions ?? [];
    return fns
      .map((f) => ({ f, sig: displaySig(f), s: score(displaySig(f), query) }))
      .filter((h) => h.s !== null)
      .sort((a, b) => (a.s! - b.s!) || a.sig.localeCompare(b.sig));
  });

  $effect(() => {
    query;
    cursor = 0;
  });

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || (e.ctrlKey && e.key === "n")) {
      e.preventDefault();
      cursor = Math.min(cursor + 1, hits.length - 1);
      keepVisible();
    } else if (e.key === "ArrowUp" || (e.ctrlKey && e.key === "p")) {
      e.preventDefault();
      cursor = Math.max(cursor - 1, 0);
      keepVisible();
    } else if (e.key === "Enter") {
      e.preventDefault();
      const hit = hits[cursor];
      if (hit) onpick(hit.f);
    }
  }

  function keepVisible() {
    queueMicrotask(() => {
      listEl?.querySelector<HTMLElement>(".hit.on")?.scrollIntoView({ block: "nearest" });
    });
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="scrim" onclick={onclose}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="panel" onclick={(e) => e.stopPropagation()} onkeydown={onKey}>
    <!-- svelte-ignore a11y_autofocus -->
    <input placeholder="Jump to function…" bind:value={query} spellcheck="false" autofocus />

    <div class="list" bind:this={listEl}>
      {#if !module}
        <p class="empty">No outline for this file.</p>
      {:else if !hits.length}
        <p class="empty">No function matches “{query}”.</p>
      {:else}
        {#each hits as hit, i (hit.sig)}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div
            class="hit"
            class:on={i === cursor}
            onclick={() => onpick(hit.f)}
            onmouseenter={() => (cursor = i)}
          >
            <span class="dot" class:priv={hit.f.visibility === "private"}></span>
            <span class="sig">{hit.sig}</span>
            {#if hit.f.clauses > 1}<span class="badge">{hit.f.clauses} clauses</span>{/if}
            <span class="spacer"></span>
            <span class="ln">line {hit.f.line}</span>
          </div>
        {/each}
      {/if}
    </div>

    <footer>
      <span>{hits.length} of {module?.functions.length ?? 0}</span>
      <span class="spacer"></span>
      <span><kbd>↑</kbd><kbd>↓</kbd> move · <kbd>↵</kbd> jump · <kbd>esc</kbd> close</span>
    </footer>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 22;
    background: rgba(10, 12, 16, 0.35);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
  }
  .panel {
    width: min(560px, 92vw);
    max-height: 62vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-raised);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: 0 22px 60px rgba(10, 12, 16, 0.28);
    overflow: hidden;
  }
  .panel input {
    flex: none;
    font: inherit;
    font-family: var(--mono);
    font-size: 13px;
    padding: 11px 14px;
    border: 0;
    border-bottom: 1px solid var(--line-soft);
    background: transparent;
    color: var(--fg);
    outline: none;
  }

  .list {
    flex: 1;
    overflow: auto;
    min-height: 0;
  }
  .empty {
    color: var(--fg-faint);
    padding: 28px 20px;
    text-align: center;
    margin: 0;
    font-size: 12.5px;
  }

  .hit {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 7px 14px;
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
    font-size: 12.5px;
    color: var(--fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hit .badge {
    font-size: 9.5px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--fg-faint);
    border: 1px solid var(--line);
    border-radius: 999px;
    padding: 1px 6px;
    white-space: nowrap;
  }
  .hit .spacer {
    flex: 1;
  }
  .hit .ln {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg-faint);
    white-space: nowrap;
  }

  footer {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 14px;
    border-top: 1px solid var(--line-soft);
    background: var(--bg-inset);
    font-size: 10.5px;
    color: var(--fg-faint);
  }
  footer .spacer {
    flex: 1;
  }
  footer kbd {
    font-family: var(--mono);
    font-size: 9.5px;
    border: 1px solid var(--line);
    border-bottom-width: 2px;
    border-radius: 4px;
    padding: 0 4px;
    margin-right: 3px;
    color: var(--fg-dim);
  }
</style>
