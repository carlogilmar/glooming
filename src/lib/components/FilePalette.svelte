<script lang="ts">
  // ⌘T — the file you want, by name.
  //
  // Folds in what ⌘L used to do: if what you have typed looks like a path
  // rather than a search, it offers to open that directly. A path from a stack
  // trace or a PR comment is often outside the project, and having two
  // near-identical dialogs for "open a file by naming it" was one too many.
  import { projectFiles, type Project, type ProjectFile } from "$lib/ipc";

  let {
    project = null,
    onpick,
    onpickpath,
    onchoose,
    onclose,
  }: {
    project: Project | null;
    onpick: (f: ProjectFile) => void;
    onpickpath: (path: string) => void;
    onchoose: () => void;
    onclose: () => void;
  } = $props();

  let query = $state("");
  let files = $state<ProjectFile[]>([]);
  let loading = $state(false);
  let failed = $state("");
  let cursor = $state(0);
  let listEl = $state<HTMLDivElement | null>(null);

  // Walked on open rather than cached: it takes milliseconds, and a cache would
  // be inventing a staleness problem — files appear and vanish constantly while
  // you work.
  $effect(() => {
    const p = project?.path;
    if (!p) {
      files = [];
      return;
    }
    loading = true;
    failed = "";
    projectFiles(p)
      .then((f) => (files = f))
      .catch((e) => {
        failed = String(e);
        files = [];
      })
      .finally(() => (loading = false));
  });

  /** Anything with a separator is a path, not a search. */
  const looksLikePath = $derived(
    /^[~/.]/.test(query.trim()) || /\.exs?$/.test(query.trim()) && query.includes("/"),
  );

  /**
   * Match on the whole relative path, but rank filename hits above directory
   * hits — otherwise everything under a `processor/` directory outranks
   * `processor.ex` itself.
   */
  function score(f: ProjectFile, q: string): number | null {
    if (!q) return 0;
    const needle = q.toLowerCase();
    const name = f.name.toLowerCase();
    const rel = f.rel.toLowerCase();

    const inName = name.indexOf(needle);
    if (inName !== -1) return inName;
    const inRel = rel.indexOf(needle);
    if (inRel !== -1) return 100 + inRel;

    // Subsequence over the path, so `myacc` finds `my_app/accounts.ex`.
    let i = 0;
    for (const ch of needle) {
      i = rel.indexOf(ch, i);
      if (i === -1) return null;
      i++;
    }
    return 1000;
  }

  const hits = $derived.by(() => {
    const q = query.trim();
    return files
      .map((f) => ({ f, s: score(f, q) }))
      .filter((h) => h.s !== null)
      .sort((a, b) => a.s! - b.s! || a.f.rel.localeCompare(b.f.rel))
      .slice(0, 200);
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
      if (looksLikePath && !hits.length) {
        onpickpath(query.trim());
        return;
      }
      const hit = hits[cursor];
      if (hit) onpick(hit.f);
      else if (query.trim()) onpickpath(query.trim());
    }
  }

  function keepVisible() {
    queueMicrotask(() => {
      listEl?.querySelector<HTMLElement>(".hit.on")?.scrollIntoView({ block: "nearest" });
    });
  }

  /** `lib/my_app/accounts.ex` → dim directory, bright filename. */
  function split(rel: string): { dir: string; name: string } {
    const at = rel.lastIndexOf("/");
    return at === -1 ? { dir: "", name: rel } : { dir: rel.slice(0, at + 1), name: rel.slice(at + 1) };
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="scrim" onclick={onclose}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="panel" onclick={(e) => e.stopPropagation()} onkeydown={onKey}>
    <header>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        placeholder={project ? `Search ${project.name}…` : "Paste a path, or open a folder"}
        bind:value={query}
        spellcheck="false"
        autocapitalize="off"
        autofocus
      />
      <button class="btn" onclick={onchoose}>
        {project ? "Change folder" : "Open folder…"}
      </button>
    </header>

    <div class="list" bind:this={listEl}>
      {#if !project}
        <p class="empty">
          No folder open yet. <b>Open a folder</b> to search it by name — or paste a full path above.
        </p>
      {:else if loading}
        <p class="empty">Reading {project.name}…</p>
      {:else if failed}
        <p class="empty bad">{failed}</p>
      {:else if !files.length}
        <p class="empty">No Elixir files under {project.name}.</p>
      {:else if !hits.length}
        <p class="empty">
          Nothing matches “{query}”.
          {#if looksLikePath}<br /><b>↵</b> to open it as a path anyway.{/if}
        </p>
      {:else}
        {#each hits as hit, i (hit.f.path)}
          {@const parts = split(hit.f.rel)}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div
            class="hit"
            class:on={i === cursor}
            onclick={() => onpick(hit.f)}
            onmouseenter={() => (cursor = i)}
          >
            <span class="name">{parts.name}</span>
            <span class="dir">{parts.dir}</span>
          </div>
        {/each}
      {/if}
    </div>

    <footer>
      <span>{project ? `${hits.length} of ${files.length}` : "no folder"}</span>
      <span class="spacer"></span>
      <span><kbd>↑</kbd><kbd>↓</kbd> move · <kbd>↵</kbd> open · <kbd>esc</kbd> close</span>
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
    padding-top: 10vh;
  }
  .panel {
    width: min(680px, 92vw);
    max-height: 70vh;
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
    font-family: var(--mono);
    font-size: 12.5px;
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
    flex: 1;
    overflow: auto;
    min-height: 0;
  }
  .empty {
    color: var(--fg-faint);
    padding: 28px 22px;
    text-align: center;
    margin: 0;
    font-size: 12.5px;
    line-height: 1.6;
  }
  .empty.bad {
    color: var(--priv);
  }
  .empty b {
    color: var(--fg-dim);
  }

  .hit {
    display: flex;
    align-items: baseline;
    gap: 9px;
    padding: 6px 14px;
    cursor: pointer;
    border-left: 2px solid transparent;
    min-width: 0;
  }
  .hit.on {
    background: var(--sel);
    border-left-color: var(--accent);
  }
  .hit .name {
    font-family: var(--mono);
    font-size: 12.5px;
    color: var(--fg);
    white-space: nowrap;
    flex: none;
  }
  /* The directory is context, not the thing you are looking for. */
  .hit .dir {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--fg-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
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
