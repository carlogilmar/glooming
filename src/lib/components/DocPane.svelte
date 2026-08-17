<script lang="ts">
  import { createMarkdownIt } from "$lib/markdownit";
  import { locate } from "$lib/select";
  import { focus } from "$lib/stores/focus.svelte";
  import type { Outline } from "$lib/ipc";

  let {
    markdown = $bindable(""),
    outline = null,
    filename = "",
    dirty = false,
    stale = false,
    onreconcile,
  }: {
    markdown: string;
    outline: Outline | null;
    filename: string;
    dirty: boolean;
    stale: boolean;
    onreconcile?: () => void;
  } = $props();

  let editing = $state(false);
  let container = $state<HTMLDivElement | null>(null);

  const md = $derived(createMarkdownIt(outline));
  const html = $derived(md.render(markdown));

  function select(target: HTMLElement) {
    // Table rows, treemap tiles and the reach block's own functions are all the
    // same gesture — every one of them carries data-sig.
    const row = target.closest<HTMLElement>(
      ".fnrow[data-line], .tm-tile[data-sig], .lgtm-deps .fn[data-sig]",
    );
    if (!row) return false;
    const sig = row.dataset.sig ?? "";
    const at = locate(sig, outline?.modules?.[0] ?? null);
    if (at) focus.set(sig, at.ranges, at.related, at.spec, at.doc);
    return true;
  }

  // The rendered block is raw HTML, so rows are wired by delegation rather than
  // per-row handlers.
  function onClick(e: MouseEvent) {
    select(e.target as HTMLElement);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key !== "Enter" && e.key !== " ") return;
    if (select(e.target as HTMLElement)) e.preventDefault();
  }

  // ---- the reach block ----------------------------------------------------
  // Hovering previews any function's connections; selecting one *pins* them, so
  // the picture still answers "what does this reach" while you are over in the
  // code reading it. Without pinning, the view you clicked for disappears the
  // moment the pointer moves away.

  function reachHost(): HTMLElement | null {
    return container?.querySelector<HTMLElement>(".lgtm-deps") ?? null;
  }

  function clearReach(host: HTMLElement) {
    for (const el of host.querySelectorAll(".lit, .on")) el.classList.remove("lit", "on");
  }

  function neutralReach(host: HTMLElement) {
    host.querySelector("svg")?.classList.remove("focusing");
    const readout = host.querySelector(".readout");
    if (readout) readout.innerHTML = `<span class="muted">${host.dataset.rest ?? ""}</span>`;
  }

  function paintFunction(host: HTMLElement, sig: string): boolean {
    const fn = host.querySelector<HTMLElement>(`.fn[data-fn="${CSS.escape(sig)}"]`);
    if (!fn) return false;

    host.querySelector("svg")?.classList.add("focusing");
    fn.classList.add("on");

    const edges = [...host.querySelectorAll<HTMLElement>(`.edge[data-from="${CSS.escape(sig)}"]`)];
    const targets = edges.map((e) => e.dataset.to ?? "");
    edges.forEach((e) => e.classList.add("lit"));
    for (const to of targets) {
      host.querySelector(`.rfn[data-to="${CSS.escape(to)}"]`)?.classList.add("lit");
    }

    const readout = host.querySelector(".readout");
    if (readout) {
      readout.innerHTML = targets.length
        ? `<b>${sig}</b> reaches ${targets.length} · ` +
          targets.map((t) => `<span class="muted">${t}</span>`).join(" &nbsp;")
        : `<b>${sig}</b> <span class="muted">reaches nothing — pure within this module</span>`;
    }
    return true;
  }

  function paintRemote(host: HTMLElement, rfn: HTMLElement) {
    const to = rfn.dataset.to ?? "";
    const callers = (rfn.dataset.callers ?? "").split("|").filter(Boolean);

    host.querySelector("svg")?.classList.add("focusing");
    rfn.classList.add("on", "lit");
    for (const c of callers) {
      host.querySelector(`.fn[data-fn="${CSS.escape(c)}"]`)?.classList.add("on");
      host
        .querySelector(`.edge[data-from="${CSS.escape(c)}"][data-to="${CSS.escape(to)}"]`)
        ?.classList.add("lit");
    }

    const readout = host.querySelector(".readout");
    if (readout) {
      readout.innerHTML =
        `<b>${to}</b> <span class="muted">called from</span> ` +
        callers.map((c) => `<b>${c}</b>`).join(" &nbsp;");
    }
  }

  /**
   * Repaint the block. `hovered` wins when the pointer is over something;
   * otherwise it falls back to the pinned selection, and only then to neutral.
   */
  function litReach(host: HTMLElement, hovered: HTMLElement | null) {
    clearReach(host);

    const fn = hovered?.closest<HTMLElement>(".fn");
    if (fn) {
      paintFunction(host, fn.dataset.fn ?? "");
      return;
    }
    const rfn = hovered?.closest<HTMLElement>(".rfn");
    if (rfn) {
      paintRemote(host, rfn);
      return;
    }
    // Nothing hovered: hold the selected function's connections, if it has any
    // here, so clicking through to the code doesn't discard the view.
    if (focus.active && paintFunction(host, focus.sig)) return;
    neutralReach(host);
  }

  // ---- treemap tooltip ----------------------------------------------------
  // Native <title> is slow to appear and unstyleable, and the tiles need a
  // label the moment the pointer lands on them.
  let tip = $state<{ text: string; sub: string; x: number; y: number } | null>(null);

  function onMove(e: MouseEvent) {
    const reach = reachHost();
    if (reach) litReach(reach, e.target as HTMLElement);

    const tile = (e.target as HTMLElement).closest<HTMLElement>(".tm-tile[data-tip]");
    if (!tile) {
      tip = null;
      return;
    }
    const host = container?.getBoundingClientRect();
    tip = {
      text: tile.dataset.tip ?? "",
      sub: `${tile.dataset.lines} lines · ${tile.dataset.pct}%`,
      x: e.clientX - (host?.left ?? 0),
      y: e.clientY - (host?.top ?? 0),
    };
  }

  // Mark the focused row, so both panes show the same selection — and repaint
  // the reach block, so a selection pins its connections. Depends on `html` too:
  // re-rendering the doc wipes these classes, and they have to be put back.
  $effect(() => {
    const sig = focus.sig;
    html;
    if (!container) return;

    for (const el of container.querySelectorAll(".fnrow, .tm-tile, .lgtm-deps .fn")) {
      el.classList.toggle("active", focus.active && (el as HTMLElement).dataset.sig === sig);
    }
    const host = reachHost();
    if (host) litReach(host, null);
  });
</script>

<div class="pane">
  <div class="panehead">
    {#if dirty}<span class="dot" title="unsaved"></span>{/if}
    <span>{filename ? `${filename}.md` : "no doc"}</span>
    <span class="spacer"></span>
    {#if stale}
      <button class="btn icon warn" onclick={() => onreconcile?.()}>⟳ Code changed — reconcile</button>
    {/if}
    <div class="toggle">
      <button class:on={!editing} onclick={() => (editing = false)}>Preview</button>
      <button class:on={editing} onclick={() => (editing = true)}>Edit</button>
    </div>
  </div>

  <div class="panebody">
    {#if editing}
      <textarea class="raw" bind:value={markdown} spellcheck="false"></textarea>
    {:else}
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div
        class="doc"
        bind:this={container}
        onclick={onClick}
        onkeydown={onKey}
        onmousemove={onMove}
        onmouseleave={() => {
          tip = null;
          const reach = reachHost();
          if (reach) litReach(reach, null);
        }}
      >
        {@html html}
        {#if tip}
          <div class="tmtip" style:left="{tip.x}px" style:top="{tip.y}px">
            <b>{tip.text}</b>
            <span>{tip.sub}</span>
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    height: 100%;
  }
  .panehead {
    background: var(--doc-bg);
    border-bottom-color: var(--doc-line);
  }
  .panebody {
    flex: 1;
    overflow: auto;
    background: var(--doc-bg);
    color: var(--doc-fg);
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--priv);
    display: inline-block;
  }
  .warn {
    color: var(--priv);
    border-color: color-mix(in srgb, var(--priv) 40%, transparent);
  }

  .toggle {
    display: flex;
    border: 1px solid var(--line);
    border-radius: 5px;
    overflow: hidden;
  }
  .toggle button {
    font: inherit;
    font-size: 11px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    background: transparent;
    color: var(--fg-faint);
    border: 0;
    padding: 3px 10px;
    cursor: pointer;
  }
  .toggle button.on {
    background: var(--bg-inset);
    color: var(--fg);
  }

  .raw {
    display: block;
    width: 100%;
    height: 100%;
    resize: none;
    border: 0;
    outline: none;
    background: var(--doc-bg);
    color: var(--doc-fg);
    font-family: var(--mono);
    font-size: 12.5px;
    line-height: 1.7;
    padding: 26px 30px;
  }

  .doc {
    /* Enough tail room to clear the window edge, not a screenful of nothing. */
    padding: 26px 30px 40px;
    position: relative;
  }
  /* Prose keeps a readable measure; the visual blocks take the whole pane, so
     widening the window actually buys you a bigger picture. Capping the doc
     itself left dead space to the right of every diagram. */
  :global(.doc > p),
  :global(.doc > h1),
  :global(.doc > h2),
  :global(.doc > blockquote),
  :global(.doc > ul),
  :global(.doc > ol) {
    max-width: 760px;
  }

  /* Treemap tooltip: follows the pointer, names the function, and gets out of
     the way. Offset up-left so it never sits under the cursor. */
  .tmtip {
    position: absolute;
    z-index: 5;
    transform: translate(12px, -34px);
    pointer-events: none;
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 4px 9px;
    border-radius: 6px;
    background: var(--fg);
    color: var(--bg);
    font-size: 11.5px;
    white-space: nowrap;
    box-shadow: 0 6px 20px rgba(10, 12, 16, 0.28);
  }
  .tmtip b {
    font-family: var(--mono);
    font-weight: 600;
  }
  .tmtip span {
    opacity: 0.7;
    font-family: var(--mono);
    font-size: 10.5px;
  }

  /* The rendered doc is injected HTML, so these rules are global. */
  :global(.doc h1) {
    font-size: 20px;
    margin: 0 0 6px;
    letter-spacing: -0.01em;
    color: var(--doc-fg);
  }
  :global(.doc h2) {
    font-size: 13px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--fg-faint);
    margin: 30px 0 10px;
  }
  :global(.doc p) {
    color: var(--fg-dim);
    line-height: 1.7;
    margin: 0 0 14px;
  }
  :global(.doc blockquote) {
    margin: 0 0 20px;
    padding: 0 0 0 14px;
    border-left: 2px solid var(--line);
    color: var(--fg-dim);
    font-style: italic;
  }
  :global(.doc code) {
    font-family: var(--mono);
    font-size: 12px;
    background: var(--bg-inset);
    padding: 1px 5px;
    border-radius: 4px;
    color: var(--fg-dim);
  }

  /* ---- the lgtm:functions block ---- */
  :global(.lgtm-block) {
    border: 1px solid var(--doc-line);
    border-radius: 8px;
    background: var(--code-bg);
    overflow: hidden;
    margin: 0 0 22px;
    box-shadow: var(--shadow);
  }
  :global(.lgtm-block > header) {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 12px;
    border-bottom: 1px solid var(--line-soft);
    background: var(--bg-inset);
    font-size: 10.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--fg-faint);
  }
  :global(.lgtm-block > header .tag) {
    font-family: var(--mono);
    color: var(--mark);
    text-transform: none;
    letter-spacing: 0;
  }
  :global(.lgtm-block > header .count) {
    margin-left: auto;
    font-family: var(--mono);
  }
  :global(.lgtm-block .grp) {
    padding: 4px 0;
  }
  :global(.lgtm-block .grp + .grp) {
    border-top: 1px solid var(--line-soft);
  }
  :global(.lgtm-block .grp > .label) {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 8px 12px 4px;
    font-size: 10.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }
  :global(.lgtm-block .grp.public > .label) {
    color: var(--pub);
  }
  :global(.lgtm-block .grp.private > .label) {
    color: var(--priv);
  }
  :global(.lgtm-block .grp > .label .bar) {
    width: 3px;
    height: 11px;
    border-radius: 2px;
    background: currentColor;
  }

  /* One row per function: signature line, explanation beneath. Long names get
     the full width instead of being squeezed into a first column. */
  :global(.fnrow) {
    position: relative;
    display: block;
    padding: 8px 12px;
    cursor: pointer;
    border-left: 2px solid transparent;
    transition:
      background 0.14s ease,
      border-color 0.14s ease;
  }
  :global(.fnrow .sigline) {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 8px;
  }
  :global(.fnrow .badge) {
    font-size: 9.5px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--fg-faint);
    border: 1px solid var(--line);
    border-radius: 999px;
    padding: 1px 7px;
    white-space: nowrap;
  }
  :global(.fnrow .badge.gone) {
    color: var(--priv);
    border-color: color-mix(in srgb, var(--priv) 35%, transparent);
  }
  :global(.fnrow.static) {
    cursor: default;
  }
  :global(.fnrow:not(.static):hover) {
    background: var(--bg-inset);
    border-left-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }
  :global(.fnrow .sig) {
    font-family: var(--mono);
    font-size: 12.5px;
    color: var(--fg);
    word-break: break-word;
  }
  :global(.fnrow .sig .ar) {
    color: var(--fg-faint);
  }
  :global(.fnrow .why) {
    color: var(--fg-dim);
    font-size: 12.5px;
    line-height: 1.55;
    margin-top: 3px;
    padding-left: 1px;
  }
  :global(.fnrow .why.empty) {
    color: var(--fg-faint);
    font-style: italic;
    opacity: 0.7;
  }
  :global(.fnrow .why.empty::before) {
    content: "explain…";
  }
  :global(.fnrow.removed .sig) {
    color: var(--fg-faint);
  }

  /* Selected: breathes on the same 2.1s cycle as the code pane, glows, and
     grows a caret pointing at the code. */
  :global(.fnrow.active) {
    background: var(--sel);
    border-left-color: var(--accent);
    animation: fnPulse 2.1s ease-in-out infinite;
  }
  :global(.fnrow.active .sig) {
    color: var(--accent);
    font-weight: 600;
  }
  :global(.fnrow.active .why.empty) {
    opacity: 1;
  }
  :global(.fnrow.active::after) {
    content: "◂";
    position: absolute;
    right: 8px;
    top: 9px;
    color: var(--accent);
    font-size: 11px;
    animation: caretNudge 2.1s ease-in-out infinite;
  }
  @keyframes fnPulse {
    0%,
    100% {
      box-shadow:
        inset 3px 0 0 -1px var(--accent),
        0 0 0 0 color-mix(in srgb, var(--accent) 22%, transparent);
    }
    50% {
      box-shadow:
        inset 3px 0 0 0 var(--accent),
        0 0 0 4px color-mix(in srgb, var(--accent) 12%, transparent);
    }
  }
  @keyframes caretNudge {
    0%,
    100% {
      opacity: 0.35;
      transform: translateX(2px);
    }
    50% {
      opacity: 1;
      transform: translateX(-2px);
    }
  }

  /* ---- the lgtm:treemap block ----
     Function sizes as area. No header, no legend, no footer: the chart IS the
     information, and chrome was eating the space it needed. Anything a cell
     can't label is one hover away in its tooltip. */
  :global(.lgtm-treemap) {
    border: 1px solid var(--doc-line);
    border-radius: 8px;
    background: var(--code-bg);
    overflow: hidden;
    margin: 0 0 22px;
    box-shadow: var(--shadow);
  }
  :global(.lgtm-treemap.empty) {
    padding: 18px;
    color: var(--fg-faint);
    font-size: 12.5px;
  }
  /* Uniform scaling (xMidYMid meet) — stretching the viewBox to a fixed height
     would squash the labels. Height follows from the 1000x700 viewBox, which is
     tall enough to read at the doc pane's width. */
  :global(.lgtm-treemap svg) {
    display: block;
    width: 100%;
    height: auto;
  }

  :global(.tm-tile) {
    cursor: pointer;
  }
  :global(.tm-cell) {
    transition:
      opacity 0.14s ease,
      stroke 0.14s ease;
    stroke: transparent;
    stroke-width: 2;
  }
  /* Public and private are different colors — the split you most want to see
     at a glance is what's exposed versus what's internal. */
  :global(.tm-public) {
    fill: var(--pub);
  }
  :global(.tm-private) {
    fill: var(--priv);
  }
  :global(.tm-tile:hover .tm-cell) {
    stroke: var(--fg);
    opacity: 0.9;
  }
  :global(.tm-tile.active .tm-cell) {
    stroke: var(--accent);
    stroke-width: 3;
  }
  :global(.tm-num) {
    fill: #fff;
    font-family: var(--mono);
    font-weight: 700;
    pointer-events: none;
  }
  :global(.tm-name) {
    fill: rgba(255, 255, 255, 0.85);
    font-family: var(--mono);
    pointer-events: none;
  }

  /* The three biggest functions breathe. A pulsing outline was invisible at
     this size — the fill is what the eye actually catches. Staggered so they
     read as three things, not one flashing block. */
  :global(.tm-top) {
    animation: tmBreathe 1.9s ease-in-out infinite;
  }
  :global(.tm-tile:nth-of-type(2) .tm-top) {
    animation-delay: 0.25s;
  }
  :global(.tm-tile:nth-of-type(3) .tm-top) {
    animation-delay: 0.5s;
  }
  @keyframes tmBreathe {
    0%,
    100% {
      filter: brightness(1) saturate(1);
    }
    50% {
      filter: brightness(1.55) saturate(1.35);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    :global(.tm-top) {
      animation: none;
      filter: brightness(1.3);
    }
  }

  /* ---- the lgtm:stats block ----
     The facts you'd otherwise go and look up, in one line of eye movement. */
  :global(.lgtm-stats) {
    border: 1px solid var(--doc-line);
    border-radius: 8px;
    background: var(--bg-raised);
    margin: 0 0 22px;
    box-shadow: var(--shadow);
    overflow: hidden;
  }
  :global(.lgtm-stats .grid) {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(96px, 1fr));
  }
  :global(.lgtm-stats .stat) {
    padding: 10px 12px;
    border-right: 1px solid var(--line-soft);
    min-width: 0;
  }
  :global(.lgtm-stats .stat:last-child) {
    border-right: 0;
  }
  :global(.lgtm-stats .stat b) {
    display: block;
    font-size: 19px;
    font-weight: 600;
    line-height: 1.15;
    color: var(--fg);
    font-variant-numeric: tabular-nums;
  }
  :global(.lgtm-stats .stat .lbl) {
    display: block;
    font-size: 9.5px;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--fg-faint);
    margin-top: 3px;
  }
  :global(.lgtm-stats .stat .sub) {
    display: block;
    font-size: 10.5px;
    color: var(--fg-dim);
    margin-top: 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  :global(.lgtm-stats .who) {
    border-top: 1px solid var(--line-soft);
    padding: 7px 12px;
    font-size: 11px;
    color: var(--fg-dim);
    background: var(--bg-inset);
  }

  /* ---- the lgtm:deps block ----
     The boundary. Ported from mockup/deps.html — if these drift, that file is
     the contract. Colours come from the app's tokens, one per reach. */
  :global(.lgtm-deps) {
    border: 1px solid var(--doc-line);
    border-radius: 8px;
    background: var(--code-bg);
    box-shadow: var(--shadow);
    overflow: hidden;
    margin: 0 0 22px;
  }
  :global(.lgtm-deps.empty) {
    padding: 18px;
    color: var(--fg-faint);
    font-size: 12.5px;
  }
  :global(.lgtm-deps svg) {
    display: block;
    width: 100%;
    height: auto;
  }

  :global(.lgtm-deps .bound) {
    fill: none;
    stroke: var(--line);
    stroke-width: 1.5;
  }
  :global(.lgtm-deps .bound-label) {
    font-family: var(--mono);
    font-size: 11px;
    fill: var(--fg-faint);
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  :global(.lgtm-deps .fn) {
    cursor: pointer;
  }
  :global(.lgtm-deps .fn-name) {
    font-family: var(--mono);
    font-size: 12.5px;
    fill: var(--fg);
  }
  /* Reaching nothing is information too: pure functions stay quiet. */
  :global(.lgtm-deps .fn.pure .fn-name) {
    fill: var(--fg-faint);
  }
  :global(.lgtm-deps .fn-line) {
    font-family: var(--mono);
    font-size: 9.5px;
    fill: var(--fg-faint);
  }
  :global(.lgtm-deps .fn-hit) {
    fill: var(--bg-inset);
    opacity: 0;
  }
  :global(.lgtm-deps .fn:hover .fn-hit),
  :global(.lgtm-deps .fn.on .fn-hit) {
    opacity: 1;
  }
  /* `.on` is hover, `.active` is the selection shared with the code pane —
     they coexist, so they must not look the same. */
  :global(.lgtm-deps .fn.active .fn-hit) {
    opacity: 1;
    fill: var(--sel);
  }
  :global(.lgtm-deps .fn.active .fn-name) {
    fill: var(--accent);
    font-weight: 600;
  }

  /* The puncture — where a call leaves the module. */
  :global(.lgtm-deps .pierce) {
    fill: var(--code-bg);
    stroke: var(--fg-faint);
    stroke-width: 1.5;
  }

  :global(.lgtm-deps .edge) {
    fill: none;
    stroke-linecap: round;
    opacity: 0.5;
    transition: opacity 0.12s ease;
  }
  :global(.lgtm-deps .edge.app) {
    stroke: var(--accent);
  }
  :global(.lgtm-deps .edge.lib) {
    stroke: var(--mark);
  }
  :global(.lgtm-deps .edge.std) {
    stroke: var(--fg-faint);
  }
  :global(.lgtm-deps svg.focusing .edge) {
    opacity: 0.07;
  }
  :global(.lgtm-deps svg.focusing .edge.lit) {
    opacity: 0.95;
  }

  :global(.lgtm-deps .mod-name) {
    font-family: var(--mono);
    font-size: 12px;
    font-weight: 600;
  }
  :global(.lgtm-deps .mod-name.app) {
    fill: var(--accent);
  }
  :global(.lgtm-deps .mod-name.lib) {
    fill: var(--mark);
  }
  :global(.lgtm-deps .mod-name.std) {
    fill: var(--fg-faint);
  }
  :global(.lgtm-deps .mod-kind) {
    font-size: 9px;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    fill: var(--fg-faint);
  }
  :global(.lgtm-deps .rfn) {
    cursor: pointer;
  }
  :global(.lgtm-deps .rfn-name) {
    font-family: var(--mono);
    font-size: 11.5px;
    fill: var(--fg-dim);
  }
  :global(.lgtm-deps .rfn-hit) {
    fill: var(--bg-inset);
    opacity: 0;
  }
  :global(.lgtm-deps .rfn:hover .rfn-hit),
  :global(.lgtm-deps .rfn.on .rfn-hit) {
    opacity: 1;
  }
  :global(.lgtm-deps svg.focusing .rfn-name) {
    opacity: 0.3;
  }
  :global(.lgtm-deps svg.focusing .rfn.lit .rfn-name) {
    opacity: 1;
    fill: var(--fg);
  }
  :global(.lgtm-deps .dot.app) {
    fill: var(--accent);
  }
  :global(.lgtm-deps .dot.lib) {
    fill: var(--mark);
  }
  :global(.lgtm-deps .dot.std) {
    fill: var(--fg-faint);
  }
  :global(.lgtm-deps svg.focusing .dot) {
    opacity: 0.18;
  }
  :global(.lgtm-deps svg.focusing .rfn.lit .dot) {
    opacity: 1;
  }

  :global(.lgtm-deps .readout) {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 12px;
    border-top: 1px solid var(--line-soft);
    background: var(--bg-inset);
    font-size: 11.5px;
    color: var(--fg-dim);
    min-height: 32px;
  }
  :global(.lgtm-deps .readout b) {
    font-family: var(--mono);
    color: var(--fg);
    font-weight: 600;
  }
  :global(.lgtm-deps .readout .muted) {
    color: var(--fg-faint);
  }
</style>
