<script lang="ts">
  // `?` — the shortcuts, and nothing else.
  //
  // It used to be a manual: every gesture, every view, every reference form, in
  // ten sections. Nobody reads a manual in a modal; what you open `?` for is the
  // key you forgot. These are the six the app is driven by — everything else is
  // on screen, or one hover away.

  let { onclose }: { onclose: () => void } = $props();

  const keys: { keys: string[]; what: string }[] = [
    { keys: ["⌘", "K"], what: "The Hub" },
    { keys: ["⌘", "B"], what: "Open or close the file tree" },
    { keys: ["⌘", "N"], what: "New gloom" },
    { keys: ["⌘", "1"], what: "Switch between the Woop and the Gloom" },
    { keys: ["⌘", "E"], what: "Edit the gloom" },
    { keys: ["⌘", "R"], what: "Reading mode" },
  ];
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="scrim" onclick={onclose}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="panel" onclick={(e) => e.stopPropagation()}>
    <header>
      <b>Shortcuts</b>
      <span class="spacer"></span>
      <button class="btn" onclick={onclose}>Close</button>
    </header>

    <dl>
      {#each keys as row (row.what)}
        <dt>{#each row.keys as k, i (i)}<kbd>{k}</kbd>{/each}</dt>
        <dd>{row.what}</dd>
      {/each}
    </dl>
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
    padding-top: 14vh;
  }
  .panel {
    width: min(400px, 94vw);
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
    align-items: center;
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
  header .spacer {
    flex: 1;
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 10px 16px;
    margin: 0;
    padding: 16px 18px 18px;
    align-items: center;
  }
  dt {
    white-space: nowrap;
  }
  dd {
    margin: 0;
    font-size: 13px;
    color: var(--fg);
  }

  kbd {
    display: inline-block;
    min-width: 22px;
    text-align: center;
    font-family: var(--mono);
    font-size: 12px;
    border: 1px solid var(--line);
    border-bottom-width: 2px;
    border-radius: 5px;
    padding: 2px 6px;
    margin-right: 4px;
    color: var(--fg);
    background: var(--bg);
  }
</style>
