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
  import {
    seedDepsBlock,
    settingsOf,
    suiteScope,
    summariseFile,
    surfaceOf,
    testsOf,
  } from "$lib/explore";
  import type { Range, ReadingFile } from "$lib/ipc";

  let {
    file = null,
    files = [],
    selected = "",
    onselect,
    onjump,
    oncursor,
  }: {
    file: ReadingFile | null;
    /** The whole reading — a reached module that is in it can be jumped to. */
    files: ReadingFile[];
    selected: string;
    /**
     * `span` is what makes a test select as a test: without it the shell falls
     * back to `locate(sig, module)`, which looks a **function signature** up in
     * the outline — and a test name is not one, so it returned null and dropped
     * to a one-line cursor. `tag` rides in the `@spec` slot, which is exactly
     * what a `@tag` is to a test.
     */
    onselect?: (sig: string, line: number, span?: Range, tag?: Range | null) => void;
    onjump?: (path: string, line: number) => void;
    /** Go to a line without selecting anything — for a container. */
    oncursor?: (line: number) => void;
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
  const scope = $derived(suiteScope(file?.outline?.tests ?? null));

  /**
   * Describes the reader has folded away, keyed by file so two suites in one
   * gloom do not share a fold.
   *
   * Session state on purpose: folding is "let me see the shape for a second",
   * not a preference worth persisting — and a suite that reopens with half its
   * describes hidden looks broken rather than tidy.
   */
  let folded = $state<string[]>([]);
  const foldKey = (line: number) => `${file?.path ?? ""}:${line}`;
  const isFolded = (line: number) => folded.includes(foldKey(line));

  function toggleFold(line: number) {
    const k = foldKey(line);
    folded = folded.includes(k) ? folded.filter((x) => x !== k) : [...folded, k];
  }

  const allFolded = $derived(
    describes.length > 0 && describes.every((d) => isFolded(d.line)),
  );

  function foldAll(shut: boolean) {
    folded = shut ? describes.map((d) => foldKey(d.line)) : [];
  }
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

    // A container — a describe — is *gone to*, not selected. Selecting forty
    // lines dims nothing useful and claims you are reading all of them, when
    // what you are doing is travelling to one of the tests inside. So it lands
    // as a line cursor, which is the app's existing word for a position.
    const box = t.closest<HTMLElement>("[data-cursor]");
    if (box) {
      const line = parseInt(box.dataset.cursor ?? "0", 10);
      if (line > 0) oncursor?.(line);
      return;
    }

    // A row in the surface: focus it in the code, which is right there and never
    // covered. A row carrying `data-end` selects that whole span instead of
    // going through the signature lookup — a test's name is not a signature.
    const own = t.closest<HTMLElement>("[data-line]");
    if (own) {
      const line = parseInt(own.dataset.line ?? "0", 10);
      if (line <= 0) return;
      const end = parseInt(own.dataset.end ?? "0", 10);
      const tagged = own.dataset.tag?.split(",").map(Number);
      onselect?.(
        own.dataset.sig ?? "",
        line,
        end >= line ? { start: line, end } : undefined,
        tagged ? { start: tagged[0], end: tagged[1] } : null,
      );
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
    <span class="tkind">{kind}</span>
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
    <!-- The suite band: what it is built on, and whether it serialises. `async:
         false` is the one fact in a suite header worth calling out — it says the
         suite touches shared state — so it carries a colour rather than sitting
         dimmed at the end of a row. -->
    <div class="tsuite">
      <span class="tmpl">{suite?.caseTemplate ?? "ExUnit.Case"}</span>
      {#if suite?.isAsync}
        <span class="tchip async">async</span>
      {:else}
        <span class="tchip sync" title="this suite serialises — it touches shared state">
          async: false
        </span>
      {/if}
      {#each suite?.moduleTags ?? [] as g (g)}
        <span class="tchip quiet" title="@moduletag — applies to every test in this file">
          @{g}
        </span>
      {/each}
      <span class="ln">{nums}</span>
    </div>

    <!-- Module scope, stated once. A describe below shows only what it adds:
         the old view repeated these keys on every describe row, under a band
         that had already listed them. -->
    {#if scope.setups.length}
      <p class="sec">
        Every test starts from<span class="after">module scope</span>
      </p>
      <div class="tstack">
        {#each scope.setups as su (su.kind + su.line)}
          <button
            class="tsu"
            data-line={su.line}
            data-end={su.endLine}
            data-sig={su.kind}
            title="select the whole {su.kind} block"
          >
            <span class="tkind">{su.kind}</span>
            <span class="tlbl">{su.named ? "runs" : "provides"}</span>
            {#if su.named}
              <span class="tunk">:{su.named}</span>
            {:else if su.provides === null}
              <span class="tunk">unknown</span>
            {:else if su.provides.length}
              {#each su.provides as k (k)}<span class="tkey">:{k}</span>{/each}
            {:else}
              <span class="tunk">no context</span>
            {/if}
            <span class="ln">{su.line}</span>
          </button>
        {/each}
      </div>
    {/if}

    <p class="sec">
      Describes
      {#if describes.length > 1}
        <button class="foldall" onclick={() => foldAll(!allFolded)}>
          {allFolded ? "unfold all" : "fold all"}
        </button>
      {:else}
        <span class="after">a test selects whole; a describe goes there</span>
      {/if}
    </p>
    <!-- No inner scroll. The region above the grip already scrolls and is
         already resizable — one drag, remembered per gloom — so a second
         scroller inside it turned a fifty-test suite into a porthole inside a
         pane you were free to make taller. The cap was right for the *surface*,
         where the reach diagram sits below it and a 40-function module would
         otherwise bury it; nothing sits below this list. -->
    <div class="list tlist">
      {#each describes as d (d.name + d.line)}
        {@const shut = isFolded(d.line)}
        <section class="dgroup" class:shut>
          <!-- The fold toggle is a SIBLING of the header, never nested in it: a
               button inside a button is invalid, and it would need a
               stopPropagation to keep folding from also being a jump. -->
          <div class="dwrap">
            <button
              class="fold"
              aria-expanded={!shut}
              title="{shut ? 'unfold' : 'fold'} this describe"
              onclick={() => toggleFold(d.line)}
            >
              {shut ? "▸" : "▾"}
            </button>
            <button
              class="tdesc"
              data-cursor={d.line}
              title="go to line {d.line} — {d.endLine - d.line + 1} lines, {d.tests.length} tests"
            >
              <span class="tdn">
                describe <span class="q">"</span>{d.name}<span class="q">"</span>
              </span>
              <span class="tdelta">
                {#each d.adds.keys as k (k)}<span class="tkey">+:{k}</span>{/each}
                {#each d.adds.named as n (n)}<span class="tunk">+ :{n} ?</span>{/each}
                {#if d.adds.unknown && !d.adds.named.length}
                  <span class="tunk" title="a setup here contributes keys that cannot be read">
                    +?
                  </span>
                {/if}
              </span>
              <!-- How big this group is, so you can judge it before folding it
                   open. With nine describes the count is what you scan. -->
              <span class="dn">{d.tests.length}</span>
              <span class="ln">{d.line}</span>
            </button>
          </div>
          <div class="dtests">
            {#each d.tests as t (t.name + t.line)}
              <button
                class="trow"
                class:on={selected === t.name}
                data-line={t.line}
                data-end={t.endLine}
                data-tag={t.tagRange ? `${t.tagRange.start},${t.tagRange.end}` : undefined}
                data-sig={t.name}
                title="{t.endLine - t.line + 1} lines{t.tags.length
                  ? ' · @' + t.tags.join(' @')
                  : ''}"
              >
                <span class="tname" class:skipped={t.skipped}>{t.name}</span>
                {#each t.tags as g (g)}<span class="badge">@{g}</span>{/each}
                <!-- One bar per assertion, coloured by KIND. The old strip shaded
                     squares by assertion *count*, which reported how the author
                     liked to write rather than what the test checks: one
                     `assert {:ok, %User{email: ^e}} = …` checks four things and
                     drew palest. -->
                <span
                  class="tspine"
                  class:none={!t.assertions.length}
                  title={t.assertions.length
                    ? t.assertions.map((a) => a.kind).join(", ")
                    : "this test asserts nothing"}
                  aria-hidden="true"
                >
                  {#each t.assertions as a, i (a.line + "-" + i)}<i class={a.kind}></i>{/each}
                </span>
                <span class="ln">{t.line}</span>
              </button>
            {/each}
          </div>
        </section>
      {/each}
    </div>
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
  /* ============================================================
     A test suite. Candidate A from `mockup/tests.html`: the suite
     band, module scope stated once, then a BOUNDED list where a
     test selects whole and a describe is gone to.
     ============================================================ */
  .tsuite {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    padding: 7px 9px;
    margin-bottom: 12px;
    background: var(--doc-raised);
    border: 1px solid var(--doc-line);
    border-radius: 7px;
    font-size: 11.5px;
  }
  .tmpl {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--syn-mod);
    font-weight: 600;
  }
  .tchip {
    font-size: 10px;
    padding: 1.5px 6px;
    border-radius: 10px;
    font-weight: 600;
    border: 1px solid transparent;
    white-space: nowrap;
  }
  /* `async: false` says the suite touches shared state, which is the most
     reliably reviewable fact in a test file. It was a dim `async false` at the
     end of a row; it is context, so it carries a colour. */
  .tchip.sync {
    background: color-mix(in srgb, var(--priv) 14%, transparent);
    color: var(--priv);
    border-color: color-mix(in srgb, var(--priv) 32%, transparent);
  }
  .tchip.async {
    background: color-mix(in srgb, var(--pub) 13%, transparent);
    color: var(--pub);
  }
  .tchip.quiet {
    background: var(--bg-inset);
    color: var(--fg-dim);
    font-family: var(--mono);
  }

  .tstack {
    margin-bottom: 12px;
  }
  .tsu {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    font: inherit;
    font-size: 11.5px;
    text-align: left;
    cursor: pointer;
    background: transparent;
    border: 0;
    border-left: 2px solid var(--doc-line);
    padding: 3px 8px 3px 10px;
    color: var(--doc-fg);
  }
  .tsu:hover {
    background: var(--doc-raised);
  }
  .tkind {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--mark);
    font-weight: 600;
  }
  .tlbl {
    color: var(--fg-faint);
    font-size: 10px;
  }
  .tkey {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--syn-atom);
    background: color-mix(in srgb, var(--syn-atom) 11%, transparent);
    padding: 0 4px;
    border-radius: 3px;
  }
  /* Unknown, said out loud. A named callback lives elsewhere in the file, so
     its keys cannot be read from here — and unknown is not "provides nothing". */
  .tunk {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg-faint);
    border: 1px dashed var(--line);
    padding: 0 4px;
    border-radius: 3px;
  }

  .tdesc {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
    background: transparent;
    border: 0;
    padding: 5px 9px 5px 2px;
    color: var(--doc-fg);
  }
  /* The whole header lights up, not just the jump button — the fold sits inside
     it and a hover that stopped at the chevron would read as two rows. */
  .dwrap:hover {
    background: color-mix(in srgb, var(--read) 14%, var(--doc-raised));
  }
  .tdn {
    font-family: var(--mono);
    font-size: 11px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tdn .q {
    color: var(--fg-faint);
  }
  /* The delta chips are their own group at a tighter gap than the row's, or
     `+:user +:now` reads as two separate columns rather than one list. */
  .tdelta {
    display: flex;
    gap: 4px;
    min-width: 0;
  }

  .trow {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    font: inherit;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
    background: transparent;
    border: 0;
    padding: 3px 9px 3px 20px;
    color: var(--doc-fg);
  }
  .trow:hover {
    background: var(--doc-raised);
  }
  .trow.on {
    background: color-mix(in srgb, var(--accent) 11%, transparent);
  }
  .trow.on .tname {
    color: var(--accent);
    font-weight: 600;
  }
  .tname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tname.skipped {
    text-decoration: line-through;
    color: var(--fg-faint);
  }

  /* One bar per assertion, coloured by what it checks. Not shaded by count:
     that reported the author's style as though it were the test's thoroughness. */
  .tspine {
    display: flex;
    gap: 2px;
    margin-left: auto;
    align-items: center;
    flex: 0 0 auto;
  }
  .tspine i {
    width: 3px;
    height: 12px;
    border-radius: 1.5px;
    display: block;
    background: var(--pub);
  }
  .tspine i.error {
    background: var(--priv);
  }
  .tspine i.message {
    background: var(--mark);
  }
  /* A test that asserts nothing is a finding, not a pale square. */
  .tspine.none::after {
    content: "no assertions";
    font-family: var(--sans);
    font-size: 9.5px;
    color: var(--fg-faint);
    border: 1px dashed var(--line);
    padding: 0 4px;
    border-radius: 3px;
  }

  /* The list is a framed panel here, which it is not in the surface: the surface
     is two bare columns under a heading, where this is one bounded region with
     sticky describe headers inside it — so it needs an edge to be sticky
     against. Scoped to `.tlist`, or the frame lands on the surface too.

     `max-height: none` is the point. `.list`'s six-row cap exists so a
     40-function surface cannot bury the reach diagram *below* it; nothing sits
     below this list, so the cap bought nothing and cost everything — a 57-test
     suite became a 236px porthole inside a region that already scrolls and that
     the grip already resizes. Two scroll regions in one column means every
     gesture starts by deciding which one you are in, which is the argument this
     whole pane was built on. */
  /* `clip`, not `hidden`. Both clip the children to the rounded corner, but
     `hidden` makes this a scroll container — and a sticky describe header inside
     a scroll container that cannot scroll is pinned to a box that never moves,
     so it would never stick to anything. `clip` does not create a scrollport, so
     the headers stay sticky against the navigation region, which is the thing
     that actually scrolls. Same WebKit envelope as the `color-mix()` and `:has()`
     this UI already leans on. */
  .list.tlist {
    max-height: none;
    overflow: clip;
    border: 1px solid var(--doc-line);
    border-radius: 7px;
    background: var(--doc-bg);
  }
  .dgroup {
    border-bottom: 1px solid var(--doc-line);
  }
  .dgroup:last-child {
    border-bottom: 0;
  }
  /* Folding is `display`, not height. There is no version of an animated
     collapse that is right here: it has to move everything below it, so it goes
     through layout however it is written — the finding that outlived the
     explore drawer. */
  .dgroup.shut .dtests {
    display: none;
  }
  /* The header row is the sticky thing, so the toggle sticks with it. Its own
     element rather than the button, because the two buttons inside it do
     different jobs and neither should own the background. */
  .dwrap {
    display: flex;
    align-items: stretch;
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--doc-raised);
    border-bottom: 1px solid var(--doc-line);
  }
  .dgroup:last-child .dwrap {
    border-bottom: 0;
  }
  .dgroup:not(.shut):last-child .dwrap {
    border-bottom: 1px solid var(--doc-line);
  }
  .fold {
    font: inherit;
    font-size: 10px;
    line-height: 1;
    background: transparent;
    border: 0;
    color: var(--fg-faint);
    cursor: pointer;
    padding: 0 4px 0 7px;
  }
  .fold:hover {
    color: var(--accent);
  }
  /* How many tests this describe holds. With nine describes the count is the
     thing you scan, and it is what makes folding a decision rather than a
     guess. */
  .tdesc .dn {
    font-family: var(--mono);
    font-size: 9.5px;
    color: var(--fg-dim);
    background: var(--bg-inset);
    padding: 0 5px;
    border-radius: 8px;
    margin-left: auto;
  }
  .foldall {
    margin-left: auto;
    font: inherit;
    font-size: 10px;
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
    color: var(--fg-dim);
    background: transparent;
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 1px 6px;
    cursor: pointer;
  }
  .foldall:hover {
    color: var(--accent);
    border-color: var(--accent);
  }

  /* Line numbers in this view are mono and tabular, so the digits stack down
     the right-hand edge the way the surface table's do. The shared `.ln` is
     9.5px sans, which is right for a one-line summary and wrong for a column. */
  .tsu .ln,
  .tdesc .ln,
  .trow .ln {
    font-family: var(--mono);
    font-size: 10px;
    font-variant-numeric: tabular-nums;
  }

  /* A tag is written `@slow` in the source, so it is set in mono here. */
  .trow .badge {
    font-family: var(--mono);
    font-size: 9.5px;
    color: var(--fg-dim);
    white-space: nowrap;
  }

  .sec .after {
    margin-left: auto;
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
    color: var(--fg-faint);
  }

</style>
