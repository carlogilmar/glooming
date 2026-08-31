<script lang="ts">
  // What you navigate the current file by, above your note in the same scroll.
  //
  // This started as a fixed-height drawer with its own scrollbar and a collapse
  // toggle, and both were wrong. A band permanently taking 270px from the pane
  // you *write* in costs you the thing that pane is for, and two independent
  // scroll regions in one column means neither can be scrolled past. So it is
  // one column: surface, then the boundary, then your prose. Scroll down to
  // write, scroll up to look something up.
  //
  // Being able to scroll away is what makes the diagram affordable inline — and
  // inline is the only place it does its job. Behind a button it is a thing you
  // remember to open; in the column it is a map you glance at while reading.
  //
  // Rendered *inside* DocPane's scroll container as a snippet, so there is
  // exactly one scrollbar and read mode's measurements keep the element they
  // have always measured.

  import { renderDeps } from "$lib/deps";
  import { moduleOf } from "$lib/fileset";
  import { seedDepsBlock, settingsOf, summariseFile, surfaceOf, testsOf } from "$lib/explore";
  import type { ReadingFile } from "$lib/ipc";

  let {
    file = null,
    files = [],
    selected = "",
    onselect,
    onjump,
  }: {
    file: ReadingFile | null;
    /** The whole reading — a reached module that is in it can be jumped to. */
    files: ReadingFile[];
    selected: string;
    onselect?: (sig: string, line: number) => void;
    onjump?: (path: string, line: number) => void;
  } = $props();

  /**
   * The directory arrives in order, once per file.
   *
   * Gated on the *path* rather than on render, for the same reason `.arriving` in
   * `DocPane` is gated on the doc id: these sections re-render whenever anything
   * upstream changes, and a stagger that re-cascades while you type is exactly
   * the behaviour that gets animation switched off.
   *
   * `shown` is a plain `let`, not `$state` — it is bookkeeping only this effect
   * consults, and reading *and* writing one `$state` inside an effect is an
   * infinite loop.
   */
  let arriving = $state(false);
  let shown = "";
  $effect(() => {
    const path = file?.path ?? "";
    if (path === shown) return;
    shown = path;
    arriving = true;
    // A different file is a different boundary; an isolate held across the
    // switch would be answering a question about a module you have left.
    isolated = null;
    only = null;
    const off = setTimeout(() => (arriving = false), 900);
    return () => clearTimeout(off);
  });

  const kind = $derived(file?.outline?.kind ?? "plain");
  const module = $derived(moduleOf(file));
  const pub = $derived(surfaceOf(module, "public"));
  const priv = $derived(surfaceOf(module, "private"));
  const settings = $derived(settingsOf(file?.outline?.config?.groups ?? []));
  const imports = $derived(file?.outline?.config?.imports ?? []);
  const describes = $derived(testsOf(file?.outline?.tests ?? null));
  const suite = $derived(file?.outline?.tests ?? null);
  const nums = $derived(summariseFile(file, []));

  /**
   * The diagram, drawn by the same renderer the `lgtm:deps` block uses.
   *
   * `renderDeps` parses block *text*, so the live outline is written back into
   * that grammar rather than a second drawing routine living beside the first.
   * One set of layout arithmetic — the barycentre ordering, the both-columns
   * height, the pipe-shifted arities.
   */
  /**
   * Which outside function the boundary is currently answering *about*.
   *
   * Null is the whole picture: every local function, every line. Set, the module
   * is redrawn with only the functions that call it — "where is this used", which
   * is the question you actually have when your eye lands on `Repo.insert/1`, and
   * which two lit lines inside twelve grey rows do not really answer.
   */
  let isolated = $state<string | null>(null);

  /**
   * The mirror: a local function, isolating the boundary on *what it touches*.
   *
   * Clicking a name on the right asks "where is this used"; clicking one on the
   * left asks "what does this reach", and both are questions you have while
   * reading a function. Only one can be open — they are two answers to the same
   * picture, and holding both would draw a boundary that answers neither.
   */
  let only = $state<string | null>(null);

  const diagram = $derived(
    module && module.deps.length
      ? renderDeps(
          seedDepsBlock(module).split("\n").slice(1, -2).join("\n"),
          module,
          isolated,
          only,
        )
      : "",
  );

  /** The file an isolated function lives in, when it is one of ours. */
  const isolatedFile = $derived.by(() => {
    if (!isolated) return null;
    const mod = isolated.slice(0, isolated.lastIndexOf("."));
    return inReading.get(mod.split(".").pop() ?? "") ?? null;
  });

  function jumpTo(to: string) {
    const mod = to.slice(0, to.lastIndexOf("."));
    const fnName = to.slice(to.lastIndexOf(".") + 1);
    const target = inReading.get(mod.split(".").pop() ?? "");
    if (!target) return;
    const hit = target.outline?.modules
      ?.flatMap((m) => m.functions)
      .find((f) => `${f.name}/${f.arity}` === fnName || f.name === fnName.split("/")[0]);
    if (hit) onjump?.(target.path, hit.line);
  }

  /** Modules this file reaches that are themselves under review. */
  const inReading = $derived.by(() => {
    const out = new Map<string, ReadingFile>();
    for (const dep of module?.deps ?? []) {
      const short = dep.module.split(".").pop();
      const hit = files.find((f) =>
        f.outline?.modules?.some((m) => m.name.split(".").pop() === short),
      );
      if (hit && hit.path !== file?.path) out.set(short ?? "", hit);
    }
    return out;
  });

  function onClick(e: MouseEvent) {
    const t = e.target as HTMLElement;

    // A local function *in the boundary* does two things at once, and both are
    // what you meant by clicking it: the code selects it, and the picture narrows
    // to what it reaches. Clicking the isolated one again puts the boundary back.
    const local = t.closest<HTMLElement>(".lgtm-deps .fn[data-fn]");
    if (local) {
      const key = local.dataset.fn ?? "";
      isolated = null;
      only = only === key ? null : key;
      const line = parseInt(local.dataset.line ?? "0", 10);
      if (line > 0) onselect?.(local.dataset.sig ?? "", line);
      return;
    }

    // A row in the surface: focus it in the code, which is right there and never
    // covered.
    const own = t.closest<HTMLElement>("[data-line]");
    if (own) {
      const line = parseInt(own.dataset.line ?? "0", 10);
      if (line > 0) onselect?.(own.dataset.sig ?? "", line);
      return;
    }

    // An outside function: isolate the boundary on it, or clear it if it is the
    // one already isolated.
    //
    // Click used to jump straight into the other file, which was the *second*
    // question — you ask "who calls this" before you ask "what does it do". The
    // jump did not go away; it moved to a control under the diagram that names
    // the file it will open, which is more discoverable than an invisible click
    // target on a dot.
    const rfn = t.closest<HTMLElement>(".rfn[data-to]");
    if (!rfn) {
      // Empty space *inside the diagram* is the third way out, alongside the
      // button and clicking the isolated function again. Scoped to the diagram
      // so a click on the hint below it — or on the jump button in it — is not
      // silently an exit.
      if ((isolated || only) && t.closest(".reachwrap")) {
        isolated = null;
        only = null;
      }
      return;
    }
    const to = rfn.dataset.to ?? "";
    only = null;
    isolated = isolated === to ? null : to;
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="explore" class:arriving onclick={onClick}>
  <div class="head">
    <b>{file?.filename ?? ""}</b>
    <span class="kind">{kind}</span>
    <span class="nums">{nums}</span>
  </div>

  {#if kind === "config"}
    <p class="sec">Settings</p>
    {#each settings as g (g.app + (g.target ?? "") + g.line)}
      <div class="grp">
        <span>{g.app}</span>
        {#if g.target}<span class="tgt">{g.target}</span>{/if}
        <span class="ln">{g.line}</span>
      </div>
      {#each g.rows as r (r.key + r.line)}
        <button class="row inset" data-line={r.line} data-sig={r.key}>
          <span class="sig">{r.key}</span>
          <span class="val {r.kind === 'env!' ? 'envbang' : r.kind}">
            {#if r.kind === "secret"}secret{:else if r.kind === "literal"}= {r.value}{:else}{r.kind}
              {r.value}{/if}
          </span>
          <span class="ln">{r.line}</span>
        </button>
      {/each}
    {/each}
    {#if imports.length}
      <p class="sec">Imports</p>
      {#each imports as i (i)}
        <div class="row static"><span class="dim">import_config</span><span class="sig">{i}</span></div>
      {/each}
    {/if}
  {:else if kind === "test"}
    <p class="sec">Suite</p>
    <div class="row static">
      <span class="sig">{suite?.caseTemplate ?? "ExUnit.Case"}</span>
      <span class="dim">async {suite?.isAsync ?? false}</span>
      <span class="ln">{suite?.setups.length ?? 0} module setup</span>
    </div>
    <p class="sec">Describes</p>
    {#each describes as d (d.name + d.line)}
      <div class="grp">
        <span>describe "{d.name}"</span>
        {#if d.provides.length}
          <span class="setup">{d.provides.map((k) => ":" + k).join(" ")}</span>
        {/if}
        {#if d.unknown}
          <span class="setup" title="a named setup contributes keys that cannot be read from here">+?</span>
        {/if}
        {#each d.named as n (n)}<span class="setup dim">runs :{n}</span>{/each}
        <span class="ln">{d.line}</span>
      </div>
      {#each d.tests as t (t.name + t.line)}
        <button class="row inset" data-line={t.line} data-sig={t.name}>
          <span class="sig" class:skipped={t.skipped}>{t.name}</span>
          {#each t.tags as g (g)}<span class="badge">@{g}</span>{/each}
          <span class="asserts">{t.asserts}</span>
          <span class="strip" aria-hidden="true">
            {#each Array(Math.min(t.asserts, 3)) as _, i}<i class="a{i + 1}"></i>{/each}
          </span>
          <span class="ln">{t.line}</span>
        </button>
      {/each}
    {/each}
  {:else if !module}
    <p class="quiet">
      No module, config or test suite in this file — so there is nothing to
      navigate by. The code is on the left, and the note is yours.
    </p>
  {:else}
    <p class="sec">Surface</p>
    <div class="surface">
      {#each [["public", pub], ["private", priv]] as const as [k, rows]}
        <div class="col {k}">
          <div class="clabel"><span class="bar"></span>{k}<span class="n">{rows.length}</span></div>
          {#if rows.length}
            <!-- Six rows, then it scrolls. A table because the columns align: the
                 name you scan and the line you are going to, and tabular figures
                 so the digits stack. No badges — `default args` and `3 clauses`
                 are true, but at forty rows they turn a catalog into a wall of
                 annotations, and the arity already hints at the first. -->
            <div class="list">
              <table>
                <tbody>
                  {#each rows as r, i (r.sig)}
                    <tr
                      style:--i={Math.min(i, 12)}
                      class:sel={r.sig === selected}
                      data-line={r.line}
                      data-sig={r.sig}
                      title={r.flags.length ? r.flags.join(" · ") : undefined}
                    >
                      <td>{r.name}<span class="ar">{r.arity}</span></td>
                      <td class="ln">{r.line}</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {:else}
            <p class="none">nothing {k} here</p>
          {/if}
        </div>
      {/each}
    </div>

    <p class="sec">Reach</p>
    {#if diagram}
      <!-- Inline, always. The boundary is a map you glance at while reading the
           file, and a map behind a button is a map you forget you have.
           `renderDeps` returns its own `.lgtm-deps` frame and readout bar, which
           is why this is not wrapped again here. -->
      <div class="reachwrap">
        {@html diagram}
        {#if isolated || only}
          <!-- The way out, on the picture rather than under it: the diagram is
               where you are looking, and an exit you have to go and find is the
               reason a filtered view feels like a trap. -->
          <button
            class="isoclose"
            onclick={() => {
              isolated = null;
              only = null;
            }}
            title="Show the whole boundary"
          >
            <i>×</i> whole boundary
          </button>
        {/if}
      </div>
      {#if only}
        <p class="hint act">
          <b>{only}</b>
          <span>— showing only what it reaches</span>
        </p>
      {:else if isolated}
        <p class="hint act">
          <b>{isolated}</b>
          <span>— showing only what calls it</span>
          {#if isolatedFile}
            <button class="mini go" onclick={() => jumpTo(isolated!)}>
              open {isolatedFile.filename} ↗
            </button>
          {/if}
        </p>
      {:else}
        <p class="hint">
          Click anything on the right to see just the functions here that call it{inReading.size
            ? ` — ${[...inReading.keys()].join(", ")} ${inReading.size === 1 ? "is" : "are"} also in this reading`
            : ""}
        </p>
      {/if}
    {:else}
      <p class="quiet">
        {module.name.split(".").pop()} reaches nothing outside itself. That silence
        is the finding, not an empty state.
      </p>
    {/if}
  {/if}
</div>

<style>
  /* The navigation half's contents. It has no height of its own — the region
     around it owns that, and the grip below owns where it ends, so the labelled
     rule that used to close it went with the duplication: the grip already says
     "your note" and saying it twice, four pixels apart, is one place too many.

     The inset matches the note's (`.doc` is `26px 30px 40px`), because the two
     halves have to line up on the same left edge to read as one column split in
     two rather than as two unrelated panels. */
  .explore {
    padding: 22px 30px 26px;
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: 9px;
    flex-wrap: wrap;
    padding: 0 0 8px;
    margin: 0 0 16px;
    border-bottom: 1px solid var(--doc-line);
  }
  .head b {
    font-family: var(--mono);
    font-size: 13px;
    letter-spacing: -0.01em;
  }
  .kind {
    font-size: 8.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    padding: 1px 6px;
    border-radius: 3px;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }
  .nums {
    margin-left: auto;
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg-faint);
  }

  /* A heading with a rule running off to the right, so Surface and Reach read as
     sections rather than as small grey labels floating above content. */
  .sec {
    display: flex;
    align-items: center;
    gap: 9px;
    margin: 0 0 7px;
    font-size: 9.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--fg-faint);
  }
  .sec::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--doc-line);
    opacity: 0.7;
  }
  .sec:not(:first-of-type) {
    margin-top: 26px;
  }
  .quiet {
    max-width: 760px;
    margin: 0;
    padding: 9px 11px;
    font-size: 11px;
    font-style: italic;
    color: var(--fg-faint);
    border: 1px dashed var(--doc-line);
    border-radius: 7px;
  }
  .hint {
    max-width: 760px;
    margin: 7px 0 0;
    font-size: 10.5px;
    color: var(--fg-faint);
  }

  /* A catalog you look a name up in, framed the same way the diagram is so the
     two read as siblings rather than one loose list and one card. */
  .surface {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0;
    border: 1px solid var(--doc-line);
    border-radius: 8px;
    background: var(--code-bg);
    box-shadow: var(--shadow);
    overflow: hidden;
  }
  .col {
    /* Six rows of this height is the cap. Derived from the row rather than a
       pixel guess, so it stays six rows when the doc font is stepped. */
    --row: 23px;
    min-width: 0;
  }
  .col + .col {
    border-left: 1px solid var(--line-soft);
  }
  .clabel {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 7px 10px;
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--fg-faint);
    background: var(--doc-raised);
    border-bottom: 1px solid var(--line-soft);
  }
  .clabel .bar {
    width: 12px;
    height: 2px;
    border-radius: 1px;
    background: var(--pub);
  }
  .col.private .clabel .bar {
    background: var(--priv);
  }
  .clabel .n {
    margin-left: auto;
    font-family: var(--mono);
    font-size: 9px;
    font-weight: 400;
    padding: 0 5px;
    border-radius: 3px;
    background: var(--code-bg);
    color: var(--fg-dim);
  }

  .list {
    max-height: calc(var(--row) * 6);
    overflow: auto;
    scrollbar-width: thin;
    /* The page scrolls too, so a column that hits its end must not hand the
       scroll on and jump the note. */
    overscroll-behavior: contain;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  tbody tr {
    cursor: pointer;
  }
  tbody tr:hover {
    background: var(--sel);
  }
  tbody tr:hover td {
    color: var(--accent);
  }
  tbody tr.sel {
    background: var(--sel);
  }
  tbody tr.sel td {
    color: var(--accent);
  }
  td {
    height: var(--row);
    padding: 0 10px;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--fg-dim);
    border-bottom: 1px solid var(--line-soft);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  tbody tr:last-child td {
    border-bottom: 0;
  }
  /* The arity is part of the name, dimmed so the name is what you scan. */
  td .ar {
    color: var(--fg-faint);
  }
  /* The only other column: where it is. Tabular figures so the digits line up,
     which is the whole reason this is a table and not a list. */
  td.ln {
    width: 1%;
    text-align: right;
    font-size: 9.5px;
    color: var(--fg-faint);
    font-variant-numeric: tabular-nums;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    padding: 2.5px 6px;
    font: inherit;
    font-family: var(--mono);
    font-size: 11px;
    text-align: left;
    color: var(--fg-dim);
    background: transparent;
    border: 0;
    border-radius: 5px;
    cursor: pointer;
  }
  .row.static {
    cursor: default;
  }
  .row:not(.static):hover {
    background: var(--bg-inset);
    color: var(--fg);
  }
  .row.sel {
    background: var(--sel);
    color: var(--accent);
  }
  .row.inset {
    padding-left: 16px;
  }
  .dim {
    color: var(--fg-faint);
  }
  .badge {
    font-family: var(--sans);
    font-size: 8.5px;
    padding: 0 4px;
    border-radius: 3px;
    background: var(--bg-inset);
    color: var(--fg-faint);
  }
  .ln {
    margin-left: auto;
    font-size: 9.5px;
    color: var(--fg-faint);
  }
  .none {
    margin: 0;
    padding: 2px 6px;
    font-size: 10.5px;
    font-style: italic;
    color: var(--fg-faint);
  }

  .grp {
    display: flex;
    align-items: center;
    gap: 7px;
    margin: 9px 0 2px;
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg);
  }
  .grp .tgt,
  .grp .ln {
    color: var(--fg-faint);
  }
  .grp .setup {
    font-size: 9px;
    color: var(--mark);
  }
  .grp .setup.dim {
    color: var(--fg-faint);
  }
  .val {
    font-size: 10px;
  }
  .val.env {
    color: var(--pub);
  }
  .val.envbang {
    color: var(--priv);
  }
  .val.secret {
    color: var(--priv);
    font-style: italic;
  }
  .val.literal {
    color: var(--fg-faint);
  }
  .sig.skipped {
    text-decoration: line-through;
    color: var(--fg-faint);
  }
  .asserts {
    font-family: var(--sans);
    font-size: 9px;
    color: var(--fg-faint);
  }
  .strip {
    display: flex;
    gap: 2px;
  }
  .strip i {
    width: 7px;
    height: 7px;
    border-radius: 2px;
    background: var(--pub);
    opacity: 0.3;
  }
  .strip i.a2 {
    opacity: 0.6;
  }
  .strip i.a3 {
    opacity: 1;
  }

  /* Where the reference material ends and your writing begins — the most
     important boundary on the page, so it is labelled rather than left as an
     unexplained hairline. */
  /* The directory arrives in order — 16ms a row, capped at row 12. Two hundred
     functions would otherwise cascade for 3.4s, and waiting on an animation to
     look a name up is worse than no animation at all. Both columns share the
     index, so public and private arrive together rather than in two waves.

     Opacity only, no `translateY`: `transform` does not reliably apply to a
     table row across engines, and a fade that works everywhere beats a slide
     that works here. */
  .arriving tbody tr {
    animation: rowIn var(--fast) var(--ease-out) both;
    animation-delay: calc(var(--i, 0) * 16ms);
  }
  @keyframes rowIn {
    from {
      opacity: 0;
    }
  }

  /* The boundary assembles in the order you read it: the shape, then what is
     inside it top to bottom, then what lies beyond, and the lines last — a
     connection cannot arrive before both of its ends exist. Same 16ms beat as
     the table above it, so the two read as one arrival rather than two.

     `--i` and its cap come from `deps.ts`, which is the only place that knows
     the drawing order; every edge shares the last index, so they draw together.

     Opacity only, again: the diagram is one laid-out picture and sliding its
     parts in from an offset would misplace them relative to the lines. */
  .arriving :global(.lgtm-deps .bound),
  .arriving :global(.lgtm-deps .bound-label) {
    animation: partIn var(--fast) var(--ease-out) both;
  }
  .arriving :global(.lgtm-deps .fn),
  .arriving :global(.lgtm-deps .pierce),
  .arriving :global(.lgtm-deps .mod-name),
  .arriving :global(.lgtm-deps .mod-kind),
  .arriving :global(.lgtm-deps .rfn),
  .arriving :global(.lgtm-deps .edge) {
    animation: partIn var(--fast) var(--ease-out) both;
    animation-delay: calc(var(--i, 0) * 16ms);
  }
  @keyframes partIn {
    from {
      opacity: 0;
    }
  }

  .reachwrap {
    position: relative;
  }
  /* Three ways out, like the focus pill's four: this button, clicking the
     isolated function again, and clicking empty space in the diagram. One exit is
     never enough — the discoverable one has to be visible where you are looking. */
  .isoclose {
    position: absolute;
    top: 8px;
    right: 8px;
    display: flex;
    align-items: center;
    gap: 5px;
    font: inherit;
    font-size: 10px;
    letter-spacing: 0.02em;
    color: var(--fg-dim);
    background: var(--code-bg);
    border: 1px solid var(--doc-line);
    border-radius: 5px;
    padding: 3px 8px 3px 6px;
    cursor: pointer;
    box-shadow: var(--shadow);
  }
  .isoclose i {
    font-style: normal;
    font-size: 13px;
    line-height: 1;
    color: var(--fg-faint);
  }
  .isoclose:hover {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }
  .isoclose:hover i {
    color: var(--accent);
  }

  /* The isolate line: what the boundary is answering, and the way on from it. A
     hint rather than a toolbar — it appears only while a question is being asked,
     and it says in words what the picture is showing. */
  .hint.act {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .hint.act b {
    font-family: var(--mono);
    font-size: 10.5px;
    font-weight: 600;
    color: var(--fg-dim);
  }
  .mini {
    font: inherit;
    font-size: 10px;
    color: var(--fg-dim);
    background: var(--code-bg);
    border: 1px solid var(--doc-line);
    border-radius: 4px;
    padding: 2px 7px;
    cursor: pointer;
  }
  .mini:hover {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }
  .mini.go {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 35%, transparent);
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }

  @media (prefers-reduced-motion: reduce) {
    /* The rows and the diagram's parts are already where they belong; the
       cascade was the only motion, so there is nothing to preserve but the end
       state. */
    .arriving tbody tr {
      animation: none;
    }
    .arriving :global(.lgtm-deps .bound),
    .arriving :global(.lgtm-deps .bound-label) {
      animation: none;
    }
    .arriving :global(.lgtm-deps .fn),
    .arriving :global(.lgtm-deps .pierce),
    .arriving :global(.lgtm-deps .mod-name),
    .arriving :global(.lgtm-deps .mod-kind),
    .arriving :global(.lgtm-deps .rfn),
    .arriving :global(.lgtm-deps .edge) {
      animation: none;
    }
  }
</style>
