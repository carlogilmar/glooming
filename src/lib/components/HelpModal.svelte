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
        { action: "click the filename", what: "Copies the file's full path, ready to paste into your editor" },
      ],
    },
    {
      title: "Navigating",
      rows: [
        { keys: ["↑", "↓"], what: "Move the review cursor one line. Also `j` / `k`, and `5j` for five" },
        { keys: ["[", "]"], what: "Previous / next function, selected whole. Does not wrap at either end" },
        {
          action: "⇧click, or drag the line numbers",
          what:
            "Select a run of lines. A click sets the review cursor — one line, \"where am I\" — " +
            "where a range is \"this bit here\", which is what you want when the thing you are " +
            "writing about is four lines of a pipeline rather than a whole function. Dragging " +
            "happens in the gutter because the code itself is selectable text, and a drag over it " +
            "has to keep meaning copy",
        },
        { keys: ["⌘P"], what: "Jump to a function by name. Type a prefix, or an abbreviation like cu" },
        {
          keys: ["⌥1", "⌥2"],
          what:
            "The right pane's split: navigation only, note only, the same key again to come " +
            "back. Or drag the grip between them to any height — double-click resets it. " +
            "Remembered per gloom, since a config script and a forty-function module want " +
            "different balances",
        },
        { keys: ["⌘R"], what: "Gloom: walk the code in the order your prose takes it — every reference, across modules. `esc` also leaves it" },
        { keys: ["⌘E"], what: "Edit / preview the note. Entering edit stops glooming; typing under a scroll-driven selection is chaos" },
        {
          action: "the views",
          what:
            "The right pane's top half, always live from disk: a module's surface and the " +
            "boundary it reaches through; a config script's settings, marked env or literal; a " +
            "suite's describes and the context each one starts with. They follow the file on " +
            "screen, with or without a gloom open",
        },
        {
          action: "the sidebar",
          what:
            "Every file in the project as a tree. A teal dot marks a file the open gloom mentions. " +
            "Drag its right edge to resize it (double-click resets); `‹` or `⌘\\` hides it, and the " +
            "strip left behind brings it back. Read mode always hides it",
        },
        {
          keys: ["⌘[", "⌘]"],
          what:
            "Back and forward along your trail — every module you opened, from the tree, ⌘T, a " +
            "reference or the reach diagram. Also the ← → in the code pane's header",
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
      title: "Glooms",
      note:
        "A gloom is a note in this project's knowledge base: one thing you worked out, in your " +
        "own words, beside the code it is about. It stores your prose and nothing else — no copy " +
        "of the code — so it always describes the code as it is now.",
      rows: [
        {
          action: "Start a gloom",
          what:
            "`+ New` in the band at the top. It opens with its name selected; exploring without " +
            "one saves nothing at all. `Glooms ▾` beside it lists this project's glooms",
        },
        {
          action: "open a gloom",
          what:
            "Lands the code pane on the first module its prose names, which is where your " +
            "understanding of it starts. `close ×` in its band goes back to exploring and keeps " +
            "your place in the code",
        },
        { action: "Preview / Edit", what: "The rendered note, or its raw markdown" },
        {
          action: "Copy / Save…",
          what:
            "The note's markdown, exactly as written — every reference carries its full module " +
            "name, so it reads correctly pasted into a PR comment or opened on another machine. " +
            "Your source files are never touched",
        },
        {
          action: "A− / A+",
          what: "Text size for both Preview and Edit, kept separately from the code pane's",
        },
        {
          keys: ["/"],
          what:
            "While editing, insert a reference to any function in a file you have open or have " +
            "walked through. It inserts the full module name and no arity — the footer shows the " +
            "exact text. Only fires at a word boundary, so a path or a date stays prose",
        },
        {
          action: "hover the reach diagram",
          what:
            "The module drawn as a closed shape, with lines only where a call leaves it. Click " +
            "something outside to see who here calls it, and open it from there — any module in " +
            "the project can be followed",
        },
        { keys: ["⌘S"], what: "Force a save. Autosave already runs 800ms after you stop typing" },
        { keys: ["⌘K"], what: "Search every gloom in this project — by name, tag or what it says" },
      ],
    },
    {
      title: "References",
      note:
        "Inline code naming something in the project becomes a reference, looked up by name every " +
        "time the note renders — so it follows a function that moved, and says so when it is " +
        "gone. No new syntax: the markdown still reads correctly anywhere.",
      rows: [
        {
          action: "`MyApp.Billing.charge`",
          what:
            "Every arity of it; `charge/2` narrows to one. Shown at its shortest unique name — " +
            "`Billing.charge` — while the text keeps the full one",
        },
        {
          action: "`MyApp.Billing.charge/2:3-6`",
          what:
            "Lines 3–6 of that function, counted from its first line, so an edit elsewhere in the " +
            "file cannot move them. Plain line numbers are gone: they could not follow anything",
        },
        { action: "`Weather.Pipeline`", what: "The module itself — a step that opens it" },
        {
          action: "an unqualified name",
          what:
            "Means the module the prose is currently about: after `MyApp.Billing.charge/2`, a bare " +
            "`to_cents/1` is Billing's. Only that module — across a whole codebase, a bare " +
            "`changeset/1` could be thirty things",
        },
        {
          action: "a dashed reference",
          what:
            "Ambiguous: `Pipeline.run` with two Pipelines in the project. Click it to choose, and " +
            "the text is rewritten to the full name. Read mode skips it until then",
        },
        {
          action: "a struck-through reference",
          what:
            "The function it names is gone, or the lines are past its end. It stays visible " +
            "rather than becoming plain text, and read mode skips it",
        },
        {
          action: "`attrs`, `String.trim`",
          what:
            "Left as prose. A bare word that names no function, and a module that is not in this " +
            "project, are things you write about code — not broken references to it",
        },
      ],
    },
    {
      title: "Reading",
      rows: [
        {
          action: "▷ Gloom",
          what:
            "Scroll the note and the code follows your prose order — not the file's — crossing " +
            "into whichever module the next reference names. Every reference is a step, in the " +
            "order you wrote them — several on one line are walked one by one. To gloom is to " +
            "read a codebase this way",
        },
        {
          keys: ["↓", "↑"],
          what:
            "While glooming: the next or previous reference, exactly one at a time (`j`/`k` too). " +
            "The code pane's line cursor stands down while the prose is driving it",
        },
        {
          action: "one step per reference",
          what:
            "Every reference is a stop, triggered as you reach the sentence naming it — so a " +
            "paragraph that names three functions walks the code through all three",
        },
        {
          action: "crossing a module",
          what:
            "The pane dips and its header lights up in gold, rather than crossfading line ranges " +
            "between two files that share nothing",
        },
      ],
    },
    {
      title: "Other kinds of file",
      note:
        "A module is not the only shape a file comes in, so the views change with what lgtm " +
        "finds.",
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
            "Describes as foldable sections, the setup each one inherits, and every assertion " +
            "marked by kind inside a selected test",
        },
        {
          action: "anything else",
          what: "One line saying there is nothing structural to navigate by — never empty boxes",
        },
      ],
    },
    {
      title: "Projects and files",
      rows: [
        { action: "← Home", what: "Back to your projects. Saves anything pending first" },
        {
          keys: ["⌘T", "⌘O"],
          what:
            "Find a module or file by name in the project. Matches the whole path, so `web/proc` " +
            "and `my_app/acc` both narrow, and a pasted full path works too",
        },
        {
          action: "Open a folder…",
          what:
            "On home. A project is a folder, remembered with its glooms. Build output and " +
            "dependencies are never listed",
        },
        { keys: ["⌘⇧O"], what: "The system file picker, for a file outside the project" },
        {
          action: "coming back to the window",
          what:
            "Re-reads what you have open and rebuilds the module index, so an edit made in your " +
            "editor is there when you return",
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
