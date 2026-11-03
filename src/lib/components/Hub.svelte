<script lang="ts">
  // The Hub: a project's glooms as a table of contents. `mockup/hub.html` is the
  // contract.
  //
  // Opening a project lands here, in the space the two empty panes used to take;
  // once you are reading, `⌂ Hub` (⌘K) brings the same page back in place of the
  // two panes — a modal over the work was built first and cut. It replaced the
  // library, so it also searches prose and deletes.
  //
  // It is an index, so it is dense: one 26px line per gloom, topics flowing in
  // columns across the width. The first cut gave every gloom a two-line card —
  // pleasant at seven glooms, a scroll at thirty. Detail is a Space away, in a
  // quick look, rather than on every row.
  import { tick } from "svelte";
  import * as ipc from "$lib/ipc";
  import type { Doc, Hub, IndexedModule, Project, ReadingFile } from "$lib/ipc";
  import { createMarkdownIt } from "$lib/markdownit";
  import { byRecent, factsOf, groupsOf, matches, moveGloom, moveTopic, type Group } from "$lib/hub";
  import { parseTags, tagHue } from "$lib/tags";
  import { when } from "$lib/when";

  let {
    project,
    data,
    files,
    index,
    known,
    labels,
    back = false,
    current = null,
    onopen,
    onnew,
    onchanged,
    ondeleted,
    onback,
  }: {
    project: Project;
    data: Hub;
    files: ReadingFile[];
    index: IndexedModule[];
    known: string[];
    labels: Map<string, string>;
    /** A gloom or a file is open behind the Hub, so there is somewhere to go back to. */
    back?: boolean;
    /** The gloom open right now, if any. */
    current?: number | null;
    /** Open a gloom; `read` goes straight into read mode. */
    onopen: (id: number, read: boolean) => void;
    onnew: () => void;
    /** Topics or places changed — reload the Hub. */
    onchanged: () => void;
    ondeleted: (id: number) => void;
    onback?: () => void;
  } = $props();

  // ---- what is shown --------------------------------------------------------

  type View = "topics" | "recent";
  const viewKey = $derived(`hubView:${project.id}`);
  let view = $state<View>("topics");
  $effect(() => {
    // Remembered per project: if you mostly go back to what you were just
    // writing, the Hub should open on Recent next time.
    try {
      view = localStorage.getItem(viewKey) === "recent" ? "recent" : "topics";
    } catch {
      /* no storage — the default */
    }
  });
  function setView(v: View) {
    view = v;
    try {
      localStorage.setItem(viewKey, v);
    } catch {
      /* not worth interrupting for */
    }
  }

  let query = $state("");
  let tag = $state<string | null>(null);
  let selected = $state<number | null>(null);
  /** The gloom in the quick look. */
  let looking = $state<number | null>(null);
  let confirmingDelete = $state(false);
  let filterEl = $state<HTMLInputElement | null>(null);

  /**
   * The arrangement on screen. Rebuilt from `data` whenever the shell reloads it,
   * and changed in place by a drag so the row lands before the save returns.
   */
  let groups = $state<Group[]>([]);
  $effect(() => {
    groups = groupsOf(data.topics, data.glooms);
  });

  const byId = $derived(new Map(data.glooms.map((g) => [g.id, g])));
  const tags = $derived([...new Set(data.glooms.flatMap((g) => parseTags(g.label)))].sort());

  // One renderer for every gloom — the note's own, so the Hub and the note can
  // never disagree about what is broken.
  const md = $derived(createMarkdownIt(files, null, known, labels));
  const facts = $derived(new Map(data.glooms.map((g) => [g.id, factsOf(g.markdown, md, index, files, labels)])));
  const brokenCount = $derived([...facts.values()].filter((f) => f.broken.length).length);

  const shown = (id: number) => {
    const g = byId.get(id);
    return !!g && matches(g, query, tag);
  };
  const recent = $derived(byRecent(data.glooms).filter((g) => matches(g, query, tag)));

  /** Every visible gloom in screen order — what ↑/↓ walk. */
  const order = $derived(
    view === "recent" ? recent.map((g) => g.id) : groups.flatMap((g) => g.ids.filter(shown)),
  );

  /**
   * Each gloom's place in the reading order, across the whole Hub — the order is
   * the path a newcomer reads, top to bottom and topic to topic. Unsorted has no
   * numbers: it is not part of the path yet.
   */
  const numbers = $derived.by(() => {
    const out = new Map<number, number>();
    let n = 0;
    for (const g of groups) if (g.topic) for (const id of g.ids) if (shown(id)) out.set(id, ++n);
    return out;
  });

  const topicName = (id: number) =>
    groups.find((g) => g.ids.includes(id))?.topic?.name ?? "Unsorted";

  // ---- changing the arrangement --------------------------------------------

  async function save(next: Group[]) {
    groups = next;
    try {
      await ipc.arrangeHub(
        project.id,
        next.flatMap((g) => (g.topic ? [g.topic.id] : [])),
        next.flatMap((g) => g.ids.map((id) => ({ id, topicId: g.topic?.id ?? null }))),
      );
    } finally {
      onchanged();
    }
  }

  let renamingTopic = $state<number | null>(null);
  let topicDraft = $state("");

  async function newTopic() {
    const t = await ipc.createTopic(project.id, "New topic");
    onchanged();
    renamingTopic = t.id;
    topicDraft = t.name;
    await tick();
    document.querySelector<HTMLInputElement>(".hub .tin")?.select();
  }

  async function commitTopic() {
    const id = renamingTopic;
    renamingTopic = null;
    if (id === null || !topicDraft.trim()) return;
    await ipc.renameTopic(id, topicDraft);
    onchanged();
  }

  async function removeTopic(id: number) {
    await ipc.deleteTopic(id);
    onchanged();
  }

  /**
   * Dragging, with pointer events rather than HTML5 drag and drop.
   *
   * HTML5 DnD was the first cut and did nothing inside the app: Tauri's window
   * has its own handler for files dropped onto it, and it takes the drag events
   * before the page sees them. Pointer events are the page's own, behave the same
   * in every webview, and give the two things DnD made awkward — a ghost that
   * follows the pointer, and a press that is still a click until it moves.
   *
   * A press becomes a drag only after 5px of movement, so clicking a row still
   * opens it; the click that ends a real drag is swallowed. The target is found
   * with `elementFromPoint` against `data-gloom` / `data-topic`, so nothing has
   * to be registered per row.
   */
  let drag = $state<{ kind: "gloom" | "topic"; id: number; label: string } | null>(null);
  let ghost = $state<{ x: number; y: number } | null>(null);
  let over = $state<{ id: number; after: boolean } | null>(null);
  let overGroup = $state<number | null | undefined>(undefined);
  let overTopic = $state<number | null>(null);
  let topicsEl = $state<HTMLElement | null>(null);

  /** The press, until it has moved far enough to be a drag. Not reactive: only the handlers read it. */
  let pressed: { kind: "gloom" | "topic"; id: number; label: string; x: number; y: number } | null = null;
  let swallowClick = false;

  function press(e: PointerEvent, kind: "gloom" | "topic", id: number, label: string) {
    if (e.button !== 0 || view !== "topics") return;
    // A press here is a drag or a click, never the start of a text selection.
    // `user-select: none` on the rows was not enough: WebKit still extends a
    // selection from the press point into whatever selectable text the pointer
    // crosses — the topic names in other columns. Cancelling the press stops the
    // selection before it begins; the click still fires.
    e.preventDefault();
    window.getSelection()?.removeAllRanges();
    pressed = { kind, id, label, x: e.clientX, y: e.clientY };
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", endDrag);
  }

  function onPointerMove(e: PointerEvent) {
    if (!pressed) return;
    if (!drag) {
      if (Math.hypot(e.clientX - pressed.x, e.clientY - pressed.y) < 5) return;
      drag = { kind: pressed.kind, id: pressed.id, label: pressed.label };
      document.body.style.cursor = "grabbing";
      // And nothing on the page is selectable while something is being carried.
      document.body.style.userSelect = "none";
      document.body.style.webkitUserSelect = "none";
    }
    ghost = { x: e.clientX, y: e.clientY };
    locate(e.clientX, e.clientY);
    // Near the top or bottom edge, the index scrolls under the pointer — a long
    // Hub must not need a drop, a scroll and a second drag.
    const box = topicsEl?.getBoundingClientRect();
    if (box && topicsEl) {
      if (e.clientY < box.top + 40) topicsEl.scrollTop -= 14;
      else if (e.clientY > box.bottom - 40) topicsEl.scrollTop += 14;
    }
  }

  /** What is under the pointer: a row to land beside, a topic to land in, or a topic to go before. */
  function locate(x: number, y: number) {
    over = null;
    overGroup = undefined;
    overTopic = null;
    if (!drag) return;
    const el = document.elementFromPoint(x, y) as HTMLElement | null;
    const section = el?.closest<HTMLElement>(".hub .topic");
    if (!section) return;
    const topic = section.dataset.topic === "unsorted" ? null : Number(section.dataset.topic);
    if (drag.kind === "topic") {
      // Unsorted is always last, so a topic can go before any other topic but not it.
      if (topic !== null && topic !== drag.id) overTopic = topic;
      return;
    }
    const row = el?.closest<HTMLElement>(".hub .row[data-gloom]");
    const id = row ? Number(row.dataset.gloom) : null;
    if (row && id !== null && id !== drag.id) {
      const r = row.getBoundingClientRect();
      over = { id, after: y > r.top + r.height / 2 };
    } else if (!row) {
      overGroup = topic;
    }
  }

  function onPointerUp() {
    const d = drag;
    if (d) {
      swallowClick = true;
      setTimeout(() => (swallowClick = false), 0);
      if (d.kind === "topic" && overTopic !== null) void save(moveTopic(groups, d.id, overTopic));
      else if (d.kind === "gloom" && over) {
        const target = over;
        const grp = groups.find((g) => g.ids.includes(target.id));
        if (grp) {
          const at = grp.ids.indexOf(target.id);
          const before = target.after
            ? (grp.ids.slice(at + 1).find((x) => x !== d.id) ?? null)
            : target.id;
          void save(moveGloom(groups, d.id, grp.topic?.id ?? null, before));
        }
      } else if (d.kind === "gloom" && overGroup !== undefined) {
        void save(moveGloom(groups, d.id, overGroup, null));
      }
    }
    endDrag();
  }

  function endDrag() {
    pressed = null;
    drag = null;
    ghost = null;
    over = null;
    overGroup = undefined;
    overTopic = null;
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
    document.body.style.webkitUserSelect = "";
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    window.removeEventListener("pointercancel", endDrag);
  }

  // A drag in flight when the Hub goes away must not leave listeners behind.
  $effect(() => () => endDrag());

  // ---- opening, looking, deleting -------------------------------------------

  function open(id: number, read = false) {
    looking = null;
    onopen(id, read);
  }

  function look(id: number) {
    selected = id;
    looking = id;
    confirmingDelete = false;
  }

  async function remove(id: number) {
    if (!confirmingDelete) {
      confirmingDelete = true;
      return;
    }
    await ipc.deleteDoc(id);
    confirmingDelete = false;
    looking = null;
    ondeleted(id);
  }

  /**
   * The Hub's keys, offered first by the shell's window handler while the Hub is
   * on screen. One handler owns the window, so the Hub answers when asked rather
   * than racing a second listener; `true` means the key was used here.
   */
  export function key(e: KeyboardEvent): boolean {
    const typing = e.target instanceof HTMLInputElement;
    if (e.metaKey || e.ctrlKey || e.altKey) return false;

    if (e.key === "Escape") {
      if (drag) {
        endDrag();
        return true;
      }
      if (renamingTopic !== null) return false; // its own input handles it
      if (looking !== null) looking = null;
      else if (typing) filterEl?.blur();
      else if (query || tag) {
        query = "";
        tag = null;
      } else if (back) onback?.();
      else return false;
      return true;
    }
    if (typing) {
      if (e.target !== filterEl) return false;
      if (e.key === "ArrowDown" || e.key === "Enter") {
        e.preventDefault();
        filterEl?.blur();
        if (e.key === "Enter" && order.length) open(selected ?? order[0]);
        else selected = order[0] ?? null;
        return true;
      }
      return false;
    }

    if (e.key === "/") {
      e.preventDefault();
      filterEl?.focus();
      return true;
    }
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const i = selected === null ? -1 : order.indexOf(selected);
      const next = order[Math.max(0, Math.min(order.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)))];
      if (next !== undefined) {
        selected = next;
        if (looking !== null) looking = next;
        void tick().then(() => document.querySelector(".hub .row.sel")?.scrollIntoView({ block: "nearest" }));
      }
      return true;
    }
    if (e.key === " " && selected !== null) {
      e.preventDefault();
      if (looking === null) look(selected);
      else looking = null;
      return true;
    }
    if (e.key === "Enter" && selected !== null) {
      e.preventDefault();
      open(selected);
      return true;
    }
    return false;
  }

  const lookingAt = $derived(looking === null ? null : (byId.get(looking) ?? null));
  const lookingFacts = $derived(looking === null ? null : (facts.get(looking) ?? null));
</script>

{#snippet row(g: Doc, lead: string | null, where: string | null)}
  {@const f = facts.get(g.id)}
  {@const issues = (f?.broken.length ?? 0) + (f?.ambiguous.length ?? 0)}
  <!-- A div, not a button: the ⓘ inside it is a button, and a button inside a
       button is invalid. -->
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div
    class="row"
    class:sel={selected === g.id}
    class:current={current === g.id}
    class:dragging={drag?.kind === "gloom" && drag.id === g.id}
    class:over-before={over?.id === g.id && !over.after}
    class:over-after={over?.id === g.id && over.after}
    data-gloom={g.id}
    title={[g.title || "Unnamed gloom", parseTags(g.label).join(", "), f?.modules.slice(0, 4).join(" · ")]
      .filter(Boolean)
      .join("\n")}
    onclick={() => !swallowClick && open(g.id)}
    onpointerdown={(e) => press(e, "gloom", g.id, g.title || "Unnamed gloom")}
  >
    {#if lead !== null}<span class="n">{lead}</span>{/if}
    <span class="title" class:untitled={!g.title}>{g.title || "Unnamed gloom"}</span>
    <span class="gap"></span>
    {#if where}<span class="where" title="Topic">{where}</span>{/if}
    <span class="tags">{parseTags(g.label).join(" · ")}</span>
    {#if f?.pending}
      <span class="checking" title="Reading the files it names">…</span>
    {:else if issues}
      <span class="bad" title="{f?.broken.length} broken, {f?.ambiguous.length} ambiguous">⚠ {issues}</span>
    {/if}
    <span class="when">{when(g.updatedAt)}</span>
    <button
      class="look"
      title="Look inside (Space)"
      onpointerdown={(e) => e.stopPropagation()}
      onclick={(e) => {
        e.stopPropagation();
        look(g.id);
      }}>i</button
    >
  </div>
{/snippet}

<div class="hub">
  {#if !data.glooms.length && !data.topics.length}
    <div class="first">
      <h3>Nothing written about {project.name} yet</h3>
      <p>
        Explore the code from the sidebar or <kbd>⌘T</kbd>. When you work something out — how a
        request gets handled, where a session starts — start a gloom. They collect here, in the order
        a newcomer should read them.
      </p>
      <button class="tbtn primary" onclick={onnew}>+ Start a gloom</button>
    </div>
  {:else}
    <div class="ihead">
      <h2>Hub</h2>
      <div class="view">
        <button class:on={view === "topics"} onclick={() => setView("topics")} title="The reading order, by topic">
          Topics
        </button>
        <button class:on={view === "recent"} onclick={() => setView("recent")} title="Every gloom, newest first">
          Recent
        </button>
      </div>
      <span class="sum">
        <b>{data.glooms.length}</b>
        {data.glooms.length === 1 ? "gloom" : "glooms"} · <b>{data.topics.length}</b>
        {data.topics.length === 1 ? "topic" : "topics"}
        {#if brokenCount}· <span class="badsum">{brokenCount} with broken references</span>{/if}
      </span>
      <span class="spacer"></span>
      <input
        class="q"
        bind:this={filterEl}
        bind:value={query}
        placeholder="Filter by name, tag or prose…  /"
        spellcheck="false"
      />
      {#if view === "topics"}
        <button class="tbtn" onclick={newTopic}>+ Topic</button>
      {/if}
      <button class="tbtn primary" onclick={onnew}>+ New gloom</button>
      {#if back}
        <button class="tbtn" onclick={() => onback?.()} title="Back to what you had open (esc)">← Back</button>
      {/if}
      {#if tags.length}
        <div class="filters">
          {#each tags as t (t)}
            <button
              class="ftag"
              class:on={tag === t}
              class:off={tag !== null && tag !== t}
              style="--hue:{tagHue(t)}"
              onclick={() => (tag = tag === t ? null : t)}>{t}</button
            >
          {/each}
        </div>
      {/if}
    </div>

    {#if view === "recent"}
      <!-- One list, newest first, every topic mixed. No numbers — a place in the
           reading order means nothing here — and no dragging, because the order
           is time's, not yours. The topic is named on the row instead. -->
      <div class="recent">
        <div class="list">
          {#each recent as g (g.id)}
            {@render row(g, null, topicName(g.id))}
          {:else}
            <p class="none">Nothing matches.</p>
          {/each}
        </div>
      </div>
    {:else}
      <div class="topics" bind:this={topicsEl}>
        {#each groups as grp (grp.topic?.id ?? "unsorted")}
          {@const visible = grp.ids.filter(shown)}
          {#if !(query || tag) || visible.length}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <section
              class="topic"
              class:unsorted={!grp.topic}
              class:dragging={drag?.kind === "topic" && drag.id === grp.topic?.id}
              class:over-topic={overTopic !== null && overTopic === grp.topic?.id}
              data-topic={grp.topic?.id ?? "unsorted"}
            >
              <div class="thead">
                {#if grp.topic}
                  <span
                    class="grab"
                    title="Drag to reorder topics"
                    role="presentation"
                    onpointerdown={(e) => press(e, "topic", grp.topic!.id, grp.topic!.name)}>⋮⋮</span
                  >
                {:else}
                  <span class="grab"></span>
                {/if}
                {#if grp.topic && renamingTopic === grp.topic.id}
                  <input
                    class="tin"
                    bind:value={topicDraft}
                    spellcheck="false"
                    onblur={commitTopic}
                    onkeydown={(e) => {
                      e.stopPropagation();
                      if (e.key === "Enter") commitTopic();
                      else if (e.key === "Escape") renamingTopic = null;
                    }}
                  />
                {:else if grp.topic}
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <span
                    class="tname"
                    title="Double-click to rename"
                    ondblclick={async () => {
                      renamingTopic = grp.topic!.id;
                      topicDraft = grp.topic!.name;
                      await tick();
                      document.querySelector<HTMLInputElement>(".hub .tin")?.select();
                    }}>{grp.topic.name}</span
                  >
                {:else}
                  <span class="tname" title="New glooms land here">Unsorted</span>
                {/if}
                <span class="tcount">{grp.ids.length}</span>
                <span class="spacer"></span>
                {#if grp.topic}
                  <button
                    class="tdel"
                    title="Remove the topic — its glooms move to Unsorted"
                    onclick={() => removeTopic(grp.topic!.id)}>remove</button
                  >
                {/if}
              </div>
              <div class="rows" class:over-empty={overGroup === (grp.topic?.id ?? null) && drag?.kind === "gloom"}>
                {#each visible as id (id)}
                  {@const g = byId.get(id)}
                  {#if g}
                    {@render row(g, grp.topic ? String(numbers.get(id) ?? "") : "·", null)}
                  {/if}
                {:else}
                  <p class="none">{grp.topic ? "Drag a gloom here." : "Everything has a topic."}</p>
                {/each}
              </div>
            </section>
          {/if}
        {/each}
      </div>
    {/if}
  {/if}

  {#if drag && ghost}
    <!-- What you are carrying, under the pointer. Never a hit target itself, or
         `elementFromPoint` would find the ghost instead of the row beneath it. -->
    <div class="ghost" style:left="{ghost.x + 12}px" style:top="{ghost.y + 8}px">{drag.label}</div>
  {/if}

  {#if lookingAt && lookingFacts}
    {@const f = lookingFacts}
    <!-- Everything the row leaves out. The lead is the note's own render, so its
         references look exactly as they will once it is open. -->
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="qscrim" onclick={(e) => e.target === e.currentTarget && (looking = null)}>
      <div class="quick">
        <div class="crumb">{topicName(lookingAt.id)}</div>
        <h3 class:untitled={!lookingAt.title}>{lookingAt.title || "Unnamed gloom"}</h3>
        <div class="ptags">
          {#each parseTags(lookingAt.label) as t (t)}
            <span class="rtag" style="--hue:{tagHue(t)}">{t}</span>
          {/each}
          <span>edited {when(lookingAt.updatedAt)}</span>
        </div>
        {#if f.pending}
          <div class="issues quiet">Reading the files it names…</div>
        {:else if f.broken.length || f.ambiguous.length}
          <div class="issues">
            {#each f.broken as r (r)}<p><code class="ref broken">{r}</code> — no longer in the code</p>{/each}
            {#each f.ambiguous as r (r)}<p><code class="ref ambiguous">{r}</code> — names more than one module</p>{/each}
          </div>
        {/if}
        {#if f.modules.length}
          <div class="walk">
            {#each f.modules as m, i (m)}{#if i}<span> → </span>{/if}{m}{/each}
          </div>
        {/if}
        <div class="prose">
          {#if f.lead}
            {@html f.lead}
          {:else}
            <p class="fade">Nothing written yet.</p>
          {/if}
        </div>
        <div class="acts">
          <button class="tbtn primary" onclick={() => open(lookingAt.id)}>Open</button>
          {#if f.refs}
            <button class="read" onclick={() => open(lookingAt.id, true)}>▷ Gloom</button>
          {/if}
          <button class="del" class:armed={confirmingDelete} onclick={() => remove(lookingAt.id)}>
            {confirmingDelete ? "Delete it — no code is touched" : "Delete…"}
          </button>
          <span class="hint">space or esc to close</span>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .hub {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    position: relative;
    background: var(--code-bg);
    color: var(--fg);
  }

  .ihead {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 22px 10px;
    border-bottom: 1px solid var(--line);
    flex-wrap: wrap;
  }
  .ihead h2 {
    margin: 0;
    font-family: var(--display);
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.24em;
    text-transform: uppercase;
    color: var(--gloom-deep);
  }
  .view {
    display: flex;
    border: 1px solid var(--line);
    border-radius: 6px;
    overflow: hidden;
  }
  .view button {
    font: inherit;
    font-size: 11px;
    color: var(--fg-dim);
    background: transparent;
    border: 0;
    padding: 3px 10px;
    cursor: pointer;
  }
  .view button + button {
    border-left: 1px solid var(--line);
  }
  .view button.on {
    background: color-mix(in srgb, var(--gloom-deep) 12%, transparent);
    color: var(--gloom-deep);
    font-weight: 600;
  }
  .sum {
    font-size: 11.5px;
    color: var(--fg-faint);
  }
  .sum b {
    color: var(--fg-dim);
    font-weight: 600;
  }
  .badsum {
    color: var(--priv);
  }
  .spacer {
    flex: 1;
  }
  .q {
    width: 240px;
    font: inherit;
    font-size: 12px;
    color: var(--fg);
    padding: 4px 9px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--bg-raised);
    outline: none;
  }
  .q:focus {
    border-color: var(--gloom-deep);
  }
  .tbtn {
    font: inherit;
    font-size: 11.5px;
    color: var(--fg-dim);
    background: transparent;
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 3px 9px;
    cursor: pointer;
    white-space: nowrap;
  }
  .tbtn:hover {
    color: var(--gloom-deep);
    border-color: var(--gloom-deep);
  }
  .tbtn.primary {
    background: var(--gloom-deep);
    border-color: var(--gloom-deep);
    color: var(--bg);
    font-weight: 600;
  }
  .tbtn.primary:hover {
    filter: brightness(1.1);
    color: var(--bg);
  }
  .filters {
    width: 100%;
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  /* The tag's own hue, the same everywhere — band, row tooltip, quick look. */
  .ftag,
  .rtag {
    font: inherit;
    font-family: var(--mono);
    font-size: 10px;
    line-height: 17px;
    padding: 0 7px;
    border: 0;
    border-radius: 999px;
    color: oklch(0.45 0.12 var(--hue));
    background: oklch(0.45 0.12 var(--hue) / 0.12);
  }
  :global(.dark) .ftag,
  :global(.dark) .rtag {
    color: oklch(0.86 0.09 var(--hue));
    background: oklch(0.86 0.09 var(--hue) / 0.14);
  }
  .ftag {
    cursor: pointer;
  }
  .ftag.on {
    color: var(--bg);
    background: oklch(0.5 0.12 var(--hue));
  }
  .ftag.off {
    opacity: 0.4;
  }

  /* Topics flow in columns across the width: an index you take in at a glance,
     not a list you scroll. */
  .topics {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 6px 22px 40px;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(330px, 1fr));
    gap: 4px 28px;
    align-content: start;
  }
  .topic {
    min-width: 0;
    padding-top: 10px;
  }
  .topic.dragging {
    opacity: 0.4;
  }
  .topic.over-topic {
    box-shadow: inset 3px 0 0 var(--gloom-deep);
  }
  .thead {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    border-bottom: 1px solid var(--line);
    margin-bottom: 2px;
  }
  .grab {
    flex: none;
    width: 12px;
    touch-action: none;
    font-size: 10px;
    letter-spacing: -2px;
    color: var(--fg-faint);
    cursor: grab;
    user-select: none;
    opacity: 0;
  }
  .topic:hover .grab {
    opacity: 1;
  }
  .tname {
    min-width: 0;
    font-family: var(--serif);
    font-size: 14.5px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .unsorted .tname {
    font-style: italic;
    font-weight: 500;
    color: var(--fg-dim);
  }
  .tin {
    flex: 1;
    min-width: 0;
    font-family: var(--serif);
    font-size: 14.5px;
    font-weight: 600;
    color: var(--fg);
    background: transparent;
    border: 0;
    border-bottom: 1.5px solid var(--gloom-deep);
    outline: none;
    padding: 0;
  }
  .tcount {
    font-family: var(--mono);
    font-size: 10px;
    color: var(--fg-faint);
  }
  .tdel {
    font: inherit;
    font-size: 10.5px;
    color: var(--fg-faint);
    background: transparent;
    border: 0;
    cursor: pointer;
    opacity: 0;
  }
  .topic:hover .tdel {
    opacity: 1;
  }
  .tdel:hover {
    color: var(--priv);
  }
  .rows {
    min-height: 10px;
    border-radius: 5px;
  }
  .rows.over-empty {
    background: color-mix(in srgb, var(--gloom-deep) 8%, transparent);
  }
  .none {
    margin: 0;
    padding: 4px 0 4px 26px;
    font-size: 11px;
    font-style: italic;
    color: var(--fg-faint);
  }

  /* One line per gloom. The number is its place in the reading order. */
  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 7px;
    height: 26px;
    padding: 0 4px;
    border-radius: 5px;
    cursor: pointer;
  }
  .row:hover {
    background: var(--bg-inset);
  }
  .row.sel {
    background: color-mix(in srgb, var(--gloom-deep) 11%, transparent);
  }
  .row.dragging {
    opacity: 0.35;
  }
  /* Rows and handles are pressed to drag, so a press must not start selecting
     the titles instead. */
  .row,
  .grab {
    user-select: none;
    -webkit-user-select: none;
  }
  .ghost {
    position: fixed;
    z-index: 50;
    max-width: 320px;
    padding: 4px 10px;
    font-family: var(--serif);
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--fg);
    background: var(--bg-raised);
    border: 1px solid var(--gloom-deep);
    border-radius: 6px;
    box-shadow: 0 8px 20px color-mix(in srgb, black 20%, transparent);
    pointer-events: none;
  }
  .row.over-before::before,
  .row.over-after::after {
    content: "";
    position: absolute;
    left: 24px;
    right: 4px;
    height: 2px;
    border-radius: 1px;
    background: var(--gloom-deep);
  }
  .row.over-before::before {
    top: -1px;
  }
  .row.over-after::after {
    bottom: -1px;
  }
  .n {
    flex: none;
    width: 16px;
    font-family: var(--mono);
    font-size: 10px;
    text-align: right;
    color: var(--fg-faint);
  }
  .title {
    flex-shrink: 0;
    max-width: 72%;
    min-width: 0;
    font-family: var(--serif);
    font-size: 13.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .title.untitled {
    font-style: italic;
    color: var(--fg-faint);
  }
  .row.sel .title,
  .row.current .title {
    color: var(--gloom-deep);
    font-weight: 600;
  }
  .gap {
    flex: 1;
  }
  /* Tags as their words, small and grey. They were coloured dots in the mockup's
     first cut, and nobody could read them without a legend. Words need no key;
     they give way before the name does. */
  .tags {
    flex: 0 1 auto;
    min-width: 0;
    max-width: 38%;
    font-family: var(--mono);
    font-size: 10px;
    color: var(--fg-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .where {
    flex: none;
    max-width: 150px;
    margin-left: 8px;
    font-size: 10.5px;
    color: var(--fg-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* Health only when something is wrong: a tick on every row would be a wall. */
  .bad {
    flex: none;
    font-size: 10px;
    font-weight: 600;
    color: var(--priv);
  }
  .checking {
    flex: none;
    font-size: 10px;
    color: var(--fg-faint);
  }
  .when {
    flex: none;
    width: 58px;
    text-align: right;
    font-size: 10px;
    color: var(--fg-faint);
    font-variant-numeric: tabular-nums;
  }
  .look {
    flex: none;
    width: 16px;
    height: 16px;
    padding: 0;
    font: inherit;
    font-size: 10px;
    line-height: 14px;
    color: var(--fg-faint);
    background: var(--code-bg);
    border: 1px solid var(--line);
    border-radius: 50%;
    cursor: pointer;
    opacity: 0;
  }
  .row:hover .look,
  .row.sel .look {
    opacity: 1;
  }
  .look:hover {
    color: var(--gloom-deep);
    border-color: var(--gloom-deep);
  }

  .recent {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 10px 22px 40px;
  }
  .recent .list {
    max-width: 860px;
  }
  .recent .row {
    height: 28px;
    padding: 0 8px;
  }

  .first {
    max-width: 520px;
    margin: 70px auto;
    text-align: center;
    color: var(--fg-dim);
    line-height: 1.6;
  }
  .first h3 {
    margin: 0 0 8px;
    font-family: var(--serif);
    font-size: 22px;
    color: var(--fg);
  }
  .first kbd {
    font-family: var(--mono);
    font-size: 11px;
  }
  .first .tbtn {
    margin-top: 14px;
  }

  /* The quick look sits over the Hub only, on the doc pane's paper. */
  .qscrim {
    position: absolute;
    inset: 0;
    z-index: 5;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, black 18%, transparent);
  }
  .quick {
    width: min(540px, calc(100% - 40px));
    max-height: 80%;
    overflow: auto;
    padding: 18px 22px 18px;
    color: var(--doc-fg);
    background: var(--doc-bg);
    border: 1px solid var(--doc-line);
    border-radius: 10px;
    box-shadow: 0 20px 50px color-mix(in srgb, black 28%, transparent);
    animation: lookIn var(--fast) var(--ease-out) backwards;
  }
  @keyframes lookIn {
    from {
      opacity: 0;
      transform: scale(0.98);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .quick {
      animation: lookFade var(--fast) ease backwards;
    }
    @keyframes lookFade {
      from {
        opacity: 0;
      }
    }
  }
  .crumb {
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--fg-faint);
  }
  .quick h3 {
    margin: 4px 0 8px;
    font-family: var(--serif);
    font-size: 20px;
    line-height: 1.25;
  }
  .quick h3.untitled {
    font-style: italic;
    color: var(--fg-faint);
  }
  .ptags {
    display: flex;
    align-items: center;
    gap: 5px;
    margin-bottom: 12px;
    font-size: 10.5px;
    color: var(--fg-faint);
  }
  .issues {
    margin-bottom: 10px;
    padding: 2px 0 2px 10px;
    border-left: 3px solid var(--priv);
    font-size: 11.5px;
    line-height: 1.55;
  }
  .issues.quiet {
    border-color: var(--line);
    color: var(--fg-faint);
  }
  .issues p {
    margin: 0;
  }
  .walk {
    margin-bottom: 10px;
    font-family: var(--mono);
    font-size: 11px;
    line-height: 1.7;
    color: var(--gloom-deep);
  }
  .walk span {
    color: var(--fg-faint);
  }
  .prose {
    font-size: 13.5px;
    line-height: 1.6;
  }
  .prose :global(p) {
    margin: 0 0 8px;
  }
  .prose .fade {
    font-size: 12px;
    font-style: italic;
    color: var(--fg-faint);
  }
  .quick :global(code.ref) {
    font-family: var(--mono);
    font-size: 0.86em;
    padding: 0 3px;
    border-radius: 3px;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }
  .quick :global(code.ref.broken) {
    color: var(--priv);
    background: color-mix(in srgb, var(--priv) 12%, transparent);
    text-decoration: line-through;
  }
  .quick :global(code.ref.ambiguous) {
    color: var(--priv);
    background: transparent;
    border: 1px dashed color-mix(in srgb, var(--priv) 60%, transparent);
  }
  .acts {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 14px;
  }
  .read {
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    color: var(--read-ink);
    background: var(--read);
    border: 0;
    border-radius: 6px;
    padding: 4px 11px;
    cursor: pointer;
  }
  .del {
    font: inherit;
    font-size: 11.5px;
    color: var(--fg-faint);
    background: transparent;
    border: 0;
    cursor: pointer;
  }
  .del:hover,
  .del.armed {
    color: var(--priv);
  }
  .del.armed {
    font-weight: 600;
  }
  .hint {
    margin-left: auto;
    font-size: 10.5px;
    color: var(--fg-faint);
  }
</style>
