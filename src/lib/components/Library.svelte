<script lang="ts">
  // Everything you've written, and the only way back to it. Built for a few
  // hundred docs rather than a few: search, three orderings, folder grouping,
  // and full keyboard navigation — a flat list stops being usable at about ten.
  import { listDocs, deleteDoc, type DocSummary } from "$lib/ipc";
  import { when } from "$lib/when";

  let { onopen, onclose }: { onopen: (d: DocSummary) => void; onclose: () => void } = $props();

  type Sort = "recent" | "name" | "folder";

  let query = $state("");
  let sort = $state<Sort>("recent");
  let docs = $state<DocSummary[]>([]);
  let loading = $state(true);
  /** The doc awaiting a second click to confirm deletion. */
  let confirming = $state<number | null>(null);
  /** Keyboard cursor over the flattened visible rows. */
  let cursor = $state(0);
  let listEl = $state<HTMLDivElement | null>(null);

  async function refresh() {
    loading = true;
    try {
      docs = await listDocs(query || undefined, 500);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    query;
    refresh();
  });

  // A new search means the old cursor position is meaningless.
  $effect(() => {
    query;
    sort;
    cursor = 0;
    confirming = null;
  });

  const sorted = $derived.by(() => {
    const list = [...docs];
    if (sort === "name") {
      list.sort((a, b) => a.title.localeCompare(b.title));
    } else if (sort === "folder") {
      list.sort(
        (a, b) => folderOf(a).localeCompare(folderOf(b)) || a.filename.localeCompare(b.filename),
      );
    } else {
      list.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
    }
    return list;
  });

  /** Grouped for display; a single unnamed group when we're not grouping. */
  const groups = $derived.by(() => {
    if (sort !== "folder") return [{ label: "", items: sorted }];
    const out: { label: string; items: DocSummary[] }[] = [];
    for (const doc of sorted) {
      const label = prettyFolder(folderOf(doc));
      const last = out[out.length - 1];
      if (last && last.label === label) last.items.push(doc);
      else out.push({ label, items: [doc] });
    }
    return out;
  });

  /** Flat order, so ↑/↓ crosses group boundaries the way the eye does. */
  const flat = $derived(groups.flatMap((g) => g.items));

  function folderOf(d: DocSummary): string {
    return d.path.replace(/\/[^/]+$/, "");
  }

  function prettyFolder(path: string): string {
    const home = "/Users/";
    if (!path.startsWith(home)) return path;
    const rest = path.slice(home.length);
    const cut = rest.indexOf("/");
    return cut === -1 ? "~" : "~" + rest.slice(cut);
  }

  async function confirmDelete(doc: DocSummary) {
    await deleteDoc(doc.id);
    confirming = null;
    refresh();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || (e.key === "n" && e.ctrlKey)) {
      e.preventDefault();
      cursor = Math.min(cursor + 1, flat.length - 1);
      scrollCursorIntoView();
    } else if (e.key === "ArrowUp" || (e.key === "p" && e.ctrlKey)) {
      e.preventDefault();
      cursor = Math.max(cursor - 1, 0);
      scrollCursorIntoView();
    } else if (e.key === "Enter") {
      e.preventDefault();
      const doc = flat[cursor];
      if (doc) onopen(doc);
    } else if (e.key === "Escape" && confirming !== null) {
      // Back out of a pending delete before closing the whole panel.
      e.preventDefault();
      e.stopPropagation();
      confirming = null;
    }
  }

  function scrollCursorIntoView() {
    queueMicrotask(() => {
      listEl?.querySelector<HTMLElement>(".row.cursor")?.scrollIntoView({ block: "nearest" });
    });
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="scrim" onclick={onclose}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="panel" onclick={(e) => e.stopPropagation()} onkeydown={onKey}>
    <header>
      <!-- svelte-ignore a11y_autofocus -->
      <input placeholder="Search title, file, path or branch…" bind:value={query} autofocus />

      <div class="seg" role="group" aria-label="Sort">
        <button class:on={sort === "recent"} onclick={() => (sort = "recent")}>Recent</button>
        <button class:on={sort === "name"} onclick={() => (sort = "name")}>Name</button>
        <button class:on={sort === "folder"} onclick={() => (sort = "folder")}>Folder</button>
      </div>

      <button class="btn" onclick={onclose}>Close</button>
    </header>

    <div class="list" bind:this={listEl}>
      {#if loading}
        <p class="empty">Loading…</p>
      {:else if !flat.length}
        <p class="empty">
          {query ? `Nothing matches “${query}”.` : "No docs yet — open a file to start one."}
        </p>
      {:else}
        {#each groups as group (group.label)}
          {#if group.label}
            <div class="grouphead">{group.label} <span>{group.items.length}</span></div>
          {/if}
          {#each group.items as doc (doc.id)}
            {@const idx = flat.indexOf(doc)}
            <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
            <div
              class="row"
              class:cursor={idx === cursor}
              class:danger={confirming === doc.id}
              onclick={() => (confirming === doc.id ? null : onopen(doc))}
              onmouseenter={() => (cursor = idx)}
            >
              {#if confirming === doc.id}
                <div class="confirm">
                  <div class="what">
                    <b>Delete this explanation?</b>
                    <span>
                      Removes the markdown you wrote for <code>{doc.filename}</code> and the source
                      snapshot saved with it. This cannot be undone.
                      <em>Your {doc.filename} file on disk is not touched.</em>
                    </span>
                  </div>
                  <div class="acts">
                    <button class="btn" onclick={(e) => (e.stopPropagation(), (confirming = null))}>
                      Cancel
                    </button>
                    <button class="btn del" onclick={(e) => (e.stopPropagation(), confirmDelete(doc))}>
                      Delete explanation
                    </button>
                  </div>
                </div>
              {:else}
                <div class="main">
                  <b>{doc.title}</b>
                  <span class="path">{doc.filename} · {prettyFolder(folderOf(doc))}</span>
                </div>
                <div class="meta">
                  {#if doc.branch}<span class="branch">⑂ {doc.branch}</span>{/if}
                  <span class="ago">{when(doc.updatedAt)}</span>
                  <button
                    class="del"
                    title="Delete this explanation"
                    aria-label="Delete this explanation"
                    onclick={(e) => (e.stopPropagation(), (confirming = doc.id))}
                  >
                    ×
                  </button>
                </div>
              {/if}
            </div>
          {/each}
        {/each}
      {/if}
    </div>

    <footer>
      <span>{flat.length} {flat.length === 1 ? "doc" : "docs"}</span>
      <span class="spacer"></span>
      <span class="keys"><kbd>↑</kbd><kbd>↓</kbd> move · <kbd>↵</kbd> open · <kbd>esc</kbd> close</span>
    </footer>
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
    padding-top: 8vh;
  }
  .panel {
    width: min(880px, 94vw);
    height: min(74vh, 720px);
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
    flex: none;
  }
  header input {
    flex: 1;
    min-width: 0;
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

  .seg {
    display: flex;
    border: 1px solid var(--line);
    border-radius: 6px;
    overflow: hidden;
    flex: none;
  }
  .seg button {
    font: inherit;
    font-size: 11px;
    background: transparent;
    color: var(--fg-faint);
    border: 0;
    padding: 4px 11px;
    cursor: pointer;
  }
  .seg button + button {
    border-left: 1px solid var(--line);
  }
  .seg button.on {
    background: var(--bg-inset);
    color: var(--fg);
  }

  .list {
    flex: 1;
    overflow: auto;
    min-height: 0;
  }
  .empty {
    color: var(--fg-faint);
    padding: 40px 24px;
    text-align: center;
    margin: 0;
  }

  .grouphead {
    position: sticky;
    top: 0;
    z-index: 1;
    display: flex;
    gap: 8px;
    align-items: baseline;
    padding: 7px 14px;
    background: var(--bg-inset);
    border-bottom: 1px solid var(--line-soft);
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg-faint);
  }
  .grouphead span {
    color: var(--fg-faint);
    opacity: 0.7;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 9px 14px;
    cursor: pointer;
    border-left: 2px solid transparent;
    border-bottom: 1px solid var(--line-soft);
  }
  .row.cursor {
    background: var(--bg-inset);
    border-left-color: var(--accent);
  }
  .row.danger {
    background: color-mix(in srgb, var(--priv) 8%, transparent);
    border-left-color: var(--priv);
    cursor: default;
  }
  .main {
    min-width: 0;
    flex: 1;
  }
  .main b {
    display: block;
    font-weight: 550;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .path {
    display: block;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--fg-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 11px;
    color: var(--fg-faint);
    white-space: nowrap;
    flex: none;
  }
  .branch {
    font-family: var(--mono);
    padding: 1px 7px;
    border: 1px solid var(--line);
    border-radius: 999px;
  }
  .ago {
    font-variant-numeric: tabular-nums;
  }
  .meta .del {
    background: none;
    border: 0;
    color: var(--fg-faint);
    font-size: 16px;
    cursor: pointer;
    line-height: 1;
    padding: 0 2px;
    opacity: 0;
  }
  /* The destructive control only appears on the row you're actually on. */
  .row.cursor .meta .del,
  .row:hover .meta .del {
    opacity: 1;
  }
  .meta .del:hover {
    color: var(--priv);
  }

  /* ---- the confirmation step ----
     Inline, in place of the row, saying exactly what is lost and what is not. */
  .confirm {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
  }
  .confirm .what {
    min-width: 0;
    flex: 1;
  }
  .confirm .what b {
    display: block;
    font-size: 12.5px;
    color: var(--fg);
  }
  .confirm .what span {
    display: block;
    font-size: 11.5px;
    color: var(--fg-dim);
    line-height: 1.45;
    margin-top: 2px;
  }
  .confirm .what code {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--fg);
  }
  .confirm .what em {
    font-style: normal;
    color: var(--pub);
  }
  .confirm .acts {
    display: flex;
    gap: 6px;
    flex: none;
  }
  .confirm .btn.del {
    color: #fff;
    background: var(--priv);
    border-color: var(--priv);
  }
  .confirm .btn.del:hover {
    filter: brightness(1.08);
  }

  footer {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 14px;
    border-top: 1px solid var(--line-soft);
    background: var(--bg-inset);
    font-size: 11px;
    color: var(--fg-faint);
  }
  footer .spacer {
    flex: 1;
  }
  footer kbd {
    font-family: var(--mono);
    font-size: 10px;
    border: 1px solid var(--line);
    border-bottom-width: 2px;
    border-radius: 4px;
    padding: 0 4px;
    margin-right: 3px;
    color: var(--fg-dim);
  }
</style>
