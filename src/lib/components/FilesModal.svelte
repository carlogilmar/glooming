<script lang="ts">
  // The files a reading covers — manage them, and switch between them.
  //
  // This replaced the tab strip, and it was a measurement rather than a taste:
  // ten filenames need about 1200px of tabs and the left pane has about 750, so
  // three of ten were off-screen before you started. The strip's whole job was
  // "which files, which one am I in" and it stopped doing it at exactly the size
  // a real PR is.
  //
  // What made the strip affordable to lose is that **navigation moved elsewhere**:
  // references in the note, and the drawer's reaches list. Switching file is no
  // longer the main way you get around, so it can cost a keystroke.
  //
  // Same idiom as the Library, scoped to one reading: filter, ↑↓↵, grouped by
  // directory. Removing asks first, in place.

  import type { ReadingFile } from "$lib/ipc";

  let {
    files = [],
    current = null,
    referenced = new Set<string>(),
    onpick,
    onremove,
    onadd,
    onclose,
  }: {
    files: ReadingFile[];
    current: string | null;
    /** Paths the prose references — the state word on each row. */
    referenced: Set<string>;
    onpick: (path: string) => void;
    onremove: (path: string) => void;
    /** Find a file in the project and add it — ⌘T. */
    onadd: () => void;
    onclose: () => void;
  } = $props();

  /** One frame, so there is a state to transition from — see RefMenu. */
  let mounted = $state(false);
  $effect(() => {
    const id = requestAnimationFrame(() => (mounted = true));
    return () => cancelAnimationFrame(id);
  });

  let query = $state("");
  let cursor = $state(0);
  let confirming = $state<string | null>(null);
  let input = $state<HTMLInputElement | null>(null);
  let list = $state<HTMLDivElement | null>(null);

  type State = "written" | "unread" | "stale" | "missing";

  function stateOf(f: ReadingFile): State {
    if (f.missing) return "missing";
    if (f.stale) return "stale";
    return referenced.has(f.path) ? "written" : "unread";
  }

  /** Said in words, not only in a dot — a ten-file list has room for it. */
  const WHY: Record<State, string> = {
    written: "referenced in your note",
    unread: "not mentioned yet",
    stale: "changed on disk",
    missing: "not on disk",
  };

  const dirOf = (p: string) => p.slice(0, p.lastIndexOf("/")) || "/";
  const baseOf = (p: string) => p.slice(p.lastIndexOf("/") + 1);

  /** Substring first, then subsequence — the same rule ⌘T and ⌘P use. */
  function score(hay: string, q: string): number | null {
    if (!q) return 0;
    const h = hay.toLowerCase();
    const n = q.toLowerCase();
    const at = h.indexOf(n);
    if (at !== -1) return at;
    let i = 0;
    for (const ch of n) {
      i = h.indexOf(ch, i);
      if (i === -1) return null;
      i++;
    }
    return 1000;
  }

  const hits = $derived.by(() =>
    files
      .map((f, i) => ({
        f,
        i,
        s: score(baseOf(f.path), query) ?? score(f.path, query),
      }))
      .filter((h) => h.s !== null)
      .sort((a, b) => a.s! - b.s! || a.i - b.i),
  );

  // Rows in order, with a directory header wherever it changes.
  const rows = $derived.by(() => {
    const out: { h: (typeof hits)[number]; n: number; head: string | null }[] = [];
    let seen: string | null = null;
    hits.forEach((h, n) => {
      const d = dirOf(h.f.path);
      out.push({ h, n, head: d === seen ? null : d });
      seen = d;
    });
    return out;
  });

  $effect(() => {
    query;
    cursor = 0;
    confirming = null;
  });

  // Land on the file you are in, so the modal opens where you already are.
  $effect(() => {
    const at = hits.findIndex((h) => h.f.path === current);
    if (at >= 0) cursor = at;
    input?.focus();
  });

  function keepVisible() {
    queueMicrotask(() =>
      list?.querySelector<HTMLElement>(".frow.cur")?.scrollIntoView({ block: "nearest" }),
    );
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      cursor = Math.min(cursor + 1, hits.length - 1);
      keepVisible();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      cursor = Math.max(cursor - 1, 0);
      keepVisible();
    } else if (e.key === "Enter") {
      e.preventDefault();
      const h = hits[cursor];
      if (h) onpick(h.f.path);
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (confirming) confirming = null;
      else onclose();
    } else if (e.key === "Backspace" && query === "") {
      // Only with an empty filter, or backspacing a typo would arm a delete.
      const h = hits[cursor];
      if (h && !h.f.origin) {
        e.preventDefault();
        confirming = h.f.path;
      }
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div
  class="scrim"
  class:mounted
  onclick={(e) => e.target === e.currentTarget && onclose()}
>
  <!-- A modal appears centred, so `transform-origin: center` is right here — the
       rule about scaling from a trigger is for popovers anchored to one. 200ms
       sits at the bottom of the 200–500ms budget a modal is allowed, because this
       one opens over work you were in the middle of. -->
  <div class="panel" role="dialog" aria-label="Files in this gloom">
    <div class="top">
      <h2>Files in this gloom</h2>
      <span class="c">{hits.length} of {files.length}</span>
      <span class="spacer"></span>
      <button class="btn" onclick={onclose}>esc</button>
    </div>

    <input
      bind:this={input}
      bind:value={query}
      onkeydown={onKey}
      placeholder="Filter by name or path…"
      spellcheck="false"
      autocomplete="off"
    />

    <div class="flist" bind:this={list}>
      {#each rows as row (row.h.f.path)}
        {#if row.head}
          <div class="fgroup">{row.head}/</div>
        {/if}
        {@const st = stateOf(row.h.f)}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div
          class="frow {st}"
          class:cur={row.n === cursor}
          class:on={row.h.f.path === current}
          class:origin={row.h.f.origin}
          onclick={() => onpick(row.h.f.path)}
          onmouseenter={() => (cursor = row.n)}
        >
          <i></i>
          <span class="name">{baseOf(row.h.f.path)}</span>
          <span class="kind">{row.h.f.outline?.kind ?? "text"}</span>
          <span class="why">{WHY[st]}</span>
          {#if !row.h.f.origin}
            <!-- Only on the row you are pointing at, so a destructive control is
                 never sitting under an idle cursor. The origin has none: it is
                 what the reading is anchored to. -->
            <span
              class="x"
              role="button"
              tabindex="-1"
              title="Remove from this gloom"
              onclick={(e) => {
                e.stopPropagation();
                confirming = confirming === row.h.f.path ? null : row.h.f.path;
              }}>×</span
            >
          {/if}
        </div>

        {#if confirming === row.h.f.path}
          <div class="confirm">
            <b>Remove {baseOf(row.h.f.path)}?</b>
            <span>
              Its snapshot leaves this reading. Your note is untouched, and so is the
              file on disk.
            </span>
            <span class="spacer"></span>
            <button
              class="go"
              onclick={(e) => {
                e.stopPropagation();
                const p = row.h.f.path;
                confirming = null;
                onremove(p);
              }}>Remove</button
            >
            <button onclick={(e) => (e.stopPropagation(), (confirming = null))}>Keep</button>
          </div>
        {/if}
      {:else}
        <p class="none">no file matches “{query}”</p>
      {/each}

      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div class="addrow" onclick={onadd}>
        + &nbsp;Add a file to this gloom… <kbd>⌘T</kbd>
      </div>
    </div>

    <footer>
      <kbd>↑</kbd><kbd>↓</kbd> move · <kbd>↵</kbd> open · <kbd>⌫</kbd> remove ·
      <kbd>⌘T</kbd> add · <kbd>esc</kbd> close
    </footer>
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
    width: 620px;
    max-width: calc(100vw - 40px);
    max-height: 74vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--bg-raised);
    border: 1px solid var(--line);
    border-radius: 12px;
    box-shadow: 0 22px 60px rgba(10, 12, 16, 0.3);
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
    gap: 9px;
    padding: 11px 13px;
    border-bottom: 1px solid var(--line);
  }
  .top h2 {
    margin: 0;
    font-size: 13px;
    font-weight: 650;
  }
  .top .c {
    font-size: 10.5px;
    color: var(--fg-faint);
  }
  .spacer {
    flex: 1;
  }
  input {
    width: 100%;
    font: inherit;
    font-size: 13px;
    padding: 9px 13px;
    color: var(--fg);
    background: var(--bg);
    border: 0;
    border-bottom: 1px solid var(--line);
    outline: none;
  }
  input::placeholder {
    color: var(--fg-faint);
  }

  .flist {
    flex: 1;
    overflow: auto;
    padding: 5px;
  }
  /* Sticky, so you never lose which directory you are scrolling through. */
  .fgroup {
    position: sticky;
    top: 0;
    z-index: 1;
    padding: 5px 9px 3px;
    font-family: var(--mono);
    font-size: 9.5px;
    color: var(--fg-faint);
    background: var(--bg-raised);
    border-bottom: 1px solid var(--line-soft);
  }
  .frow {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 6px 9px;
    border-radius: 7px;
    border-left: 2px solid transparent;
    cursor: pointer;
  }
  .frow.cur {
    background: var(--bg-inset);
    border-left-color: var(--accent);
  }
  .frow.on {
    background: var(--sel);
  }
  .frow i {
    width: 6px;
    height: 6px;
    flex: none;
    border-radius: 50%;
    background: var(--pub);
  }
  .frow.unread i {
    background: transparent;
    box-shadow: inset 0 0 0 1.5px var(--fg-faint);
  }
  .frow.stale i,
  .frow.missing i {
    background: var(--priv);
  }
  .frow .name {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--fg);
  }
  .frow.missing .name {
    text-decoration: line-through;
  }
  .frow .kind {
    font-size: 8.5px;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    padding: 0 5px;
    border-radius: 3px;
    color: var(--fg-faint);
    background: var(--bg-inset);
  }
  .frow .why {
    margin-left: auto;
    font-size: 10px;
    color: var(--fg-faint);
  }
  .x {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    border-radius: 4px;
    font-size: 13px;
    color: var(--fg-faint);
    opacity: 0;
    transition: opacity 0.12s;
  }
  .frow:hover .x,
  .frow.cur .x {
    opacity: 0.7;
  }
  .x:hover {
    opacity: 1;
    color: var(--priv);
    background: color-mix(in srgb, var(--priv) 14%, transparent);
  }

  .confirm {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 2px 0 4px;
    padding: 8px 10px;
    border-radius: 7px;
    font-size: 11px;
    color: var(--priv);
    background: color-mix(in srgb, var(--priv) 9%, transparent);
  }
  .confirm b {
    flex: none;
    font-size: 11.5px;
  }
  .confirm button {
    flex: none;
    font: inherit;
    font-size: 10.5px;
    padding: 3px 9px;
    border-radius: 5px;
    cursor: pointer;
    color: var(--fg-dim);
    background: var(--bg);
    border: 1px solid var(--line);
  }
  .confirm .go {
    color: #fff;
    background: var(--priv);
    border-color: var(--priv);
  }
  .confirm .go:hover {
    filter: brightness(1.08);
  }

  .addrow {
    display: flex;
    align-items: center;
    gap: 9px;
    margin: 3px 0 0;
    padding: 7px 9px;
    border-radius: 7px;
    cursor: pointer;
    font-size: 12px;
    color: var(--accent);
    border: 1px dashed color-mix(in srgb, var(--accent) 40%, transparent);
  }
  .addrow:hover {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }
  .none {
    margin: 0;
    padding: 14px 12px;
    font-size: 11.5px;
    color: var(--fg-faint);
  }
  footer {
    padding: 6px 13px;
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
