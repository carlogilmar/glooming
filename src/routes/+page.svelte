<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import CodePane from "$lib/components/CodePane.svelte";
  import DocPane from "$lib/components/DocPane.svelte";
  import Divider from "$lib/components/Divider.svelte";
  import Library from "$lib/components/Library.svelte";
  import FnPalette from "$lib/components/FnPalette.svelte";
  import HelpModal from "$lib/components/HelpModal.svelte";
  import FilePalette from "$lib/components/FilePalette.svelte";
  import FilesModal from "$lib/components/FilesModal.svelte";
  import ExploreSections from "$lib/components/ExploreSections.svelte";
  import { byPath, origin as originOf } from "$lib/fileset";
  import { displaySig, locate } from "$lib/select";
  import { when } from "$lib/when";
  import { theme } from "$lib/stores/theme.svelte";
  import { focus } from "$lib/stores/focus.svelte";
  import * as ipc from "$lib/ipc";
  import type { Doc, DocSummary, OpenedFile, ReadingFile } from "$lib/ipc";

  /**
   * A reading is a set of files, not one.
   *
   * `files` is the single source of truth for the left pane — including during
   * the moment before a doc exists, when it holds exactly one synthesized entry.
   * One shape everywhere was worth more than saving that conversion: everything
   * downstream asks "which file am I looking at" and gets the same answer.
   */
  let files = $state<ReadingFile[]>([]);
  let currentPath = $state<string | null>(null);
  let doc = $state<Doc | null>(null);
  /** Paths the prose actually references — the strip's hollow-dot signal. */
  let referenced = $state<Set<string>>(new Set());
  /** The doc pane instance, so ⌘R and ⌘E can reach its mode toggles. */
  let docPane = $state<DocPane | null>(null);
  /** Read mode takes the whole pane; the reference sections stand down. */
  let readingNow = $state(false);
  /** ⌘⇧T — the reading's own files: switch, add, remove. */
  let showReadingFiles = $state(false);
  /**
   * Bumped when a *different* reading opens — the doc pane's cue to play its
   * arrival animations once.
   *
   * It has to be a separate token rather than "did `doc` change", because adding
   * or removing a file also replaces `doc`, and re-cascading the whole surface
   * block because you opened one more file is exactly the kind of restlessness
   * that gets animation turned off.
   */
  let opened = $state(0);
  let markdown = $state("");
  let basis = $state(52);
  let saving = $state(false);
  let dirty = $state(false);
  let error = $state<string | null>(null);
  let showLibrary = $state(false);
  /** ⌘T — search the open folder by name, or paste a path. */
  let showFiles = $state(false);
  /** The folder being searched. Remembered, so it is picked once per project. */
  let project = $state<ipc.Project | null>(null);
  let projects = $state<ipc.Project[]>([]);
  /** ⌘P — jump to a function by name. */
  let showPalette = $state(false);
  /** `?` — what everything does. */
  let showHelp = $state(false);
  /** The last few readings, offered on the welcome screen for a one-click reopen. */
  let recents = $state<DocSummary[]>([]);

  // ---- opening takes a moment, and the moment should be honest -------------
  let loading = $state(false);
  let loadingFile = $state("");
  let loadingStep = $state("");
  let loadTimer: ReturnType<typeof setTimeout> | null = null;

  /**
   * Announce a load, but not immediately.
   *
   * Parsing a small file is instant; showing a loader for 40ms is a flash of
   * nothing, which reads worse than no loader at all. So it only surfaces once
   * the work has actually taken long enough to be worth explaining — which in
   * practice means a big file, or `git log` on a long history.
   */
  function beginLoad(path: string) {
    loadingFile = path.split("/").pop() ?? path;
    loadingStep = "Reading the file";
    if (loadTimer) clearTimeout(loadTimer);
    loadTimer = setTimeout(() => (loading = true), 150);
  }

  function endLoad() {
    if (loadTimer) clearTimeout(loadTimer);
    loadTimer = null;
    loading = false;
    loadingStep = "";
  }
  /** Existing docs for a just-opened path, awaiting your choice. */
  let chooser = $state<DocSummary[] | null>(null);

  const file = $derived(byPath(files, currentPath) ?? files[0] ?? null);
  const origin = $derived(originOf(files));

  /**
   * The gloom's name.
   *
   * A gloom is one revision journey — a note and the files it led you through —
   * and it is the only thing in the app that needs a name of its own: the files
   * already have theirs, and "accounts.ex" says nothing about *why* you opened
   * it. `docs.title` has existed since the first migration as "seeded from the
   * module name, editable"; this is the editing.
   *
   * `null` when not renaming, so an empty string is a real value you can be
   * holding mid-edit.
   */
  let renaming = $state<string | null>(null);
  let renameEl = $state<HTMLInputElement | null>(null);
  $effect(() => {
    renameEl?.select();
  });

  /**
   * What the title would be if you never touched it.
   *
   * Compared rather than stored, because a flag would have to be maintained by
   * every path that writes a title and this cannot drift. An untouched name is
   * shown as an invitation — the same idiom as the Explain section's, one level
   * up: nothing is missing, there is just something worth doing.
   */
  const seededTitle = $derived(
    origin?.outline?.tests?.module ??
      origin?.outline?.modules?.[0]?.name ??
      origin?.filename ??
      "",
  );

  /**
   * The moment a gloom stops being called after its module.
   *
   * Diffed against the *previous* title, so this fires on the transition rather
   * than on the state — and the first paint of a gloom you named last week is
   * seeded silently, because that is not an achievement. Exactly the file dot's
   * rule, one level up. `lastTitle` is a plain `let`: bookkeeping only this
   * effect reads, and a `$state` read and written in one effect is a freeze.
   */
  let named = $state(false);
  let lastTitle = "";
  $effect(() => {
    const now = doc?.title ?? "";
    const was = lastTitle;
    lastTitle = now;
    if (!was || !now || now === was) return;
    named = true;
    const off = setTimeout(() => (named = false), 900);
    return () => clearTimeout(off);
  });

  function startRename() {
    if (doc) renaming = doc.title;
  }

  async function commitRename() {
    if (!doc || renaming === null) return;
    const title = renaming.trim();
    renaming = null;
    if (!title || title === doc.title) return;
    // An autosave may be in flight, so the server's markdown is not necessarily
    // the sentence you are half-way through typing. Same one-payload habit as
    // every other mutation here: take the row, put your own markdown back.
    const keep = markdown;
    try {
      doc = { ...(await ipc.saveDoc({ id: doc.id, title })), markdown: keep };
    } catch (e) {
      error = String(e);
    }
  }
  const outline = $derived(file?.outline ?? null);
  /**
   * Staleness is per-file now, which is the whole reason there is a snapshot per
   * file. The reconcile button in the note still belongs to the origin, though —
   * it is the only file with an `lgtm:functions` block to merge.
   */
  const stale = $derived(!!origin?.stale);
  const currentStale = $derived(!!file && !file.origin && (file.stale || file.missing));
  /**
   * The dot on the file button: the state of the file you are in.
   *
   * Hollow means your prose has not mentioned it — the same nudge an unwritten
   * Explain section is, and the one signal the tab strip was carrying that the
   * button has to keep.
   */
  const fileState = $derived(
    !file ? "" : file.missing || file.stale ? "stale" : referenced.has(file.path) ? "" : "unread",
  );

  /**
   * The dot earns its colour: it fills, and one ring leaves.
   *
   * A hollow dot means "you opened this and never wrote about it", and the moment
   * your prose first names something in the file that stops being true. A state
   * change *you* caused is the one place a small reward is honest — and it moved
   * here from the tab strip along with everything else the strip was carrying.
   *
   * `seenRefs` is **not** `$state`: an effect that reads and writes the same
   * reactive value re-triggers itself forever, which froze the whole window once
   * already. Read reactive state or write it, never both.
   */
  let seenRefs: Set<string> | null = null;
  let earnTimer: ReturnType<typeof setTimeout> | null = null;
  let earnedPath = $state<string | null>(null);

  $effect(() => {
    const now = referenced; // the only reactive read in here
    const before = seenRefs;
    seenRefs = new Set(now);
    // First paint of an already-written note is not an achievement.
    if (before === null) return;

    const fresh = [...now].filter((p) => !before.has(p));
    if (!fresh.length) return;

    earnedPath = fresh[0];
    if (earnTimer) clearTimeout(earnTimer);
    earnTimer = setTimeout(() => (earnedPath = null), 900);
  });
  const moduleCount = $derived(outline?.modules?.length ?? 0);

  /**
   * Switching file fades the pane rather than crossfading line ranges.
   *
   * Within one file the outgoing range lingers under the incoming one, and that
   * overlap is what makes a jump read as a connection. Across two different
   * files there is nothing shared to fade between, so the same treatment reads
   * as a glitch — the pane dips instead, and a badge names where you landed.
   */
  let swapping = $state(false);
  let swapped = $state("");
  let swapTimer: ReturnType<typeof setTimeout> | null = null;

  async function showFile(path: string) {
    if (!path || path === currentPath) return;
    swapping = true;
    await new Promise((r) => setTimeout(r, 130));
    currentPath = path;
    swapping = false;
    swapped = path.split("/").pop() ?? path;
    if (swapTimer) clearTimeout(swapTimer);
    swapTimer = setTimeout(() => (swapped = ""), 1100);
  }

  /** A click on a tab is a move out of whatever was selected in the old file. */
  async function switchTo(path: string) {
    focus.clear();
    await showFile(path);
  }

  /**
   * Follow a call from the drawer into another file of the reading.
   *
   * Switch first, then select — a span means nothing over the wrong source. This
   * is the one gesture that makes a multi-file reading feel like one thing rather
   * than several files open at once.
   */
  async function jumpTo(path: string, line: number) {
    await showFile(path);
    const f = byPath(files, path);
    const fn = f?.outline?.modules?.[0]?.functions.find((x) => x.line === line);
    if (fn) selectFunction(fn);
    else focus.gotoLine(line, (f?.source.split("\n").length ?? line) || line);
  }

  /** Pick something in the reference sections: focus it in the code beside it. */
  function selectFromExplore(sig: string, line: number) {
    const at = locate(sig, outline?.modules?.[0] ?? null);
    if (at) focus.select(sig, at.ranges, at.related, at.spec, at.doc);
    else focus.gotoLine(line, file?.source.split("\n").length ?? line);
  }

  /** The pre-doc moment still has to feed the code pane, so it gets one entry. */
  function asReadingFile(o: OpenedFile): ReadingFile {
    return {
      path: o.path,
      filename: o.filename,
      lang: o.lang,
      source: o.source,
      sourceSha: o.sourceSha,
      snapshotSha: o.sourceSha,
      stale: false,
      missing: false,
      outline: o.outline,
      hasGit: o.hasGit,
      branch: o.branch,
      origin: true,
    };
  }

  /** Adopt a whole reading in one go — every mutation returns one of these. */
  function adopt(r: ipc.Reading, prefer?: string | null) {
    if (doc?.id !== r.doc.id) opened += 1;
    doc = r.doc;
    files = r.files;
    markdown = r.doc.markdown;
    dirty = false;
    const want = prefer && r.files.some((f) => f.path === prefer) ? prefer : null;
    currentPath = want ?? originOf(r.files)?.path ?? null;
  }

  const loadRecents = () =>
    ipc
      .listDocs(undefined, 6)
      .then((r) => (recents = r))
      .catch(() => (recents = []));

  // The welcome screen is the only place these are shown, so only load them
  // while it's up. Deleting from the library has to say so explicitly — it
  // changes the same query without touching any state this effect reads.
  $effect(() => {
    if (files.length) return;
    loadRecents();
  });

  // ---- opening ------------------------------------------------------------

  async function pickFile() {
    const picked = await openDialog({
      multiple: false,
      filters: [{ name: "Elixir", extensions: ["ex", "exs"] }],
    });
    if (typeof picked === "string") await load(picked);
  }

  /** Pick a folder to search. Once per project, not once per file. */
  async function chooseProject() {
    const picked = await openDialog({ directory: true, multiple: false });
    if (typeof picked !== "string") return;
    try {
      project = await ipc.openProject(picked);
      projects = await ipc.recentProjects();
      // Land straight in the search: picking a folder and being returned to an
      // unchanged screen leaves you wondering whether it worked.
      showFiles = true;
    } catch (e) {
      error = String(e);
    }
  }

  async function useProject(p: ipc.Project) {
    project = await ipc.openProject(p.path).catch(() => p);
    projects = await ipc.recentProjects();
    showFiles = true;
  }

  $effect(() => {
    ipc
      .recentProjects()
      .then((r) => {
        projects = r;
        // The last folder is almost always the one you want again.
        if (!project && r.length) project = r[0];
      })
      .catch(() => (projects = []));
  });

  /**
   * Open a file.
   *
   * **With a reading already open, this adds to it.** That is the whole flow the
   * feature exists for: during a review you open file after file, and those files
   * *are* the group — there is no separate gesture for creating one. Opening
   * something unrelated by accident is undone by the `×` on its tab, which is
   * cheaper than being asked "same reading or new one?" every single time. The
   * deliberate out is ← Home, which starts cold.
   */
  async function load(path: string) {
    error = null;
    if (doc) return addFile(path);

    focus.clear();
    beginLoad(path);
    try {
      const opened = await ipc.openFile(path);
      files = [asReadingFile(opened)];
      currentPath = opened.path;
      if (opened.existing.length) {
        // Your past reading of this file is worth more than a fresh start — and
        // that includes a reading where this file is one of several, not the one
        // it started from.
        chooser = opened.existing;
        doc = null;
        markdown = "";
      } else {
        chooser = null;
        await startFreshDoc(files[0]);
      }
    } catch (e) {
      error = String(e);
    } finally {
      endLoad();
    }
  }

  /**
   * Bring a file into the open reading. Nothing is seeded: it contributes source
   * to read and functions the `/` menu can offer, and leaves your prose alone.
   */
  async function addFile(path: string) {
    if (!doc) return;
    focus.clear();
    beginLoad(path);
    loadingStep = "Adding it to this reading";
    try {
      const r = await ipc.addDocFile(doc.id, path);
      // The markdown is the one thing not to adopt here: an autosave may be in
      // flight, and replacing what you are typing with the server's copy would
      // lose the sentence you are in the middle of.
      const pending = markdown;
      adopt(r, path);
      markdown = pending;
      currentPath = path;
    } catch (e) {
      error = String(e);
    } finally {
      endLoad();
    }
  }

  async function removeFile(path: string) {
    if (!doc) return;
    try {
      const r = await ipc.removeDocFile(doc.id, path);
      const pending = markdown;
      adopt(r, path === currentPath ? null : currentPath);
      markdown = pending;
      focus.clear();
    } catch (e) {
      error = String(e);
    }
  }

  /** Accept the file's current state as what you read. */
  async function acceptCurrent() {
    if (!doc || !file) return;
    try {
      const r = await ipc.resnapshotDocFile(doc.id, file.path);
      const pending = markdown;
      adopt(r, file.path);
      markdown = pending;
    } catch (e) {
      error = String(e);
    }
  }

  /** Chooser button: seeding and saving take the same beat as a fresh open. */
  async function startFreshFrom(opened: ReadingFile) {
    beginLoad(opened.path);
    try {
      await startFreshDoc(opened);
    } catch (e) {
      error = String(e);
    } finally {
      endLoad();
    }
  }

  async function startFreshDoc(opened: ReadingFile) {
    // Named per IPC call, so the message is what is actually happening rather
    // than a spinner pretending. Seeding is the slow one: it shells out to
    // `git log --follow`, which on a long history is the whole wait.
    loadingStep = "Reading history and seeding the doc";
    const seeded = opened.outline
      ? await ipc.seedDoc(opened.path, opened.outline, opened.source)
      : `# ${opened.filename}\n\n> _what is this file for?_\n`;

    loadingStep = "Saving";
    doc = await ipc.createDoc({
      path: opened.path,
      lang: opened.lang ?? "text",
      // A test suite names itself; a config or a script has only its filename.
      title:
        opened.outline?.tests?.module ??
        opened.outline?.modules?.[0]?.name ??
        opened.filename,
      branch: opened.branch,
      markdown: seeded,
      source: opened.source,
    });
    markdown = doc.markdown;
    dirty = false;
    chooser = null;

    // Re-read the reading rather than trusting the local shape: the origin's
    // file row is created server-side, and this is the one call that returns it.
    adopt(await ipc.openReading(doc.id), opened.path);

    // A NEW gloom opens with its name selected, so naming it costs a sentence
    // and skipping it costs one key. This is the only moment it happens: on a
    // gloom you already named, stealing the caret would be an interruption
    // rather than an invitation.
    renaming = doc.title;
  }

  async function openExisting(summary: DocSummary, prefer?: string | null) {
    beginLoad(summary.path);
    try {
      await openExistingInner(summary, prefer);
    } finally {
      endLoad();
    }
  }

  /**
   * `prefer` is the path you arrived by. Choosing a reading of accounts.ex from
   * having just opened billing.ex should land you on billing.ex — you asked for
   * that file, the reading is only how you are going to read it.
   */
  async function openExistingInner(summary: DocSummary, prefer?: string | null) {
    loadingStep = "Re-reading the files";
    const r = await ipc.openReading(summary.id);
    adopt(r, prefer ?? null);
    chooser = null;

    // A reading survives its files moving: each one carries its own snapshot,
    // which is exactly what gets shown when the path no longer resolves.
    const gone = r.files.filter((f) => f.missing).map((f) => f.filename);
    if (gone.length) {
      error =
        `Not on disk any more: ${gone.join(", ")} — showing the snapshot saved ` +
        `with this reading.`;
    }
  }

  /**
   * Back to the welcome screen.
   *
   * Flushes first: autosave is on an 800ms debounce, so leaving mid-sentence
   * would otherwise fire the timer into a doc that is no longer open and lose
   * the last thing you typed. `doc` is cleared before `markdown` so the
   * autosave effect sees no doc rather than an empty one.
   */
  async function goHome() {
    if (timer) clearTimeout(timer);
    await save();

    focus.clear();
    files = [];
    currentPath = null;
    doc = null;
    markdown = "";
    chooser = null;
    dirty = false;
    error = null;
    referenced = new Set();
  }

  /** Re-read every file in the reading from disk, and re-check staleness. */
  /**
   * A reading was deleted in the library.
   *
   * The recents list is a cached query and the query's answer just changed, so it
   * is reloaded. And if the deleted reading is the one on screen, it has to go:
   * leaving it open means the next autosave writes to a row that no longer
   * exists, which surfaces as a "not found" banner several seconds later with no
   * obvious cause.
   */
  function onDocDeleted(id: number) {
    loadRecents();
    if (doc?.id !== id) return;

    // Cleared without saving, deliberately — there is nothing left to save to,
    // and `doc` goes first so the autosave effect sees no doc rather than an
    // empty one.
    if (timer) clearTimeout(timer);
    focus.clear();
    doc = null;
    markdown = "";
    files = [];
    currentPath = null;
    chooser = null;
    dirty = false;
    referenced = new Set();
  }

  async function reparseNow() {
    if (doc) {
      const pending = markdown;
      adopt(await ipc.openReading(doc.id), currentPath);
      markdown = pending;
      return;
    }
    if (file) {
      files = [asReadingFile(await ipc.reparse(file.path))];
      currentPath = files[0].path;
    }
  }

  /**
   * Reconcile the origin. Only it has an `lgtm:functions` block — every other
   * file joined the reading without being seeded — so it is the only one there
   * is anything to merge.
   */
  async function reconcile() {
    if (!doc || !origin?.outline) return;
    await ipc.reconcileDoc(doc.id, origin.outline, origin.source);
    const r = await ipc.openReading(doc.id);
    adopt(r, currentPath);
  }

  // ---- autosave -----------------------------------------------------------

  let timer: ReturnType<typeof setTimeout> | null = null;

  function scheduleSave() {
    if (timer) clearTimeout(timer);
    timer = setTimeout(save, 800);
  }

  async function save() {
    if (!doc || !dirty) return;
    saving = true;
    try {
      doc = await ipc.saveDoc({ id: doc.id, markdown });
      dirty = false;
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }

  // Autosave on every edit, and never leave a pending edit unsaved on exit.
  $effect(() => {
    const text = markdown;
    if (!doc || text === doc.markdown) return;
    dirty = true;
    scheduleSave();
  });

  $effect(() => {
    const flush = () => {
      if (timer) clearTimeout(timer);
      save();
    };
    window.addEventListener("blur", flush);
    window.addEventListener("beforeunload", flush);
    return () => {
      window.removeEventListener("blur", flush);
      window.removeEventListener("beforeunload", flush);
    };
  });

  // ---- keyboard -----------------------------------------------------------

  /** Select a whole function: every clause, its @spec and its @doc. */
  function selectFunction(f: ipc.FnInfo) {
    const sig = displaySig(f);
    const at = locate(sig, outline?.modules?.[0] ?? null);
    // `select`, not `set` — navigation must never toggle focus off.
    if (at) focus.select(sig, at.ranges, at.related, at.spec, at.doc);
  }

  /** True while a text field owns the keyboard — never steal keys from typing. */
  function isTyping(target: EventTarget | null): boolean {
    const el = target as HTMLElement | null;
    if (!el) return false;
    return el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable;
  }

  function onKeydown(e: KeyboardEvent) {
    const meta = e.metaKey || e.ctrlKey;

    if (e.key === "Escape") {
      if (showReadingFiles) showReadingFiles = false;
      else if (showHelp) showHelp = false;
      else if (showPalette) showPalette = false;
      else if (showFiles) showFiles = false;
      else if (showLibrary) showLibrary = false;
      else focus.clear();
      return;
    }

    // ⌘O is the muscle memory for "open", so it opens the thing you almost always
    // want: search the folder you are already in. The system picker is for the
    // rarer case of a file outside the project, and keeps ⇧.
    //
    // Every one of these compares `e.key` LOWERCASED. `⇧O` arrives as "O", so a
    // bare `=== "o"` meant the shifted form silently did nothing — and the same
    // is true of every shortcut here with Caps Lock on.
    if (meta && e.key.toLowerCase() === "o") {
      e.preventDefault();
      if (e.shiftKey) pickFile();
      else showFiles = true;
      return;
    }
    if (meta && e.key.toLowerCase() === "s") {
      e.preventDefault();
      save();
      return;
    }
    if (meta && e.key.toLowerCase() === "k") {
      e.preventDefault();
      showLibrary = !showLibrary;
      return;
    }
    if (meta && e.key.toLowerCase() === "t") {
      e.preventDefault();
      // ⌘T adds a file to the reading; ⇧ manages the ones already in it. Paired
      // on purpose — they are the two halves of "the files of this review".
      if (e.shiftKey) {
        if (doc) showReadingFiles = !showReadingFiles;
      } else {
        showFiles = !showFiles;
      }
      return;
    }
    // ⌘R would reload the webview in a dev build, so it is prevented before
    // anything else can act on it.
    if (meta && e.key.toLowerCase() === "r") {
      e.preventDefault();
      docPane?.toggleRead();
      return;
    }
    // ⌘E flips preview/edit. A toggle rather than two bindings: you are always in
    // one of the two, so there is nothing for a second key to say.
    if (meta && e.key.toLowerCase() === "e") {
      e.preventDefault();
      docPane?.toggleEdit();
      return;
    }
    if (meta && e.key.toLowerCase() === "p") {
      e.preventDefault();
      if (outline?.modules?.[0]?.functions.length) showPalette = !showPalette;
      return;
    }

    // `?` is a plain character inside a text field, so it only opens help when
    // nothing is being typed into.
    if (e.key === "?" && !meta && !isTyping(e.target)) {
      e.preventDefault();
      showHelp = !showHelp;
      return;
    }

    // Everything else that moves around the code — j/k, [ ], gg, G, H/M/L,
    // { }, ⌃d/⌃u, zz, yy — belongs to CodePane, which owns the viewport those
    // motions are measured against.
  }
</script>

<!-- Surface and the boundary for whatever file is on screen, rendered at the top
     of the note's own scroll container so the right pane is one column you scroll
     down rather than two panes competing for height. -->
{#snippet exploreSections()}
  {#if file}
    <ExploreSections
      {file}
      {files}
      selected={focus.sig}
      onselect={selectFromExplore}
      onjump={jumpTo}
    />
  {/if}
{/snippet}

<svelte:window onkeydown={onKeydown} />

<div class="app">
  <!-- Row 1: the window's own chrome. Draggable, and kept nearly empty — it
       shares this strip with the macOS traffic lights. -->
  <div class="titlebar" data-tauri-drag-region>
    <!-- Only the element carrying the attribute is draggable, so the label and
         the empty stretch carry it too — otherwise the grabbable area is just
         the few pixels of gap between them. -->
    <span class="brand" data-tauri-drag-region>LGTM</span>
    <span class="spacer" data-tauri-drag-region></span>
    <button class="btn icon" onclick={() => theme.cycle()} title="Cycle theme">
      {theme.label}
    </button>
    <button class="btn icon" onclick={() => (showHelp = true)} title="What everything does (?)">
      ?
    </button>
  </div>

  <!-- Row 2: the app's own header. Everything about the file being read lives
       here, where there is room for it — and during a load it would still be
       describing the file you just left, so it steps aside. -->
  {#if file && !loading}
    <div class="apphead">
      <button class="btn home" onclick={goHome} title="Back to your recent glooms">
        ← Home
      </button>


      <!-- The file's identity lives in the code pane's own header, where the
           filename doubles as a copy-the-path button. This row is for the
           reading's context and the actions. -->
      {#if file.branch}
        <span class="branch" title="Branch, read from .git/HEAD">⑂ {file.branch}</span>
      {/if}
      {#if doc}
        <!-- One button instead of a strip of tabs. Ten filenames need about
             1200px of tabs and the pane has about 750 — three of ten were
             off-screen at exactly the size a real review is. What made the strip
             affordable to lose is that navigation moved to the note's references
             and the drawer's reaches list. -->
        <button
          class="filebtn"
          onclick={() => (showReadingFiles = true)}
          title="Files in this gloom (⌘⇧T)"
        >
          <i class={fileState} class:earned={earnedPath === file?.path}></i>
          <span>{file?.filename ?? ""}</span>
          {#if files.length > 1}
            <span class="n">{files.length} files</span>
          {/if}
          <span class="caret">▾</span>
        </button>
      {/if}

      <span class="spacer"></span>

      <span class="save" class:dirty>{saving ? "Saving…" : dirty ? "Unsaved" : doc ? "Saved ✓" : ""}</span>
      <button class="btn" onclick={reparseNow}>Re-parse</button>
      <button class="btn" onclick={() => (showFiles = true)} title="Find a file by name (⌘T)">
        Find…
      </button>
      <button class="btn" onclick={() => (showLibrary = true)}>Library</button>
      <button class="btn primary" onclick={pickFile}>Open file…</button>
    </div>
  {/if}

  {#if error}
    <div class="banner">
      {error}
      <button onclick={() => (error = null)}>×</button>
    </div>
  {/if}

  {#if loading}
    <div class="loading">
      <div class="orb"></div>
      <b>{loadingFile}</b>
      <span>{loadingStep}…</span>
    </div>
  {:else if !file}
    <div class="welcome">
      <img src="/app-icon.png" alt="" class="hero" />
      <h1>lgtm</h1>
      <!-- Where the word is introduced, once, to someone who has never seen it.
           A name nobody defines is a name nobody adopts. -->
      <p>
        Open an Elixir file to start a <b>gloom</b> — one revision journey. The
        source goes left, your explanation goes right.
      </p>
      <div class="actions">
        <button class="btn primary" onclick={() => (showFiles = true)}>
          Find a file… <kbd>⌘O</kbd><kbd>⌘T</kbd>
        </button>
        <button class="btn" onclick={pickFile}>Anywhere on disk… <kbd>⌘⇧O</kbd></button>
        <button class="btn" onclick={() => (showLibrary = true)}>Library <kbd>⌘K</kbd></button>
        <button class="btn" onclick={() => (showHelp = true)}>What it does <kbd>?</kbd></button>
      </div>

      <div class="projects">
        {#if project}
          <button class="proj on" onclick={() => (showFiles = true)}>
            <span class="folder">▸</span>
            <b>{project.name}</b>
            <span class="path">{project.path}</span>
          </button>
        {/if}
        <button class="proj pick" onclick={chooseProject}>
          {project ? "Open a different folder…" : "Open a folder…"}
        </button>
      </div>

      {#if recents.length}
        <div class="recents">
          <div class="rhead">
            <span>Pick up where you left off</span>
            <button class="more" onclick={() => (showLibrary = true)}>all glooms →</button>
          </div>
          {#each recents as doc (doc.id)}
            <button class="recent" onclick={() => openExisting(doc)}>
              <span class="title">{doc.title}</span>
              <span class="file">{doc.filename}</span>
              <span class="spacer"></span>
              {#if doc.branch}<span class="branch">⑂ {doc.branch}</span>{/if}
              <span class="ago">{when(doc.updatedAt)}</span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  {:else}
    <!-- The gloom's own row: what this journey is *for*, in its own colour so it
         is not read as one more control in the toolbar above it. A file's name
         tells you what you are looking at; only this says why you opened it. -->
    {#if doc}
      <!-- Keyed on the id, so opening a gloom replays the band's arrival and
           typing in it never does. A remount is cheaper than a class and a
           timer, and there is nothing here to preserve across one. -->
      {#key doc.id}
        <div
          class="gloombar arriving"
          class:named
          class:unnamed={doc.title === seededTitle && renaming === null}
        >
          {#if renaming !== null}
            <input
              class="gname"
              bind:value={renaming}
              bind:this={renameEl}
              spellcheck="false"
              placeholder="What is this gloom for?"
              onblur={commitRename}
              onkeydown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  commitRename();
                } else if (e.key === "Escape") {
                  e.preventDefault();
                  renaming = null;
                }
                e.stopPropagation();
              }}
            />
          {:else}
            <button class="gname" onclick={startRename} title="Rename this gloom">
              {doc.title}<i>✎</i>
            </button>
          {/if}

          <span class="spacer"></span>

          {#if doc.title === seededTitle && renaming === null}
            <!-- Still carrying the name the module gave it. An invitation, not an
                 error — the same idiom as the Explain section's italic line. -->
            <span class="nudge">say what you are here to find out</span>
          {/if}

          <!-- The wordmark, not a badge: at the right-hand end of the line the
               name sits on, it reads as what the line *is* rather than as a chip
               attached to it. -->
          <span class="glabel">Gloom</span>
        </div>
      {/key}
    {/if}

    <div class="split">
      <div class="pane" style:flex="0 0 {basis}%">
        {#if currentStale}
          <!-- Only for a file that joined the reading later. The origin's
               staleness is the note's business, and DocPane offers the richer
               action there: reconcile, which merges rather than overwrites. -->
          <div class="filenote">
            <span>
              {file.missing
                ? `${file.filename} is not on disk any more — this is the snapshot.`
                : `${file.filename} has changed since you read it.`}
            </span>
            <span class="spacer"></span>
            {#if !file.missing}
              <button onclick={acceptCurrent}>Accept as read</button>
            {/if}
          </div>
        {/if}

        <!-- Keyed on the path so switching file remounts the pane. Blame and a
             search belong to the file they were run against; carrying either
             across a switch would attribute one file's authors to another. -->
        <div class="codewrap" class:swapping>
          {#key currentPath}
            <CodePane
              source={file.source}
              lang={file.lang}
              filename={file.filename}
              path={file.path}
              hasGit={file.hasGit}
              {outline}
              keysEnabled={!showLibrary &&
                !showFiles &&
                !showReadingFiles &&
                !showPalette &&
                !showHelp}
            />
          {/key}
          {#if swapped}
            <div class="swapbadge">▸ {swapped}</div>
          {/if}
        </div>
      </div>

      <Divider bind:basis />

      <div class="pane grow">
        {#if chooser}
          <div class="chooser">
            <h2>This file is already part of a gloom</h2>
            <ul>
              {#each chooser as c (c.id)}
                <li>
                  <button onclick={() => openExisting(c, file?.path ?? null)}>
                    <b>{c.title}</b>
                    <span>{c.branch ?? "no branch"} · {new Date(c.updatedAt).toLocaleDateString()}</span>
                  </button>
                </li>
              {/each}
            </ul>
            <button class="btn" onclick={() => file && startFreshFrom(file)}>Start a fresh doc</button>
          </div>
        {:else}
          <DocPane
            bind:this={docPane}
            bind:markdown
            {files}
            current={currentPath}
            filename={origin?.filename ?? file.filename}
            {dirty}
            {stale}
            onreconcile={reconcile}
            {opened}
            onshowfile={showFile}
            onrefs={(paths) => (referenced = paths)}
            onreading={(on) => (readingNow = on)}
            explore={exploreSections}
          />
        {/if}
      </div>
    </div>

    <div class="status">
      <span>
        {#if outline?.kind === "config"}
          tree-sitter-{file.lang} · config · {outline.config?.groups.length ?? 0} groups
        {:else if outline?.kind === "test"}
          tree-sitter-{file.lang} · test suite · {outline.tests?.describes.reduce(
            (n, d) => n + d.tests.length,
            0,
          ) ?? 0} tests
        {:else if outline?.kind === "module"}
          tree-sitter-{file.lang} · {outline.modules.reduce((n, m) => n + m.functions.length, 0)} functions
        {:else if outline}
          tree-sitter-{file.lang} · no module, config or test suite here
        {:else}
          no outline for this file type
        {/if}
      </span>
      {#if outline?.kind === "module" && moduleCount > 1}
        <span class="warn">{moduleCount} modules in this file — seeded from the first</span>
      {/if}
      <span class="spacer"></span>
      {#if outline?.modules?.[0]?.functions.length}
        <span class="keys">
          <kbd>↑</kbd><kbd>↓</kbd> lines · <kbd>[</kbd><kbd>]</kbd> fns ·
          <kbd>↑</kbd><kbd>↓</kbd> lines · <kbd>/</kbd> find · <kbd>⌘⇧T</kbd> files ·
          <kbd>⌘R</kbd> read · <kbd>⌘E</kbd> edit · <kbd>?</kbd> help
        </span>
      {/if}
      {#if doc}
        <span>
          doc #{doc.id} → sqlite{files.length > 1 ? ` · ${files.length} files` : ""}
        </span>
      {/if}
    </div>
  {/if}

  {#if showFiles}
    <FilePalette
      {project}
      adding={doc ? doc.title : null}
      onpick={(f) => {
        showFiles = false;
        load(f.path);
      }}
      onpickpath={(p) => {
        showFiles = false;
        load(p);
      }}
      onchoose={() => {
        showFiles = false;
        chooseProject();
      }}
      onclose={() => (showFiles = false)}
    />
  {/if}

  {#if showPalette}
    <FnPalette
      module={outline?.modules?.[0] ?? null}
      onpick={(f) => {
        showPalette = false;
        selectFunction(f);
      }}
      onclose={() => (showPalette = false)}
    />
  {/if}

  {#if showReadingFiles && doc}
    <FilesModal
      {files}
      current={currentPath}
      {referenced}
      onpick={(p) => {
        showReadingFiles = false;
        switchTo(p);
      }}
      onremove={(p) => removeFile(p)}
      onadd={() => {
        showReadingFiles = false;
        showFiles = true;
      }}
      onclose={() => (showReadingFiles = false)}
    />
  {/if}

  {#if showHelp}
    <HelpModal onclose={() => (showHelp = false)} />
  {/if}

  {#if showLibrary}
    <Library
      onopen={(d) => {
        showLibrary = false;
        openExisting(d);
      }}
      ondelete={onDocDeleted}
      onclose={() => (showLibrary = false)}
    />
  {/if}
</div>

<style>
  .app {
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* Its own band, in its own colour: this is not another control in the toolbar,
     it is the answer to "why am I reading all this". Tinted rather than filled —
     a solid teal strip across a reading tool would be the loudest thing on
     screen, and it is context, not an alarm. */
  /* One line: the name at the left where a title belongs and reading starts, the
     wordmark at the right end of it. Centring it needed a three-column grid and
     56px of height to hold a stacked caption, and it bought nothing — a title is
     the first thing on its line, not the middle of it. */
  .gloombar {
    position: relative;
    /* Clips the opening sweep to the band. Nothing here is ever positioned
       outside it — no popovers, no confirmations — so this cannot repeat the
       file strip's bug, where `overflow` on one axis silently clipped a
       confirmation that rendered every time and was visible never. */
    overflow: hidden;
    flex: none;
    height: 42px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 14px;
    background: var(--gloom-bg);
    border-bottom: 1px solid color-mix(in srgb, black 25%, var(--gloom-bg));
  }
  .gloombar .spacer {
    flex: 1;
  }
  .nudge {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .nudge {
    flex: none;
    font-size: 10.5px;
    font-style: italic;
    color: var(--gloom-dim);
  }
  /* Lettering, not a chip. Condensed and spaced out, it reads as a wordmark —
     the name of the thing you are in — where a filled pill read as a status. */
  .glabel {
    flex: none;
    font-family: var(--display);
    font-size: 13px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.28em;
    /* Tracking adds the gap on the RIGHT of the last letter too, which visually
       pulls the word off the edge. Pull it back. */
    margin-right: -0.28em;
    /* The bright teal, which is only legible now that it sits on ink: 7.6:1
       light, 9.5:1 dark. On the old pale band the same colour was 4.2:1 and had
       to be pushed darker. */
    color: var(--gloom);
  }
  .gname {
    position: relative;
    font-family: var(--serif);
    font-size: 16.5px;
    font-weight: 500;
    letter-spacing: 0;
    text-align: left;
    /* Near-white on the ink: a masthead's title. The original `color: var(--fg)`
       sat further down this same rule and quietly won — one declaration block,
       two colours, and the later one always takes it. */
    color: var(--gloom-ink);
    max-width: 620px;
    min-width: 240px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 5px;
    padding: 3px 8px;
    cursor: text;
  }
  .gname i {
    font-style: normal;
    margin-left: 8px;
    opacity: 0;
    font-size: 12px;
    color: var(--gloom);
  }
  /* On ink, "you can edit this" is a lift, not a tint: white at 8% is the only
     hover that works on a dark surface without inventing a second colour. */
  .gname:hover {
    border-color: color-mix(in srgb, white 18%, transparent);
    background: color-mix(in srgb, white 8%, transparent);
  }
  .gname:hover i {
    opacity: 1;
  }
  /* Still carrying the name its module gave it: greyed, because it is a
     placeholder that happens to be true rather than something you decided — and
     the colour is what a named gloom earns. */
  .gloombar.unnamed .gname {
    color: var(--gloom-dim);
    font-style: italic;
  }
  /* Editing happens in place, not in a box. A white field dropped into a tinted
     band is a form appearing in the middle of the chrome — and the name is one
     line of prose, so while you change it, it should look like the line it
     already was, with a rule underneath saying it is live. */
  input.gname {
    text-align: left;
    background: transparent;
    border: 0;
    border-bottom: 2px solid var(--gloom);
    border-radius: 0;
    color: var(--gloom-ink);
    outline: none;
    padding: 2px 8px 1px;
    /* Wide enough for a sentence, and it must not resize as you type: a field
       that grows under the caret moves the text you are reading. */
    width: min(620px, 46vw);
  }
  /* A gloom opens by settling into place: the band is the first thing that says
     *which* journey you are in, and it arriving from above is what makes the
     rest of the window read as its contents rather than as a new screen. */
  .gloombar.arriving {
    animation: bandIn var(--fast) var(--ease-out) both;
  }
  @keyframes bandIn {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
  }
  /* Opening a gloom says which one you are in — across the WHOLE band, not just
     behind the name. The band is the thing that means "this session"; lighting
     only the title made it read as a note about that one word.

     180ms after the band lands, so it reads as the *consequence* of arriving
     rather than as a second thing happening at the same time — the reach block's
     arrival-ring timing argument, reused. Once, then nothing: a session starting
     is an event, not a state that needs holding.

     Opacity only, so nothing in the band moves while it happens — every label in
     here is text you might already be reading. */
  /* On ink, a wash does not read. A flat overlay at 18% over a dark band is a
     change of about two percent luminance — it was there, and it was invisible.
     Light moving across a dark surface is what a dark surface *can* show, so the
     greeting is a sweep: one pass of the gloom's own teal, left to right, once.

     Two layers, because either alone is weak: the sweep says something happened,
     and a brief overall lift under it keeps the band from looking unlit while the
     sweep is still at the left-hand end. */
  .gloombar.arriving::before {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    width: 45%;
    pointer-events: none;
    background: linear-gradient(
      100deg,
      transparent,
      color-mix(in srgb, var(--gloom) 42%, transparent),
      transparent
    );
    animation: bandSweep var(--greet) var(--ease-in-out) 0.18s both;
  }
  .gloombar.arriving::after {
    content: "";
    position: absolute;
    inset: 0;
    pointer-events: none;
    background: color-mix(in srgb, var(--gloom) 14%, transparent);
    animation: bandLift var(--greet) var(--ease-in-out) 0.18s both;
  }
  /* -120% to 320% of a 45%-wide pane: fully off one edge to fully off the other,
     checked rather than eyeballed. */
  @keyframes bandSweep {
    from {
      transform: translateX(-120%);
      opacity: 0;
    }
    18% {
      opacity: 1;
    }
    82% {
      opacity: 1;
    }
    to {
      transform: translateX(320%);
      opacity: 0;
    }
  }
  /* Up quickly, down slowly. A symmetric fade reads as a flash; a long tail reads
     as settling, which is what opening a gloom is. */
  @keyframes bandLift {
    from {
      opacity: 0;
    }
    22% {
      opacity: 1;
    }
    to {
      opacity: 0;
    }
  }

  /* The name lands. One wipe under it, in the gloom's own colour, then gone —
     the same underline idiom a reference uses when it becomes the current one,
     and for the same reason: this is a state you just caused, and a state you
     caused is the one place a small reward is honest. It does not persist,
     because a permanent underline would be a decoration rather than an event. */
  .gloombar.named .gname::after {
    content: "";
    position: absolute;
    left: 8px;
    right: 8px;
    bottom: 1px;
    height: 2px;
    border-radius: 1px;
    background: var(--gloom);
    transform-origin: left center;
    animation: nameWipe var(--trace) var(--ease-out) both;
  }
  @keyframes nameWipe {
    0% {
      transform: scaleX(0);
      opacity: 1;
    }
    58% {
      transform: scaleX(1);
      opacity: 1;
    }
    100% {
      transform: scaleX(1);
      opacity: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    /* The band is already in place; arriving was the whole motion. */
    .gloombar.arriving {
      animation: none;
    }
    /* The sweep hands its job to the lift: the band still greets you in the
       gloom's colour, it just does not travel to do it. Deleting it would take
       the signal with it, and on ink the signal is the only thing there is. */
    .gloombar.arriving::before {
      animation: bandLift var(--greet) ease 0.18s both;
      width: 100%;
    }
    /* The wipe becomes a rule that appears and fades: the *event* survives, only
       the travel is dropped. Deleting it would take the signal with it. */
    .gloombar.named .gname::after {
      animation: nameHold var(--trace) ease both;
    }
    @keyframes nameHold {
      0%,
      58% {
        opacity: 1;
      }
      100% {
        opacity: 0;
      }
    }
  }
  input.gname::placeholder {
    color: var(--gloom-dim);
    font-weight: 400;
  }
  .titlebar {
    height: 38px;
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px 0 84px; /* room for the macOS traffic lights */
    background: var(--bg-raised);
    border-bottom: 1px solid var(--line);
    user-select: none;
  }
  .brand {
    font-weight: 700;
    letter-spacing: 0.08em;
    font-size: 11px;
    color: var(--fg-dim);
  }

  /* Row 2 — the file being read, with room to breathe. */
  .apphead {
    flex: none;
    height: 40px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px;
    background: var(--bg);
    border-bottom: 1px solid var(--line);
  }
  .home {
    flex: none;
  }
  /* The file you are in, and the way to the rest — 136px, constant whatever the
     file count. */
  .filebtn {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 3px 9px;
    font: inherit;
    font-family: var(--mono);
    font-size: 11.5px;
    color: var(--fg);
    background: var(--code-bg);
    border: 1px solid var(--line);
    border-radius: 6px;
    cursor: pointer;
    box-shadow: var(--shadow);
  }
  .filebtn:hover {
    border-color: var(--accent);
  }
  .filebtn i {
    width: 5px;
    height: 5px;
    flex: none;
    border-radius: 50%;
    background: var(--pub);
  }
  .filebtn i.stale {
    background: var(--priv);
  }
  .filebtn i.unread {
    background: transparent;
    box-shadow: inset 0 0 0 1.5px var(--fg-faint);
  }
  .filebtn .n {
    font-family: var(--sans);
    font-size: 9.5px;
    color: var(--fg-faint);
    border-left: 1px solid var(--line);
    padding-left: 7px;
  }
  .filebtn .caret {
    font-size: 8px;
    color: var(--fg-faint);
  }
  /* Earned: the dot fills and one ring leaves. Once — this marks a thing that
     just became true, not a state that keeps being true. */
  .filebtn i.earned {
    position: relative;
    background: var(--pub);
    box-shadow: none;
    animation: fill var(--fast) var(--ease-out) both;
  }
  .filebtn i.earned::after {
    content: "";
    position: absolute;
    inset: -3px;
    border-radius: 50%;
    border: 1.5px solid var(--pub);
    animation: dotring var(--ring) var(--ease-out) both;
  }
  /* No scale. At 5px, `scale(0.4)` is a 2px dot popping to 5px — well past the
     0.9–0.97 the physicality rule allows, and tuning it to 0.9 would make it
     invisible, which is the tell that it was never the part doing the work. The
     ring is. So the fill is now just the colour arriving. */
  @keyframes fill {
    from {
      opacity: 0.4;
    }
    to {
      opacity: 1;
    }
  }
  @keyframes dotring {
    0% {
      opacity: 0.9;
      transform: scale(0.6);
    }
    100% {
      opacity: 0;
      transform: scale(1.9);
    }
  }
  /* Motion off: the dot is simply green, which was always the actual message. */
  @media (prefers-reduced-motion: reduce) {
    .filebtn i.earned {
      animation: none;
    }
    .filebtn i.earned::after {
      display: none;
    }
  }

  /* The left pane's own column: strip, note, then the code. */
  .codewrap {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    /* A crossfade between two files — something moving, not something arriving. */
    transition: opacity 0.13s var(--ease-in-out);
  }
  .codewrap.swapping {
    opacity: 0;
  }

  /* Names where you landed after a file swap. Without it a scroll-driven jump
     across files just looks like the code changed under you. */
  .swapbadge {
    position: absolute;
    z-index: 20;
    top: 10px;
    left: 50%;
    transform: translateX(-50%);
    padding: 4px 10px;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--read-ink);
    background: var(--read);
    border-radius: 999px;
    pointer-events: none;
    animation: badge 1.1s var(--ease-out) forwards;
  }
  @keyframes badge {
    0% { opacity: 0; transform: translate(-50%, -4px); }
    14% { opacity: 1; transform: translate(-50%, 0); }
    72% { opacity: 1; }
    100% { opacity: 0; }
  }

  .filenote {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 10px;
    font-size: 11px;
    color: var(--priv);
    background: color-mix(in srgb, var(--priv) 9%, transparent);
    border-bottom: 1px solid var(--line);
  }
  .filenote .spacer {
    flex: 1;
  }
  .filenote button {
    padding: 2px 8px;
    font-size: 11px;
    color: var(--fg-dim);
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: 5px;
    cursor: pointer;
  }
  .filenote button:hover {
    color: var(--fg);
    border-color: var(--priv);
  }
  .branch {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg-dim);
    background: var(--bg-inset);
    border: 1px solid var(--line);
    border-radius: 999px;
    padding: 2px 9px;
    white-space: nowrap;
  }
  .spacer {
    flex: 1;
  }
  .save {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--pub);
    min-width: 56px;
    text-align: right;
  }
  .save.dirty {
    color: var(--priv);
  }

  .banner {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
    background: color-mix(in srgb, var(--priv) 12%, transparent);
    color: var(--priv);
    font-size: 12px;
  }
  .banner button {
    margin-left: auto;
    background: none;
    border: 0;
    color: inherit;
    cursor: pointer;
    font-size: 15px;
  }

  .split {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .pane {
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .pane.grow {
    flex: 1 1 auto;
  }

  .status {
    height: 24px;
    flex: none;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 0 12px;
    background: var(--bg-raised);
    border-top: 1px solid var(--line);
    font-family: var(--mono);
    font-size: 11px;
    color: var(--fg-faint);
  }
  .status .warn {
    color: var(--priv);
  }
  .status .keys {
    color: var(--fg-faint);
    opacity: 0.85;
  }
  .status kbd {
    font-family: var(--mono);
    font-size: 9.5px;
    border: 1px solid var(--line);
    border-bottom-width: 2px;
    border-radius: 3px;
    padding: 0 3px;
    margin-right: 2px;
    color: var(--fg-dim);
  }

  .loading {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
  }
  .loading b {
    font-family: var(--mono);
    font-size: 14px;
    font-weight: 600;
    color: var(--fg);
  }
  .loading span {
    font-size: 12px;
    color: var(--fg-faint);
  }
  /* The same 2.1s breath the focus bar uses, so waiting feels like part of the
     app rather than a borrowed spinner. */
  .orb {
    width: 46px;
    height: 46px;
    border-radius: 50%;
    margin-bottom: 10px;
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    border: 2px solid color-mix(in srgb, var(--accent) 45%, transparent);
    animation: breathe var(--slow) ease-in-out infinite;
  }
  @keyframes breathe {
    0%,
    100% {
      transform: scale(0.88);
      opacity: 0.55;
    }
    50% {
      transform: scale(1);
      opacity: 1;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .orb {
      animation: none;
    }
    /* The badge names which file you landed on after a swap — that is the whole
       message, so it stays put instead of fading. It is cleared on a timer
       either way. */
    .swapbadge {
      animation: none;
    }
    /* And the pane swap itself: the dip is what makes a file change legible
       rather than looking like the code moved under you. Without motion it just
       changes, which is honest — but the fade must not leave it invisible. */
    .codewrap {
      transition: none;
    }
    .codewrap.swapping {
      opacity: 1;
    }
  }

  .welcome {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
  }
  .hero {
    width: 168px;
    height: 168px;
    border-radius: 30px;
    margin-bottom: 18px;
    box-shadow: 0 10px 34px rgba(16, 24, 40, 0.14);
    user-select: none;
    -webkit-user-drag: none;
  }
  :global(html.dark) .hero {
    box-shadow: 0 10px 34px rgba(0, 0, 0, 0.5);
  }
  .welcome h1 {
    font-size: 34px;
    margin: 0;
    letter-spacing: -0.02em;
  }
  .welcome p {
    color: var(--fg-dim);
    margin: 0 0 14px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  kbd {
    font-family: var(--mono);
    font-size: 10px;
    opacity: 0.7;
    margin-left: 4px;
  }


  /* Recent readings — the fastest route back into a file you were already in. */
  .recents {
    width: min(560px, 84vw);
    margin-top: 28px;
    border-top: 1px solid var(--line);
    padding-top: 10px;
  }
  .rhead {
    display: flex;
    align-items: baseline;
    padding: 0 4px 6px;
    font-size: 10.5px;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--fg-faint);
  }
  .rhead .more {
    margin-left: auto;
    font: inherit;
    font-size: 10.5px;
    letter-spacing: 0.06em;
    text-transform: none;
    background: none;
    border: 0;
    color: var(--fg-faint);
    cursor: pointer;
  }
  .rhead .more:hover {
    color: var(--accent);
  }
  .recent {
    display: flex;
    align-items: baseline;
    gap: 9px;
    width: 100%;
    text-align: left;
    font: inherit;
    background: none;
    border: 0;
    border-left: 2px solid transparent;
    border-radius: 5px;
    padding: 6px 8px;
    cursor: pointer;
    color: var(--fg);
  }
  .recent:hover {
    background: var(--bg-inset);
    border-left-color: var(--accent);
  }
  /* A gloom's name is set in the serif wherever it is *shown*, so the name you
     gave it is recognisable as the same thing in the band, the library and here.
     Only the name — the filename and the path stay mono and sans, since they are
     data rather than something you wrote. */
  .recent .title {
    font-family: var(--serif);
    font-size: 13.5px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .recent .file {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg-faint);
    white-space: nowrap;
  }
  .recent .spacer {
    flex: 1;
  }
  .recent .branch {
    font-family: var(--mono);
    font-size: 10px;
    color: var(--fg-dim);
    border: 1px solid var(--line);
    border-radius: 999px;
    padding: 0 6px;
    white-space: nowrap;
  }
  .recent .ago {
    font-size: 10.5px;
    color: var(--fg-faint);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }


  /* Pick the folder once; after that it is ⌘T all the way down. */
  .projects {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 6px;
    width: min(560px, 84vw);
    margin-top: 22px;
  }
  .proj {
    display: flex;
    align-items: baseline;
    gap: 8px;
    font: inherit;
    text-align: left;
    background: none;
    border: 1px solid var(--line);
    border-radius: 7px;
    padding: 9px 12px;
    cursor: pointer;
    color: var(--fg-dim);
    min-width: 0;
  }
  .proj:hover {
    border-color: var(--accent);
    color: var(--fg);
  }
  .proj .folder {
    color: var(--accent);
  }
  .proj b {
    font-size: 13px;
    color: var(--fg);
    flex: none;
  }
  .proj .path {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }
  .proj.pick {
    justify-content: center;
    border-style: dashed;
    font-size: 12px;
  }

  .chooser {
    padding: 40px 30px;
    max-width: 520px;
  }
  .chooser h2 {
    font-size: 14px;
    margin: 0 0 12px;
    color: var(--fg-dim);
  }
  .chooser ul {
    list-style: none;
    padding: 0;
    margin: 0 0 16px;
  }
  .chooser li button {
    display: block;
    width: 100%;
    text-align: left;
    font: inherit;
    background: var(--bg-raised);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 10px 12px;
    margin-bottom: 8px;
    cursor: pointer;
    color: var(--fg);
  }
  .chooser li button:hover {
    border-color: var(--accent);
  }
  .chooser li span {
    display: block;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--fg-faint);
    margin-top: 2px;
  }
</style>
