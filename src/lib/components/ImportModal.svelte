<script lang="ts">
  // Importing a gloom from a markdown file.
  //
  // `mockup/import.html` is the contract. The flow is deliberately two steps
  // with nothing between them: you choose a file, and what appears is already
  // the answer. Loading and validating are one thought — a "Validate" button
  // would be a control whose only job is to ask a question the app can answer
  // on its own.
  //
  // **Strict.** A malformed header, a directory that is not here, one missing
  // file, or a different branch — any of them and Import is disabled. There is
  // no "anyway": a gloom is a reading of one version of a change, and every one
  // of those failures means the reading would not be the one that was written.

  import { untrack } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import * as ipc from "$lib/ipc";

  let { onopen, onclose, mode = "import", projectPath = null }: {
    /** A gloom was created — the shell adopts the reading it returns. */
    onopen: (reading: ipc.Reading) => void;
    onclose: () => void;
    /**
     * Which half you came in by. They are one panel because they are two halves
     * of one idea — the way `⌘T` and `⌘⇧T` are — and because a second modal
     * would be a second scrim, a second Escape and a second set of chrome to
     * keep in step.
     */
    mode?: "import" | "template";
    /** Pre-filled into the template: the tedious line, and the one you can get
     *  subtly wrong. */
    projectPath?: string | null;
  } = $props();

  let mounted = $state(false);
  // `mode` is the door you came in by; `view` is where you are now, and the two
  // buttons in the footer move between them. Snapshotting it once is the point,
  // so the read is untracked rather than left for Svelte to warn about: the
  // panel is mounted fresh on every open, and a later change to the prop must
  // not yank you out of the half you switched to.
  let view = $state<"import" | "template">(untrack(() => mode));
  let template = $state("");
  let copied = $state(false);
  let file = $state<string | null>(null);
  let root = $state<string | null>(null);
  let preview = $state<ipc.ImportPreview | null>(null);
  let error = $state("");
  let busy = $state(false);

  $effect(() => {
    const id = requestAnimationFrame(() => (mounted = true));
    return () => cancelAnimationFrame(id);
  });

  // Fetched rather than written here: the template lives beside the parser in
  // Rust, so a key cannot be added to one and forgotten in the other.
  $effect(() => {
    if (view !== "template" || template) return;
    ipc.gloomTemplate(projectPath).then((t) => (template = t)).catch(() => {});
  });

  async function copyTemplate() {
    try {
      await writeText(template);
      copied = true;
      setTimeout(() => (copied = false), 1600);
    } catch {
      /* no clipboard in a plain browser — the text is selectable regardless */
    }
  }

  const filename = $derived(file?.split("/").pop() ?? "");

  async function choose() {
    const picked = await openDialog({
      multiple: false,
      filters: [{ name: "Markdown", extensions: ["md", "markdown"] }],
    });
    if (typeof picked !== "string") return;
    file = picked;
    root = null;
    await validate();
  }

  /** Automatic: nothing between choosing a file and seeing what is wrong. */
  async function validate() {
    if (!file) return;
    error = "";
    try {
      preview = await ipc.previewImport(file, root);
    } catch (e) {
      preview = null;
      error = String(e);
    }
  }

  /** A colleague's checkout is somewhere else. Two clicks beats editing a path. */
  async function chooseRoot() {
    const picked = await openDialog({ directory: true, multiple: false });
    if (typeof picked !== "string") return;
    root = picked;
    await validate();
  }

  async function go() {
    if (!file || !preview?.ready || busy) return;
    busy = true;
    try {
      // Re-validated in Rust from scratch, so the branch is read at the gesture
      // rather than when this panel opened — you can check something out in a
      // terminal while a dialog is on screen.
      onopen(await ipc.importGloom(file, root));
    } catch (e) {
      error = String(e);
      await validate();
    } finally {
      busy = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    }
  }

  const missing = $derived(preview?.files.filter((f) => !f.found) ?? []);
  const kb = $derived(((preview?.noteBytes ?? 0) / 1024).toFixed(1));
</script>

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="scrim" class:mounted onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="panel">
    <div class="top">
      <h3>{view === "template" ? "New gloom file" : "Import a gloom"}</h3>
      {#if view === "import" && filename}<span class="file">{filename}</span>{/if}
    </div>

    {#if view === "template"}
      <!-- Copy it, fill in the two blanks, save it as .md. The project and the
           branch are already in — those are the lines that are tedious to type
           and easy to get subtly wrong, and they are the two we can know. -->
      <div class="body">
        <p class="hint tpl">
          Copy this, fill in the name and the files, and save it as a
          <code>.md</code> anywhere. Then come back and import it.
        </p>
        <div class="srcwrap">
          <button class="ghost copy" onclick={copyTemplate}>
            {copied ? "copied ✓" : "Copy"}
          </button>
          <pre class="src">{template}</pre>
        </div>
      </div>
    {:else if !file}
      <!-- The empty state names the other half too: you may well be here because
           you meant to write one rather than open one. -->
      <div class="pick">
        <div class="drop">
          <button class="primary" onclick={choose}>Choose a .md file…</button>
        </div>
        <p class="hint">
          A markdown file whose front matter names the project and the files, and
          whose body becomes the note.
        </p>
        <p class="hint">
          Don't have one yet?
          <button class="linkish" onclick={() => (view = "template")}>
            Start from a template
          </button>
        </p>
      </div>
    {:else if error && !preview}
      <div class="body">
        <div class="problems"><b>That file could not be read.</b> {error}</div>
      </div>
    {:else if preview}
      <div class="body">
        <div class="field">
          <span class="k">name</span>
          <span class="v"><span class="gname">{preview.name || "—"}</span></span>
        </div>

        <div class="field">
          <span class="k">project</span>
          <span class="v">
            {#if preview.rootOk}
              <span class="tick">✓</span><span class="path">{preview.root}</span>
              {#if preview.rootChosen}
                <span class="sub">chosen here — the file said {preview.project}</span>
              {/if}
            {:else}
              <span class="cross">✗</span><span class="path">{preview.root || "—"}</span>
              <span class="sub">not on this machine</span>
            {/if}
          </span>
        </div>

        <div class="field">
          <span class="k">branch</span>
          <span class="v">
            {#if preview.branch.state === "same"}
              <span class="tick">✓</span><span class="badge same">{preview.branch.branch}</span>
            {:else if preview.branch.state === "differs"}
              <span class="cross">✗</span>
              read on <span class="badge">{preview.branch.wants}</span>
              · you are on <span class="badge here">{preview.branch.here}</span>
            {:else}
              <span class="dim">not checked — {preview.branch.why}</span>
            {/if}
          </span>
        </div>

        {#if preview.files.length}
          <div class="field">
            <span class="k">files</span>
            <span class="v">
              {preview.files.length} listed · {preview.files.length - missing.length} found
              <div class="flist">
                {#each preview.files as f, i (f.path + f.line)}
                  <div class="frow" class:miss={!f.found}>
                    <span class={f.found ? "tick" : "cross"}>{f.found ? "✓" : "✗"}</span>
                    <span class="fp">{f.path}</span>
                    {#if !f.found}
                      <span class="why">not found</span>
                    {:else if i === 0}
                      <span class="org">origin</span>
                    {/if}
                  </div>
                {/each}
              </div>
            </span>
          </div>
        {/if}

        <div class="field">
          <span class="k">note</span>
          <span class="v">{kb} KB</span>
        </div>

        {#if preview.problems.length}
          <div class="problems">
            <b>This file cannot be read.</b>
            <ul>
              {#each preview.problems as p, i (i)}
                <li>{#if p.line}<span class="ln">line {p.line}</span> — {/if}{p.message}</li>
              {/each}
            </ul>
          </div>
        {:else if !preview.rootOk}
          <div class="problems">
            <b>That directory is not on this machine.</b>
            A gloom written by someone else names their checkout, not yours.
            <div class="act"><button class="ghost" onclick={chooseRoot}>Choose the directory…</button></div>
          </div>
        {:else if missing.length}
          <div class="problems">
            <b>
              {missing.length === 1 ? "One file is" : `${missing.length} files are`} missing,
              so nothing has been created.
            </b>
            They may be on another branch, or the file may name them wrongly.
            <ul>
              {#each missing as f (f.path)}
                <li><span class="ln">line {f.line}</span> — {f.path}</li>
              {/each}
            </ul>
          </div>
        {:else if preview.branch.state === "differs"}
          <div class="problems">
            <b>This gloom was read on {preview.branch.wants}.</b>
            Every file is here, but the note describes that branch — importing it
            onto {preview.branch.here} would put a reading of one version over the
            code of another. Check out {preview.branch.wants} and try again.
          </div>
        {/if}

        {#if error}<div class="problems">{error}</div>{/if}
      </div>
    {/if}

    <div class="foot">
      <span class="msg">
        {#if view === "template"}
          Nothing is created here — this is text to save as a file.
        {:else if !file}
          Nothing is created until the directory and every file resolve.
        {:else if preview?.ready}
          Ready — {preview.files.length}
          {preview.files.length === 1 ? "file" : "files"}, opening on
          {preview.files[0].path.split("/").pop()}.
        {:else}
          Nothing has been created.
        {/if}
      </span>
      <span class="btns">
        <button class="ghost" onclick={onclose}>{view === "template" ? "Done" : "Cancel"}</button>
        {#if view === "template"}
          <button class="primary" onclick={() => (view = "import")}>Import a file…</button>
        {:else}
          {#if file}
            <button class="ghost" onclick={choose}>Choose another…</button>
          {/if}
          <button class="primary" disabled={!preview?.ready || busy} onclick={go}>
            {busy ? "Importing…" : "Import"}
          </button>
        {/if}
      </span>
    </div>
  </div>
</div>

<style>
  .scrim {
    opacity: 0;
    transition: opacity 0.2s var(--ease-out);
    position: fixed;
    inset: 0;
    z-index: 24;
    background: rgba(10, 12, 16, 0.35);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 9vh;
  }
  .scrim.mounted {
    opacity: 1;
  }
  .panel {
    transform: scale(0.97);
    transition: transform 0.2s var(--ease-out);
    width: min(660px, 94vw);
    max-height: 74vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--bg-raised);
    border: 1px solid var(--line);
    border-radius: 12px;
    box-shadow: 0 18px 48px rgba(16, 24, 40, 0.18);
  }
  .scrim.mounted .panel {
    transform: scale(1);
  }
  @media (prefers-reduced-motion: reduce) {
    .panel,
    .scrim.mounted .panel {
      transform: none;
      transition: none;
    }
  }

  .top {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--line);
  }
  .top h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    font-family: var(--serif);
  }
  .top .file {
    margin-left: auto;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--fg-dim);
    background: var(--bg-inset);
    padding: 2px 7px;
    border-radius: 4px;
  }

  .body {
    padding: 6px 16px 14px;
    overflow: auto;
  }
  .pick {
    padding: 34px 16px 30px;
    text-align: center;
  }
  .drop {
    border: 1.5px dashed var(--line);
    border-radius: 10px;
    padding: 26px 18px;
    background: var(--bg);
  }
  .hint {
    margin: 14px auto 0;
    max-width: 46ch;
    color: var(--fg-faint);
    font-size: 11.5px;
    line-height: 1.6;
  }

  .hint.tpl {
    margin: 10px auto 10px;
    max-width: none;
    text-align: left;
  }
  .hint code {
    font-family: var(--mono);
    font-size: 11px;
    background: var(--bg-inset);
    padding: 1px 4px;
    border-radius: 3px;
  }
  .linkish {
    font: inherit;
    font-size: 11.5px;
    background: none;
    border: 0;
    padding: 0;
    color: var(--accent);
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .srcwrap {
    position: relative;
  }
  .srcwrap .copy {
    position: absolute;
    top: 8px;
    right: 8px;
  }
  /* Selectable, because the clipboard is a convenience and not the only way
     out — in a plain browser build `writeText` throws and this still works. */
  pre.src {
    margin: 0;
    background: var(--code-bg);
    border: 1px solid var(--line);
    border-radius: 9px;
    padding: 12px 14px;
    overflow: auto;
    max-height: 46vh;
    font-family: var(--mono);
    font-size: 11.5px;
    line-height: 1.6;
    color: var(--fg);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
  }

  .field {
    display: grid;
    grid-template-columns: 74px 1fr;
    gap: 10px;
    align-items: baseline;
    padding: 8px 0;
    border-bottom: 1px solid var(--line-soft);
  }
  .field:last-of-type {
    border-bottom: 0;
  }
  .k {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--fg-faint);
    font-weight: 600;
  }
  .v {
    min-width: 0;
    font-size: 12.5px;
  }
  .gname {
    font-family: var(--serif);
    font-size: 14px;
  }
  .path {
    font-family: var(--mono);
    font-size: 11.5px;
    word-break: break-all;
  }
  .sub {
    display: block;
    color: var(--fg-faint);
    font-size: 11px;
    margin-top: 3px;
    line-height: 1.5;
  }

  .tick,
  .cross {
    font-weight: 700;
    margin-right: 5px;
  }
  .tick {
    color: var(--pub);
  }
  .cross {
    color: var(--priv);
  }
  .dim {
    color: var(--fg-faint);
  }

  .flist {
    margin: 6px 0 0;
    border: 1px solid var(--line);
    border-radius: 7px;
    overflow: clip;
    background: var(--bg);
  }
  .frow {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 9px;
    font-family: var(--mono);
    font-size: 11.5px;
    border-bottom: 1px solid var(--line-soft);
  }
  .frow:last-child {
    border-bottom: 0;
  }
  .frow.miss {
    background: color-mix(in srgb, var(--priv) 8%, transparent);
  }
  .fp {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .why {
    margin-left: auto;
    font-family: var(--sans);
    font-size: 10.5px;
    color: var(--priv);
  }
  .org {
    margin-left: auto;
    font-family: var(--sans);
    font-size: 9.5px;
    color: var(--fg-faint);
    border: 1px solid var(--line);
    border-radius: 3px;
    padding: 0 5px;
  }

  .badge {
    font-family: var(--mono);
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--bg-inset);
    color: var(--fg-dim);
    border: 1px solid var(--line);
  }
  .badge.here {
    border-color: color-mix(in srgb, var(--priv) 40%, transparent);
    color: var(--priv);
    background: color-mix(in srgb, var(--priv) 9%, transparent);
  }
  .badge.same {
    border-color: color-mix(in srgb, var(--pub) 40%, transparent);
    color: var(--pub);
    background: color-mix(in srgb, var(--pub) 9%, transparent);
  }

  .problems {
    margin: 10px 0 0;
    border-radius: 8px;
    padding: 9px 11px;
    font-size: 12px;
    line-height: 1.55;
    border: 1px solid color-mix(in srgb, var(--priv) 32%, transparent);
    background: color-mix(in srgb, var(--priv) 8%, transparent);
  }
  .problems b {
    color: var(--priv);
  }
  .problems ul {
    margin: 5px 0 0;
    padding-left: 18px;
  }
  .problems li {
    margin: 2px 0;
  }
  .ln {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--fg-faint);
  }
  .act {
    margin-top: 8px;
  }

  .foot {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 11px 16px;
    border-top: 1px solid var(--line);
    background: var(--bg);
    font-size: 11.5px;
    color: var(--fg-dim);
  }
  .msg {
    min-width: 0;
    line-height: 1.45;
  }
  .btns {
    margin-left: auto;
    display: flex;
    gap: 8px;
    flex: none;
  }

  .ghost {
    font: inherit;
    font-size: 11.5px;
    background: var(--bg);
    color: var(--fg-dim);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 4px 10px;
    cursor: pointer;
  }
  .ghost:hover {
    border-color: var(--accent);
    color: var(--accent);
  }
  .primary {
    font: inherit;
    font-size: 12.5px;
    font-weight: 600;
    padding: 6px 15px;
    border-radius: 7px;
    cursor: pointer;
    background: var(--gloom-deep);
    color: #fff;
    border: 1px solid transparent;
  }
  .primary:hover:not(:disabled) {
    filter: brightness(1.08);
  }
  .primary:disabled {
    background: var(--bg-inset);
    color: var(--fg-faint);
    cursor: not-allowed;
    border-color: var(--line);
    filter: none;
  }
</style>
