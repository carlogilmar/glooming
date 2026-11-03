<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { when } from "$lib/when";
  import { joinTags, parseTags, tagHue, withTag, withoutTag } from "$lib/tags";
  import CodePane from "$lib/components/CodePane.svelte";
  import DocPane from "$lib/components/DocPane.svelte";
  import Divider from "$lib/components/Divider.svelte";
  import Hub from "$lib/components/Hub.svelte";
  import FnPalette from "$lib/components/FnPalette.svelte";
  import HelpModal from "$lib/components/HelpModal.svelte";
  import FilePalette from "$lib/components/FilePalette.svelte";
  import ExploreSections from "$lib/components/ExploreSections.svelte";
  import Sky from "$lib/components/Sky.svelte";
  import FileIcon, { iconFor } from "$lib/components/FileIcon.svelte";
  import { byPath, knownModules, moduleLabels, moduleOf, namedPaths } from "$lib/fileset";
  import { fileTree, mentionedIn, reveal } from "$lib/filetree";
  import { displaySig, locate } from "$lib/select";
  import { theme } from "$lib/stores/theme.svelte";
  import { focus } from "$lib/stores/focus.svelte";
  import * as ipc from "$lib/ipc";
  import type { Doc, Hub as HubData, IndexedModule, Range, ReadingFile } from "$lib/ipc";
  import { pathsToCheck } from "$lib/hub";

  /**
   * The project is what you explore; a gloom is an optional note inside it.
   *
   * `project` null is home. Inside one, three things stand where a reading's
   * file set used to:
   *
   *   index     every module in the project and its file — names only, derived
   *             from disk on open and on focus, never stored
   *   files     the files loaded this session, with their outlines: the one on
   *             screen, the ones you walked through, the ones the prose names.
   *             A cache of live reads; nothing in it is a record of anything
   *   doc       the open gloom, or null while exploring
   *
   * `mockup/workspace.html` is the contract.
   */
  let project = $state<ipc.Project | null>(null);
  let projects = $state<ipc.Project[]>([]);
  let index = $state<IndexedModule[]>([]);
  /** Every file in the project, for the sidebar's tree — any kind, not just Elixir. */
  let tree = $state<ipc.ProjectFile[]>([]);
  let files = $state<ReadingFile[]>([]);
  let currentPath = $state<string | null>(null);
  let doc = $state<Doc | null>(null);
  /** This project's glooms and topics — what the Hub draws. */
  let hubData = $state<HubData>({ topics: [], glooms: [] });
  /** Paths the prose actually references — the tree marks their modules. */
  let referenced = $state<Set<string>>(new Set());
  /** The doc pane instance, so ⌘R and ⌘E can reach its mode toggles. */
  let docPane = $state<DocPane | null>(null);
  /** Read mode takes both panes; the sidebar and the views stand down. */
  let readingNow = $state(false);
  /**
   * Bumped when a *different* gloom opens — the doc pane's cue to play its
   * arrival animations once, rather than on every keystroke's re-render.
   */
  let opened = $state(0);
  let markdown = $state("");
  let basis = $state(52);
  let saving = $state(false);
  let dirty = $state(false);
  let error = $state<string | null>(null);
  /**
   * The Hub asked for with ⌘K or `⌂ Hub` while something is open. It takes the
   * two panes, exactly as it does when nothing is — the same page, not a modal
   * over the work. A modal was built first and cut: the index is a place you go
   * to, and a sheet over the code made it a detour you had to dismiss.
   */
  let hubPage = $state(false);
  let hubRef = $state<ReturnType<typeof Hub> | null>(null);
  /** ⌘T — search the project by name, or paste a path. */
  let showFiles = $state(false);
  /** ⌘P — jump to a function by name. */
  let showPalette = $state(false);
  /** `?` — what everything does. */
  let showHelp = $state(false);

  const file = $derived(byPath(files, currentPath));
  const outline = $derived(file?.outline ?? null);
  const moduleCount = $derived(outline?.modules?.length ?? 0);

  /** Every module name, and how each is shown — see `moduleLabels`. */
  const known = $derived(knownModules(index, files));
  const labels = $derived(moduleLabels(known));

  // ---- the sidebar ---------------------------------------------------------

  /**
   * Shown by default, collapsed with ⌘\ and always in read mode. Remembered,
   * because a narrow window wants it gone and asking twice is once too many.
   */
  let side = $state(true);
  /**
   * The sidebar's width, dragged from its right edge and remembered. A deep
   * project wants room for its names and a laptop wants the code pane back, so
   * no one width is right; double-clicking the edge returns to this.
   */
  const SIDE_W = 240;
  let sideW = $state(SIDE_W);
  $effect(() => {
    try {
      side = localStorage.getItem("sidebar") !== "hidden";
      const w = Number(localStorage.getItem("sidebarWidth"));
      if (w) sideW = clampSide(w);
    } catch {
      /* no storage — keep the default */
    }
  });
  function clampSide(w: number) {
    return Math.round(Math.min(520, Math.max(160, w)));
  }
  function rememberSide() {
    try {
      localStorage.setItem("sidebarWidth", String(sideW));
    } catch {
      /* not worth interrupting for */
    }
  }
  let sideDragging = $state(false);
  function startSideDrag(e: PointerEvent) {
    e.preventDefault();
    const left = (e.currentTarget as HTMLElement).parentElement!.getBoundingClientRect().left;
    sideDragging = true;
    const move = (ev: PointerEvent) => (sideW = clampSide(ev.clientX - left));
    const up = () => {
      sideDragging = false;
      document.body.style.cursor = "";
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      rememberSide();
    };
    document.body.style.cursor = "col-resize";
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }
  function resetSide() {
    sideW = SIDE_W;
    rememberSide();
  }

  function toggleSide() {
    side = !side;
    try {
      localStorage.setItem("sidebar", side ? "shown" : "hidden");
    } catch {
      /* not worth interrupting for */
    }
  }

  /**
   * The project as a file tree, the way an editor shows it.
   *
   * Every file, not only Elixir: reading a codebase means `mix.exs`, the config,
   * the migrations and the README as much as the modules, and a tree of only what
   * lgtm can parse is a partial map that looks complete. Anything without a
   * parser opens as plain text, read-only, and the views say so. Folders first,
   * then files, each sorted case-insensitively — what every editor does, so
   * nobody has to learn it.
   */
  let treeOpen = $state<Set<string>>(new Set());

  /** The module on screen — named in the views' header. */
  const currentModule = $derived(moduleOf(file)?.name ?? null);

  const treeRows = $derived(fileTree(tree, treeOpen));

  function toggleNode(key: string) {
    const next = new Set(treeOpen);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    treeOpen = next;
  }

  /**
   * What the open gloom's prose names: the files themselves, and the folders
   * they sit in — so a reference is findable in a collapsed tree too. What the
   * file set used to be, now derived from what you wrote.
   */
  const mentioned = $derived(mentionedIn(tree, referenced));

  // ---- the gloom's name and tags -------------------------------------------

  /**
   * Renaming happens in place. `null` when not renaming, so an empty string is a
   * real value you can be holding mid-edit.
   */
  let renaming = $state<string | null>(null);
  let renameEl = $state<HTMLInputElement | null>(null);
  $effect(() => {
    renameEl?.select();
  });

  /**
   * The moment a gloom gets its name. Diffed against the previous title *of the
   * same gloom*, so it fires on the transition — including the first name, from
   * nothing — and a gloom you named last week opens silently. `last` is a plain
   * `let`: bookkeeping only this effect reads, and a `$state` read and written in
   * one effect is a freeze.
   */
  let named = $state(false);
  let last = { id: -1, title: "" };
  $effect(() => {
    const id = doc?.id ?? -1;
    const now = doc?.title ?? "";
    const was = last;
    last = { id, title: now };
    if (id !== was.id || !now || now === was.title) return;
    named = true;
    const off = setTimeout(() => (named = false), 900);
    return () => clearTimeout(off);
  });

  const tags = $derived(parseTags(doc?.label));
  let tagging = $state(false);
  let tagDraft = $state("");
  let tagInput = $state<HTMLInputElement | null>(null);
  $effect(() => {
    if (tagging) tagInput?.focus();
  });

  async function saveTags(next: string[]) {
    if (!doc) return;
    // An autosave may be in flight; take the row and put your own markdown back.
    const keep = markdown;
    try {
      doc = { ...(await ipc.saveDoc({ id: doc.id, label: joinTags(next) })), markdown: keep };
      void loadGlooms();
    } catch (e) {
      error = clean(e);
    }
  }

  function commitTag() {
    const next = withTag(tags, tagDraft);
    tagDraft = "";
    tagging = false;
    if (next !== tags) void saveTags(next);
  }

  function startRename() {
    if (doc) renaming = doc.title;
  }

  async function commitRename() {
    if (!doc || renaming === null) return;
    const title = renaming.trim();
    renaming = null;
    if (!title || title === doc.title) return;
    const keep = markdown;
    try {
      doc = { ...(await ipc.saveDoc({ id: doc.id, title })), markdown: keep };
      void loadGlooms();
    } catch (e) {
      error = clean(e);
    }
  }

  const clean = (e: unknown) => String(e).replace(/^Error:\s*/, "");

  // ---- the branch: a label, nothing pinned to it ---------------------------

  /**
   * Where the project's working tree is right now. Nothing depends on it — a
   * gloom is about live code, so there is no version to keep consistent — but it
   * is the one fact about *now* worth having in view while you read. Re-asked on
   * focus, because you switch branches in the terminal you just came from.
   */
  let liveBranch = $state<string | null>(null);
  let copiedBranch = $state("");
  let copiedTimer: ReturnType<typeof setTimeout> | null = null;

  async function copyBranch(name: string) {
    try {
      await writeText(name);
      copiedBranch = name;
      if (copiedTimer) clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => (copiedBranch = ""), 1400);
    } catch {
      /* no clipboard outside the app shell — not worth interrupting for */
    }
  }

  // ---- the dot on a file that your prose has started naming ----------------

  /**
   * The moment your prose first names something in the module on screen, its
   * row in the tree fills and one ring leaves. A state change you caused is the
   * one place a small reward is honest.
   *
   * `seenRefs` is **not** `$state`: an effect that reads and writes the same
   * reactive value re-triggers itself forever, which froze the window once.
   */
  let seenRefs: Set<string> | null = null;
  let earnTimer: ReturnType<typeof setTimeout> | null = null;
  let earnedPath = $state<string | null>(null);

  $effect(() => {
    const now = referenced;
    const before = seenRefs;
    seenRefs = new Set(now);
    if (before === null) return;
    const fresh = [...now].filter((p) => !before.has(p));
    if (!fresh.length) return;
    earnedPath = fresh[0];
    if (earnTimer) clearTimeout(earnTimer);
    earnTimer = setTimeout(() => (earnedPath = null), 900);
  });

  // ---- loading files: always live ------------------------------------------

  /**
   * Read a file from disk and keep it in the session's cache. Replaces an older
   * read of the same path, so coming back to a file shows it as it is now.
   */
  async function loadFile(path: string): Promise<ReadingFile> {
    const f = await ipc.openFile(path);
    const at = files.findIndex((x) => x.path === f.path);
    files = at === -1 ? [...files, f] : files.map((x, i) => (i === at ? f : x));
    return f;
  }

  /**
   * Switching file fades the pane rather than crossfading line ranges: across
   * two files there is nothing shared to fade between, so the pane dips and the
   * code pane's header lights up around the filename it was already showing.
   */
  let swapping = $state(false);

  async function showFile(path: string) {
    if (!path || path === currentPath) return;
    swapping = !!currentPath;
    if (swapping) await new Promise((r) => setTimeout(r, 130));
    currentPath = path;
    // Show where it lives, once, on arrival — never on render, or a folder on
    // the way to this file could not be folded while it is open.
    treeOpen = reveal(treeOpen, tree.find((f) => f.path === path)?.rel ?? null);
    swapping = false;
  }

  /**
   * The trail. Every module you open is a step you can walk back along — the
   * inspector chain, kept as back and forward rather than drawn, because the
   * picture of it belongs to a later design and ← → is what the hands expect.
   */
  let back = $state<string[]>([]);
  let fwd = $state<string[]>([]);

  /** Open a file in the code pane, as a step in the trail. */
  async function openPath(path: string, step = true) {
    error = null;
    // Opening a file is leaving the Hub, wherever it was opened from.
    hubPage = false;
    try {
      const f = byPath(files, path) ? (byPath(files, path) as ReadingFile) : await loadFile(path);
      if (step && currentPath && currentPath !== f.path) {
        back = [...back, currentPath];
        fwd = [];
      }
      if (f.path !== currentPath) focus.clear();
      await showFile(f.path);
    } catch (e) {
      error = clean(e);
    }
  }

  async function goBack() {
    const to = back[back.length - 1];
    if (!to || !currentPath) return;
    back = back.slice(0, -1);
    fwd = [...fwd, currentPath];
    await openPath(to, false);
  }

  async function goForward() {
    const to = fwd[fwd.length - 1];
    if (!to || !currentPath) return;
    fwd = fwd.slice(0, -1);
    back = [...back, currentPath];
    await openPath(to, false);
  }

  /** Open a module of the project by name, and land on `fn` in it if given. */
  async function openModule(module: string, fn?: string) {
    const at = index.find((m) => m.module === module);
    if (!at) return;
    await openPath(at.path);
    if (!fn) return;
    const mod = byPath(files, at.path)?.outline?.modules?.find((m) => m.name === module);
    const hit = mod?.functions.find(
      (f) => `${f.name}/${f.arity}` === fn || displaySig(f) === fn || f.name === fn.split("/")[0],
    );
    if (hit) selectFunction(hit, mod ?? null);
  }

  /**
   * Load every module the prose names that is not loaded yet.
   *
   * A reference resolves only against an outline, and nobody may have opened
   * `Weather.Station` this session — so after each edit the note's qualified
   * names are checked against the index and whatever is missing is read. The
   * note re-renders when it lands. Failures are quiet: a file that vanished
   * leaves its references plain, which the next edit will say again.
   */
  let naming: ReturnType<typeof setTimeout> | null = null;
  async function loadNamed() {
    const want = namedPaths(markdown, index, files);
    await Promise.all(want.map((p) => loadFile(p).catch(() => null)));
  }
  $effect(() => {
    markdown;
    index;
    if (!doc) return;
    if (naming) clearTimeout(naming);
    naming = setTimeout(() => void loadNamed(), 250);
  });

  /**
   * Back from somewhere else, re-read what is on screen.
   *
   * The code is live, so an edit made in your editor should be here when you
   * return — and so should a module someone added, which is why the index is
   * rebuilt too. One walk and a handful of reads; caching either would be
   * inventing a staleness problem.
   */
  async function refreshLive() {
    if (!project) return;
    const p = project;
    try {
      [index, tree] = await Promise.all([ipc.projectIndex(p.path), ipc.projectFiles(p.path)]);
      liveBranch = await ipc.branchOf(p.path);
    } catch {
      /* a project folder that vanished is reported when you next act in it */
    }
    const fresh = await Promise.all(files.map((f) => ipc.openFile(f.path).catch(() => f)));
    // Only adopt what changed, so an unchanged file keeps its identity and the
    // panes do not re-render for nothing.
    files = files.map((f, i) => (fresh[i].source === f.source ? f : fresh[i]));
  }

  $effect(() => {
    const returned = () => {
      if (document.visibilityState === "visible") void refreshLive();
    };
    window.addEventListener("focus", returned);
    document.addEventListener("visibilitychange", returned);
    return () => {
      window.removeEventListener("focus", returned);
      document.removeEventListener("visibilitychange", returned);
    };
  });

  // ---- projects ------------------------------------------------------------

  const loadProjects = () =>
    ipc
      .recentProjects()
      .then((r) => (projects = r))
      .catch(() => (projects = []));

  $effect(() => {
    if (!project) void loadProjects();
  });

  async function loadGlooms() {
    if (!project) return;
    try {
      hubData = await ipc.hub(project.id);
    } catch {
      hubData = { topics: [], glooms: [] };
    }
  }

  /**
   * The Hub fills the two panes when nothing is open yet, or when you asked for
   * it. Opening a gloom or a file from anywhere steps it aside; esc returns to
   * whatever was open, which is still there — the gloom, the file, the trail.
   */
  const hubFull = $derived(!!project && (hubPage || (!doc && !currentPath)));
  /** There is work behind the Hub to go back to. */
  const hubOverWork = $derived(hubFull && (!!doc || !!currentPath));

  function toggleHub() {
    if (!project) return;
    if (hubFull) {
      // Back to the work, if there is any; with nothing open the Hub is the page.
      hubPage = false;
      return;
    }
    // Read mode is a mode over the panes the Hub is about to replace.
    if (readingNow) docPane?.toggleRead();
    hubPage = true;
    void loadGlooms();
  }

  /**
   * The Hub says which glooms are broken, which needs the outlines of every file
   * they name. Read them all at once and adopt them in ONE assignment: a file at
   * a time would re-render the Hub — and every open pane — once per file.
   */
  async function loadForHub() {
    const want = pathsToCheck(hubData.glooms, index, files);
    if (!want.length) return;
    const read = (await Promise.all(want.map((p) => ipc.openFile(p).catch(() => null)))).filter(
      (f): f is ReadingFile => !!f,
    );
    const have = new Set(files.map((f) => f.path));
    const fresh = read.filter((f) => !have.has(f.path));
    if (fresh.length) files = [...files, ...fresh];
  }
  $effect(() => {
    if (!hubFull) return;
    hubData;
    index;
    void loadForHub();
  });

  /**
   * Open a gloom from the Hub, optionally straight into read mode. Reading needs
   * the note rendered and its named files loaded before there is anything to
   * walk, so it is retried for a moment rather than timed once.
   */
  async function openFromHub(id: number, read: boolean) {
    hubPage = false;
    await openGloom(id);
    if (!read) return;
    for (let i = 0; i < 20 && !readingNow; i++) {
      await new Promise((r) => setTimeout(r, 150));
      if (docPane?.toggleRead()) return;
    }
  }

  /** Enter a project: its index, its glooms, and an empty code pane. */
  async function enterProject(path: string) {
    error = null;
    try {
      const p = await ipc.openProject(path);
      await leaveProject();
      project = p;
      [index, tree] = await Promise.all([ipc.projectIndex(p.path), ipc.projectFiles(p.path)]);
      liveBranch = await ipc.branchOf(p.path).catch(() => null);
      await loadGlooms();
    } catch (e) {
      error = clean(e);
    }
  }

  async function chooseProject() {
    const picked = await openDialog({ directory: true, multiple: false });
    if (typeof picked === "string") await enterProject(picked);
  }

  /** A file from anywhere on disk — it is read, not added to anything. */
  async function pickFile() {
    const picked = await openDialog({
      multiple: false,
      filters: [{ name: "Elixir", extensions: ["ex", "exs"] }],
    });
    if (typeof picked === "string") await openPath(picked);
  }

  async function leaveProject() {
    await closeGloom();
    focus.clear();
    files = [];
    currentPath = null;
    index = [];
    tree = [];
    treeOpen = new Set();
    hubData = { topics: [], glooms: [] };
    hubPage = false;
    back = [];
    fwd = [];
    referenced = new Set();
  }

  /**
   * Back to home. Flushes first: autosave is on an 800ms debounce, so leaving
   * mid-sentence would otherwise fire the timer into a gloom that is no longer
   * open and lose the last thing you typed.
   */
  async function goHome() {
    await leaveProject();
    project = null;
    error = null;
  }

  // ---- glooms --------------------------------------------------------------

  /**
   * Start a gloom. It opens with its name selected, so naming it costs a
   * sentence and skipping it costs one key. The code pane stays where you were:
   * a gloom usually starts *because* of what you are looking at.
   */
  async function newGloom() {
    if (!project) return;
    try {
      await closeGloom();
      const d = await ipc.createDoc(project.id);
      adoptDoc(d);
      renaming = "";
      await loadGlooms();
    } catch (e) {
      error = clean(e);
    }
  }

  function adoptDoc(d: Doc) {
    doc = d;
    markdown = d.markdown;
    dirty = false;
    opened += 1;
  }

  /**
   * Open a gloom, landing on the first module its prose names — that is where
   * your understanding of it starts. A gloom that names nothing yet leaves the
   * code pane where it was.
   */
  async function openGloom(id: number) {
    if (doc?.id === id) return;
    try {
      await closeGloom();
      const d = await ipc.loadDoc(id);
      adoptDoc(d);
      const first = namedPaths(d.markdown, index, [])[0];
      await loadNamed();
      if (first) await openPath(first);
    } catch (e) {
      error = clean(e);
    }
  }

  /** Back to exploring. The code pane keeps your place. */
  async function closeGloom() {
    if (timer) clearTimeout(timer);
    await save();
    if (readingNow) docPane?.toggleRead();
    doc = null;
    markdown = "";
    dirty = false;
    renaming = null;
    referenced = new Set();
    // Its name, tags and prose may have changed since the Hub last read them.
    void loadGlooms();
  }

  /**
   * A gloom was deleted in the library. The sidebar is a cached query and its
   * answer just changed; and if it is the one on screen it has to go, or the
   * next autosave writes to a row that no longer exists.
   */
  function onDocDeleted(id: number) {
    void loadGlooms();
    if (doc?.id !== id) return;
    if (timer) clearTimeout(timer);
    doc = null;
    markdown = "";
    dirty = false;
    referenced = new Set();
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
      error = clean(e);
    } finally {
      saving = false;
    }
  }

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

  // ---- selecting from the views --------------------------------------------

  /** Select a whole function: every clause, its @spec and its @doc. */
  function selectFunction(f: ipc.FnInfo, module = outline?.modules?.[0] ?? null) {
    const sig = displaySig(f);
    const at = locate(sig, module);
    // `select`, not `set` — navigation must never toggle focus off.
    if (at) focus.select(sig, at.ranges, at.related, at.spec, at.doc);
  }

  /**
   * Pick something in the views: focus it in the code beside it. A row that
   * knows its own span — a test — says so, and that span is used directly.
   */
  function selectFromExplore(sig: string, line: number, span?: Range, tag?: Range | null) {
    if (span) {
      focus.select(sig, [span], [], tag ?? null, null);
      return;
    }
    const at = locate(sig, outline?.modules?.[0] ?? null);
    if (at) focus.select(sig, at.ranges, at.related, at.spec, at.doc);
    else focus.gotoLine(line, file?.source.split("\n").length ?? line);
  }

  /** Go to a line without selecting anything — a describe, or any container. */
  function cursorFromExplore(line: number) {
    focus.clearFunction();
    focus.gotoLine(line, file?.source.split("\n").length ?? line);
  }

  // ---- keyboard -----------------------------------------------------------

  /** True while a text field owns the keyboard — never steal keys from typing. */
  function isTyping(target: EventTarget | null): boolean {
    const el = target as HTMLElement | null;
    if (!el) return false;
    return el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable;
  }

  function onKeydown(e: KeyboardEvent) {
    const meta = e.metaKey || e.ctrlKey;
    const key = e.key.toLowerCase();

    // The Hub answers first while it is on screen — its ↑/↓, Space, ↵ and esc
    // mean something different there — unless a dialog is over it.
    if (hubFull && !showHelp && !showFiles && !showPalette && hubRef?.key(e)) return;

    if (e.key === "Escape") {
      if (showHelp) showHelp = false;
      else if (showPalette) showPalette = false;
      else if (showFiles) showFiles = false;
      else if (hubOverWork) hubPage = false;
      // Read mode is a mode, and every mode needs an escape that does not depend
      // on finding the button that started it.
      else if (readingNow) docPane?.toggleRead();
      else focus.clear();
      return;
    }

    if (e.key === "?" && !meta && !isTyping(e.target)) {
      e.preventDefault();
      showHelp = !showHelp;
      return;
    }

    if (!project) return;

    // While glooming, the arrows walk the note one reference at a time. The code
    // pane's own ↑/↓ cursor stands down for it (see `CodePane`): the code is
    // being driven, and a second thing moving it would argue with the prose.
    if (readingNow && !meta && !e.altKey && !isTyping(e.target)) {
      const step = { ArrowDown: 1, j: 1, ArrowUp: -1, k: -1 }[e.key];
      if (step) {
        e.preventDefault();
        docPane?.stepRead(step);
        return;
      }
    }

    // ⌘O and ⌘T both search the project: ⌘T is what the status bar advertises,
    // ⌘O is what fingers reach for. The system picker keeps ⇧. Every key is
    // compared LOWERCASED — `⇧O` arrives as "O".
    if (meta && (key === "o" || key === "t")) {
      e.preventDefault();
      if (key === "o" && e.shiftKey) pickFile();
      else showFiles = !showFiles;
      return;
    }
    if (meta && key === "s") {
      e.preventDefault();
      save();
      return;
    }
    if (meta && key === "k") {
      e.preventDefault();
      toggleHub();
      return;
    }
    if (meta && e.key === "\\") {
      e.preventDefault();
      toggleSide();
      return;
    }
    // The trail. `[` and `]` alone are CodePane's function motions, so these
    // take the modifier.
    if (meta && e.key === "[") {
      e.preventDefault();
      void goBack();
      return;
    }
    if (meta && e.key === "]") {
      e.preventDefault();
      void goForward();
      return;
    }
    // ⌘R would reload the webview in a dev build, so it is prevented first.
    if (meta && key === "r") {
      e.preventDefault();
      docPane?.toggleRead();
      return;
    }
    if (e.altKey && (e.key === "1" || e.code === "Digit1") && doc) {
      e.preventDefault();
      docPane?.toggleExplore();
      return;
    }
    if (e.altKey && (e.key === "2" || e.code === "Digit2") && doc) {
      e.preventDefault();
      docPane?.toggleNote();
      return;
    }
    if (meta && key === "e") {
      e.preventDefault();
      docPane?.toggleEdit();
      return;
    }
    if (meta && key === "p") {
      e.preventDefault();
      if (outline?.modules?.[0]?.functions.length) showPalette = !showPalette;
      return;
    }
    // Everything else that moves around the code — j/k, [ ], gg, G and the rest —
    // belongs to CodePane, which owns the viewport those motions are measured in.
  }
</script>

<!-- The live views of whatever module is on screen, rendered inside the doc
     pane so the reach diagram's hover and trace keep the host they need. -->
{#snippet exploreSections()}
  {#if file}
    <ExploreSections
      {file}
      {index}
      selected={focus.sig}
      onselect={selectFromExplore}
      onjump={(module, fn) => openModule(module, fn)}
      oncursor={cursorFromExplore}
    />
  {/if}
{/snippet}

<svelte:window onkeydown={onKeydown} />

<div class="app">
  <!-- Row 1: the window's own chrome. Draggable, and kept nearly empty — it
       shares this strip with the macOS traffic lights. Only the element carrying
       the attribute is draggable, so the label and the spacer carry it too. -->
  <div class="titlebar" data-tauri-drag-region>
    <span class="brand" data-tauri-drag-region>Glooming</span>
    <span class="spacer" data-tauri-drag-region></span>
    <button class="btn icon" onclick={() => theme.cycle()} title="Cycle theme">
      {theme.label}
    </button>
    <button class="btn icon" onclick={() => (showHelp = true)} title="What everything does (?)">
      ?
    </button>
  </div>

  {#if project}
    <!-- Row 2: the project, and the ways around it. -->
    <div class="apphead">
      <button class="btn home" onclick={goHome} title="Back to your projects">← Home</button>
      <span class="proj" title={project.path}>{project.name}</span>
      {#if liveBranch}
        <!-- The working tree's branch, live. A label: nothing is pinned to it,
             because a gloom is about the code as it is. Copyable, because a
             branch name is something you are about to type somewhere else. -->
        {#key liveBranch}
          <button class="branch" onclick={() => copyBranch(liveBranch ?? "")} title="The branch you are on — click to copy">
            ⑂ {copiedBranch && copiedBranch === liveBranch ? "copied ✓" : liveBranch}
          </button>
        {/key}
      {/if}
      <span class="spacer"></span>

      <span class="save" class:dirty>{saving ? "Saving…" : dirty ? "Unsaved" : doc ? "Saved ✓" : ""}</span>
      <button class="find" onclick={() => (showFiles = true)} title="Find a module or file (⌘T)">
        <span>⌕ Find a module or file…</span>
        <kbd>⌘T</kbd>
      </button>
    </div>
  {/if}

  {#if error}
    <div class="banner">
      {error}
      <button onclick={() => (error = null)}>×</button>
    </div>
  {/if}

  {#if !project}
    <!-- Home: the projects you explore. A gloom only means something inside the
         codebase it was written about, so home lists codebases, and each one's
         glooms live in its sidebar. `mockup/workspace.html` is the contract. -->
    <div class="home">
      <div class="masthead">
        <!-- Bloom, cosmos or aurora, rolled fresh each time you land here. -->
        <Sky />
        <h1>Glo<span>o</span>ming</h1>
        <p>
          Open a project and read it. A <b>gloom</b> is a note you write while you do —
          your understanding of one part of the code, kept beside the code it is about.
        </p>
      </div>

      <div class="homebody">
        <div class="hwrap">
          <section>
            <div class="hsec"><h2>Projects</h2><span class="rule"></span></div>
            {#if projects.length}
              <div class="rlist">
                {#each projects as p (p.id)}
                  <button class="rrow" onclick={() => enterProject(p.path)} title={p.path}>
                    <span class="pname">{p.name}</span>
                    <span class="ppath">{p.path}</span>
                    <span class="pcount" class:none={!p.gloomCount}>
                      {p.gloomCount ? `${p.gloomCount} ${p.gloomCount === 1 ? "gloom" : "glooms"}` : "no glooms"}
                    </span>
                    <span class="rwhen">{when(p.openedAt)}</span>
                  </button>
                {/each}
              </div>
            {:else}
              <p class="noglooms">
                No projects yet. Open the folder of an Elixir codebase and start reading
                — nothing is copied out of it, and nothing is ever written into it.
              </p>
            {/if}
            <div class="start">
              <button class="go" onclick={chooseProject}>Open a folder…</button>
              <span class="alt">
                <button onclick={() => (showHelp = true)}>What it does ?</button>
              </span>
            </div>
          </section>

          <section>
            <div class="hsec"><h2>What a gloom is</h2><span class="rule"></span></div>
            <div class="idea">
              <div>
                <h3>A page of understanding</h3>
                <p>
                  One question you worked out — how an invoice gets charged, where a
                  session starts. A project collects them, one per thing you learned.
                </p>
              </div>
              <div>
                <h3>About live code</h3>
                <p>
                  Nothing is copied. References find their function wherever it has
                  moved, and say so when it is gone.
                </p>
              </div>
              <div>
                <h3>Written by you</h3>
                <p>
                  The explanation is the work. Nothing here generates it — a reading
                  you did not do is not a reading.
                </p>
              </div>
            </div>
          </section>
        </div>
      </div>
    </div>
  {:else}
    <!-- The band is always there inside a project: it is where you are — a gloom,
         or exploring — and where you go to another one. It used to appear only
         once a gloom was open, which put the way in to glooms in the sidebar and
         made the band a surprise rather than a place. -->
    <div class="bandwrap">
      <!-- Keyed on the id, so opening a gloom replays its arrival and typing in
           it never does. -->
      {#key doc?.id ?? 0}
        <div
          class="gloombar"
          class:arriving={!!doc}
          class:named
          class:unnamed={!!doc && !doc.title && renaming === null}
        >
          <!-- The ways around the knowledge base come first, where the band starts:
               they are the same whatever gloom you are in, so they get a fixed
               place rather than drifting after a name of any length. There is no
               `close ×` any more — the Hub is how you move between glooms. -->
          <button
            class="gpick"
            class:on={hubFull}
            onclick={toggleHub}
            title="This project's glooms, as a table of contents (⌘K)"
          >
            ⌂ Hub <kbd>⌘K</kbd>
          </button>
          <button class="gnew" onclick={newGloom} title="Start a gloom">+ New</button>
          <span class="gsep"></span>
          {#if doc}
            {#if renaming !== null}
              <input
                class="gname"
                bind:value={renaming}
                bind:this={renameEl}
                spellcheck="false"
                placeholder="Name this gloom — what are you trying to understand?"
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
                {doc.title || "Unnamed gloom"}<i>✎</i>
              </button>
            {/if}

            <!-- Tags sit with the name, because they are part of what this gloom is
                 called. Each carries a hue derived from its own text. -->
            <span class="gtags">
              {#each tags as t (t)}
                <button
                  class="gtag"
                  style="--hue:{tagHue(t)}"
                  title="Remove this tag"
                  onclick={() => saveTags(withoutTag(tags, t))}
                >
                  {t}<i>×</i>
                </button>
              {/each}
              {#if tagging}
                <input
                  class="gtagin"
                  bind:this={tagInput}
                  bind:value={tagDraft}
                  placeholder="tag…"
                  spellcheck="false"
                  onblur={commitTag}
                  onkeydown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      commitTag();
                    } else if (e.key === "Escape") {
                      e.preventDefault();
                      tagDraft = "";
                      tagging = false;
                    }
                    e.stopPropagation();
                  }}
                />
              {:else}
                <button class="gtag add" onclick={() => (tagging = true)} title="Add a tag">+</button>
              {/if}
            </span>
          {:else if hubFull && !currentPath}
            <span class="ghub">
              The Hub
              <small>
                {hubData.glooms.length}
                {hubData.glooms.length === 1 ? "gloom" : "glooms"} in {hubData.topics.length}
                {hubData.topics.length === 1 ? "topic" : "topics"}
              </small>
            </span>
          {:else}
            <span class="gexplore">Exploring — nothing is saved</span>
          {/if}

          <span class="spacer"></span>

          {#if doc && !doc.title && renaming === null}
            <span class="nudge">say what you are here to find out</span>
          {/if}
          <span class="glabel">Gloom</span>
        </div>
      {/key}

    </div>

    <div class="work">
      {#if side && !readingNow}
        <!-- Where you go: the codebase itself. Its glooms are in the band. -->
        <aside class="side" style:flex-basis="{sideW}px">
          <div class="slabel">
            Files
            <span class="spacer"></span>
            <span class="scount">{tree.length}</span>
            <!-- The way out sits on the thing it hides; the way back is the rail
                 left in its place. Up in the header it was a word among words,
                 nowhere near the panel it acted on. -->
            <button class="sbtn" onclick={toggleSide} title="Hide the sidebar (⌘\)">‹</button>
          </div>
          <div class="tree">
            {#each treeRows as n (n.key)}
              {@const isOpen = !n.file && treeOpen.has(n.key)}
              <!-- One button per row: a folder folds, a file opens. The chevron is
                   part of the folder's button, so the whole row is the target. -->
              <button
                class="tnode"
                class:dir={!n.file}
                class:on={!!n.file && n.file.path === currentPath}
                style:padding-left="{4 + n.depth * 12}px"
                title={n.key}
                onclick={() => (n.file ? openPath(n.file.path) : toggleNode(n.key))}
              >
                <span class="chev">{n.file ? "" : isOpen ? "▾" : "▸"}</span>
                <FileIcon kind={n.file ? iconFor(n.name) : "folder"} open={isOpen} />
                <span class="nm">{n.name}</span>
                {#if n.file ? mentioned.files.has(n.key) : !isOpen && mentioned.dirs.has(n.key)}
                  <i
                    class="said"
                    class:inside={!n.file}
                    class:earned={!!n.file && earnedPath === n.file.path}
                    title={n.file ? "Your note mentions this file" : "Your note mentions something in here"}
                  ></i>
                {/if}
              </button>
            {:else}
              <p class="sempty">Nothing to show under {project.name}.</p>
            {/each}
          </div>
          <div class="sfoot">Live from disk · nothing is copied out of it</div>
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="sgrip"
            class:dragging={sideDragging}
            title="Drag to resize · double-click to reset"
            onpointerdown={startSideDrag}
            ondblclick={resetSide}
          ></div>
        </aside>
      {:else if !readingNow}
        <!-- The hidden sidebar's way back: a rail that says what it opens. It was a
             bare 14px strip with a ›, which read as the pane's border — something
             you had to know was clickable. -->
        <div class="srail">
          <button onclick={toggleSide} title="Show the files (⌘\)">
            <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
              <rect x="1.5" y="2.5" width="13" height="11" rx="2" fill="none" stroke="currentColor" stroke-width="1.2" />
              <path d="M6 2.5v11" stroke="currentColor" stroke-width="1.2" />
              <path d="M3 5.5h1.5M3 7.5h1.5M3 9.5h1.5" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" />
            </svg>
            <span>Files</span>
          </button>
        </div>
      {/if}

      {#if hubFull && project}
        <!-- Nothing open yet, or asked for: the Hub takes both panes. -->
        <Hub
          bind:this={hubRef}
          {project}
          data={hubData}
          {files}
          {index}
          {known}
          {labels}
          current={doc?.id ?? null}
          back={hubOverWork}
          onopen={openFromHub}
          onnew={() => {
            hubPage = false;
            void newGloom();
          }}
          onchanged={loadGlooms}
          ondeleted={onDocDeleted}
          onback={() => (hubPage = false)}
        />
      {:else}
      <div class="split">
        <!-- The code pane joins read mode's theme, so both surfaces agree about
             whether the lights are out. -->
        <div
          class="pane"
          class:dark={readingNow}
          class:reading-surface={readingNow}
          style:flex="0 0 {basis}%"
        >
          <div class="codewrap" class:swapping>
            {#if file}
              <!-- Keyed on the path so switching file remounts the pane. Blame and
                   a search belong to the file they were run against. -->
              {#key currentPath}
                <CodePane
                  source={file.source}
                  lang={file.lang}
                  filename={file.filename}
                  path={file.path}
                  hasGit={file.hasGit}
                  {outline}
                  trail={{ back: back.length > 0, forward: fwd.length > 0 }}
                  onback={goBack}
                  onforward={goForward}
                  keysEnabled={!showFiles && !showPalette && !showHelp}
                />
              {/key}
            {:else}
              <div class="nofile">
                <p>Pick a module from the sidebar, or find one by name.</p>
                <button class="btn" onclick={() => (showFiles = true)}>Find a module… <kbd>⌘T</kbd></button>
              </div>
            {/if}
          </div>
        </div>

        <Divider bind:basis />

        <div class="pane grow">
          <DocPane
            bind:this={docPane}
            bind:markdown
            docId={doc?.id ?? null}
            title={doc?.title ?? ""}
            {files}
            {known}
            {labels}
            current={currentPath}
            {dirty}
            {opened}
            gloom={!!doc}
            viewing={currentModule ? (labels.get(currentModule) ?? currentModule) : (file?.filename ?? "")}
            onshowfile={(p) => openPath(p, !readingNow)}
            onrefs={(paths) => (referenced = paths)}
            onreading={(on) => (readingNow = on)}
            explore={exploreSections}
          />
        </div>
      </div>
      {/if}
    </div>

    <div class="status">
      <span>
        {#if !file}
          {index.length} modules in {project.name}
        {:else if outline?.kind === "config"}
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
          {file.filename} · plain text, read-only — not an Elixir file
        {/if}
      </span>
      {#if outline?.kind === "module" && moduleCount > 1}
        <span class="warn">{moduleCount} modules in this file — the views show the first</span>
      {/if}
      <span class="spacer"></span>
      <span class="keys">
        <kbd>⌘T</kbd> find · <kbd>⌘[</kbd><kbd>⌘]</kbd> trail · <kbd>⌘\</kbd> sidebar ·
        <kbd>⌘R</kbd> gloom · <kbd>⌘E</kbd> edit · <kbd>?</kbd> help
      </span>
      {#if doc}
        <span>gloom #{doc.id} → sqlite</span>
      {/if}
    </div>
  {/if}

  {#if showFiles && project}
    <FilePalette
      {project}
      onpick={(f) => {
        showFiles = false;
        openPath(f.path);
      }}
      onpickpath={(p) => {
        showFiles = false;
        openPath(p);
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

  <!-- Rendered, not merely imported. `HelpModal` was once imported, toggled by
       `?` and never mounted, for five commits — `component-audit.py` exists
       because of that. -->
  {#if showHelp}
    <HelpModal onclose={() => (showHelp = false)} />
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
  /* A tag is a small, coloured, removable word. The hue comes from the text, so
     `retry` is the same colour in the band, on home and in the library. */
  .gtags {
    display: flex;
    align-items: center;
    gap: 5px;
    flex-wrap: wrap;
    min-width: 0;
  }
  .gtag {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font: inherit;
    font-family: var(--mono);
    font-size: 10px;
    line-height: 1;
    padding: 3px 7px;
    border-radius: 999px;
    cursor: pointer;
    color: oklch(0.86 0.09 var(--hue));
    background: oklch(0.86 0.09 var(--hue) / 0.16);
    border: 1px solid oklch(0.86 0.09 var(--hue) / 0.4);
  }
  .gtag i {
    font-style: normal;
    opacity: 0;
    margin-right: -2px;
  }
  .gtag:hover i {
    opacity: 0.8;
  }
  .gtag.add {
    color: var(--gloom-dim);
    background: transparent;
    border-style: dashed;
    border-color: color-mix(in srgb, white 22%, transparent);
    padding: 3px 8px;
  }
  .gtag.add:hover {
    color: var(--gloom-ink);
    border-color: color-mix(in srgb, white 40%, transparent);
  }
  .gtagin {
    width: 88px;
    font: inherit;
    font-family: var(--mono);
    font-size: 10px;
    color: var(--gloom-ink);
    background: color-mix(in srgb, white 10%, transparent);
    border: 1px solid var(--gloom);
    border-radius: 999px;
    padding: 3px 8px;
    outline: none;
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
  /* The app is called Glooming, set in the wordmark face — it is the same name
     the masthead carries, so the window strip and home agree. `lgtm` survives as
     the *phrase*, on the button that ends a reading. */
  .brand {
    font-family: var(--display);
    font-weight: 600;
    letter-spacing: 0.22em;
    text-transform: uppercase;
    font-size: 12px;
    color: var(--fg-dim);
  }

  /* Row 2 — the file being read, with room to breathe. */
  /* The header is a small THEME, not a set of overrides.
     It redefines the neutral tokens for its own subtree, so every control in it —
     `.btn`, `.btn.primary`, the branch pill, the file button — adapts without a
     single component knowing it is on a dark strip. Exactly what makes read mode
     work, at the scale of one row. */
  .apphead {
    flex: none;
    height: 42px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px;
    background: var(--head-bg);
    border-bottom: 1px solid color-mix(in srgb, black 22%, var(--head-bg));

    --fg: var(--gloom-ink);
    --fg-dim: #c2d6d3;
    /* 4.9:1 on the file button's own raised surface — the softer #93aeaa measured
       3.8:1 there, and the file count is 11px. */
    --fg-faint: #adc4c0;
    --line: color-mix(in srgb, white 16%, transparent);
    --line-soft: color-mix(in srgb, white 10%, transparent);
    --bg: transparent;
    --bg-raised: color-mix(in srgb, white 8%, transparent);
    --bg-inset: color-mix(in srgb, white 6%, transparent);
    --sel: color-mix(in srgb, white 12%, transparent);
    --accent: var(--gloom);
    --shadow: none;
  }
  /* ← Home is how you leave one gloom for another, now that Library has gone from
     this row. It was a text link among five buttons; it is the first thing here
     and it should look like a way out. */
  .apphead .home {
    flex: none;
    font-weight: 600;
    color: var(--gloom-ink);
    background: color-mix(in srgb, white 10%, transparent);
    border-color: color-mix(in srgb, white 22%, transparent);
  }
  .apphead .home:hover {
    background: color-mix(in srgb, white 18%, transparent);
    border-color: color-mix(in srgb, white 34%, transparent);
    color: #fff;
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

  /* The left pane's own column: strip, note, then the code. */
  .codewrap {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    /* A crossfade between two files — something moving, not something arriving.
       It dips to a *tenth*, not to nothing: at zero the pane goes blank for 130ms
       and reads as a load, where a deep dip reads as the same pane changing what
       it is showing. The file that arrives then plays its own gutter cascade,
       because `CodePane` is keyed on the path and a mount is a new file. */
    transition:
      opacity 0.13s var(--ease-in-out),
      transform 0.13s var(--ease-in-out);
  }
  .codewrap.swapping {
    opacity: 0.1;
    /* Down, and back. A file swap is a move, and 4px is enough to say so without
       the two files appearing to slide past one another. */
    transform: translateY(4px);
  }

  /* Which branch you are standing on. It was a grey pill among grey pills, and it
     is the one piece of context that silently changes what every file under it
     says. It now carries the gloom's own accent, and arrives when it changes. */
  /* A button, because a branch name is something you are about to type somewhere
     else — the tooltip says copy, and the confirmation lands on the name itself. */
  .branch {
    font: inherit;
    font-family: var(--mono);
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent);
    border-radius: 999px;
    padding: 2px 10px;
    white-space: nowrap;
    animation: branchIn var(--greet) var(--ease-out) both;
  }
  .branch:hover {
    background: color-mix(in srgb, var(--accent) 24%, transparent);
  }

  /* One breath as it arrives — the label only re-mounts when the branch actually
     changes, so this fires exactly when it has something to say. */
  @keyframes branchIn {
    from {
      opacity: 0;
      transform: translateY(-3px);
    }
    35% {
      opacity: 1;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .branch {
      animation: branchFade var(--greet) ease both;
    }
    @keyframes branchFade {
      from {
        opacity: 0;
      }
    }
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
    /* The pane has to PAINT its surface, not merely define the token for it.
       The file-swap dip fades `.codewrap` to nothing, and with a transparent pane
       that revealed whatever was behind — the app's own `--bg`, which is white in
       light mode. So every cross-file jump flashed white, and in read mode it
       flashed white through a dark theme, which is where it was impossible to
       miss. Both panes carry their own background for the same reason; DocPane
       already did. */
    transition: background 0.3s ease;
  }
  /* The LEFT pane paints the code surface; the right one is the doc pane's own
     and `.panebody` there already paints it. */
  .pane:not(.grow) {
    background: var(--code-bg);
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

  @media (prefers-reduced-motion: reduce) {
    /* The masthead still greets you; it just stops travelling to do it. */
    .masthead::before {
      animation: bandLift var(--greet) ease 0.18s both;
      width: 100%;
    }
    /* And the pane swap itself: the dip is what makes a file change legible
       rather than looking like the code moved under you. Without motion it just
       changes, which is honest — but the fade must not leave it invisible. */
    .codewrap {
      transition: none;
    }
    .codewrap.swapping {
      opacity: 1;
      transform: none;
    }
  }

  /* ---- home ---------------------------------------------------------------
     Ported from `mockup/home.html`. Three bands: an ink masthead saying what the
     app makes, the glooms you were in, and one obvious way to start another. */
  .home {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .masthead {
    flex: none;
    position: relative;
    overflow: hidden;
    padding: 34px 40px 30px;
    background: var(--gloom-bg);
    color: var(--gloom-ink);
    border-bottom: 1px solid color-mix(in srgb, black 25%, var(--gloom-bg));
  }
  /* The words sit above the weather. */
  .masthead h1,
  .masthead p {
    position: relative;
    z-index: 1;
  }
  /* The wordmark is the gloom's teal, not the ink's near-white. White is what the
     prose under it is; the name of the app should be the colour of the thing it
     makes — and it is the only word on the screen allowed to be. */
  .masthead h1 {
    margin: 0;
    font-family: var(--display);
    font-weight: 600;
    font-size: 34px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--gloom);
  }
  .masthead h1 span {
    color: var(--gloom-ink);
  }
  .masthead p {
    margin: 10px 0 0;
    max-width: 54ch;
    font-family: var(--serif);
    font-size: 16px;
    line-height: 1.6;
    color: var(--gloom-dim);
  }
  .masthead p b {
    color: var(--gloom-ink);
    font-weight: 500;
  }
  /* The same greeting the gloom band gives — same surface, same sweep. */
  .masthead::before {
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
      color-mix(in srgb, var(--gloom) 30%, transparent),
      transparent
    );
    animation: bandSweep var(--greet) var(--ease-in-out) 0.18s both;
  }

  .homebody {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 26px 40px 44px;
  }
  .hwrap {
    max-width: 1080px;
    margin: 0 auto;
    display: grid;
    gap: 26px;
  }
  .hsec {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin: 0 0 12px;
  }
  .hsec h2 {
    margin: 0;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--fg-faint);
  }
  .hsec .rule {
    flex: 1;
    height: 1px;
    background: var(--line-soft);
  }

  /* A list, not a grid.
     Not for density — measured, twelve cards take 392px and twelve rows 432px —
     but because a grid has no scan line: names sit at three different x positions
     and you read in a zig-zag. A row also carries a date and a group header
     without growing, which is how you find the one from last Tuesday among forty,
     and the thirteenth gloom extends a list where it reflows a grid.

     Capped at 720px. A row of one short name stretched across 1080 is mostly
     whitespace with a date lost at the far end — the eye has to travel the width
     of the window to pair a name with its age. */
  .rlist {
    max-width: 720px;
    border: 1px solid var(--doc-line);
    border-radius: 10px;
    overflow: hidden;
    background: var(--bg-raised);
  }
  .rrow {
    display: flex;
    align-items: baseline;
    gap: 12px;
    width: 100%;
    padding: 6px 12px;
    font: inherit;
    text-align: left;
    color: var(--fg);
    background: transparent;
    border: 0;
    border-bottom: 1px solid var(--line-soft);
    /* Explicit, and on the children too: a row is one target, and the text inside
       it must not offer an I-beam for a selection it will never make. */
    cursor: pointer;
  }
  .rrow * {
    cursor: pointer;
  }
  .rrow:last-child {
    border-bottom: 0;
  }
  .rrow:hover {
    background: color-mix(in srgb, var(--gloom-deep) 8%, transparent);
  }

  /* The origin filename was here and went: the name identifies the gloom, and a
     second identifier in every row is a column you read past. */
  .rwhen {
    flex: none;
    text-align: right;
    font-family: var(--mono);
    font-size: 10px;
    color: var(--fg-faint);
  }
  .rwhen {
    width: 64px;
  }

  .noglooms {
    margin: 0;
    padding: 22px;
    border: 1px dashed var(--line);
    border-radius: 12px;
    color: var(--fg-faint);
    font-size: 12.5px;
    font-style: italic;
  }

  /* One primary way in, the rest as quiet text. Four buttons of equal weight is
     four decisions before you have started. */
  .start {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
    padding: 18px;
    border: 1px dashed var(--line);
    border-radius: 12px;
  }
  .start .go {
    display: flex;
    align-items: center;
    gap: 9px;
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    color: var(--gloom-ink);
    background: var(--head-bg);
    border: 1px solid color-mix(in srgb, black 20%, var(--head-bg));
    border-radius: 8px;
    padding: 9px 16px;
    cursor: pointer;
  }
  .start .go:hover {
    background: var(--gloom-bg);
  }
  .start .alt {
    margin-left: auto;
    display: flex;
    gap: 8px;
  }
  .start .alt button {
    font: inherit;
    font-size: 11.5px;
    color: var(--fg-dim);
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 6px 10px;
    cursor: pointer;
  }
  .start .alt button:hover {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }

  /* Three sentences, not a feature list: the features are discoverable, the idea
     is not. */
  .idea {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 18px;
  }
  .idea div {
    display: grid;
    gap: 5px;
  }
  .idea h3 {
    margin: 0;
    font-family: var(--display);
    font-size: 12px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--gloom-deep);
  }
  .idea p {
    margin: 0;
    max-width: 40ch;
    font-size: 12.5px;
    line-height: 1.6;
    color: var(--fg-dim);
  }

  /* ---- the project workspace ---------------------------------------------
     `mockup/workspace.html` is the contract. Three columns, one job each: the
     sidebar is where you go, the code pane is what you read, the right pane is
     the live views and — when one is open — your note. */
  .proj {
    font-weight: 600;
    font-size: 12.5px;
    color: var(--fg);
    white-space: nowrap;
  }
  .find {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 220px;
    font: inherit;
    font-size: 11.5px;
    color: var(--fg-dim);
    background: var(--bg-raised);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 4px 10px;
    cursor: pointer;
  }
  .find:hover {
    color: var(--fg);
  }
  .find kbd {
    margin-left: auto;
    font-family: var(--mono);
    font-size: 10px;
    opacity: 0.8;
  }
  /* The band's place for a gloom when there is none: said once, here, where the
     name would be — which is why the views no longer carry a bar saying it. */
  .gexplore {
    font-family: var(--serif);
    font-size: 15px;
    font-style: italic;
    color: var(--gloom-dim);
    white-space: nowrap;
  }
  /* The Hub, filled in the band's own bright teal: the one way around the
     knowledge base, so it is the strongest thing on the line. Ink on teal, 7.4:1
     light and 11.1:1 dark. */
  .gpick,
  .gnew {
    flex: none;
    font: inherit;
    font-size: 11.5px;
    font-weight: 600;
    border-radius: 5px;
    padding: 3px 10px;
    cursor: pointer;
    white-space: nowrap;
  }
  .gpick {
    color: var(--gloom-bg);
    background: var(--gloom);
    border: 1px solid var(--gloom);
  }
  .gpick:hover {
    filter: brightness(1.08);
  }
  /* Already on the Hub: the button stays, hollowed, so it reads as "you are here"
     and a click goes back to what you had open. */
  .gpick.on {
    color: var(--gloom);
    background: transparent;
  }
  /* New, in its own hue — a tint rather than a fill, so it sits one step below
     the Hub. */
  .gnew {
    color: var(--gloom-new);
    background: color-mix(in srgb, var(--gloom-new) 14%, transparent);
    border: 1px solid color-mix(in srgb, var(--gloom-new) 45%, transparent);
  }
  .gnew:hover {
    background: color-mix(in srgb, var(--gloom-new) 24%, transparent);
  }
  /* Between the actions and the name: a hairline, so the name reads as the
     start of its own thing rather than a third button. */
  .gsep {
    flex: none;
    width: 1px;
    height: 18px;
    margin: 0 2px;
    background: color-mix(in srgb, white 16%, transparent);
  }
  .bandwrap {
    position: relative;
    flex: none;
  }
  /* The Hub names itself where a gloom's name would be: on the Hub, that is
     where you are. */
  .ghub {
    font-family: var(--serif);
    font-size: 16px;
    color: var(--gloom-ink);
    white-space: nowrap;
  }
  .ghub small {
    margin-left: 8px;
    font-family: var(--sans);
    font-size: 11px;
    color: var(--gloom-dim);
  }
  .gpick kbd {
    margin-left: 4px;
    font-family: var(--mono);
    font-size: 9.5px;
    font-weight: 400;
    /* Whatever the button's ink is, a step quieter — it is teal-on-ink when
       hollow and ink-on-teal when filled, and one fixed grey fails one of them. */
    color: inherit;
    opacity: 0.7;
  }

  /* The sidebar's edge is its resize handle: a hairline that lights when you
     find it, over a wider invisible target so it does not have to be aimed at. */
  .sgrip {
    position: absolute;
    top: 0;
    right: -3px;
    bottom: 0;
    width: 6px;
    z-index: 2;
    cursor: col-resize;
  }
  .sgrip:hover,
  .sgrip.dragging {
    background: linear-gradient(
      to right,
      transparent 2px,
      var(--accent) 2px,
      var(--accent) 4px,
      transparent 4px
    );
  }
  /* What is left of a hidden sidebar: a strip you can click to bring it back,
     where the sidebar was — not a word in the header. */
  .srail {
    flex: none;
    width: 30px;
    padding-top: 8px;
    display: flex;
    justify-content: center;
    background: var(--bg-raised);
    border-right: 1px solid var(--line);
  }
  /* A real button at the top of the rail — where the sidebar's own header and
     its ‹ were — with the word on it, set vertically to fit the rail. */
  .srail button {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 7px;
    font: inherit;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--fg-dim);
    background: transparent;
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 7px 0 9px;
    width: 22px;
    cursor: pointer;
  }
  .srail button span {
    writing-mode: vertical-rl;
  }
  .srail button:hover {
    color: var(--gloom-deep);
    border-color: var(--gloom-deep);
    background: var(--bg-inset);
  }
  .work {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  /* Narrow on purpose: the code pane is the reason the tool exists, and three
     columns on a laptop squeeze it. ⌘\ takes this away entirely. */
  .side {
    position: relative;
    flex: 0 0 240px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg-raised);
    border-right: 1px solid var(--line);
  }
  .slabel {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 12px 12px 5px;
    font-size: 9.5px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--fg-faint);
  }
  .sbtn {
    font: inherit;
    font-size: 10.5px;
    letter-spacing: 0;
    line-height: 1;
    color: var(--fg-dim);
    background: transparent;
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 2px 6px;
    cursor: pointer;
  }
  .sbtn:hover {
    color: var(--gloom-deep);
    border-color: var(--gloom-deep);
  }
  .scount {
    font-family: var(--mono);
    letter-spacing: 0;
  }
  .sempty {
    margin: 2px 8px 6px;
    font-size: 11px;
    line-height: 1.45;
    font-style: italic;
    color: var(--fg-faint);
  }
  .tree {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 0 6px 14px;
    font-size: 12px;
  }
  .tnode {
    display: flex;
    align-items: center;
    gap: 5px;
    width: 100%;
    height: 23px;
    font: inherit;
    text-align: left;
    color: var(--fg);
    background: transparent;
    border: 0;
    border-radius: 5px;
    padding-right: 6px;
    white-space: nowrap;
    cursor: pointer;
  }
  .tnode:hover {
    background: var(--bg-inset);
  }
  .tnode.on {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }
  .tnode .chev {
    flex: none;
    width: 10px;
    margin-right: -2px;
    font-size: 9px;
    text-align: center;
    color: var(--fg-faint);
  }
  .tnode .nm {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tnode.on .nm {
    color: var(--accent);
    font-weight: 600;
  }
  /* Your note names this module. What the hollow file dot used to say, inverted:
     the set is derived from your prose, so the mark is for what you HAVE written
     about, not what you have not. */
  .said {
    position: relative;
    flex: none;
    width: 6px;
    height: 6px;
    margin-left: 6px;
    border-radius: 50%;
    background: var(--gloom-deep);
  }
  /* On a folded folder: something in here is mentioned. Hollow, because the
     folder itself is not what the prose names. */
  .said.inside {
    background: transparent;
    box-shadow: inset 0 0 0 1.5px var(--gloom-deep);
  }
  .said.earned {
    animation: fill var(--ring) var(--ease-out) both;
  }
  .said.earned::after {
    content: "";
    position: absolute;
    inset: -3px;
    border-radius: 50%;
    border: 1.5px solid var(--gloom-deep);
    animation: dotring var(--ring) var(--ease-out) both;
  }
  @media (prefers-reduced-motion: reduce) {
    .said.earned::after {
      animation: none;
      opacity: 0.5;
    }
  }
  .sfoot {
    flex: none;
    padding: 8px 12px;
    border-top: 1px solid var(--line);
    font-size: 10.5px;
    line-height: 1.4;
    color: var(--fg-faint);
  }
  .nofile {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    color: var(--fg-faint);
    font-size: 12.5px;
  }
  .nofile p {
    margin: 0;
  }
  .nofile kbd {
    font-family: var(--mono);
    font-size: 10px;
    margin-left: 6px;
    opacity: 0.8;
  }
  .pname {
    flex: none;
    font-weight: 600;
    font-size: 13.5px;
  }
  .ppath {
    flex: 1;
    min-width: 0;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--fg-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pcount {
    flex: none;
    font-size: 11.5px;
    color: var(--gloom-deep);
  }
  .pcount.none {
    color: var(--fg-faint);
  }
</style>
