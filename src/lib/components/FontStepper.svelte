<script lang="ts">
  // A− / A+ for whichever pane owns it. The current size lives in the tooltips
  // rather than a readout: a number between the buttons was the widest thing in
  // a crowded header and told you something you can see anyway.
  import type { FontSize } from "$lib/stores/fontSize.svelte";

  let { font, label = "text" }: { font: FontSize; label?: string } = $props();
</script>

<div class="fontsize">
  <button
    onclick={() => font.set(font.value - 0.5)}
    disabled={font.value <= font.min}
    aria-label="Smaller {label}"
    title="Smaller {label} ({font.value}px)"
  >
    A<small>−</small>
  </button>
  <button
    onclick={() => font.set(font.value + 0.5)}
    disabled={font.value >= font.max}
    aria-label="Larger {label}"
    title="Larger {label} ({font.value}px)"
  >
    A<small>+</small>
  </button>
</div>

<style>
  .fontsize {
    display: flex;
    align-items: stretch;
    border: 1px solid var(--line);
    border-radius: 5px;
    overflow: hidden;
    flex: none;
  }
  .fontsize button {
    font: inherit;
    font-size: 11px;
    background: transparent;
    color: var(--fg-dim);
    border: 0;
    padding: 2px 7px;
    cursor: pointer;
    line-height: 1.6;
  }
  .fontsize button + button {
    border-left: 1px solid var(--line);
  }
  .fontsize button small {
    font-size: 9px;
    vertical-align: super;
  }
  .fontsize button:hover:not(:disabled) {
    background: var(--bg-inset);
    color: var(--fg);
  }
  .fontsize button:disabled {
    opacity: 0.35;
    cursor: default;
  }
</style>
