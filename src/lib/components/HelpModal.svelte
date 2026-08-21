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
          action: "click test / describe / setup / config",
          what:
            "In a test suite or a config script the same gesture works on the keyword opening a " +
            "block, and selects the whole block",
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
        { keys: ["↑", "↓"], what: "Move the review cursor one line. Also `j` / `k`, and `5j` for five" },
        { keys: ["[", "]"], what: "Previous / next function, selected whole. Does not wrap at either end" },
        { keys: ["⌘P"], what: "Jump to a function by name. Type a prefix, or an abbreviation like cu" },
        { keys: ["⌘R"], what: "Read mode on / off — scroll the note and the code follows your prose" },
        { keys: ["⌘E"], what: "Edit / preview the note. Entering edit leaves read mode; typing under a scroll-driven selection is chaos" },
        {
          action: "the top of the note",
          what:
            "What you navigate the current file by, above your writing in the same scroll: a " +
            "module's surface and the boundary it reaches through; a config script's settings, " +
            "marked env or literal; a suite's describes and the context each one starts with. " +
            "It follows the file you are in — scroll down to write, up to look something up",
        },
        {
          keys: ["⌘⇧T"],
          what:
            "The files in this reading — filter, ↑↓↵ to switch, × or ⌫ to remove one you opened " +
            "by accident. `⌘T` adds another. The button in the header does the same",
        },
        { keys: ["Esc"], what: "Clear the selection" },
      ],
    },
    {
      title: "Search",
      note:
        "Every occurrence in the file is marked, not just the one you jumped to — seeing where a " +
        "name appears is most of why you searched for it. smartcase, as vim does it: a lowercase " +
        "query ignores case, a capital letter makes it exact. A selected function keeps its " +
        "highlight but stops dimming the file, so no match is ever hidden.",
      rows: [
        { keys: ["/"], what: "Open the search bar and start typing" },
        { keys: ["↵"], what: "Jump to the first match at or after the line you're on" },
        { keys: ["n", "N"], what: "Next / previous occurrence, wrapping around the file" },
        { keys: ["Esc"], what: "Clear the search and its highlights" },
      ],
    },
    {
      title: "Vim motions",
      note:
        "Always on, no mode to remember — the code pane is read-only, so only the motions exist. " +
        "Counts work: 5j, 42G. Two deviations from vim: ? opens this help rather than searching " +
        "backwards (use / then N), and [ / ] step functions with one key instead of [[ / ]].",
      rows: [
        { keys: ["j", "k"], what: "Down / up a line — 5j moves five" },
        { keys: ["⌃d", "⌃u"], what: "Half a screen down / up, measured from what's actually visible" },
        { keys: ["g g", "G"], what: "First / last line. 42G goes to line 42" },
        { keys: ["H", "M", "L"], what: "Highest / middle / lowest line on screen" },
        { keys: ["{", "}"], what: "Previous / next blank-line block — in Elixir, function and pipeline boundaries" },
        { keys: ["z z"], what: "Centre the current line in the pane" },
        { keys: ["y y"], what: "Copy the current line" },
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
        { action: "A− / A+", what: "Code font size, 10–22px. Remembered between sessions" },
        {
          action: "◫ Blame",
          what:
            "Tints each line with its author's colour and names them in the gutter — runs of lines by " +
            "one person read as blocks. Stops any selection from dimming the file, so every author " +
            "stays visible. Only shown inside a git repo, and only runs git blame when pressed",
        },
        {
          action: "long lines",
          what: "Always soft-wrapped, so reading never means scrolling sideways",
        },
      ],
    },
    {
      title: "Your explanation",
      note: "Plain markdown with five extra blocks. The values live in the text, so you can edit any of them.",
      rows: [
        { action: "Preview / Edit", what: "Rendered blocks, or the raw markdown source" },
        {
          action: "A− / A+",
          what:
            "Text size for both Preview and Edit, kept separately from the code pane's. Headings, " +
            "inline code and the markdown source all scale with it; the blocks keep their own " +
            "sizes, since they are data rather than text",
        },
        {
          keys: ["/"],
          what:
            "While editing, insert a reference without remembering its arity. Filters as you type " +
            "— ↑↓ then ↵. Only fires at a word boundary, so a path or a date stays prose",
        },
        {
          action: "the surface block",
          what:
            "Public left, private right, sorted by name and scrolling separately — the directory. " +
            "Click a row to focus it in the code",
        },
        { action: "click a table row", what: "Focuses that function in the code" },
        { action: "click a treemap tile", what: "The same — sized by lines of code, biggest three labelled" },
        {
          action: "hover the reach block",
          what:
            "The module drawn as a closed shape, with lines only where a call leaves it. Hover a " +
            "function to see what it reaches, or something outside to see who reaches it. Functions " +
            "that reach nothing stay quiet — that silence is the point",
        },
        {
          action: "click a function there",
          what:
            "Focuses it in the code and pins its connections, so the picture still answers " +
            "“what does this reach” while you read",
        },
        { keys: ["⌘S"], what: "Force a save. Autosave already runs 800ms after you stop typing" },
        {
          action: "Code changed — reconcile",
          what: "Appears when the file on disk no longer matches this reading. Merging keeps every word you wrote",
        },
      ],
    },
    {
      title: "Reading",
      note:
        "Write about the code and lgtm walks it for you. Any inline code naming a function in " +
        "the reading — `create_user`, or `Billing.charge` for one in another module — " +
        "becomes a reference, and `L30-34` points at plain lines. No new syntax, so the markdown " +
        "still reads correctly pasted into a PR comment.",
      rows: [
        {
          action: "▷ Read",
          what:
            "Scroll the doc and the code follows your prose order — not the file's. The rest of " +
            "the file recedes further than usual, and what you are leaving lingers for a beat so " +
            "the jump reads as a connection rather than a cut",
        },
        {
          action: "one step per paragraph",
          what:
            "The first reference in a paragraph is its step; later mentions stay clickable but " +
            "don't re-trigger, so a paragraph naming three functions doesn't fire three jumps",
        },
        {
          action: "a struck-through reference",
          what:
            "The function it names is gone from the reading. It stays visible rather than quietly " +
            "becoming plain text — you should see when code moves out from under your explanation",
        },
        {
          action: "a reference with a left edge",
          what:
            "It points into another file of the reading. Clicking it, or scrolling onto it, swaps " +
            "the code pane first — the pane dips and names where you landed, rather than " +
            "crossfading line ranges between two files that share nothing",
        },
        {
          action: "a module is its last segment",
          what:
            "`SingleTarget.foo`, not the whole path to it — the prefix is the same for every " +
            "module in the reading. Any longer piece of the path still works, and two modules " +
            "sharing a last segment both keep their full names rather than becoming ambiguous",
        },
        {
          action: "the arity is optional",
          what:
            "`get_user` means every arity of it — they are one function to a reader, and both " +
            "get highlighted. Write `get_user/2` when you mean exactly that one. This is what " +
            "`/` inserts, because `search/1..2` was never readable in a sentence",
        },
        {
          action: "`attrs`, `String.trim`",
          what:
            "Left as ordinary prose, not struck through. A bare word that names no function, and " +
            "a module that isn't one of your files, are things you write about code — not broken " +
            "references to it. Anything explicit still strikes through when it goes missing",
        },
        {
          action: "an unqualified name",
          what:
            "Means the file the prose is currently about: after `MyApp.Billing.charge/2`, a bare " +
            "`L25-29` is still billing's. That depends only on the order of your prose, never on " +
            "which tab happens to be open — so a reading walks the same path every time",
        },
      ],
    },
    {
      title: "A reading of several files",
      note:
        "One note can cover more than one file, because a change worth reviewing usually does. " +
        "There is no gesture for creating a group: with a reading open, opening a file joins it, " +
        "and the files you open during a review are the set. Adding a file seeds nothing — your " +
        "note stays yours — it just widens what you can reference.",
      rows: [
        {
          action: "the file tabs",
          what:
            "Switcher and progress at once. A green dot means your note references that file, " +
            "amber that it has changed on disk since you read it, and hollow that you opened it " +
            "and never mentioned it — the same nudge an empty explanation slot is",
        },
        {
          action: "× on a tab",
          what:
            "Removes a file you opened by accident. Its snapshot leaves the reading; your note " +
            "and the file on disk are untouched. The file the reading started from has no × — " +
            "delete the whole reading instead",
        },
        {
          keys: ["/", "stats"],
          what:
            "Block commands, for when your explanation wants a reader to see something: " +
            "`/stats` for size and history, `/surface` for the directory, `/deps`, `/treemap`. " +
            "Add a space and a filename — `/surface impact_stage.ex` — for any other file in " +
            "the reading. Nothing is inserted for you; add one where you want it and delete it " +
            "when you don't",
        },
        {
          keys: ["/"],
          what:
            "While editing, offers every function in the reading, grouped by module. Every " +
            "reference it inserts is module-qualified — a one-file reading becomes a multi-file " +
            "one as soon as you open another, and what you already wrote has to still mean it. " +
            "The footer shows the exact text ↵ will give you",
        },
        {
          action: "← Home, then open",
          what: "The way to start a separate reading rather than adding to this one",
        },
      ],
    },
    {
      title: "Other kinds of file",
      note:
        "A module is not the only shape a file comes in, and the blocks that suit one say nothing " +
        "about another — so lgtm seeds different blocks depending on what it finds.",
      rows: [
        {
          action: "a config script",
          what:
            "Settings grouped by app, with every value marked as literal or read from the " +
            "environment. A hardcoded credential is reported without its value",
        },
        {
          action: "a test suite",
          what:
            "Describes with a strip of one square per test, shaded by how much it asserts, plus " +
            "the setup each group inherits and the context its tests start with",
        },
        {
          action: "anything else",
          what:
            "A script, a one-off .exs, a file that doesn't parse — a title, the size, and a blank " +
            "page. No error, and no empty blocks pretending there was something to say",
        },
      ],
    },
    {
      title: "Files and docs",
      rows: [
        { action: "← Home", what: "Back to your recent readings. Saves anything pending first" },
        {
          keys: ["⌘O", "⌘T"],
          what:
            "Find a file by name in the open folder — and with a reading open, add it to that " +
            "reading. Matches the whole path, so `web/proc` and `my_app/acc` both narrow, and " +
            "pasting a full path works too, for anything outside the project",
        },
        {
          action: "Open a folder…",
          what:
            "Pick the project once; it is remembered, and the last one is reopened next launch. " +
            "Build output and dependencies are never listed",
        },
        {
          keys: ["⌘⇧O"],
          what:
            "The system file picker, for a file outside the project. ⌘O searches the open folder " +
            "instead — that is the one you want almost every time",
        },
        {
          keys: ["⌘K"],
          what:
            "Library: search, sort by recent / name / folder, ↑↓↵ to open. A reading of several " +
            "files shows a `+n` beside its first one",
        },
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
