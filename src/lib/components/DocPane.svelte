<script lang="ts">
  import { createMarkdownIt } from "$lib/markdownit";
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

  /** Line/end-line for a signature, from the live outline. */
  function locate(sig: string): { line: number; end: number } | null {
    const fns = outline?.modules?.[0]?.functions ?? [];
    const bare = sig.replace(/~~/g, "");
    const slash = bare.lastIndexOf("/");
    const name = slash === -1 ? bare : bare.slice(0, slash);
    const arity = parseInt((bare.slice(slash + 1).split("..").pop() ?? "0"), 10) || 0;
    const hit = fns.find((f) => f.name === name && f.arity === arity);
    return hit ? { line: hit.line, end: hit.endLine } : null;
  }

  // The rendered block is raw HTML, so rows are wired by delegation rather than
  // per-row handlers.
  function onClick(e: MouseEvent) {
    const row = (e.target as HTMLElement).closest<HTMLElement>(".fnrow[data-line]");
    if (!row) return;
    const sig = row.dataset.sig ?? "";
    const at = locate(sig);
    if (at) focus.set(sig, at.line, at.end);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key !== "Enter" && e.key !== " ") return;
    const row = (e.target as HTMLElement).closest<HTMLElement>(".fnrow[data-line]");
    if (!row) return;
    e.preventDefault();
    const sig = row.dataset.sig ?? "";
    const at = locate(sig);
    if (at) focus.set(sig, at.line, at.end);
  }

  // Mark the focused row, so both panes show the same selection.
  $effect(() => {
    const sig = focus.sig;
    if (!container) return;
    for (const el of container.querySelectorAll(".fnrow")) {
      el.classList.toggle("active", focus.active && (el as HTMLElement).dataset.sig === sig);
    }
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
      <div class="doc" bind:this={container} onclick={onClick} onkeydown={onKey}>
        {@html html}
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
    padding: 26px 30px 60vh;
    max-width: 760px;
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

  :global(.fnrow) {
    position: relative;
    display: grid;
    grid-template-columns: minmax(150px, auto) 1fr;
    gap: 14px;
    align-items: baseline;
    padding: 7px 12px;
    cursor: pointer;
    border-left: 2px solid transparent;
    transition:
      background 0.14s ease,
      border-color 0.14s ease;
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
  }
  :global(.fnrow .sig .ar) {
    color: var(--fg-faint);
  }
  :global(.fnrow .clauses) {
    color: var(--fg-faint);
    font-size: 10.5px;
    margin-left: 5px;
  }
  :global(.fnrow .why) {
    color: var(--fg-dim);
    font-size: 12.5px;
    line-height: 1.5;
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
    top: 50%;
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
      transform: translate(2px, -50%);
    }
    50% {
      opacity: 1;
      transform: translate(-2px, -50%);
    }
  }
</style>
