<script lang="ts">
  // Everything you've written, and the only way back to it. Built for a few
  // hundred docs rather than a few: search, three orderings, folder grouping,
  // and full keyboard navigation — a flat list stops being usable at about ten.
  import { listDocs, deleteDoc, type DocSummary } from "$lib/ipc";
  import { when } from "$lib/when";
  import { parseTags, tagHue } from "$lib/tags";

  let {
    onopen,
    onclose,
    ondelete,
  }: {
    onopen: (d: DocSummary) => void;
    onclose: () => void;
    /**
     * A reading was deleted. The library refreshes itself, but anything else
     * holding a list of docs — the welcome screen's recents, the reading that is
     * currently open — has no way to know the row went away.
     */
    ondelete?: (id: number) => void;
  } = $props();

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
    ondelete?.(doc.id);
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
      <input placeholder="Search name, tag, file, path or branch…" bind:value={query} autofocus />

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
          {query ? `Nothing matches “${query}”.` : "No glooms yet — open a file to start one."}
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
                    <b>Delete this gloom?</b>
                    <span>
                      Removes the markdown you wrote and the
                      {doc.fileCount > 1
                        ? `${doc.fileCount} source snapshots saved with it`
                        : "source snapshot saved with it"}. This cannot be undone.
                      <em>
                        {doc.fileCount > 1
                          ? `None of the ${doc.fileCount} files on disk are touched.`
                          : `Your ${doc.filename} file on disk is not touched.`}
                      </em>
                    </span>
                  </div>
                  <div class="acts">
                    <button class="btn" onclick={(e) => (e.stopPropagation(), (confirming = null))}>
                      Cancel
                    </button>
                    <button class="btn del" onclick={(e) => (e.stopPropagation(), confirmDelete(doc))}>
                      Delete gloom
                    </button>
                  </div>
                </div>
              {:else}
                <!-- The same row home uses: the name, its tags, then the counts and
                     the age in fixed columns at the right so they line up down the
                     list. The origin filename went with the redesign — the name is
                     what identifies a gloom, and the folder is the group header. -->
                <span class="rname" class:unnamed={doc.title === doc.filename}>{doc.title}</span>
                {#if doc.label}
                  <span class="rtags">
                    {#each parseTags(doc.label).slice(0, 4) as t (t)}
                      <span class="rtag" style="--hue:{tagHue(t)}">{t}</span>
                    {/each}
                  </span>
                {/if}
                <span class="rcount">
                  {doc.fileCount}
                  {doc.fileCount === 1 ? "file" : "files"}
                </span>
                <span class="rwhen">{when(doc.updatedAt)}</span>
                <button
                  class="del"
                  title="Delete this gloom"
                  aria-label="Delete this gloom"
                  onclick={(e) => (e.stopPropagation(), (confirming = doc.id))}
                >
                  ×
                </button>
              {/if}
            </div>
          {/each}
        {/each}
      {/if}
    </div>

    <footer>
      <span>{flat.length} {flat.length === 1 ? "gloom" : "glooms"}</span>
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
  /* The destructive control only appears on the row you're actually on. */
  .row.cursor .del,
  .row:hover .del {
    opacity: 1;
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

  /* The row, as home draws it: the name, its tags, then fixed columns at the
     right so counts and ages line up down the list. */
  .rname {
    flex: 1;
    min-width: 0;
    font-family: var(--serif);
    font-size: 14.5px;
    color: var(--fg);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .rname.unnamed {
    color: var(--fg-faint);
    font-style: italic;
  }
  .rtags {
    flex: none;
    display: flex;
    gap: 4px;
    min-width: 0;
  }
  /* The hue comes from the tag's own text, so `retry` is the same colour here, on
     home and in the gloom band — nobody picks a colour for a word they typed in
     two seconds, and a random one would differ between two views of one gloom. */
  .rtag {
    font-family: var(--mono);
    font-size: 9.5px;
    line-height: 1;
    padding: 2px 6px;
    border-radius: 999px;
    white-space: nowrap;
    color: oklch(0.45 0.12 var(--hue));
    background: oklch(0.45 0.12 var(--hue) / 0.12);
  }
  :global(html.dark) .rtag {
    color: oklch(0.86 0.09 var(--hue));
    background: oklch(0.86 0.09 var(--hue) / 0.14);
  }
  .rcount,
  .rwhen {
    flex: none;
    text-align: right;
    font-family: var(--mono);
    font-size: 10px;
    color: var(--fg-faint);
  }
  .rcount {
    width: 52px;
  }
  .rwhen {
    width: 64px;
  }
</style>
