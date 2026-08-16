<script lang="ts">
  // Drag to resize; double-click to reset. The clamp keeps either pane from
  // collapsing to nothing.
  let { basis = $bindable(52), min = 20, max = 80, defaultBasis = 52 } = $props();

  let dragging = $state(false);

  function start(e: PointerEvent) {
    e.preventDefault();
    dragging = true;
    const split = (e.currentTarget as HTMLElement).parentElement;
    if (!split) return;

    function move(ev: PointerEvent) {
      const box = split!.getBoundingClientRect();
      const pct = ((ev.clientX - box.left) / box.width) * 100;
      basis = Math.min(max, Math.max(min, pct));
    }
    function up() {
      dragging = false;
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      document.body.style.cursor = "";
    }
    document.body.style.cursor = "col-resize";
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="divider"
  class:dragging
  role="separator"
  aria-orientation="vertical"
  onpointerdown={start}
  ondblclick={() => (basis = defaultBasis)}
></div>

<style>
  .divider {
    flex: 0 0 5px;
    cursor: col-resize;
    position: relative;
    background: var(--line);
    transition: background 0.12s ease;
  }
  /* fat invisible hit area, so you don't have to aim */
  .divider::after {
    content: "";
    position: absolute;
    inset: 0 -4px;
  }
  .divider:hover,
  .divider.dragging {
    background: var(--accent);
  }
</style>
