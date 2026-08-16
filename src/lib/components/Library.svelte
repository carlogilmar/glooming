<script lang="ts">
  // Saved docs, newest first, with search. Without this, docs are unreachable
  // after the sitting in which they were written.
  import { listDocs, deleteDoc, type DocSummary } from "$lib/ipc";

  let { onopen, onclose }: { onopen: (d: DocSummary) => void; onclose: () => void } = $props();

  let query = $state("");
  let docs = $state<DocSummary[]>([]);
  let loading = $state(true);

  async function refresh() {
    loading = true;
    try {
      docs = await listDocs(query || undefined);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    query;
    refresh();
  });

  async function remove(e: MouseEvent, doc: DocSummary) {
    e.stopPropagation();
    await deleteDoc(doc.id);
    refresh();
  }

  function when(iso: string): string {
    const days = Math.floor((Date.now() - new Date(iso).getTime()) / 86_400_000);
    if (days <= 0) return "today";
    if (days === 1) return "yesterday";
    if (days < 30) return `${days}d ago`;
    if (days < 365) return `${Math.floor(days / 30)}mo ago`;
    return `${Math.floor(days / 365)}y ago`;
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="scrim" onclick={onclose}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="panel" onclick={(e) => e.stopPropagation()}>
    <header>
      <!-- svelte-ignore a11y_autofocus -->
      <input placeholder="Search docs…" bind:value={query} autofocus />
      <button class="btn" onclick={onclose}>Close</button>
    </header>

    <div class="list">
      {#if loading}
        <p class="empty">Loading…</p>
      {:else if !docs.length}
        <p class="empty">{query ? "Nothing matches." : "No docs yet — open a file to start one."}</p>
      {:else}
        {#each docs as doc (doc.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div class="row" onclick={() => onopen(doc)}>
            <div class="main">
              <b>{doc.title}</b>
              <span class="path">{doc.path}</span>
            </div>
            <div class="meta">
              {#if doc.branch}<span class="branch">{doc.branch}</span>{/if}
              <span>{when(doc.updatedAt)}</span>
              <button class="del" title="Delete" onclick={(e) => remove(e, doc)}>×</button>
            </div>
          </div>
        {/each}
      {/if}
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 20;
    background: rgba(10, 12, 16, 0.35);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
  }
  .panel {
    width: min(680px, 92vw);
    max-height: 68vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-raised);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: 0 22px 60px rgba(10, 12, 16, 0.28);
    overflow: hidden;
  }
  header {
    display: flex;
    gap: 8px;
    padding: 10px;
    border-bottom: 1px solid var(--line-soft);
  }
  header input {
    flex: 1;
    font: inherit;
    padding: 6px 10px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--bg);
    color: var(--fg);
    outline: none;
  }
  header input:focus {
    border-color: var(--accent);
  }
  .list {
    overflow: auto;
  }
  .empty {
    color: var(--fg-faint);
    padding: 24px;
    text-align: center;
    margin: 0;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 9px 14px;
    cursor: pointer;
    border-left: 2px solid transparent;
  }
  .row:hover {
    background: var(--bg-inset);
    border-left-color: var(--accent);
  }
  .main {
    min-width: 0;
    flex: 1;
  }
  .main b {
    display: block;
    font-weight: 550;
  }
  .path {
    display: block;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--fg-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 11px;
    color: var(--fg-faint);
    white-space: nowrap;
  }
  .branch {
    font-family: var(--mono);
    padding: 1px 6px;
    border: 1px solid var(--line);
    border-radius: 999px;
  }
  .del {
    background: none;
    border: 0;
    color: var(--fg-faint);
    font-size: 15px;
    cursor: pointer;
    line-height: 1;
  }
  .del:hover {
    color: var(--priv);
  }
</style>
