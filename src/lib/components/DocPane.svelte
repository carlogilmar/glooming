<script lang="ts">
  import { createMarkdownIt } from "$lib/markdownit";
  import { locate } from "$lib/select";
  import RefMenu from "$lib/components/RefMenu.svelte";
  import FontStepper from "$lib/components/FontStepper.svelte";
  import { fontSize } from "$lib/stores/fontSize.svelte";
  import { focus } from "$lib/stores/focus.svelte";
  import { byPath, hasModules, moduleOf, origin as originOf, vocabulary } from "$lib/fileset";
  import type { ReadingFile } from "$lib/ipc";

  let {
    markdown = $bindable(""),
    files = [],
    current = null,
    filename = "",
    dirty = false,
    stale = false,
    onreconcile,
    onshowfile,
    onrefs,
    opened = 0,
  }: {
    markdown: string;
    /** Every file the reading covers — references may point into any of them. */
    files: ReadingFile[];
    /** The file on screen in the code pane. */
    current: string | null;
    filename: string;
    dirty: boolean;
    stale: boolean;
    onreconcile?: () => void;
    /**
     * Bring a file to the front. Awaited, because a selection is only meaningful
     * once the code pane is actually showing the file it refers to.
     */
    onshowfile?: (path: string) => Promise<void> | void;
    /** Which files the prose references — the file strip's hollow-dot signal. */
    onrefs?: (paths: Set<string>) => void;
    /**
     * Bumped whenever a *different* reading is opened.
     *
     * The arrival animations have to be gated on something, because `html` is
     * derived from `markdown` and re-renders on every keystroke — an unguarded
     * stagger would re-cascade the surface block while you type, which is the
     * exact behaviour that makes people disable animation. This token changes
     * once per doc, so `.arriving` is on for one beat and then gone.
     */
    opened?: number;
  } = $props();

  /**
   * Play the arrival animations once per reading.
   *
   * `.arriving` is what every "this just appeared" rule hangs off, so the class
   * is added on a new `opened` token and removed a beat later. Re-renders in
   * between — every keystroke — carry no class and therefore no animation.
   */
  $effect(() => {
    opened;
    const el = container;
    if (!el) return;
    el.classList.add("arriving");
    const off = setTimeout(() => el.classList.remove("arriving"), 900);
    // The invitation is pointed at *after* the page has settled. Both at once
    // reads as one busy event; sequenced, the second one is clearly pointing.
    const ping = setTimeout(pulseInvitation, 260);
    return () => {
      clearTimeout(off);
      clearTimeout(ping);
      el.classList.remove("arriving");
    };
  });

  /**
   * One ring around the Explain invitation, on a doc nobody has written in.
   *
   * Identified structurally rather than by a marker: the last paragraph of the
   * doc, containing nothing but emphasis. That is what `seed.rs` writes — and
   * the moment you type a word it stops matching, which is exactly when the
   * nudge should stop. Nothing has to remember whether the doc is "fresh".
   */
  function pulseInvitation() {
    if (!container) return;
    const paras = container.querySelectorAll<HTMLElement>(":scope > p");
    const last = paras[paras.length - 1];
    const em = last?.firstElementChild;
    if (!last || last.children.length !== 1 || em?.tagName !== "EM") return;
    if (em.textContent?.trim() !== last.textContent?.trim()) return;

    last.classList.remove("invite-pulse");
    void last.offsetWidth;
    last.classList.add("invite-pulse");
    setTimeout(() => last.classList.remove("invite-pulse"), 1000);
  }

  const originFile = $derived(originOf(files));
  const currentFile = $derived(byPath(files, current) ?? originFile);

  /** Switch the code pane if a reference points somewhere else. */
  async function switchIfNeeded(path: string | null | undefined) {
    if (path && path !== current) await onshowfile?.(path);
  }

  /** The file a clicked element belongs to — every block carries `data-path`. */
  function ownerOf(el: HTMLElement): string | null {
    return el.closest<HTMLElement>("[data-path]")?.dataset.path ?? current;
  }

  // Scales the prose only — the blocks are data displays, not text, and keep
  // their own sizes so a table doesn't reflow when you nudge the reading size.
  const font = fontSize("docFontSize", 14);
  $effect(() => font.load());

  let editing = $state(false);
  let container = $state<HTMLDivElement | null>(null);
  let body = $state<HTMLDivElement | null>(null);
  let pane = $state<HTMLDivElement | null>(null);
  let bandTop = $state(0);
  let bandEl = $state<HTMLDivElement | null>(null);

  // ---- scroll-driven reading ----------------------------------------------
  // The doc is the text and the code pane is the sticky graphic, which is the
  // exact geometry scrollytelling wants — so this is one wire, not a rewrite.
  // Only for modules: a config or a test suite is a directory, not a narrative.
  let reading = $state(false);
  /** Paragraphs carrying a reference, in prose order — the steps. */
  let steps = $state<HTMLElement[]>([]);
  let activeStep = $state(-1);
  /**
   * Index of the chip that is current, not its signature.
   *
   * A name is not an identity: the same function can be referenced from three
   * paragraphs. Keying on the signature meant every duplicate lit up at once,
   * and clicking the third one scrolled back to the first.
   */
  let activeRef = $state<number | null>(null);

  // Any module anywhere in the reading is something to walk. A config script or
  // a test suite on its own is a directory, not a narrative.
  const canRead = $derived(hasModules(files));

  // ---- `/` to insert a reference --------------------------------------------
  // The function list costs nothing: the outline is already parsed. What this
  // buys is not having to remember an exact `name/arity` while writing prose.
  let editor = $state<HTMLTextAreaElement | null>(null);
  let mirror = $state<HTMLDivElement | null>(null);
  let menu = $state<RefMenu | null>(null);
  /** Caret offset of the `/` that opened the menu, or null when closed. */
  let slashAt = $state<number | null>(null);
  let slashQuery = $state("");
  let menuX = $state(0);
  let menuY = $state(0);
  /**
   * Which edge `menuY` measures from.
   *
   * Near the bottom of a long note there is no room below the caret, and the
   * menu was rendering off the end of the pane where `overflow: auto` clips it —
   * present every time, visible never. Anchoring by the *bottom* edge flips it
   * above the caret without anyone having to measure how tall it currently is.
   */
  let menuFlip = $state(false);

  /** RefMenu's own `max-height`. Flip when the menu cannot fit below the caret. */
  const MENU_MAX = 264;

  /**
   * Where the caret is, in pixels within the textarea.
   *
   * There is no API for this, so a hidden div mirrors the textarea's own
   * metrics, holds the text up to the caret, and a marker span at the end
   * reports its position. Fiddly, but the alternative is a menu that appears
   * somewhere unrelated to what you are typing.
   */
  function caretXY(): { x: number; y: number; flip: boolean } {
    if (!editor || !mirror) return { x: 0, y: 0, flip: false };
    const before = editor.value.slice(0, editor.selectionStart);
    mirror.textContent = before;
    const marker = document.createElement("span");
    marker.textContent = "\u200b";
    mirror.appendChild(marker);
    const m = marker.getBoundingClientRect();
    const box = editor.getBoundingClientRect();
    mirror.textContent = "";

    const top = m.top - box.top - editor.scrollTop;
    // Measured rather than assumed — a constant here would drift the moment the
    // editor's font size changed.
    const lineH = m.height || 22;
    const H = editor.clientHeight;
    const below = H - (top + lineH);
    // Only flip when there is genuinely more room the other way, so a short pane
    // doesn't bounce the menu upward into even less space.
    const flip = below < MENU_MAX && top > below;

    return {
      // Clamped, or a caret near the right edge puts a 268px menu off the side.
      x: Math.max(0, Math.min(m.left - box.left - editor.scrollLeft, editor.clientWidth - 276)),
      // Distance from the pane's bottom to the caret's top when flipped, so the
      // menu grows upward from the line you are typing on.
      y: flip ? H - top + 4 : top + lineH,
      flip,
    };
  }

  function closeMenu() {
    slashAt = null;
    slashQuery = "";
  }

  /** Re-read the query from the text, and close if the `/` context is gone. */
  function syncMenu() {
    if (slashAt === null || !editor) return;
    const caret = editor.selectionStart;
    if (caret <= slashAt) return closeMenu();
    const typed = editor.value.slice(slashAt + 1, caret);
    // A space or a newline ends it — `/` in ordinary prose must stay prose.
    if (/[\s`]/.test(typed)) return closeMenu();
    slashQuery = typed;
  }

  function onEditorInput() {
    syncMenu();
  }

  function onEditorKey(e: KeyboardEvent) {
    if (slashAt !== null && menu?.handleKey(e)) {
      e.preventDefault();
      return;
    }
    // Only at a word boundary, so a path or a date in prose doesn't open it.
    if (e.key === "/" && editor) {
      const before = editor.value.slice(0, editor.selectionStart);
      if (before === "" || /[\s(\[]$/.test(before)) {
        const at = editor.selectionStart;
        queueMicrotask(() => {
          const p = caretXY();
          menuX = p.x;
          menuY = p.y;
          menuFlip = p.flip;
          slashAt = at;
          slashQuery = "";
        });
      }
    }
  }

  /** Replace `/query` with the reference and put the caret after it. */
  function insertRef(text: string) {
    if (!editor || slashAt === null) return;
    const caret = editor.selectionStart;
    const next = editor.value.slice(0, slashAt) + text + editor.value.slice(caret);
    markdown = next;
    closeMenu();
    const to = slashAt + text.length;
    queueMicrotask(() => {
      editor?.focus();
      editor?.setSelectionRange(to, to);
    });
  }

  // The whole file set goes in: a reference may name a function in any of them,
  // and each block finds its own file from the `module=` it already carried.
  const md = $derived(createMarkdownIt(files, current));
  const html = $derived(md.render(markdown));

  /**
   * Settings and test rows aren't functions, but they are still *blocks* — a
   * test, a describe, a setup, a multi-line setting. Clicking one selects its
   * whole span in the code, the same as clicking a function does, rather than
   * dropping a cursor on the line it happens to start at.
   */
  async function selectBlock(target: HTMLElement): Promise<boolean> {
    // Inline references are here too: their span was already computed when the
    // reference was resolved (clauses, plus the @spec and @doc above), and an
    // `L30-34` reference has no function name to look up at all.
    const row = target.closest<HTMLElement>(
      ".lgtm-settings [data-line], .lgtm-tests [data-line], .doc code.ref[data-line]",
    );
    if (!row) return false;

    const start = parseInt(row.dataset.line ?? "0", 10);
    if (start <= 0) return true;
    const end = parseInt(row.dataset.end ?? String(start), 10) || start;
    // A reference with no arity selects every arity, so a chip may carry several
    // spans. Anything without `data-ranges` — a settings or test row — is one.
    const spans = rangesOf(row) ?? [{ start, end }];

    // A reference can point into any file of the reading, so the pane has to be
    // showing the right one before the span means anything.
    const path = ownerOf(row);
    const crossed = !!path && path !== current;
    await switchIfNeeded(path);

    // The chip you clicked, not the first one that shares its name.
    activeRef = row.dataset.ref !== undefined ? Number(row.dataset.ref) : null;
    focus.path = path;
    const sig = row.dataset.sig ?? `line ${start}`;
    // Crossing a file is a move, never a toggle: arriving somewhere new and
    // being handed an empty selection would read as the click failing.
    if (crossed) focus.select(sig, spans);
    else focus.set(sig, spans);
    return true;
  }

  /** `12-17,24-26` → two spans. Null when the element carries none. */
  function rangesOf(el: HTMLElement): { start: number; end: number }[] | null {
    const raw = el.dataset.ranges;
    if (!raw) return null;
    const spans = raw
      .split(",")
      .map((part) => part.split("-").map((n) => parseInt(n, 10)))
      .filter(([a, b]) => Number.isFinite(a) && Number.isFinite(b))
      .map(([a, b]) => ({ start: a, end: b }));
    return spans.length ? spans : null;
  }

  async function select(target: HTMLElement) {
    // Table rows, treemap tiles and the reach block's own functions are all the
    // same gesture — every one of them carries data-sig.
    const row = target.closest<HTMLElement>(
      ".fnrow[data-line], .tm-tile[data-sig], .lgtm-deps .fn[data-sig], " +
        ".lgtm-surface .row[data-sig]",
    );
    if (!row) return false;
    const sig = row.dataset.sig ?? "";
    // A block belongs to the file whose module it names, which may not be the
    // file on screen — the reach diagram of one module is still clickable while
    // you are standing in another.
    const path = ownerOf(row);
    const at = locate(sig, moduleOf(byPath(files, path) ?? currentFile));
    if (at) {
      const crossed = !!path && path !== current;
      await switchIfNeeded(path);
      // Clicked a table row or a tile, so there is no particular chip in mind —
      // the first mention of that name is the reasonable one to light.
      const chip = container?.querySelector<HTMLElement>(
        `.doc code.ref[data-sig="${CSS.escape(sig)}"]`,
      );
      activeRef = chip?.dataset.ref !== undefined ? Number(chip.dataset.ref) : null;
      focus.path = path;
      if (crossed) focus.select(sig, at.ranges, at.related, at.spec, at.doc);
      else focus.set(sig, at.ranges, at.related, at.spec, at.doc);
    }
    return true;
  }

  // The rendered block is raw HTML, so rows are wired by delegation rather than
  // per-row handlers.
  async function onClick(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if ((await selectBlock(target)) || (await select(target))) alignReading(target);
  }

  async function onKey(e: KeyboardEvent) {
    if (e.key !== "Enter" && e.key !== " ") return;
    const target = e.target as HTMLElement;
    // Prevented up front: the switch is awaited, and by the time it resolves the
    // default scroll a space bar causes has already happened.
    e.preventDefault();
    if ((await selectBlock(target)) || (await select(target))) alignReading(target);
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

  /**
   * Land the arrival ring where the trace was heading.
   *
   * Timed rather than instant, so the hit reads as the *consequence* of the dash
   * getting there — firing both together says "two things lit up", which is the
   * static picture again with extra noise.
   *
   * Restarting a CSS animation needs the class off, a forced reflow, then the
   * class back on: without the reflow the browser coalesces both changes in one
   * frame and nothing runs at all.
   */
  let arriveTimer: ReturnType<typeof setTimeout> | null = null;
  function arriveAt(host: HTMLElement, targets: string[]) {
    if (arriveTimer) clearTimeout(arriveTimer);
    for (const el of host.querySelectorAll(".arrival.hit")) el.classList.remove("hit");
    if (!targets.length) return;

    arriveTimer = setTimeout(() => {
      for (const to of targets) {
        const ring = host.querySelector<SVGElement>(`.arrival[data-to="${CSS.escape(to)}"]`);
        if (!ring) continue;
        ring.classList.remove("hit");
        void (ring as unknown as HTMLElement).offsetWidth;
        ring.classList.add("hit");
      }
    }, 260);
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
      host
        .querySelector(`.head[data-from="${CSS.escape(sig)}"][data-to="${CSS.escape(to)}"]`)
        ?.classList.add("lit");
    }
    arriveAt(host, targets);

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
      host
        .querySelector(`.head[data-from="${CSS.escape(c)}"][data-to="${CSS.escape(to)}"]`)
        ?.classList.add("lit");
    }
    arriveAt(host, [to]);

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

  /** The hand-over line, as a fraction of the pane — the scrollytelling convention. */
  const TRIGGER = 0.38;

  function triggerY(): number {
    if (!body) return 0;
    const box = body.getBoundingClientRect();
    return box.top + box.height * TRIGGER;
  }

  /**
   * Push the first step down so it starts just *below* the trigger.
   *
   * Measured rather than guessed: on a tall window the trigger line sits below
   * the first paragraph or two at rest, and without this the reading opens
   * already at step 2 — how far in depending on the monitor.
   */
  function sizeLead() {
    if (!container || !body || !steps.length) return;

    let lead = container.querySelector<HTMLElement>(".reading-lead");
    let tail = container.querySelector<HTMLElement>(".reading-tail");
    if (!lead) {
      lead = document.createElement("div");
      lead.className = "reading-lead";
      steps[0].parentNode?.insertBefore(lead, steps[0]);
    }
    if (!tail) {
      tail = document.createElement("div");
      tail.className = "reading-tail";
      container.appendChild(tail);
    }

    if (!reading) {
      lead.style.height = "0px";
      tail.style.height = "0px";
      return;
    }

    const H = body.clientHeight;
    const paneTop = body.getBoundingClientRect().top;

    // Offset of the first step within the scrollable content — measured with
    // the lead collapsed, and independent of where the reader currently is.
    lead.style.height = "0px";
    const firstOffset =
      steps[0].getBoundingClientRect().top - paneTop + body.scrollTop;
    lead.style.height = Math.max(0, H * TRIGGER + 16 - firstOffset) + "px";

    // The last step has to be able to reach the trigger too. At the bottom of
    // the scroll there must be at least (1 - TRIGGER) of a pane below its top,
    // otherwise it simply never fires — and how badly depends on the window
    // height, so a fixed fraction is not good enough.
    const lastH = steps[steps.length - 1].getBoundingClientRect().height;
    tail.style.height = Math.max(0, H * (1 - TRIGGER) - lastH) + "px";

    bandTop = H * TRIGGER + (paneTop - (pane?.getBoundingClientRect().top ?? paneTop));
  }

  /**
   * Mark step `i` as current — on the block, and on the row that contains it.
   *
   * One helper because two call sites set this (scrolling and clicking), and they
   * were already required never to disagree about which reference is current.
   */
  function markNow(i: number) {
    for (const el of container?.querySelectorAll(".now") ?? []) el.classList.remove("now");
    const block = steps[i];
    if (!block) return;
    block.classList.add("now");
    block.closest("li")?.classList.add("now");
  }

  function stepAt(): number {
    const line = triggerY();
    let hit = -1;
    steps.forEach((p, i) => {
      if (p.getBoundingClientRect().top <= line) hit = i;
    });
    return hit;
  }

  let ticking = false;
  function onDocScroll() {
    if (!reading || ticking) return;
    ticking = true;
    requestAnimationFrame(() => {
      ticking = false;
      const i = stepAt();
      if (i === activeStep) return;
      activeStep = i;

      // Before the first paragraph reaches the trigger the reading has not
      // begun: the file sits at rest rather than pre-armed on step one.
      if (i < 0) {
        focus.rest();
        activeRef = null;
        markNow(-1);
        return;
      }
      // `:not(.mention)` is the block's own step — a nested block's reference is
      // a mention from out here, and its own lead from in there.
      const ref = steps[i].querySelector<HTMLElement>("code.ref[data-line]:not(.mention)");
      if (!ref) return;
      const start = parseInt(ref.dataset.line ?? "0", 10);
      const end = parseInt(ref.dataset.end ?? "0", 10) || start;
      if (start > 0) {
        activeRef = Number(ref.dataset.ref ?? -1);
        const path = ref.dataset.path ?? current;
        const spans = rangesOf(ref) ?? [{ start, end }];
        // A reading crosses files, so a step may be a file swap and a selection.
        // The swap goes first — a span means nothing over the wrong source — and
        // `focus.step` is told the path so it can skip the crossfade, which has
        // nothing to fade between across two files.
        void (async () => {
          await switchIfNeeded(path);
          focus.step(ref.dataset.sig ?? `line ${start}`, spans, null, null, path);
        })();
      }

      // Only the block is marked here. The chip is marked by the selection
      // effect, keyed on focus.sig — one mechanism, so scrolling and clicking
      // can never disagree about which reference is current.
      markNow(i);

      // And the band takes one flash, so the hand-over is something you *see*
      // happen rather than a line you infer. Only on an actual step change —
      // firing every scroll frame is noise, and noise is what gets animation
      // switched off.
      if (bandEl) {
        bandEl.classList.remove("fire");
        void bandEl.offsetWidth;
        bandEl.classList.add("fire");
      }
    });
  }

  /**
   * Keep the reading in step with a click.
   *
   * Selecting something jumps the code, but leaves the doc wherever it was — so
   * you are reading paragraph two while the code shows step four, and the next
   * scroll event snaps back. Scrolling the matching paragraph up to the trigger
   * makes the click a move within the reading rather than a detour out of it.
   */
  function alignReading(clicked?: HTMLElement | null) {
    if (!reading || !body || !focus.active) return;

    // Prefer the paragraph the click was actually in. Falling back to a search
    // by name would land on the *first* mention of it, which is why clicking a
    // repeated reference used to scroll backwards.
    const own = clicked?.closest<HTMLElement>(".step");
    const i = own
      ? steps.indexOf(own)
      : steps.findIndex((p) => p.querySelector(`code.ref[data-sig="${CSS.escape(focus.sig)}"]`));
    if (i < 0) return; // selected something the prose never mentions

    const delta = steps[i].getBoundingClientRect().top - triggerY();
    // +2 so the paragraph lands just *past* the line and the step registers.
    body.scrollTo({ top: body.scrollTop + delta + 2, behavior: "smooth" });

    activeStep = i;
    markNow(i);
  }

  function toggleReading() {
    reading = !reading;
    focus.reading = reading;
    if (!reading) {
      activeStep = -1;
      focus.rest();
    }
    // Wait for the class change to land before measuring.
    queueMicrotask(() => {
      sizeLead();
      activeStep = -2;
      onDocScroll();
    });
  }

  // Re-measure whenever the content, the mode or the window changes.
  $effect(() => {
    steps;
    reading;
    queueMicrotask(sizeLead);
  });

  $effect(() => {
    const onResize = () => {
      sizeLead();
      activeStep = -2;
      onDocScroll();
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  });

  // Leaving the doc pane must not leave the app stuck in reading state.
  $effect(() => () => {
    focus.reading = false;
  });

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

  /**
   * After every render, find the paragraphs that carry a reference.
   *
   * **The first reference in a block is that block's step**; later ones in the
   * same paragraph stay clickable but do not re-trigger. Without that rule a
   * paragraph naming three functions fires three code scrolls inside about
   * 60px of scrolling, and they step on each other.
   */
  $effect(() => {
    html;
    if (!container) {
      steps = [];
      return;
    }
    // Every block that carries a reference — but a reference can sit inside more
    // than one of them, and only the INNERMOST may own it as a step.
    //
    // Two shapes make that happen, and both are ordinary markdown:
    //
    //   a *loose* list      `<li><p>ref</p></li>`  — li and p both match
    //   a *nested* list     `<li>ref<ul><li>ref</li></ul></li>`
    //
    // Left alone, a loose three-item list becomes six steps and every second
    // scroll does nothing. Taking the innermost owner fixes that, and handles
    // nesting properly at the same time: a parent keeps whatever reference it
    // holds directly, and the child keeps its own.
    const SEL = "code.ref[data-line]";
    const blocks = [...container.querySelectorAll<HTMLElement>("p, li, blockquote")];
    const carriers = blocks.filter((b) => b.querySelector(SEL));
    const allRefs = [...container.querySelectorAll<HTMLElement>(`.doc ${SEL}`)];

    // Carriers come out in document order, so for any reference the LAST carrier
    // containing it is the innermost one.
    const owner = new Map<HTMLElement, HTMLElement>();
    for (const r of allRefs) {
      const chain = carriers.filter((b) => b.contains(r));
      if (chain.length) owner.set(r, chain[chain.length - 1]);
    }

    // The first reference a block owns is its step; the rest stay clickable.
    const lead = new Map<HTMLElement, HTMLElement>();
    for (const r of allRefs) {
      const own = owner.get(r);
      if (own && !lead.has(own)) lead.set(own, r);
    }

    let n = 0;
    for (const r of allRefs) {
      r.classList.toggle("mention", lead.get(owner.get(r) as HTMLElement) !== r);
      // A stable per-render index, so duplicates of one name stay distinct.
      r.dataset.ref = String(n++);
    }

    const found = carriers.filter((b) => lead.has(b));
    for (const b of found) {
      b.classList.add("step");
      // In a loose list the step is the inner <p>, but the *row* is the <li>.
      // Marking it lets the row dim and highlight as one thing — otherwise the
      // number in the gutter stays bright while the text beside it fades.
      const row = b.closest("li");
      if (row && row !== b) row.classList.add("steprow");
    }
    steps = found;
    activeRef = null;

    // Which files the prose actually reaches. The file strip draws a hollow dot
    // for the others: opening a file to check something and never referring to
    // it again is the normal accident of a review, and the dot is the same nudge
    // an empty explanation slot is, one level up.
    const reached = new Set<string>();
    for (const r of container.querySelectorAll<HTMLElement>(".doc code.ref[data-path]")) {
      if (r.dataset.path) reached.add(r.dataset.path);
    }
    onrefs?.(reached);
  });

  // Mark the focused row, so both panes show the same selection — and repaint
  // the reach block, so a selection pins its connections. Depends on `html` too:
  // re-rendering the doc wipes these classes, and they have to be put back.
  $effect(() => {
    const sig = focus.sig;
    html;
    activeRef;
    if (!container) return;

    // Blocks are one row per signature, so matching by name is right there.
    for (const el of container.querySelectorAll(
      ".fnrow, .tm-tile, .lgtm-deps .fn, .lgtm-surface .row, " +
        ".lgtm-tests [data-sig], .lgtm-settings [data-sig]",
    )) {
      el.classList.toggle("active", focus.active && (el as HTMLElement).dataset.sig === sig);
    }
    // Chips are not: the same name can appear in five paragraphs, and only the
    // one being read is current.
    for (const el of container.querySelectorAll<HTMLElement>(".doc code.ref[data-ref]")) {
      el.classList.toggle("active", focus.active && el.dataset.ref === String(activeRef));
    }
    const host = reachHost();
    if (host) litReach(host, null);
  });
</script>

<div
  class="pane"
  class:reading
  class:dark={reading}
  class:reading-surface={reading}
  bind:this={pane}
>
  {#if reading}
    <!-- Where one paragraph hands over to the next. Quiet on purpose: it makes
         the mechanic legible without becoming a debug overlay. -->
    <div class="band" style:top="{bandTop}px" bind:this={bandEl}></div>
  {/if}
  <div class="panehead">
    {#if dirty}<span class="dot" title="unsaved"></span>{/if}
    <span>{filename ? `${filename}.md` : "no doc"}</span>
    <span class="spacer"></span>
    {#if stale}
      <button class="btn icon warn" onclick={() => onreconcile?.()}>⟳ Code changed — reconcile</button>
    {/if}
    <FontStepper {font} label="text" />
    {#if canRead && !editing && steps.length}
      <button
        class="btn icon read"
        class:on={reading}
        onclick={toggleReading}
        title="Scroll the doc and the code follows your reading"
      >
        {reading ? "▶ Reading" : "▷ Read"}
      </button>
    {/if}
    <div class="toggle">
      <button
        class:on={!editing}
        onclick={() => {
          editing = false;
        }}>Preview</button
      >
      <button
        class:on={editing}
        onclick={() => {
          // Scroll-driven state while typing is chaos.
          editing = true;
          if (reading) toggleReading();
        }}>Edit</button
      >
    </div>
  </div>

  <div
    class="panebody"
    style:--doc-font="{font.value}px"
    bind:this={body}
    onscroll={onDocScroll}
  >
    {#if editing}
      <div class="editwrap">
        <textarea
          class="raw"
          bind:this={editor}
          bind:value={markdown}
          spellcheck="false"
          oninput={onEditorInput}
          onkeydown={onEditorKey}
          onblur={closeMenu}
          onscroll={closeMenu}
        ></textarea>
        <!-- metrics twin for locating the caret; never visible -->
        <div class="mirror" bind:this={mirror} aria-hidden="true"></div>
        {#if slashAt !== null && canRead}
          <RefMenu
            bind:this={menu}
            entries={vocabulary(files, current)}
            query={slashQuery}
            x={menuX}
            y={menuY}
            flip={menuFlip}
            onpick={insertRef}
            onclose={closeMenu}
          />
        {/if}
      </div>
    {:else}
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div
        class="doc"
        class:reading
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
    position: relative;
  }
  .band {
    position: absolute;
    left: 0;
    right: 0;
    height: 0;
    z-index: 3;
    pointer-events: none;
    border-top: 1px solid color-mix(in srgb, var(--read) 30%, transparent);
  }
  /* One flash per step. Without it the band shows you *where* the hand-over
     happens but never *that* it happened — which is the difference between "the
     code changed on its own" and "I did that by scrolling". Drawn on ::after so
     the hairline itself never moves. */
  .band::after {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    top: -1px;
    height: 1.5px;
    background: var(--read);
    opacity: 0;
  }
  .band:global(.fire)::after {
    animation: bandfire var(--ring) var(--ease) both;
  }
  @keyframes bandfire {
    0% {
      opacity: 0.85;
    }
    100% {
      opacity: 0;
    }
  }
  /* Read mode's surface is `.dark` for the semantic colours plus
     `.reading-surface` for a warm set of neutrals — both live in app.css, and
     nothing about it is declared here beyond the transition, so the surface
     fades rather than snapping.
     It is a third surface on purpose: the doc pane is warm paper in light mode,
     so its lights-out form should still be warm, and it has to be tellable from
     the app's own cool dark at a glance. Because only the neutrals are
     overridden, "current"/"public"/"private" keep meaning the same thing in both
     panes. */
  .panehead {
    background: var(--doc-bg);
    border-bottom-color: var(--doc-line);
    transition: background 0.3s ease;
  }
  .panebody {
    flex: 1;
    overflow: auto;
    background: var(--doc-bg);
    color: var(--doc-fg);
    transition:
      background 0.3s ease,
      color 0.3s ease;
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

  .editwrap {
    position: relative;
    height: 100%;
  }
  /* The mirror must match the textarea in every metric that affects where a
     glyph lands — font, size, line height, padding, width, wrapping. Any drift
     and the caret measurement is silently wrong. */
  .mirror,
  .raw {
    font-family: var(--mono);
    /* One control scales both views. The ratio is the one the two defaults
       already had — 12.5px of mono reads about the same size as 14px of prose —
       so stepping keeps the relationship rather than converging on one number.
       Both selectors MUST keep identical metrics or the caret measurement that
       positions the `/` menu goes wrong, which is why they share this block. */
    font-size: calc(var(--doc-font, 14px) * 0.89);
    line-height: 1.7;
    padding: 26px 30px;
    white-space: pre-wrap;
    overflow-wrap: break-word;
    box-sizing: border-box;
  }
  .mirror {
    position: absolute;
    inset: 0;
    visibility: hidden;
    pointer-events: none;
    overflow: hidden;
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
  }

  .doc {
    /* Enough tail room to clear the window edge, not a screenful of nothing. */
    padding: 26px 30px 40px;
    position: relative;
    /* The base the prose is measured against. Only the text elements below use
       `em`; every block keeps absolute sizes, because they are data displays
       and should not reflow when you nudge the reading size. */
    font-size: var(--doc-font, 14px);
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
    font-size: 1.43em;
    margin: 0 0 6px;
    letter-spacing: -0.01em;
    color: var(--doc-fg);
  }
  :global(.doc h2) {
    font-size: 0.93em;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--fg-faint);
    margin: 30px 0 10px;
  }
  :global(.doc p) {
    color: var(--fg-dim);
    font-size: 1em;
    line-height: 1.7;
    margin: 0 0 14px;
  }
  /* ---- lists ----
     A reading is usually a numbered list of steps, and a step is the unit read
     mode walks — so a list item is a **row**, not a line of prose with a bullet
     in front of it. Ordinary markdown, styled: nothing new to type, and the same
     text still reads correctly pasted into a PR comment.

     The marker lives in the gutter the row's own left padding leaves, so a
     wrapped second line aligns with the first rather than tucking under the
     number. */
  :global(.doc ul),
  :global(.doc ol) {
    margin: 0 0 14px;
    padding: 0;
    list-style: none;
    counter-reset: step;
  }
  :global(.doc li) {
    position: relative;
    padding: 5px 10px 5px 34px;
    margin: 1px 0;
    border-radius: 7px;
    border-left: 2px solid transparent;
    color: var(--fg-dim);
    line-height: 1.65;
    transition: background 0.12s ease;
  }
  :global(.doc li:hover) {
    background: color-mix(in srgb, var(--fg) 4%, transparent);
  }
  /* A loose list wraps each item's text in a paragraph. It must not bring the
     paragraph's own margins into the row. */
  :global(.doc li > p) {
    margin: 0;
  }
  :global(.doc li > p + p) {
    margin-top: 8px;
  }
  :global(.doc li > ul),
  :global(.doc li > ol) {
    margin: 4px 0 0;
  }

  :global(.doc ol > li) {
    counter-increment: step;
  }
  :global(.doc ol > li::before) {
    content: counter(step);
    position: absolute;
    left: 0;
    top: 5px; /* the row's own padding-top */
    width: 24px;
    text-align: right;
    font-family: var(--mono);
    font-size: 0.82em;
    font-weight: 600;
    /* 0.82em × 2.01 = 1.65em of the row — the same line box the content has, so
       the number's baseline sits with the text's at every doc font size instead
       of drifting the moment the stepper is nudged. */
    line-height: 2.01;
    color: var(--fg-faint);
  }

  /* Depth carries in the marker as well as the indent, so a sub-point still
     reads as one at a glance when the indent is only 34px. Filled square, then
     hollow circle, then a dash — three levels is more than any reading needs. */
  :global(.doc ul > li::before) {
    content: "";
    position: absolute;
    left: 13px;
    /* Centred on the first line: 5px padding + half a 1.65em line box, less half
       the dot. In `em` so it tracks the font size rather than being a constant
       that is only right at 14px. */
    top: calc(2.5px + 0.825em);
    width: 5px;
    height: 5px;
    border-radius: 1.5px;
    background: var(--fg-faint);
  }
  :global(.doc ul ul > li::before) {
    background: transparent;
    border-radius: 50%;
    box-shadow: inset 0 0 0 1.5px var(--fg-faint);
  }
  :global(.doc ul ul ul > li::before) {
    width: 6px;
    height: 1.5px;
    border-radius: 0;
    background: var(--fg-faint);
    box-shadow: none;
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
    font-size: 0.86em;
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
    animation: fnPulse var(--slow) ease-in-out infinite;
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
    animation: caretNudge var(--slow) ease-in-out infinite;
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
    animation: tmBreathe var(--calm) ease-in-out infinite;
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
    /* The two that ran forever and ignored the setting entirely. Same rule as
       everywhere else: keep the bright end of the pulse, drop the pulse. */
    :global(.fnrow.active) {
      animation: none;
      box-shadow:
        inset 3px 0 0 0 var(--accent),
        0 0 0 4px color-mix(in srgb, var(--accent) 12%, transparent);
    }
    :global(.fnrow.active::after) {
      animation: none;
      opacity: 0.75;
    }
    /* And the transient ones: each keeps its END state, so the ring still says
       "here", the underline is still drawn, and a step still marks its band. */
    :global(.doc p.invite-pulse::after) {
      animation: none;
      opacity: 0.35;
    }
    :global(.doc code.ref.active::after) {
      animation: none;
      transform: scaleX(1);
    }
    :global(.doc.arriving .lgtm-surface .row) {
      animation: none;
    }
    .band:global(.fire)::after {
      animation: none;
    }
    :global(.lgtm-deps svg.focusing .edge.lit) {
      stroke-dasharray: none;
      animation: none;
    }
    :global(.lgtm-deps .arrival.hit) {
      animation: none;
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
    /* The dash TRAVELS, outward. This is the only thing in the block that can
       say which way the call goes — a static boundary shows you *that*
       create_user/1 connects to Repo.insert/1, never that it reaches out. */
    stroke-dasharray: 7 9;
    animation: trace var(--trace) linear infinite;
  }
  @keyframes trace {
    to {
      stroke-dashoffset: -32;
    }
  }

  /* The arrowhead is the reduced-motion stand-in for the travelling dash:
     hidden while the dash is doing the job, and the only thing carrying
     direction once motion is off. */
  :global(.lgtm-deps .head) {
    opacity: 0;
  }
  @media (prefers-reduced-motion: reduce) {
    :global(.lgtm-deps svg.focusing .head.lit) {
      opacity: 0.95;
    }
    :global(.lgtm-deps .head.app) {
      fill: var(--accent);
    }
  }

  /* The hit on arrival: one ring, timed to land when the trace would have got
     there. Scaled by transform on a fill-box origin — `r` is not reliably
     animatable, and this needs no geometry from the renderer. */
  :global(.lgtm-deps .arrival) {
    fill: none;
    stroke: var(--accent);
    stroke-width: 1.5;
    opacity: 0;
    transform-box: fill-box;
    transform-origin: center;
  }
  :global(.lgtm-deps .arrival.hit) {
    animation: arrive var(--ring) var(--ease) both;
  }
  @keyframes arrive {
    0% {
      opacity: 0.9;
      transform: scale(0.6);
    }
    70% {
      opacity: 0.18;
      transform: scale(2.6);
    }
    100% {
      opacity: 0;
      transform: scale(3);
    }
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

  /* ---- the lgtm:surface block ----
     The directory. Ported from mockup/surface.html. Public left, private right,
     each sorted by name and scrolling on its own. */
  :global(.lgtm-surface) {
    border: 1px solid var(--doc-line);
    border-radius: 8px;
    background: var(--code-bg);
    box-shadow: var(--shadow);
    overflow: hidden;
    margin: 0 0 22px;
  }
  :global(.lgtm-surface.empty) {
    padding: 18px;
    color: var(--fg-faint);
    font-size: 12.5px;
  }
  :global(.lgtm-surface > header) {
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
  :global(.lgtm-surface > header .tag) {
    font-family: var(--mono);
    color: var(--mark);
    text-transform: none;
    letter-spacing: 0;
  }
  :global(.lgtm-surface > header .count) {
    margin-left: auto;
    font-family: var(--mono);
  }

  :global(.lgtm-surface .cols) {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }
  :global(.lgtm-surface .col) {
    min-width: 0;
  }
  :global(.lgtm-surface .col + .col) {
    border-left: 1px solid var(--line-soft);
  }
  /* Sticky, so scrolling one column never loses which side you are on. */
  :global(.lgtm-surface .col > .label) {
    position: sticky;
    top: 0;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 8px 12px;
    background: var(--code-bg);
    border-bottom: 1px solid var(--line-soft);
    font-size: 10.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }
  :global(.lgtm-surface .col.public > .label) {
    color: var(--pub);
  }
  :global(.lgtm-surface .col.private > .label) {
    color: var(--priv);
  }
  :global(.lgtm-surface .col > .label .bar) {
    width: 3px;
    height: 11px;
    border-radius: 2px;
    background: currentColor;
  }
  :global(.lgtm-surface .col > .label .n) {
    margin-left: auto;
    font-family: var(--mono);
    color: var(--fg-faint);
  }

  :global(.lgtm-surface .list) {
    height: 300px;
    overflow-y: auto;
    /* Don't hand the scroll to the doc pane when a column bottoms out. */
    overscroll-behavior: contain;
  }
  :global(.lgtm-surface .row) {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 5px 12px;
    cursor: pointer;
    border-left: 2px solid transparent;
  }
  :global(.lgtm-surface .row:hover) {
    background: var(--bg-inset);
    border-left-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }
  :global(.lgtm-surface .row.active) {
    background: var(--sel);
    border-left-color: var(--accent);
  }
  :global(.lgtm-surface .row .sig) {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  :global(.lgtm-surface .row.active .sig) {
    color: var(--accent);
    font-weight: 600;
  }
  :global(.lgtm-surface .row .sig .ar) {
    color: var(--fg-faint);
  }
  :global(.lgtm-surface .row .badge) {
    font-size: 9px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--fg-faint);
    border: 1px solid var(--line);
    border-radius: 999px;
    padding: 0 5px;
    white-space: nowrap;
    flex: none;
  }
  :global(.lgtm-surface .row .spacer) {
    flex: 1;
  }
  :global(.lgtm-surface .row .ln) {
    font-family: var(--mono);
    font-size: 10px;
    color: var(--fg-faint);
    flex: none;
    font-variant-numeric: tabular-nums;
  }
  :global(.lgtm-surface .list .none) {
    padding: 14px 12px;
    color: var(--fg-faint);
    font-size: 12px;
    font-style: italic;
    margin: 0;
  }

  /* ---- the lgtm:settings block ----
     A config has no functions, so no chart — the value is the grouping plus one
     marking: which values come from the environment. Ported from
     mockup/kinds.html. */
  :global(.lgtm-settings) {
    border: 1px solid var(--doc-line);
    border-radius: 8px;
    background: var(--code-bg);
    box-shadow: var(--shadow);
    overflow: hidden;
    margin: 0 0 22px;
  }
  :global(.lgtm-settings.empty),
  :global(.lgtm-tests.empty) {
    padding: 18px;
    color: var(--fg-faint);
    font-size: 12.5px;
  }
  :global(.lgtm-settings > header),
  :global(.lgtm-tests > header) {
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
  :global(.lgtm-settings > header .tag),
  :global(.lgtm-tests > header .tag) {
    font-family: var(--mono);
    color: var(--mark);
    text-transform: none;
    letter-spacing: 0;
  }
  :global(.lgtm-settings > header .count),
  :global(.lgtm-tests > header .count) {
    margin-left: auto;
    font-family: var(--mono);
  }
  :global(.lgtm-settings > footer) {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 12px;
    border-top: 1px solid var(--line-soft);
    background: var(--bg-inset);
    font-size: 11px;
    color: var(--fg-dim);
  }
  :global(.lgtm-settings > footer .lbl) {
    color: var(--fg-faint);
  }
  :global(.lgtm-settings > footer .path) {
    font-family: var(--mono);
  }
  :global(.lgtm-settings > footer .path b) {
    color: var(--fg);
  }
  :global(.lgtm-settings > footer .wins) {
    margin-left: auto;
    color: var(--fg-faint);
  }

  :global(.lgtm-settings .grp) {
    border-bottom: 1px solid var(--line-soft);
  }
  :global(.lgtm-settings .grp:last-child) {
    border-bottom: 0;
  }
  :global(.lgtm-settings .grp > .head) {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 8px 12px 6px;
    background: var(--bg-raised);
    cursor: pointer;
  }
  :global(.lgtm-settings .grp > .head .app) {
    font-family: var(--mono);
    font-size: 12.5px;
    color: var(--accent);
    font-weight: 600;
  }
  :global(.lgtm-settings .grp > .head .target) {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--fg);
  }
  :global(.lgtm-settings .grp > .head .sep) {
    color: var(--fg-faint);
  }
  :global(.lgtm-settings .grp > .head .envn) {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--priv);
  }
  :global(.lgtm-settings .grp > .head .n) {
    margin-left: auto;
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg-faint);
  }

  :global(.lgtm-settings .kv) {
    display: grid;
    grid-template-columns: minmax(140px, 34%) 1fr;
    gap: 10px;
    padding: 4px 12px 4px 22px;
    align-items: baseline;
    cursor: pointer;
    border-left: 2px solid transparent;
  }
  :global(.lgtm-settings .kv:hover) {
    background: var(--bg-inset);
    border-left-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }
  :global(.lgtm-settings .kv .k) {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--fg-dim);
  }
  :global(.lgtm-settings .kv .v) {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* The finding: deploy-time values stand out from baked-in ones. */
  :global(.lgtm-settings .kv.fromenv .v) {
    color: var(--priv);
  }
  :global(.lgtm-settings .kv .from) {
    font-size: 9px;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--priv);
    margin-right: 5px;
  }
  :global(.lgtm-settings .kv.secret .v) {
    color: var(--fg-faint);
    font-style: italic;
  }

  /* ---- the lgtm:tests block ---- */
  :global(.lgtm-tests) {
    border: 1px solid var(--doc-line);
    border-radius: 8px;
    background: var(--code-bg);
    box-shadow: var(--shadow);
    overflow: hidden;
    margin: 0 0 22px;
  }

  /* setup is infrastructure, not a test — violet, like every other meta thing */
  :global(.lgtm-tests .modsetup) {
    border-bottom: 1px solid var(--line);
    background: var(--bg-inset);
    padding-bottom: 4px;
  }
  :global(.lgtm-tests .scopelbl) {
    padding: 7px 12px 4px;
    font-size: 9.5px;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--fg-faint);
  }
  :global(.lgtm-tests .su) {
    display: flex;
    align-items: baseline;
    gap: 7px;
    padding: 3px 12px;
    cursor: pointer;
    border-left: 2px solid transparent;
  }
  :global(.lgtm-tests .su.desc) {
    padding-left: 24px;
  }
  :global(.lgtm-tests .su:hover) {
    background: var(--bg-raised);
    border-left-color: color-mix(in srgb, var(--mark) 55%, transparent);
  }
  :global(.lgtm-tests .su .gear),
  :global(.lgtm-tests .ctx .gear) {
    color: var(--mark);
    font-size: 11px;
  }
  :global(.lgtm-tests .su .kind) {
    font-family: var(--mono);
    font-size: 11.5px;
    color: var(--mark);
  }
  :global(.lgtm-tests .su .lbl) {
    font-size: 10px;
    color: var(--fg-faint);
  }
  :global(.lgtm-tests .key) {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg);
    background: color-mix(in srgb, var(--mark) 12%, transparent);
    border-radius: 3px;
    padding: 0 4px;
    margin-right: 3px;
  }
  :global(.lgtm-tests .named) {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg-dim);
  }
  :global(.lgtm-tests .unknown) {
    font-size: 10.5px;
    color: var(--fg-faint);
    font-style: italic;
  }
  :global(.lgtm-tests .su .spacer) {
    flex: 1;
  }
  :global(.lgtm-tests .su .ln),
  :global(.lgtm-tests .t .ln) {
    font-family: var(--mono);
    font-size: 10px;
    color: var(--fg-faint);
    flex: none;
  }

  /* what a describe's tests actually start with */
  :global(.lgtm-tests .ctx) {
    display: inline-flex;
    align-items: baseline;
    gap: 4px;
    flex: none;
    font-family: var(--mono);
    font-size: 10px;
    color: var(--fg-faint);
    border: 1px solid var(--line);
    border-radius: 999px;
    padding: 1px 7px;
  }

  :global(.lgtm-tests .desc) {
    border-bottom: 1px solid var(--line-soft);
  }
  :global(.lgtm-tests .desc:last-child) {
    border-bottom: 0;
  }
  :global(.lgtm-tests .desc > .head) {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    cursor: pointer;
    background: var(--bg-raised);
  }
  :global(.lgtm-tests .desc > .head:hover) {
    background: var(--bg-inset);
  }
  :global(.lgtm-tests .desc > .head .name) {
    font-family: var(--mono);
    font-size: 12.5px;
    color: var(--fg);
  }
  :global(.lgtm-tests .desc > .head .name.loose) {
    color: var(--fg-faint);
  }
  :global(.lgtm-tests .desc > .head .name .q) {
    color: var(--fg-faint);
  }
  :global(.lgtm-tests .desc > .head .n) {
    margin-left: auto;
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg-faint);
  }

  /* one square per test, shaded by how much it asserts */
  :global(.lgtm-tests .strip) {
    display: flex;
    gap: 3px;
    flex: none;
  }
  :global(.lgtm-tests .strip i) {
    width: 11px;
    height: 11px;
    border-radius: 2px;
    display: block;
    background: var(--pub);
  }
  :global(.lgtm-tests .strip i.a1) {
    opacity: 0.28;
  }
  :global(.lgtm-tests .strip i.a2) {
    opacity: 0.55;
  }
  :global(.lgtm-tests .strip i.a3) {
    opacity: 1;
  }
  :global(.lgtm-tests .strip i.tagged) {
    box-shadow: inset 0 0 0 1.5px var(--priv);
  }
  :global(.lgtm-tests .strip i.skipped) {
    background: var(--fg-faint);
    opacity: 0.35;
  }

  :global(.lgtm-tests .tests) {
    padding: 2px 0 6px;
  }
  :global(.lgtm-tests .t) {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 4px 12px 4px 24px;
    cursor: pointer;
    border-left: 2px solid transparent;
  }
  :global(.lgtm-tests .t:hover) {
    background: var(--bg-inset);
    border-left-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }
  :global(.lgtm-tests .t .dot) {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--pub);
    flex: none;
    align-self: center;
  }
  :global(.lgtm-tests .t.skipped .dot) {
    background: var(--fg-faint);
  }
  :global(.lgtm-tests .t .name) {
    font-size: 12px;
    color: var(--fg-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  :global(.lgtm-tests .t .badge) {
    font-size: 9px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--priv);
    border: 1px solid color-mix(in srgb, var(--priv) 40%, transparent);
    border-radius: 999px;
    padding: 0 5px;
    white-space: nowrap;
    flex: none;
  }
  :global(.lgtm-tests .t .spacer) {
    flex: 1;
  }
  :global(.lgtm-tests .t .as) {
    font-family: var(--mono);
    font-size: 10px;
    color: var(--fg-faint);
    flex: none;
  }

  /* A selected test / describe / setup / setting, marked the same way a
     selected function row is. */
  :global(.lgtm-tests [data-sig].active),
  :global(.lgtm-settings [data-sig].active) {
    background: var(--sel);
    border-left-color: var(--accent);
  }
  :global(.lgtm-tests .t.active .name),
  :global(.lgtm-tests .desc > .head.active .name),
  :global(.lgtm-settings .kv.active .k) {
    color: var(--accent);
    font-weight: 600;
  }

  /* ---- arrival: the invitation ----
     A doc you have not written in has an empty Explain section, and that
     emptiness is the whole nudge of the tool — so the ring expands from the
     invitation rather than from the pane. "A file arrived" is decoration; you
     know, you just opened it. "You, here" is not.

     Once. An infinite pulse around the thing you are about to type into would
     be unbearable inside a minute. */
  :global(.doc p.invite-pulse) {
    position: relative;
  }
  :global(.doc p.invite-pulse::after) {
    content: "";
    position: absolute;
    inset: -7px -9px;
    border-radius: 8px;
    border: 1.5px solid var(--accent);
    pointer-events: none;
    animation: ring var(--ring) var(--ease) both;
  }
  @keyframes ring {
    0% {
      opacity: 0.9;
      transform: scale(0.985);
    }
    70% {
      opacity: 0.14;
      transform: scale(1.02);
    }
    100% {
      opacity: 0;
      transform: scale(1.03);
    }
  }

  /* ---- arrival: the directory ----
     Forty names appearing at once reads as a wall; cascading reads as arriving,
     and the eye follows the first column down — which is the order the block
     wants to be read in anyway.

     The stagger is CAPPED. A 200-function module would otherwise cascade for
     four seconds, and waiting on an animation to look a name up is worse than
     no animation at all. Both columns share the index so they arrive together. */
  :global(.doc.arriving .lgtm-surface .row) {
    animation: rowin var(--fast) var(--ease) both;
    animation-delay: calc(min(var(--i, 0), 12) * 16ms);
  }
  @keyframes rowin {
    from {
      opacity: 0;
      transform: translateY(3px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  /* ---- scroll-driven reading ----
     Inline code that names something in this file. No new syntax: the markdown
     stays portable, which is why references are backticks and not `{{…}}`. */
  :global(.doc code.ref) {
    /* A reference is prose, so it scales with the prose. */
    font-size: 0.89em;
    cursor: pointer;
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    color: var(--accent);
    border-bottom: 1.5px solid transparent;
    /* Deliberately NOT transitioning the fill. Fading it meant the outgoing
       chip was still blue while the incoming one lit, so two references looked
       current at once — which reads as a glitch. The overlap belongs in the
       code pane, where it means something; here the hand-over is instant. */
    transition: border-color 0.2s ease;
  }
  /* Not on an active chip: it already has an underline wiping in, and both at
     once is two lines under one word. */
  :global(.doc code.ref:not(.active):hover) {
    border-bottom-color: var(--accent);
  }

  /* The arrival. An underline wipes in from the left on the chip that just
     became current — and ONLY on that one. The outgoing chip loses its state in
     the same frame, which is the rule the comment above is about: animating the
     fill left two references looking current at once. */
  :global(.doc code.ref) {
    position: relative;
  }
  :global(.doc code.ref::after) {
    content: "";
    position: absolute;
    left: 2px;
    right: 2px;
    bottom: -2.5px;
    height: 1.5px;
    border-radius: 1px;
    background: var(--accent);
    transform: scaleX(0);
    transform-origin: left;
  }
  :global(.doc code.ref.active::after) {
    animation: wipe var(--fast) var(--ease) forwards;
  }
  @keyframes wipe {
    to {
      transform: scaleX(1);
    }
  }
  :global(.doc code.ref.active) {
    background: var(--accent);
    color: #fff;
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 18%, transparent);
  }
  /* Points into a different file of the reading. Clicking it swaps the code
     pane, which is a bigger move than scrolling — so it is worth being able to
     see it coming. Marked with a leading edge rather than a different colour:
     the meaning is still "a reference", and colour already means public /
     private / current elsewhere. */
  :global(.doc code.ref.away) {
    border-left: 2px solid color-mix(in srgb, var(--accent) 45%, transparent);
    padding-left: 3px;
    border-top-left-radius: 2px;
    border-bottom-left-radius: 2px;
  }
  :global(.doc code.ref.away.active) {
    border-left-color: #fff;
  }
  /* A later mention in the same paragraph: still a link, not a step. */
  :global(.doc code.ref.mention) {
    background: transparent;
    border: 1px dashed color-mix(in srgb, var(--accent) 35%, transparent);
  }
  /* The code moved out from under the explanation. Say so — never fall back to
     plain text and lose the fact quietly. */
  :global(.doc code.ref.broken) {
    background: color-mix(in srgb, var(--priv) 12%, transparent);
    color: var(--priv);
    text-decoration: line-through;
    cursor: not-allowed;
  }

  :global(.doc.reading p.step),
  :global(.doc.reading li.step),
  :global(.doc.reading blockquote.step) {
    transition: opacity 0.35s ease;
  }
  :global(.doc.reading .step:not(.now)) {
    opacity: 0.45;
  }
  /* When the step is a paragraph inside a list item, the ROW carries the
     dimming — and the block inside it must not dim again, or the two multiply
     to 0.2 and the text goes almost invisible. */
  :global(.doc.reading li.steprow:not(.now)) {
    opacity: 0.45;
  }
  :global(.doc.reading li.steprow .step) {
    opacity: 1;
  }
  /* The current step, as a row. Covers both shapes: a tight list item is itself
     the step, a loose one only contains it. */
  :global(.doc.reading li.step.now),
  :global(.doc.reading li.steprow.now) {
    background: color-mix(in srgb, var(--accent) 7%, transparent);
    border-left-color: var(--accent);
  }
  :global(.doc.reading li.step.now::before),
  :global(.doc.reading li.steprow.now::before) {
    color: var(--accent);
  }
  :global(.reading-lead),
  :global(.reading-tail) {
    height: 0;
    transition: height 0.2s ease;
  }
</style>
