<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import CodePane from "$lib/components/CodePane.svelte";
  import DocPane from "$lib/components/DocPane.svelte";
  import Divider from "$lib/components/Divider.svelte";
  import Library from "$lib/components/Library.svelte";
  import { theme } from "$lib/stores/theme.svelte";
  import { focus } from "$lib/stores/focus.svelte";
  import * as ipc from "$lib/ipc";
  import type { Doc, DocSummary, OpenedFile } from "$lib/ipc";

  let file = $state<OpenedFile | null>(null);
  let doc = $state<Doc | null>(null);
  let markdown = $state("");
  let basis = $state(52);
  let saving = $state(false);
  let dirty = $state(false);
  let error = $state<string | null>(null);
  let showLibrary = $state(false);
  /** Existing docs for a just-opened path, awaiting your choice. */
  let chooser = $state<DocSummary[] | null>(null);

  const outline = $derived(file?.outline ?? null);
  /** The file on disk has changed since this doc was written. */
  const stale = $derived(!!doc && !!file && doc.sourceSha !== file.sourceSha);
  const moduleCount = $derived(outline?.modules?.length ?? 0);

  // ---- opening ------------------------------------------------------------

  async function pickFile() {
    const picked = await openDialog({
      multiple: false,
      filters: [{ name: "Elixir", extensions: ["ex", "exs"] }],
    });
    if (typeof picked === "string") await load(picked);
  }

  async function load(path: string) {
    error = null;
    focus.clear();
    try {
      const opened = await ipc.openFile(path);
      file = opened;
      if (opened.existing.length) {
        // Your past reading of this file is worth more than a fresh start.
        chooser = opened.existing;
        doc = null;
        markdown = "";
      } else {
        chooser = null;
        await startFreshDoc(opened);
      }
    } catch (e) {
      error = String(e);
    }
  }

  async function startFreshDoc(opened: OpenedFile) {
    const seeded = opened.outline
      ? await ipc.seedDoc(opened.path, opened.outline, opened.source)
      : `# ${opened.filename}\n\n> _what is this file for?_\n`;

    doc = await ipc.createDoc({
      path: opened.path,
      lang: opened.lang ?? "text",
      title: opened.outline?.modules?.[0]?.name ?? opened.filename,
      branch: opened.branch,
      markdown: seeded,
      source: opened.source,
    });
    markdown = doc.markdown;
    dirty = false;
    chooser = null;
  }

  async function openExisting(summary: DocSummary) {
    const loaded = await ipc.loadDoc(summary.id);
    doc = loaded;
    markdown = loaded.markdown;
    dirty = false;
    chooser = null;

    // Opening from the library may mean no file is loaded yet.
    if (!file || file.path !== loaded.path) {
      try {
        file = await ipc.reparse(loaded.path);
      } catch {
        // The file moved or was deleted. The doc still opens — it carries its
        // own snapshot of the source, which is the point of storing it.
        file = {
          path: loaded.path,
          filename: loaded.filename,
          source: loaded.source,
          sourceSha: loaded.sourceSha,
          lang: loaded.lang,
          outline: null,
          branch: loaded.branch,
          hasGit: false,
          existing: [],
        };
        error = "File not found on disk — showing the snapshot saved with this doc.";
      }
    }
  }

  async function reparseNow() {
    if (!file) return;
    file = await ipc.reparse(file.path);
  }

  async function reconcile() {
    if (!doc || !file?.outline) return;
    const merged = await ipc.reconcileDoc(doc.id, file.outline, file.source);
    doc = merged;
    markdown = merged.markdown;
    dirty = false;
  }

  // ---- autosave -----------------------------------------------------------

  let timer: ReturnType<typeof setTimeout> | null = null;

  function scheduleSave() {
    if (timer) clearTimeout(timer);
    timer = setTimeout(save, 800);
  }

  async function save() {
    if (!doc || !dirty) return;
    saving = true;
    try {
      doc = await ipc.saveDoc({ id: doc.id, markdown });
      dirty = false;
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }

  // Autosave on every edit, and never leave a pending edit unsaved on exit.
  $effect(() => {
    const text = markdown;
    if (!doc || text === doc.markdown) return;
    dirty = true;
    scheduleSave();
  });

  $effect(() => {
    const flush = () => {
      if (timer) clearTimeout(timer);
      save();
    };
    window.addEventListener("blur", flush);
    window.addEventListener("beforeunload", flush);
    return () => {
      window.removeEventListener("blur", flush);
      window.removeEventListener("beforeunload", flush);
    };
  });

  // ---- keyboard -----------------------------------------------------------

  function onKeydown(e: KeyboardEvent) {
    const meta = e.metaKey || e.ctrlKey;
    if (e.key === "Escape") {
      if (showLibrary) showLibrary = false;
      else focus.clear();
      return;
    }
    if (meta && e.key === "o") {
      e.preventDefault();
      pickFile();
    } else if (meta && e.key === "s") {
      e.preventDefault();
      save();
    } else if (meta && e.key === "k") {
      e.preventDefault();
      showLibrary = !showLibrary;
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="app">
  <!-- Row 1: the window's own chrome. Draggable, and kept nearly empty — it
       shares this strip with the macOS traffic lights. -->
  <div class="titlebar" data-tauri-drag-region>
    <!-- Only the element carrying the attribute is draggable, so the label and
         the empty stretch carry it too — otherwise the grabbable area is just
         the few pixels of gap between them. -->
    <span class="brand" data-tauri-drag-region>LGTM</span>
    <span class="spacer" data-tauri-drag-region></span>
    <button class="btn icon" onclick={() => theme.cycle()} title="Cycle theme">
      {theme.label}
    </button>
  </div>

  <!-- Row 2: the app's own header. Everything about the file being read lives
       here, where there is room for it. -->
  {#if file}
    <div class="apphead">
      <div class="ident">
        <span class="name">{file.filename}</span>
        <span class="path">{file.path.replace(/\/[^/]+$/, "")}</span>
      </div>
      {#if file.branch}
        <span class="branch" title="Branch, read from .git/HEAD">⑂ {file.branch}</span>
      {/if}

      <span class="spacer"></span>

      <span class="save" class:dirty>{saving ? "Saving…" : dirty ? "Unsaved" : doc ? "Saved ✓" : ""}</span>
      <button class="btn" onclick={reparseNow}>Re-parse</button>
      <button class="btn" onclick={() => (showLibrary = true)}>Library</button>
      <button class="btn primary" onclick={pickFile}>Open file…</button>
    </div>
  {/if}

  {#if error}
    <div class="banner">
      {error}
      <button onclick={() => (error = null)}>×</button>
    </div>
  {/if}

  {#if !file}
    <div class="welcome">
      <img src="/app-icon.png" alt="" class="hero" />
      <h1>lgtm</h1>
      <p>Open an Elixir file. The source goes left, your explanation goes right.</p>
      <div class="actions">
        <button class="btn primary" onclick={pickFile}>Open a file… <kbd>⌘O</kbd></button>
        <button class="btn" onclick={() => (showLibrary = true)}>Library <kbd>⌘K</kbd></button>
      </div>
    </div>
  {:else}
    <div class="split">
      <div class="pane" style:flex="0 0 {basis}%">
        <CodePane
          source={file.source}
          lang={file.lang}
          filename={file.filename}
          path={file.path}
          hasGit={file.hasGit}
          {outline}
        />
      </div>

      <Divider bind:basis />

      <div class="pane grow">
        {#if chooser}
          <div class="chooser">
            <h2>You've read this file before</h2>
            <ul>
              {#each chooser as c (c.id)}
                <li>
                  <button onclick={() => openExisting(c)}>
                    <b>{c.title}</b>
                    <span>{c.branch ?? "no branch"} · {new Date(c.updatedAt).toLocaleDateString()}</span>
                  </button>
                </li>
              {/each}
            </ul>
            <button class="btn" onclick={() => file && startFreshDoc(file)}>Start a fresh doc</button>
          </div>
        {:else}
          <DocPane
            bind:markdown
            {outline}
            filename={file.filename}
            {dirty}
            {stale}
            onreconcile={reconcile}
          />
        {/if}
      </div>
    </div>

    <div class="status">
      <span>
        {#if outline}
          tree-sitter-{file.lang} · {outline.modules.reduce((n, m) => n + m.functions.length, 0)} functions
        {:else}
          no outline for this file type
        {/if}
      </span>
      {#if moduleCount > 1}
        <span class="warn">{moduleCount} modules in this file — seeded from the first</span>
      {/if}
      <span class="spacer"></span>
      {#if doc}<span>doc #{doc.id} → sqlite</span>{/if}
    </div>
  {/if}

  {#if showLibrary}
    <Library
      onopen={(d) => {
        showLibrary = false;
        openExisting(d);
      }}
      onclose={() => (showLibrary = false)}
    />
  {/if}
</div>

<style>
  .app {
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .titlebar {
    height: 38px;
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px 0 84px; /* room for the macOS traffic lights */
    background: var(--bg-raised);
    border-bottom: 1px solid var(--line);
    user-select: none;
  }
  .brand {
    font-weight: 700;
    letter-spacing: 0.08em;
    font-size: 11px;
    color: var(--fg-dim);
  }

  /* Row 2 — the file being read, with room to breathe. */
  .apphead {
    flex: none;
    height: 46px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px;
    background: var(--bg);
    border-bottom: 1px solid var(--line);
  }
  .ident {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.25;
  }
  .ident .name {
    font-size: 13px;
    font-weight: 600;
    color: var(--fg);
  }
  .ident .path {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl; /* truncate the front of a long path, not the end */
    text-align: left;
  }
  .branch {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg-dim);
    background: var(--bg-inset);
    border: 1px solid var(--line);
    border-radius: 999px;
    padding: 2px 9px;
    white-space: nowrap;
  }
  .spacer {
    flex: 1;
  }
  .save {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--pub);
    min-width: 56px;
    text-align: right;
  }
  .save.dirty {
    color: var(--priv);
  }

  .banner {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
    background: color-mix(in srgb, var(--priv) 12%, transparent);
    color: var(--priv);
    font-size: 12px;
  }
  .banner button {
    margin-left: auto;
    background: none;
    border: 0;
    color: inherit;
    cursor: pointer;
    font-size: 15px;
  }

  .split {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .pane {
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .pane.grow {
    flex: 1 1 auto;
  }

  .status {
    height: 24px;
    flex: none;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 0 12px;
    background: var(--bg-raised);
    border-top: 1px solid var(--line);
    font-family: var(--mono);
    font-size: 11px;
    color: var(--fg-faint);
  }
  .status .warn {
    color: var(--priv);
  }

  .welcome {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
  }
  .hero {
    width: 168px;
    height: 168px;
    border-radius: 30px;
    margin-bottom: 18px;
    box-shadow: 0 10px 34px rgba(16, 24, 40, 0.14);
    user-select: none;
    -webkit-user-drag: none;
  }
  :global(html.dark) .hero {
    box-shadow: 0 10px 34px rgba(0, 0, 0, 0.5);
  }
  .welcome h1 {
    font-size: 34px;
    margin: 0;
    letter-spacing: -0.02em;
  }
  .welcome p {
    color: var(--fg-dim);
    margin: 0 0 14px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  kbd {
    font-family: var(--mono);
    font-size: 10px;
    opacity: 0.7;
    margin-left: 4px;
  }

  .chooser {
    padding: 40px 30px;
    max-width: 520px;
  }
  .chooser h2 {
    font-size: 14px;
    margin: 0 0 12px;
    color: var(--fg-dim);
  }
  .chooser ul {
    list-style: none;
    padding: 0;
    margin: 0 0 16px;
  }
  .chooser li button {
    display: block;
    width: 100%;
    text-align: left;
    font: inherit;
    background: var(--bg-raised);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 10px 12px;
    margin-bottom: 8px;
    cursor: pointer;
    color: var(--fg);
  }
  .chooser li button:hover {
    border-color: var(--accent);
  }
  .chooser li span {
    display: block;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--fg-faint);
    margin-top: 2px;
  }
</style>
