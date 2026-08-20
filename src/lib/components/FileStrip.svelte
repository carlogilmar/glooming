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

  /**
   * Paths whose dot has just turned green.
   *
   * A hollow dot means "you opened this and never wrote about it". The moment
   * your prose first names something in the file, that stops being true — and a
   * state change *you* caused is the one place a small reward is honest. Tracked
   * against the previous set rather than the current one, because the animation
   * is about the transition, not the state.
   */
  /**
   * The previous set is **deliberately not `$state`**.
   *
   * It is bookkeeping, not something the template renders — and an effect that
   * both reads and writes the same `$state` re-triggers itself forever. This was
   * that bug: the strip only mounts once a reading has two files, so adding a
   * second file pinned the main thread and the whole window stopped responding.
   * `svelte-check` cannot see it; nothing is mistyped.
   *
   * The rule this leaves behind: inside an effect, read reactive state or write
   * it, never both. `earned` is written here and read only by the template.
   */
  let seen: Set<string> | null = null;
  let earnTimer: ReturnType<typeof setTimeout> | null = null;
  let earned = $state<Set<string>>(new Set());

  $effect(() => {
    const now = referenced; // the only reactive read in here
    const before = seen;
    seen = new Set(now);

    // First paint of an already-written note is not an achievement: seed the
    // baseline silently, and only celebrate what changes after it.
    if (before === null) return;

    const fresh = [...now].filter((p) => !before.has(p));
    if (!fresh.length) return;

    earned = new Set(fresh);
    if (earnTimer) clearTimeout(earnTimer);
    earnTimer = setTimeout(() => (earned = new Set()), 900);
  });

  $effect(() => () => {
    if (earnTimer) clearTimeout(earnTimer);
  });

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

<div class="files">
  <div class="strip" role="tablist" aria-label="Files in this reading">
    {#each files as f (f.path)}
      {@const st = stateOf(f)}
      <div class="tabwrap">
        <button
          class="tab {st}"
          class:on={f.path === current}
          class:earned={earned.has(f.path)}
          class:removable={!f.origin}
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
        </button>

        {#if !f.origin}
          <!-- A sibling button rather than one nested inside the tab: nested
               interactive elements are invalid, and it means no stopPropagation
               to get right. Only visible on the tab you are pointing at, so a
               destructive control is never sitting under an idle cursor. -->
          <button
            class="x"
            aria-label="Remove {f.filename} from this reading"
            title="Remove from this reading"
            onclick={() => (confirming = confirming === f.path ? null : f.path)}>×</button
          >
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

  {#if confirming}
    <!-- A row below the strip, NOT a popover inside it. The strip scrolls
         sideways when there are many tabs, and `overflow-x: auto` forces
         `overflow-y` from visible to auto — so anything absolutely positioned
         below a tab is clipped by the strip's own 32px. It rendered every time
         and was never once visible. -->
    <div class="confirm">
      <b>Remove {confirming.split("/").pop()}?</b>
      <span>
        Its snapshot leaves this reading. Your note is untouched, and so is the
        file on disk.
      </span>
      <span class="spacer"></span>
      <button
        class="go"
        onclick={() => {
          const path = confirming!;
          confirming = null;
          onremove?.(path);
        }}>Remove</button
      >
      <button onclick={() => (confirming = null)}>Keep</button>
    </div>
  {/if}
</div>

<style>
  .files {
    flex: none;
  }

  .strip {
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
    display: flex;
    align-items: center;
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
  /* Earning it: the dot fills, and one ring leaves. */
  .tab.earned i {
    animation: fill var(--fast) var(--ease) both;
  }
  .tab.earned i::after {
    content: "";
    position: absolute;
    inset: -3px;
    border-radius: 50%;
    border: 1.5px solid var(--pub);
    animation: dotring var(--ring) var(--ease) both;
  }
  @keyframes fill {
    from {
      transform: scale(0.4);
    }
    to {
      transform: scale(1);
    }
  }
  @keyframes dotring {
    0% {
      opacity: 0.9;
      transform: scale(0.6);
    }
    100% {
      opacity: 0;
      transform: scale(1.9);
    }
  }
  /* Motion off: the dot is simply green, which was always the actual message. */
  @media (prefers-reduced-motion: reduce) {
    .tab.earned i {
      animation: none;
    }
    .tab.earned i::after {
      display: none;
    }
  }
  .tab.missing .name {
    text-decoration: line-through;
  }
  /* Room for the × that sits on top of the tab's right edge. */
  .tab.removable {
    padding-right: 22px;
  }

  .x {
    position: absolute;
    right: 4px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    padding: 0;
    border: none;
    background: transparent;
    border-radius: 4px;
    font-size: 13px;
    line-height: 1;
    color: var(--fg-faint);
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.12s;
  }
  .tabwrap:hover .x,
  .x:focus-visible {
    opacity: 0.75;
  }
  .x:hover {
    opacity: 1;
    color: var(--priv);
    background: color-mix(in srgb, var(--priv) 16%, transparent);
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
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    background: color-mix(in srgb, var(--priv) 9%, transparent);
    border-bottom: 1px solid var(--line);
  }
  .confirm b {
    flex: none;
    font-size: 11.5px;
    color: var(--priv);
  }
  .confirm > span {
    font-size: 11px;
    color: var(--fg-dim);
  }
  .confirm button {
    flex: none;
    padding: 3px 9px;
    font-size: 11px;
    color: var(--fg-dim);
    background: var(--bg);
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
