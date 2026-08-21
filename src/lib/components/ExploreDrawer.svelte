<script lang="ts">
  // What you navigate by, for the file in front of you.
  //
  // This is where `lgtm:surface` and `lgtm:deps` went. In the note they were
  // pinned to a position in a narrative and only ever served the file the reading
  // started from — which is what made a multi-file reading uneven. Out here they
  // follow the tab, so **every file behaves the same and nothing is seeded**.
  //
  // Deliberately *not* stats. Size and history are not consulted while
  // navigating; they are context you want recorded, which is what `/stats` is for.
  //
  // Collapsed it is one strip, and the strip still carries the counts — so the
  // cost of leaving it closed while you write is 29px and you are still oriented.

  import { moduleOf } from "$lib/fileset";
  import {
    reachOf,
    settingsOf,
    summariseFile,
    surfaceOf,
    testsOf,
    type ReachLine,
  } from "$lib/explore";
  import type { ReadingFile } from "$lib/ipc";

  let {
    file = null,
    files = [],
    open = $bindable(true),
    height = $bindable(270),
    selected = "",
    onselect,
    onjump,
  }: {
    /** The file on screen. Everything here is about this one. */
    file: ReadingFile | null;
    /** The whole reading — needed to know which reached modules are jumps. */
    files: ReadingFile[];
    open: boolean;
    height: number;
    /** Signature currently focused, so the row can show it. */
    selected: string;
    onselect?: (sig: string, line: number) => void;
    /** Follow a call into another file of the reading. */
    onjump?: (path: string, line: number) => void;
  } = $props();

  /**
   * What the drawer shows is decided by `FileKind`.
   *
   * That table used to decide which blocks a doc got *seeded* with; now it decides
   * what you navigate a file by, which is the job it was always describing. A
   * config script has settings, a suite has describes, and neither has functions —
   * which is why neither ever had a surface worth showing.
   */
  const kind = $derived(file?.outline?.kind ?? "plain");
  const module = $derived(moduleOf(file));
  const pub = $derived(surfaceOf(module, "public"));
  const priv = $derived(surfaceOf(module, "private"));
  const reach = $derived(reachOf(module?.deps ?? [], files));
  const settings = $derived(settingsOf(file?.outline?.config?.groups ?? []));
  const imports = $derived(file?.outline?.config?.imports ?? []);
  const describes = $derived(testsOf(file?.outline?.tests ?? null));
  const suite = $derived(file?.outline?.tests ?? null);
  const nums = $derived(summariseFile(file, reach));

  /** The note keeps a floor whatever the drag does; the grip counts against it. */
  const MIN_DOC = 140;
  const GRIP = 5;
  const MIN = 90;

  let host = $state<HTMLDivElement | null>(null);
  let dragging = $state(false);

  function startDrag(e: PointerEvent) {
    dragging = true;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    e.preventDefault();
  }

  function onDrag(e: PointerEvent) {
    if (!dragging || !host) return;
    const pane = host.parentElement;
    if (!pane) return;
    const top = pane.getBoundingClientRect().top;
    height = Math.max(MIN, Math.min(e.clientY - top, pane.clientHeight - MIN_DOC - GRIP));
  }

  function endDrag(e: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    (e.target as HTMLElement).releasePointerCapture(e.pointerId);
  }

  function label(r: ReachLine): string {
    return r.filename ? `in ${r.filename}` : "outside the reading";
  }
</script>

<div
  class="drawer"
  class:open
  class:dragging
  style:height="{open ? height : 29}px"
  bind:this={host}
>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <button
    class="head"
    onclick={() => (open = !open)}
    title={open ? "Collapse (⌥⇥)" : "Expand (⌥⇥)"}
  >
    <span class="caret">▶</span>
    <b>{file?.filename ?? "no file"}</b>
    <span class="kindtag">{kind}</span>
    <span class="nums">{nums}</span>
    <span class="spacer"></span>
    <span class="hint">⌥⇥</span>
  </button>

  {#if open}
    <div class="body">
      {#if kind === "config"}
        <p class="sec">Settings</p>
        {#each settings as g (g.app + (g.target ?? "") + g.line)}
          <div class="grp">
            <span>{g.app}</span>
            {#if g.target}<span class="tgt">{g.target}</span>{/if}
            <span class="ln">{g.line}</span>
          </div>
          {#each g.rows as r (r.key + r.line)}
            <button class="srow inset" onclick={() => onselect?.(r.key, r.line)}>
              <span class="sig">{r.key}</span>
              <!-- env versus literal is the whole finding: a value you can change
                   at deploy time, versus one baked into the release. A `secret`
                   shows no value at all — that it is hardcoded is the finding, and
                   notes get pasted into PR comments. -->
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
          <div class="reaches">
            {#each imports as i (i)}
              <div class="rline">
                <span class="from">import_config</span><span class="to">{i}</span>
              </div>
            {/each}
          </div>
        {/if}
      {:else if kind === "test"}
        <p class="sec">Suite</p>
        <div class="reaches">
          <div class="rline">
            <span class="to">{suite?.caseTemplate ?? "ExUnit.Case"}</span>
            <span class="from">async {suite?.isAsync ?? false}</span>
            <span class="where">
              {suite?.setups.length ?? 0} module setup{(suite?.setups.length ?? 0) === 1 ? "" : "s"}
            </span>
          </div>
        </div>
        <p class="sec">Describes</p>
        {#each describes as d (d.name + d.line)}
          <div class="grp">
            <span>describe "{d.name}"</span>
            <!-- The context its tests can destructure, accumulated from module
                 setup down. A named callback lives elsewhere in the file, so its
                 keys are unknown — and unknown is not the same as "nothing". -->
            {#if d.provides.length}
              <span class="setup">{d.provides.map((k) => ":" + k).join(" ")}</span>
            {/if}
            <!-- `+?` is "and something I cannot see" — a named callback's keys live
                 elsewhere in the file. Saying `+?` beside the keys that *are*
                 known beats guessing, and beats hiding the known ones. -->
            {#if d.unknown}<span class="setup" title="a named setup contributes keys that cannot be read from here">+?</span>{/if}
            {#each d.named as n (n)}<span class="setup dim">runs :{n}</span>{/each}
            <span class="ln">{d.line}</span>
          </div>
          {#each d.tests as t (t.name + t.line)}
            <button class="srow inset" onclick={() => onselect?.(t.name, t.line)}>
              <span class="sig" class:skipped={t.skipped}>{t.name}</span>
              {#each t.tags as g (g)}<span class="badge">@{g}</span>{/each}
              <span class="asserts">{t.asserts}</span>
              <span class="strip" aria-hidden="true">
                {#each Array(Math.min(t.asserts, 3)) as _, i}
                  <i class="a{i + 1}"></i>
                {/each}
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
          {#each [["public", pub], ["private", priv]] as const as [kind, rows]}
            <div class="col {kind}">
              <div class="clabel">
                <span class="bar"></span>{kind}<span class="n">{rows.length}</span>
              </div>
              <!-- Each column scrolls on its own, so the drawer's height stops
                   depending on the size of the file: a 65-function module costs
                   what a 3-function one does. -->
              <div class="list">
                {#each rows as r (r.sig)}
                  <button
                    class="srow"
                    class:sel={r.sig === selected}
                    onclick={() => onselect?.(r.sig, r.line)}
                  >
                    <span class="sig">{r.sig}</span>
                    {#each r.flags as f}<span class="badge">{f}</span>{/each}
                    <span class="ln">{r.line}</span>
                  </button>
                {:else}
                  <p class="none">nothing {kind} here</p>
                {/each}
              </div>
            </div>
          {/each}
        </div>

        <p class="sec">Reaches</p>
        {#if !reach.length}
          <p class="quiet">
            {module.name.split(".").pop()} reaches nothing outside itself. That
            silence is the finding, not an empty state.
          </p>
        {:else}
          <div class="reaches">
            {#each reach as r (r.to + r.from.join())}
              <!-- A call landing in another file of the reading is a jump: one
                   click follows it across the boundary. The rest say so, which
                   marks the edge of what you are reviewing. -->
              <button
                class="rline"
                class:jump={!!r.path}
                disabled={!r.path}
                onclick={() => r.path && r.line && onjump?.(r.path, r.line)}
              >
                <span class="to">{r.to}</span>
                <span class="arrow">←</span>
                <span class="from">{r.from.join(", ")}</span>
                <span class="where" class:hop={!!r.path}>{label(r)}</span>
              </button>
            {/each}
          </div>
        {/if}
      {/if}
    </div>
  {/if}
</div>

{#if open}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="grip"
    role="separator"
    aria-orientation="horizontal"
    onpointerdown={startDrag}
    onpointermove={onDrag}
    onpointerup={endDrag}
    onpointercancel={endDrag}
    ondblclick={() => (height = 270)}
    title="Drag to resize · double-click to reset"
  ></div>
{/if}

<style>
  .drawer {
    flex: none;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--bg);
    transition: height 0.18s var(--ease);
  }
  .drawer.dragging {
    transition: none;
  }

  .head {
    flex: none;
    height: 29px;
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 0 10px;
    font: inherit;
    text-align: left;
    cursor: pointer;
    background: var(--bg-raised);
    border: 0;
    border-bottom: 1px solid var(--line-soft);
    color: var(--fg);
  }
  .head:hover {
    background: var(--bg-inset);
  }
  .caret {
    font-size: 8px;
    color: var(--fg-faint);
    width: 8px;
    transition: transform 0.18s var(--ease);
  }
  .drawer.open .caret {
    transform: rotate(90deg);
  }
  .head b {
    font-family: var(--mono);
    font-size: 11.5px;
    font-weight: 600;
  }
  .nums {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg-faint);
  }
  .kindtag {
    font-size: 8.5px;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    padding: 0 5px;
    border-radius: 3px;
    color: var(--fg-faint);
    background: var(--bg-inset);
  }
  .spacer {
    flex: 1;
  }
  .hint {
    font-family: var(--mono);
    font-size: 9px;
    color: var(--fg-faint);
    border: 1px solid var(--line);
    border-radius: 3px;
    padding: 0 3px;
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 10px 11px 14px;
  }
  .sec {
    margin: 0 0 5px;
    font-size: 9.5px;
    text-transform: uppercase;
    letter-spacing: 0.09em;
    color: var(--fg-faint);
  }
  .sec:not(:first-of-type) {
    margin-top: 13px;
  }
  .quiet {
    margin: 0;
    padding: 9px 11px;
    font-size: 11px;
    font-style: italic;
    color: var(--fg-faint);
    border: 1px dashed var(--line);
    border-radius: 7px;
  }

  .surface {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0 18px;
  }
  .clabel {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 3px;
    font-size: 9.5px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--fg-faint);
  }
  .clabel .bar {
    width: 13px;
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
    font-size: 9.5px;
    background: var(--bg-inset);
    border-radius: 4px;
    padding: 0 5px;
  }
  .list {
    max-height: 147px;
    overflow: auto;
  }

  .srow,
  .rline {
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
  .srow:hover {
    background: var(--bg-inset);
    color: var(--fg);
  }
  .srow.sel {
    background: var(--sel);
    color: var(--accent);
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

  /* ---- config and test: a group heading, then its rows indented under it ---- */
  .grp {
    display: flex;
    align-items: center;
    gap: 7px;
    margin: 9px 0 2px;
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg);
  }
  .grp:first-of-type {
    margin-top: 0;
  }
  .grp .tgt {
    color: var(--fg-faint);
  }
  .grp .setup {
    font-size: 9px;
    color: var(--mark);
  }
  .grp .setup.dim {
    color: var(--fg-faint);
  }
  .grp .ln {
    margin-left: auto;
    font-size: 9.5px;
    color: var(--fg-faint);
  }
  .srow.inset {
    padding-left: 16px;
  }
  .val {
    font-size: 10px;
  }
  .val.env {
    color: var(--pub);
  }
  /* `env!` crashes on boot when unset, which is worth telling apart from `env`. */
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
  /* One square per assertion, up to three: shading says how much a test claims,
     and a one-liner written `test "x", do: assert(y)` still counts because the
     parser counts over the whole call rather than its do-block. */
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

  .reaches {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .rline {
    cursor: default;
  }
  .rline .arrow {
    font-size: 10px;
    color: var(--fg-faint);
  }
  .rline .from {
    color: var(--fg-faint);
  }
  .rline .where {
    margin-left: auto;
    font-family: var(--sans);
    font-size: 8.5px;
    color: var(--fg-faint);
  }
  .rline.jump {
    cursor: pointer;
  }
  .rline.jump .to {
    color: var(--accent);
  }
  .rline.jump:hover {
    background: var(--sel);
  }
  .rline.jump .where.hop {
    padding: 0 4px;
    border-radius: 3px;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  /* The pane divider idiom, turned 90°. */
  .grip {
    flex: none;
    height: 5px;
    cursor: row-resize;
    background: var(--doc-bg);
    border-bottom: 1px solid var(--line);
  }
  .grip:hover {
    background: color-mix(in srgb, var(--accent) 22%, transparent);
  }

  @media (prefers-reduced-motion: reduce) {
    .drawer {
      transition: none;
    }
    .caret {
      transition: none;
    }
  }
</style>
