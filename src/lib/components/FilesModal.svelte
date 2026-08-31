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
  // And the list became a **picture**. `mockup/stack.html` is the contract: one
  // layer per file, thickness ∝ size, the grain inside it the functions it is
  // made of, pale until your prose has named it. A list of ten filenames tells
  // you what you have open and nothing about the change; the same click now
  // answers both — which file, and which part of this is big.
  //
  // Geometry lives in `shape.ts` so the arithmetic can be probed headlessly. The
  // two things that went wrong drawing it were sums, not taste: marks running
  // past the plate that owns them, and a floor that ate the proportion at twelve
  // files.
  //
  // Filtering **dims** rather than removes: proportion is the claim the picture
  // makes, and a filtered stack that re-proportions itself would lie about it.
  // ↑↓ still walk only the matches.

  import type { ReadingFile } from "$lib/ipc";
  import { GEO, layout, plate, tailPath } from "$lib/shape";

  let {
    files = [],
    current = null,
    referenced = new Set<string>(),
    onpick,
    onremove,
    onadd,
    onclose,
    offBranch = null,
  }: {
    files: ReadingFile[];
    current: string | null;
    /** Paths the prose references — the state word on each row. */
    referenced: Set<string>;
    onpick: (path: string) => void;
    onremove: (path: string) => void;
    /** Find a file in the project and add it — ⌘T. */
    onadd: () => void;
    /**
     * Set when you are standing on a different branch from the one this gloom was
     * read on, in which case nothing may join it — the row says so instead of
     * offering a gesture that will be refused.
     */
    offBranch: { gloom: string; here: string } | null;
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

  // The state used to be said in words on each row. The shape says three of the
  // four now — filled is written about, dashed is not, amber is stale or gone —
  // and the fourth (the path) moved to the footer, where a long one fits.

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

  /**
   * The drawing. One layer per file, in the order the gloom collected them.
   *
   * Wider than the list ever needed to be: a name column sized to the longest
   * module plus room for the grain to be read means a modal of 620px clipped
   * `ImpactPipelineTelemetryReporter`. 820 is still comfortably inside the
   * smallest window this app is usable in.
   */
  const VIEW_W = 780;
  const shape = $derived(layout(files, VIEW_W));
  const box = $derived(plate(VIEW_W, shape.label));

  /** Which layers the filter keeps — dimmed, never removed. */
  const matched = $derived(new Set(hits.map((h) => h.f.path)));

  /**
   * The file under the pointer, which is not the same thing as the cursor.
   *
   * `cursor` indexes the *filtered* hits, for the keyboard; a layer knows its
   * index into `files`. Using one for the other showed the wrong path — or none —
   * the moment a filter was typed.
   */
  let hovered = $state<string | null>(null);
  const footPath = $derived(hovered ?? hits[cursor]?.f.path ?? null);

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
       rule about scaling from a trigger is for popovers anchored to one. -->
  <div class="panel" role="dialog" aria-label="Files in this gloom">
    <div class="top">
      <h2>Files in this gloom</h2>
      <span class="c">{files.length} files · {shape.layers.reduce((n, l) => n + l.lines, 0)} lines</span>
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
      <svg viewBox="0 0 {VIEW_W} {shape.height}" role="img" aria-label="The files in this gloom, as layers">
        {#each shape.layers as L, n (L.path)}
          {@const st = stateOf(files[n])}
          {@const on = L.path === current}
          {@const lit = referenced.has(L.path)}
          {@const half = L.y + L.h / 2}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <g
            class="layer {st}"
            class:bare={L.bare}
            class:on
            class:lit
            class:cur={n === cursor}
            class:muted={!matched.has(L.path)}
            style="--i:{n}"
            onclick={() => onpick(L.path)}
            onmouseenter={() => {
              hovered = L.path;
              const at = hits.findIndex((h) => h.f.path === L.path);
              if (at >= 0) cursor = at;
            }}
            onmouseleave={() => (hovered = null)}
          >
            <rect
              class="plate"
              x={box.x}
              y={L.y}
              width={box.width}
              height={L.h}
              rx="4"
            />
            <g class="grain">
              {#if !L.bare}
                <line class="water" x1={box.x + GEO.pad} y1={half} x2={box.x + box.width - GEO.pad} y2={half} />
              {/if}
              {#each L.marks as m (m.k)}
                <rect
                  class="mark {m.vis}"
                  style="--k:{m.k}"
                  x={m.x}
                  y={m.y}
                  width={m.w}
                  height={m.h}
                  rx="1.5"
                />
              {/each}
            </g>
            <g class="label">
              {#if on}<circle class="here" cx={shape.label - 6} cy={half - 0.5} r="2.6" />{/if}
              <text class="name" x={shape.label - 14} y={half + 4} text-anchor="end">{L.name}</text>
              {#if L.kind !== "module"}
                <text class="tag" x={shape.label - 14} y={half + 15} text-anchor="end">{L.kind}</text>
              {/if}
              {#if !files[n].origin}
                <!-- Only where you are pointing, so a destructive control is never
                     sitting under an idle cursor. The origin has none: it is what
                     the reading is anchored to. -->
                <text
                  class="x"
                  role="button"
                  tabindex="-1"
                  x={box.x + box.width + 2}
                  y={half + 4}
                  onclick={(e) => {
                    e.stopPropagation();
                    confirming = confirming === L.path ? null : L.path;
                  }}>×</text
                >
              {/if}
            </g>
          </g>
        {/each}
      </svg>
    </div>

    <!-- The footer says one thing at a time: what you are pointing at, why you
         cannot add, or what removing would cost. -->
    {#if confirming}
      {@const f = files.find((x) => x.path === confirming)}
      <div class="foot warn">
        <span class="msg">
          <b>Remove {baseOf(confirming)}?</b>
          Its snapshot leaves this reading. Your note is untouched, and so is the file on disk.
        </span>
        <button
          class="go"
          onclick={() => {
            const p = confirming!;
            confirming = null;
            onremove(p);
          }}>Remove</button
        >
        <button class="keep" onclick={() => (confirming = null)}>Keep</button>
      </div>
    {:else if offBranch}
      <!-- One line, and no branch names.
           They were badges, and a real branch name — `feature/impact-retry-fix` —
           made two of them wrap the footer into three lines of chrome. The names
           are already in the header chip and in the notice; here the only thing
           worth saying is that this gloom cannot grow from where you are. -->
      <div class="foot off">
        <span class="msg">Files can only be added from the branch this gloom was read on.</span>
      </div>
    {:else}
      <div class="foot">
        <!-- The path, which is the one thing a module name cannot tell you.
             Truncated from the left: the end identifies. -->
        <!-- The full path, trimmed from the left in JS rather than by CSS: a
             right-to-left ellipsis needs `direction: rtl`, which reorders the
             segments of anything with punctuation in it. -->
        <span class="path">
          {#if footPath}
            {@const shown = tailPath(footPath)}
            {@const cut = shown.lastIndexOf("/")}
            <span class="dir">{shown.slice(0, cut + 1)}</span><b>{shown.slice(cut + 1)}</b>
          {:else}
            no file matches “{query}”
          {/if}
        </span>
        <button class="add" onclick={onadd}>+ Add a file… <kbd>⌘T</kbd></button>
      </div>
    {/if}
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
    /* Wide enough for the longest module name plus a readable grain. */
    transform: scale(0.97);
    transition: transform 0.2s var(--ease-out);
    width: min(860px, 94vw);
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
  .x:hover {
    opacity: 1;
    color: var(--priv);
    background: color-mix(in srgb, var(--priv) 14%, transparent);
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

  /* ---- the stack ----------------------------------------------------------
     One layer per file. Everything the list said in words is said in the shape
     now, except the path — which is in the footer, where it can be long. */
  .flist svg {
    display: block;
    width: 100%;
    height: auto;
    overflow: visible;
  }
  .layer {
    cursor: pointer;
    transition: opacity 0.16s ease;
  }
  .layer .plate {
    fill: color-mix(in srgb, var(--gloom-deep) 5%, transparent);
    stroke: color-mix(in srgb, var(--gloom-deep) 26%, transparent);
    stroke-width: 1;
    stroke-dasharray: 4 4;
    transform-box: fill-box;
    transform-origin: top center;
  }
  /* Written about: filled, and the outline goes solid. Until then the layer is a
     dashed outline — the hollow dot, grown to the size of the file. */
  .layer.lit .plate {
    fill: color-mix(in srgb, var(--gloom-deep) 15%, transparent);
    stroke: color-mix(in srgb, var(--gloom-deep) 60%, transparent);
    stroke-dasharray: none;
  }
  /* A config and a suite are their own colours, as everywhere else in the app. */
  .layer.config .plate,
  .layer.stale .plate,
  .layer.missing .plate {
    fill: color-mix(in srgb, var(--priv) 8%, transparent);
    stroke: color-mix(in srgb, var(--priv) 45%, transparent);
  }
  .layer.on .plate {
    stroke-width: 1.8;
    stroke: var(--accent);
    stroke-dasharray: none;
  }
  .layer.cur .plate {
    stroke: color-mix(in srgb, var(--accent) 70%, transparent);
    stroke-dasharray: none;
  }
  /* Filtered out, not removed: the proportions are the claim this picture makes,
     and a stack that re-proportions itself as you type would lie about them. */
  .layer.muted {
    opacity: 0.18;
  }

  .water {
    stroke: color-mix(in srgb, var(--gloom-deep) 22%, transparent);
    stroke-width: 0.7;
  }
  .mark {
    opacity: 0.22;
  }
  .layer.lit .mark {
    opacity: 0.5;
  }
  .mark.public {
    fill: var(--pub);
  }
  .mark.private {
    fill: var(--priv);
  }
  .mark.describe {
    fill: var(--read);
  }
  .name {
    font-family: var(--mono);
    font-size: 11px;
    letter-spacing: 0.02em;
    fill: var(--fg);
  }
  .layer.on .name {
    font-weight: 700;
  }
  .layer.missing .name {
    fill: var(--priv);
  }
  .tag {
    font-family: var(--sans);
    font-size: 7.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    fill: var(--fg-faint);
  }
  .here {
    fill: var(--accent);
  }
  .x {
    font-family: var(--sans);
    font-size: 12px;
    fill: var(--fg-faint);
    opacity: 0;
    cursor: pointer;
    transition: opacity 0.12s ease;
  }
  .layer:hover .x {
    opacity: 0.55;
  }
  .x:hover {
    opacity: 1;
    fill: var(--priv);
  }

  /* ---- footer: one thing at a time --------------------------------------- */
  .foot {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 14px;
    border-top: 1px solid var(--line);
    background: var(--bg-raised);
    font-size: 11.5px;
  }
  .foot .path {
    flex: 1;
    min-width: 0;
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg-dim);
    white-space: nowrap;
    overflow: hidden;
  }
  .foot .path b {
    color: var(--fg);
    font-weight: 500;
  }
  .foot .path .dir {
    color: var(--fg-faint);
  }
  .foot .add {
    font: inherit;
    font-size: 11px;
    color: var(--gloom-deep);
    background: transparent;
    border: 1px solid color-mix(in srgb, var(--gloom-deep) 34%, transparent);
    border-radius: 6px;
    padding: 4px 9px;
    cursor: pointer;
    white-space: nowrap;
  }
  .foot .add:hover {
    background: color-mix(in srgb, var(--gloom-deep) 10%, transparent);
  }
  .foot kbd {
    font-family: var(--mono);
    font-size: 9.5px;
    color: var(--fg-faint);
  }
  .foot .msg {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    line-height: 1.45;
    color: var(--fg-dim);
  }
  .foot .msg b {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg);
    font-weight: 600;
  }
  /* Quiet, not amber: nothing has gone wrong, the gloom simply cannot grow from
     where you are standing. One line, so it stays a footer. */
  .foot.off {
    background: color-mix(in srgb, var(--fg) 5%, var(--bg-raised));
  }
  .foot.off .msg {
    display: block;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 11px;
    color: var(--fg-faint);
  }
  .foot.warn {
    background: color-mix(in srgb, var(--priv) 10%, transparent);
    border-top-color: color-mix(in srgb, var(--priv) 30%, transparent);
  }
  .foot.warn .go {
    font: inherit;
    font-size: 11px;
    color: #fff;
    background: var(--priv);
    border: 0;
    border-radius: 5px;
    padding: 4px 10px;
    cursor: pointer;
  }
  .foot.warn .keep {
    font: inherit;
    font-size: 11px;
    color: var(--fg-dim);
    background: transparent;
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 4px 10px;
    cursor: pointer;
  }

  /* ---- motion ------------------------------------------------------------
     The stack assembles once, 55ms a layer; a layer's grain settles after its own
     plate lands; and while you point at a module a highlight walks its marks in
     source order. That last one loops, and the pointer is its switch — leaving
     stops it, which is the whole difference from ambient motion. */
  .layer .plate {
    animation: grow 0.34s var(--ease-out) both;
    animation-delay: calc(var(--i) * 55ms);
  }
  @keyframes grow {
    from {
      transform: scaleY(0.05);
      opacity: 0;
    }
    40% {
      opacity: 1;
    }
  }
  .layer .grain,
  .layer .label {
    animation: settle 0.3s var(--ease-out) both;
    animation-delay: calc(var(--i) * 55ms + 150ms);
  }
  @keyframes settle {
    from {
      opacity: 0;
      transform: translateY(-3px);
    }
  }
  .flist svg:hover .layer:not(:hover) {
    opacity: 0.34;
  }
  .layer .plate,
  .layer .grain,
  .layer .label {
    transition: transform 0.16s var(--ease-out);
  }
  .layer:hover .plate,
  .layer:hover .grain,
  .layer:hover .label {
    transform: translateY(-2px);
  }
  .layer:hover .mark {
    animation: wave 1.7s ease-in-out infinite;
    animation-delay: calc(var(--k) * 55ms);
  }
  @keyframes wave {
    0%,
    55%,
    100% {
      opacity: inherit;
    }
    22% {
      opacity: 0.85;
    }
  }
  .layer.bare:hover .plate {
    animation: sheen 1.7s ease-in-out infinite;
  }
  @keyframes sheen {
    0%,
    100% {
      filter: none;
    }
    40% {
      filter: brightness(1.14);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .layer .plate {
      animation: appear 0.34s ease both;
      animation-delay: calc(var(--i) * 55ms);
    }
    .layer .grain,
    .layer .label {
      animation: appear 0.3s ease both;
      animation-delay: calc(var(--i) * 55ms + 150ms);
    }
    @keyframes appear {
      from {
        opacity: 0;
      }
    }
    .layer:hover .plate,
    .layer:hover .grain,
    .layer:hover .label {
      transform: none;
    }
    .layer:hover .mark {
      animation: none;
    }
    .layer.bare:hover .plate {
      animation: none;
    }
  }
</style>
