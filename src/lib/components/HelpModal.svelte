<script lang="ts">
  // `?` — everything the app can do, in one place.
  //
  // Most of lgtm's interactions are discoverable by clicking around, but a few
  // are not: that clicking a function *name* means something different from
  // clicking its *line*, and that there are four ways out of a selection. This
  // is where those get said out loud.

  let { onclose }: { onclose: () => void } = $props();

  type Row = { keys?: string[]; action?: string; what: string };
  type Section = { title: string; note?: string; rows: Row[] };

  const sections: Section[] = [
    {
      title: "Reading code",
      note: "The left pane is read-only, permanently. lgtm reads; your editor edits.",
      rows: [
        {
          action: "click a function name",
          what: "Selects the whole function — every clause, its @spec and its @doc — and dims the rest of the file",
        },
        {
          action: "click any other line",
          what: "Puts the review cursor on that line, for reading one line at a time. Click it again to drop it",
        },
        { action: "click empty space", what: "Clears the selection" },
        {
          action: "click the sticky header",
          what: "Once you scroll past a definition its signature pins to the top; clicking it jumps back",
        },
        { action: "click the filename", what: "Copies the file's full path, ready to paste into your editor" },
      ],
    },
    {
      title: "Navigating",
      rows: [
        { keys: ["↑", "↓"], what: "Move the review cursor one line — also j and k" },
        { keys: ["[", "]"], what: "Previous / next function. Does not wrap at either end" },
        { keys: ["⌘P"], what: "Jump to a function by name. Type a prefix, or an abbreviation like cu" },
        { keys: ["Esc"], what: "Clear the selection" },
      ],
    },
    {
      title: "Leaving a selection",
      note: "Four ways, because one is never enough.",
      rows: [
        { keys: ["Esc"], what: "From anywhere" },
        { action: "re-click the row", what: "Reach back for the thing you clicked" },
        { action: "click empty code", what: "When the mouse is already over there" },
        { action: "click the hint pill", what: "The visible button, bottom of the code pane" },
      ],
    },
    {
      title: "The code pane's controls",
      rows: [
        { action: "A− / A+", what: "Font size, 10–22px. Remembered between sessions" },
        { action: "↵ Wrap", what: "Soft wrap long lines so nothing needs horizontal scrolling" },
        {
          action: "◫ Blame",
          what: "Who last touched each line. Only shown inside a git repo, and only runs git blame when pressed",
        },
      ],
    },
    {
      title: "Your explanation",
      note: "Plain markdown with three extra blocks. The values live in the text, so you can edit any of them.",
      rows: [
        { action: "Preview / Edit", what: "Rendered blocks, or the raw markdown source" },
        { action: "click a table row", what: "Focuses that function in the code" },
        { action: "click a treemap tile", what: "The same — sized by lines of code, biggest three labelled" },
        { keys: ["⌘S"], what: "Force a save. Autosave already runs 800ms after you stop typing" },
        {
          action: "Code changed — reconcile",
          what: "Appears when the file on disk no longer matches this reading. Merging keeps every word you wrote",
        },
      ],
    },
    {
      title: "Files and docs",
      rows: [
        { keys: ["⌘O"], what: "Open a file with the picker" },
        {
          keys: ["⌘L"],
          what: "Open a path you copied — quotes, file://, ~ and escaped spaces are all handled",
        },
        { keys: ["⌘K"], what: "Library: search, sort by recent / name / folder, ↑↓↵ to open" },
        { keys: ["?"], what: "This help" },
      ],
    },
  ];
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="scrim" onclick={onclose}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="panel" onclick={(e) => e.stopPropagation()}>
    <header>
      <b>lgtm</b>
      <span>a file is done when you can say <em>looks good to me</em></span>
      <span class="spacer"></span>
      <button class="btn" onclick={onclose}>Close</button>
    </header>

    <div class="body">
      {#each sections as section (section.title)}
        <section>
          <h3>{section.title}</h3>
          {#if section.note}<p class="note">{section.note}</p>{/if}
          <dl>
            {#each section.rows as row (row.what)}
              <dt>
                {#if row.keys}
                  {#each row.keys as k (k)}<kbd>{k}</kbd>{/each}
                {:else}
                  <span class="act">{row.action}</span>
                {/if}
              </dt>
              <dd>{row.what}</dd>
            {/each}
          </dl>
        </section>
      {/each}
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 24;
    background: rgba(10, 12, 16, 0.4);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 6vh;
  }
  .panel {
    width: min(760px, 94vw);
    max-height: 84vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-raised);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: 0 24px 70px rgba(10, 12, 16, 0.3);
    overflow: hidden;
  }

  header {
    flex: none;
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--line-soft);
  }
  header b {
    font-weight: 700;
    letter-spacing: 0.06em;
    font-size: 12px;
    color: var(--fg-dim);
    text-transform: uppercase;
  }
  header span {
    font-size: 11.5px;
    color: var(--fg-faint);
  }
  header em {
    font-style: italic;
  }
  header .spacer {
    flex: 1;
  }

  .body {
    overflow: auto;
    padding: 4px 16px 16px;
    min-height: 0;
  }

  section {
    padding-top: 16px;
  }
  h3 {
    margin: 0;
    font-size: 10.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--fg-faint);
  }
  .note {
    margin: 5px 0 0;
    font-size: 11.5px;
    color: var(--fg-dim);
    line-height: 1.5;
  }

  dl {
    display: grid;
    grid-template-columns: minmax(120px, max-content) 1fr;
    gap: 6px 14px;
    margin: 9px 0 0;
    align-items: baseline;
  }
  dt {
    white-space: nowrap;
  }
  dd {
    margin: 0;
    font-size: 12px;
    color: var(--fg-dim);
    line-height: 1.5;
  }

  kbd {
    font-family: var(--mono);
    font-size: 10.5px;
    border: 1px solid var(--line);
    border-bottom-width: 2px;
    border-radius: 4px;
    padding: 1px 5px;
    margin-right: 3px;
    color: var(--fg);
    background: var(--bg);
  }
  .act {
    font-size: 11.5px;
    color: var(--fg);
    white-space: nowrap;
  }
</style>
