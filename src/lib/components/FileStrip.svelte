<script lang="ts">
  // The files a reading covers.
  //
  // This is the whole UI for the file set — there is no block in the markdown
  // saying which files a reading is about, for the same reason a single-file
  // doc's path has never been in its prose. The strip is switcher and progress
  // at once: which files are in, which one you are looking at, and from the dot
  // whether each has actually been written about yet.
  //
  // The dot is the point. Opening a file to check something and never referring
  // to it again is the normal accident of a review, and a hollow dot is the same
  // nudge an empty explanation slot is, one level up: here is something you took
  // in and did not account for.

  import type { ReadingFile } from "$lib/ipc";

  let {
    files = [],
    current = null,
    /** Paths the prose actually references — drives the dot. */
    referenced = new Set<string>(),
    onswitch,
    onremove,
    onadd,
  }: {
    files: ReadingFile[];
    current: string | null;
    referenced: Set<string>;
    onswitch?: (path: string) => void;
    onremove?: (path: string) => void;
    onadd?: () => void;
  } = $props();

  /** Removing is destructive enough to confirm, and cheap enough to confirm inline. */
  let confirming = $state<string | null>(null);

  type State = "stale" | "missing" | "written" | "unread";

  function stateOf(f: ReadingFile): State {
    if (f.missing) return "missing";
    if (f.stale) return "stale";
    return referenced.has(f.path) ? "written" : "unread";
  }

  const WHY: Record<State, string> = {
    missing: "not on disk any more — showing the snapshot saved with this reading",
    stale: "changed on disk since you read it",
    written: "referenced in your note",
    unread: "opened, but your note never mentions it",
  };

  const unreferenced = $derived(files.filter((f) => stateOf(f) === "unread").length);
</script>

<div class="strip" role="tablist" aria-label="Files in this reading">
  {#each files as f (f.path)}
    {@const st = stateOf(f)}
    <div class="tabwrap">
      <button
        class="tab {st}"
        class:on={f.path === current}
        role="tab"
        aria-selected={f.path === current}
        title="{f.path} · {WHY[st]}"
        onclick={() => {
          confirming = null;
          onswitch?.(f.path);
        }}
      >
        <i></i>
        <span class="name">{f.filename}</span>
        {#if !f.origin}
          <!-- Only on the tab you are pointing at, so a destructive control is
               never sitting under an idle cursor. -->
          <span
            class="x"
            role="button"
            tabindex="-1"
            title="Remove from this reading"
            onclick={(e) => {
              e.stopPropagation();
              confirming = confirming === f.path ? null : f.path;
            }}
            onkeydown={(e) => {
              if (e.key === "Enter") {
                e.stopPropagation();
                confirming = f.path;
              }
            }}>×</span
          >
        {/if}
      </button>

      {#if confirming === f.path}
        <!-- What is lost is small and worth saying exactly: the snapshot goes,
             the note does not, and the file on disk was never touched. -->
        <div class="confirm">
          <b>Remove {f.filename}?</b>
          <span>Its snapshot leaves this reading. Your note is untouched, and so is the file on disk.</span>
          <div class="row">
            <button
              class="go"
              onclick={() => {
                confirming = null;
                onremove?.(f.path);
              }}>Remove</button
            >
            <button onclick={() => (confirming = null)}>Keep</button>
          </div>
        </div>
      {/if}
    </div>
  {/each}

  <button class="add" title="Add a file to this reading (⌘T)" onclick={() => onadd?.()}>+</button>

  <span class="spacer"></span>

  {#if unreferenced}
    <span class="note" title="Files you opened but never referenced">
      {unreferenced} unreferenced
    </span>
  {/if}
</div>

<style>
  .strip {
    flex: none;
    display: flex;
    align-items: center;
    gap: 3px;
    padding: 5px 8px;
    background: var(--bg-raised);
    border-bottom: 1px solid var(--line);
    overflow-x: auto;
    scrollbar-width: none;
  }
  .strip::-webkit-scrollbar {
    display: none;
  }
  .spacer {
    flex: 1;
  }

  .tabwrap {
    position: relative;
    flex: none;
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    font: inherit;
    font-size: 11.5px;
    font-family: var(--mono);
    color: var(--fg-dim);
    background: transparent;
    border: 1px solid transparent;
    border-radius: 6px;
    cursor: pointer;
    white-space: nowrap;
  }
  .tab:hover {
    color: var(--fg);
    background: var(--bg-inset);
  }
  .tab.on {
    color: var(--fg);
    background: var(--code-bg);
    border-color: var(--line);
    box-shadow: var(--shadow);
  }

  /* The dot carries the state, next to the name you are already reading. */
  .tab i {
    width: 5px;
    height: 5px;
    flex: none;
    border-radius: 50%;
    background: var(--pub);
  }
  .tab.stale i {
    background: var(--priv);
  }
  .tab.missing i {
    background: var(--priv);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--priv) 25%, transparent);
  }
  /* Hollow: in the reading, absent from the note. */
  .tab.unread i {
    background: transparent;
    box-shadow: inset 0 0 0 1.5px var(--fg-faint);
  }
  .tab.missing .name {
    text-decoration: line-through;
  }

  .x {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 13px;
    height: 13px;
    margin-right: -2px;
    border-radius: 4px;
    font-size: 12px;
    line-height: 1;
    color: var(--fg-faint);
    opacity: 0;
    transition: opacity 0.12s;
  }
  .tab:hover .x,
  .tab:focus-visible .x {
    opacity: 0.75;
  }
  .x:hover {
    opacity: 1;
    color: var(--priv);
    background: color-mix(in srgb, var(--priv) 14%, transparent);
  }

  .add {
    flex: none;
    width: 22px;
    height: 22px;
    font-size: 13px;
    line-height: 1;
    color: var(--fg-faint);
    background: transparent;
    border: 1px dashed var(--line);
    border-radius: 6px;
    cursor: pointer;
  }
  .add:hover {
    color: var(--accent);
    border-color: var(--accent);
    border-style: solid;
  }

  .note {
    flex: none;
    font-size: 10.5px;
    color: var(--fg-faint);
    padding-right: 2px;
  }

  .confirm {
    position: absolute;
    z-index: 40;
    top: calc(100% + 5px);
    left: 0;
    width: 258px;
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 9px 10px;
    background: var(--bg);
    border: 1px solid var(--priv);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgba(16, 24, 40, 0.16);
    text-align: left;
  }
  .confirm b {
    font-size: 12px;
    color: var(--fg);
  }
  .confirm span {
    font-size: 11px;
    line-height: 1.45;
    color: var(--fg-dim);
  }
  .confirm .row {
    display: flex;
    gap: 6px;
    margin-top: 2px;
  }
  .confirm button {
    flex: 1;
    padding: 4px 8px;
    font-size: 11px;
    color: var(--fg-dim);
    background: var(--bg-inset);
    border: 1px solid var(--line);
    border-radius: 5px;
    cursor: pointer;
  }
  .confirm button:hover {
    color: var(--fg);
  }
  .confirm .go {
    color: #fff;
    background: var(--priv);
    border-color: var(--priv);
  }
  .confirm .go:hover {
    color: #fff;
    filter: brightness(1.08);
  }
</style>
