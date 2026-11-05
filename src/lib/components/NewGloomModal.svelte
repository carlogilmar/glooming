<script lang="ts">
  // ⌘N / + New Gloom — the name first.
  //
  // A gloom used to start unnamed, with its name selected in the band, so
  // skipping it cost one key. In use, skipping is what happened: the Hub filled
  // with "Unnamed gloom", an index entry that says nothing. Nothing is created
  // until there is a name, so cancelling leaves nothing behind.

  let {
    oncreate,
    onclose,
  }: {
    oncreate: (title: string) => Promise<void> | void;
    onclose: () => void;
  } = $props();

  let title = $state("");
  let busy = $state(false);
  let input = $state<HTMLInputElement | null>(null);
  const ok = $derived(title.trim().length > 0);

  $effect(() => {
    input?.focus();
  });

  async function submit() {
    if (!ok || busy) return;
    busy = true;
    try {
      await oncreate(title.trim());
    } finally {
      busy = false;
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="scrim" onclick={onclose}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
  <form
    class="panel"
    onclick={(e) => e.stopPropagation()}
    onsubmit={(e) => {
      e.preventDefault();
      void submit();
    }}
  >
    <b class="kicker">New gloom</b>
    <label for="gloom-name">What are you trying to understand?</label>
    <input
      id="gloom-name"
      bind:this={input}
      bind:value={title}
      spellcheck="false"
      autocomplete="off"
      placeholder="How an invoice gets charged"
      onkeydown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          onclose();
        }
        // The shell's window keys must not see what is typed here.
        e.stopPropagation();
      }}
    />
    <p class="hint">
      A gloom is a note in your own words. Its name is how you will find it in the Hub.
    </p>
    <div class="row">
      <span class="keys">↵ start · esc cancel</span>
      <button type="button" class="cancel" onclick={onclose}>Cancel</button>
      <button type="submit" class="go" disabled={!ok || busy}>Start gloom</button>
    </div>
  </form>
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
    padding-top: 18vh;
  }
  .panel {
    width: min(460px, 94vw);
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 18px 20px 16px;
    background: var(--gloom-bg);
    border: 1px solid color-mix(in srgb, var(--gloom) 30%, transparent);
    border-radius: 10px;
    box-shadow: 0 24px 70px rgba(10, 12, 16, 0.35);
  }
  .kicker {
    font-family: var(--display);
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.28em;
    text-transform: uppercase;
    color: var(--gloom-new);
  }
  label {
    font-size: 13px;
    color: var(--gloom-dim);
  }
  /* The name is set as it will appear in the band: serif, near-white on ink,
     with the rule under it that says the line is live. */
  input {
    font-family: var(--serif);
    font-size: 19px;
    color: var(--gloom-ink);
    background: transparent;
    border: 0;
    border-bottom: 1.5px solid var(--gloom);
    padding: 4px 0 6px;
    outline: none;
  }
  input::placeholder {
    color: color-mix(in srgb, var(--gloom-dim) 60%, transparent);
    font-style: italic;
  }
  .hint {
    margin: 2px 0 6px;
    font-size: 11.5px;
    color: var(--gloom-dim);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .keys {
    flex: 1;
    font-family: var(--mono);
    font-size: 10px;
    color: var(--gloom-dim);
  }
  .cancel,
  .go {
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    padding: 4px 12px;
    border-radius: 5px;
    cursor: pointer;
  }
  .cancel {
    color: var(--gloom-dim);
    background: transparent;
    border: 1px solid color-mix(in srgb, white 16%, transparent);
  }
  .go {
    color: var(--gloom-bg);
    background: var(--gloom-new);
    border: 1px solid var(--gloom-new);
  }
  .go:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
