<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { when } from "$lib/when";
  import { joinTags, parseTags, tagHue, withTag, withoutTag } from "$lib/tags";
  import CodePane from "$lib/components/CodePane.svelte";
  import DocPane from "$lib/components/DocPane.svelte";
  import Divider from "$lib/components/Divider.svelte";
  import Library from "$lib/components/Library.svelte";
  import FnPalette from "$lib/components/FnPalette.svelte";
  import HelpModal from "$lib/components/HelpModal.svelte";
  import FilePalette from "$lib/components/FilePalette.svelte";
  import FilesModal from "$lib/components/FilesModal.svelte";
  import ExploreSections from "$lib/components/ExploreSections.svelte";
  import Sky from "$lib/components/Sky.svelte";
  import { byPath, origin as originOf } from "$lib/fileset";
  import { displaySig, locate } from "$lib/select";
  import { theme } from "$lib/stores/theme.svelte";
  import { focus } from "$lib/stores/focus.svelte";
  import * as ipc from "$lib/ipc";
  import type { Doc, DocSummary, OpenedFile, Range, ReadingFile } from "$lib/ipc";

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

  /**
   * Tags, in `docs.label`.
   *
   * A gloom's name says what you were asking; a tag says what *kind* of thing it
   * was — `perf`, `retry`, `PR 412` — which is how you find it again a month
   * later among two hundred. They are searchable in the library, because a tag
   * you cannot find by is decoration.
   */
  const tags = $derived(parseTags(doc?.label));
  let tagging = $state(false);
  let tagDraft = $state("");
  let tagInput = $state<HTMLInputElement | null>(null);
  $effect(() => {
    if (tagging) tagInput?.focus();
  });

  async function saveTags(next: string[]) {
    if (!doc) return;
    // An autosave may be in flight; take the row and put your own markdown back,
    // the same one-payload habit as every other mutation here.
    const keep = markdown;
    try {
      doc = { ...(await ipc.saveDoc({ id: doc.id, label: joinTags(next) })), markdown: keep };
    } catch (e) {
      error = String(e).replace(/^Error:\s*/, "");
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
   * Whether the file on screen has moved on since you read it.
   *
   * Per file, which is the whole reason there is a snapshot per file — and now
   * purely a *statement*: a gloom shows the version it was read at, and the way to
   * read a newer one is a new gloom. Nothing here offers to merge or re-snapshot.
   */
  /**
   * Which branch the working tree is on *now*.
   *
   * Not `origin.branch`: that is sampled when the reading is built, so it says
   * where you were when you opened the gloom. Check out another branch in a
   * terminal and nothing in the app has any reason to notice — which is exactly
   * the case this guard exists for, and exactly the case it missed. Re-asked when
   * the reading changes and whenever the window comes back to the front, since
   * switching branches happens in the terminal you just came from.
   */
  let liveBranch = $state<string | null>(null);

  async function checkBranch() {
    const p = origin?.path;
    if (!p) {
      liveBranch = null;
      return;
    }
    try {
      liveBranch = await ipc.branchOf(p);
    } catch {
      liveBranch = null; // cannot tell is not a refusal
    }
  }

  $effect(() => {
    origin?.path;
    void checkBranch();
  });

  $effect(() => {
    const back = () => void checkBranch();
    window.addEventListener("focus", back);
    document.addEventListener("visibilitychange", back);
    return () => {
      window.removeEventListener("focus", back);
      document.removeEventListener("visibilitychange", back);
    };
  });

  /**
   * Standing somewhere other than where this gloom was read.
   *
   * `doc.branch` is the branch the gloom holds; `liveBranch` is where the tree is
   * now. `add_doc_file` refuses in that state, and the UI has to stop offering it —
   * otherwise the only way to learn the rule is to trip over it.
   */
  const offBranch = $derived(
    doc?.branch && liveBranch && doc.branch !== liveBranch
      ? { gloom: doc.branch, here: liveBranch }
      : null,
  );

  /**
   * A passing remark, as opposed to a problem.
   *
   * The banner is for things that went wrong and stay wrong until you deal with
   * them — a save that failed, a file that is not there — so it waits to be
   * dismissed. Being on another branch is neither: it is a *state* you are
   * standing in, you can see it in the chip and the disabled buttons, and a strip
   * that stays across the top saying so becomes furniture within a minute. So it
   * flashes in, holds long enough to read twice, and leaves.
   *
   * `leaving` exists so the exit is a fade rather than a disappearance; the timer
   * is one `let`, not `$state`, because nothing renders it.
   */
  /**
   * Structured, not a sentence: the two branch names are the readable part, and a
   * badge is what makes them scannable in a line of prose. Keeping them apart also
   * means they can be *copied* — which is the thing you actually want next, since
   * the reply to this notice is a `git checkout` in another window.
   */
  let notice = $state<{ here: string; gloom: string } | null>(null);
  let noticeLeaving = $state(false);
  let noticeTimers: ReturnType<typeof setTimeout>[] = [];

  /** Take it back. A notice belongs to the gloom it is about. */
  function hush() {
    noticeTimers.forEach(clearTimeout);
    noticeTimers = [];
    notice = null;
    noticeLeaving = false;
  }

  function say(here: string, gloom: string) {
    noticeTimers.forEach(clearTimeout);
    noticeTimers = [];
    noticeLeaving = false;
    notice = { here, gloom };
    noticeTimers.push(
      setTimeout(() => (noticeLeaving = true), 5200),
      setTimeout(() => {
        notice = null;
        noticeLeaving = false;
      }, 5800),
    );
  }

  /**
   * A branch name is something you are about to type somewhere else.
   *
   * Every place one is shown is therefore a copy button, with the confirmation on
   * the name itself — a toast for two words would be louder than the thing it is
   * confirming.
   */
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

  /**
   * Say why, once, wherever a blocked gesture is attempted.
   *
   * Awaited at the gesture, because a check that is only as fresh as the last
   * window focus is not fresh enough for the one action it guards.
   */
  async function refuseOffBranch(): Promise<boolean> {
    await checkBranch();
    if (!offBranch) return false;
    say(offBranch.here, offBranch.gloom);
    return true;
  }

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
   * as a glitch — the pane dips instead, and the code pane's header lights up in
   * read-mode gold around the filename it was already showing.
   *
   * Naming the landing used to be a pill floating over the code. It was a second
   * copy of a fact that was on screen the whole time, and it covered the first
   * line of the file to tell you which file it was.
   */
  let swapping = $state(false);

  async function showFile(path: string) {
    if (!path || path === currentPath) return;
    swapping = true;
    await new Promise((r) => setTimeout(r, 130));
    currentPath = path;
    swapping = false;
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

  /**
   * Pick something in the reference sections: focus it in the code beside it.
   *
   * A row that knows its own span says so, and that span is used directly. That
   * is what a test needed: `locate` resolves a **function signature** against
   * the outline, a test's name is not one, so it came back null and every click
   * fell through to `gotoLine` — a one-line cursor on the `test "…" do` line,
   * which is the bug this fixes. The parser has carried `endLine` all along.
   *
   * `tag` goes in the `@spec` slot, because a test's `@tag` is the same kind of
   * thing: the contract above the block, drawn in `--mark`, selected with it but
   * not part of its body.
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

  /**
   * Go to a line without selecting anything — a describe, or any container.
   *
   * The function selection is cleared first: leaving it up would keep the file
   * dimmed to 32% around a function you are no longer looking at, while the
   * cursor sat somewhere else entirely.
   */
  function cursorFromExplore(line: number) {
    focus.clearFunction();
    focus.gotoLine(line, file?.source.split("\n").length ?? line);
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

  /**
   * Recents, grouped by when you last touched them.
   *
   * The grid became a list because a grid has no scan line: names sat at three
   * different x positions, so you read in a zig-zag. It is not about density —
   * measured, twelve cards take 392px and twelve rows 432px — it is that a row
   * carries a date column and a group header without growing, which is how you
   * find the one from last Tuesday among forty.
   */
  const RECENT_BUCKETS = [
    ["Today", 1],
    ["This week", 7],
    ["Earlier", Infinity],
  ] as const;

  const grouped = $derived.by(() => {
    const out: { label: string; items: DocSummary[] }[] = [];
    for (const d of recents) {
      const days = Math.floor((Date.now() - new Date(d.updatedAt).getTime()) / 86_400_000);
      const label = (RECENT_BUCKETS.find(([, max]) => days < max) ?? RECENT_BUCKETS[2])[0];
      const last = out[out.length - 1];
      if (last && last.label === label) last.items.push(d);
      else out.push({ label, items: [d] });
    }
    return out;
  });

  const loadRecents = () =>
    ipc
      // Twelve rather than six: a card filled a grid cell and a row does not, so
      // the list can be longer without taking the screen. Anything past this is
      // ⌘K's job — home is "pick up where you left off", the library is "find any
      // of the two hundred".
      .listDocs(undefined, 12)
      .then((r) => (recents = r))
      .catch(() => (recents = []));

  // Home is the only place these are shown, so only load them while it is up. Deleting from the library has to say so explicitly — it
  // changes the same query without touching any state this effect reads.
  $effect(() => {
    if (files.length) return;
    loadRecents();
  });

  // ---- opening ------------------------------------------------------------

  async function pickFile() {
    if (doc && (await refuseOffBranch())) return;
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
      // The branch guard's refusal is a sentence written for you — "this gloom was
      // read on main, and you are on fix/x…" — so it goes to the banner as it is,
      // without the `Error:` a raw `String(e)` would prepend.
      error = String(e).replace(/^Error:\s*/, "");
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
    // The last gate before the snapshot is taken. Rust refuses too — this one is
    // here so the message is the app's sentence rather than a caught error, and
    // so the loading state never starts for something that cannot finish.
    if (await refuseOffBranch()) return;
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

    // A reading survives its files moving — each carries its own snapshot, which
    // is what gets shown when the path no longer resolves — so "not on disk" is
    // not news worth a banner. What *is* worth saying is the thing you cannot do
    // from here and how to fix it, and the overwhelmingly common reason a file has
    // vanished is that you are standing on another branch.
    await checkBranch();
    if (offBranch) {
      // One line. A banner is a glance, and the reassurance that the reading
      // still works belongs in `?`, not across the top of the window.
      say(offBranch.here, offBranch.gloom);
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
    // The notice is about the gloom you just left — it has no meaning on home, and
    // its own timer would have taken 5s to work that out.
    hush();
    referenced = new Set();
  }

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
    hush();
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
      // Read mode is a mode, and every mode needs an escape that does not depend
      // on finding the button that started it. Clearing the focus underneath it
      // would do nothing visible anyway — the next scroll frame sets it again.
      else if (readingNow) docPane?.toggleRead();
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
    // Every way of adding a file goes through the same refusal, so the rule is
    // learned once rather than per gesture.
    if (meta && (e.key.toLowerCase() === "t" || e.key.toLowerCase() === "o") && doc && offBranch) {
      e.preventDefault();
      void refuseOffBranch();
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
    // ⌥1 / ⌥2 are the split's two ends, driven from the keyboard: navigation only,
    // note only, and the same key again to come back. They are the two tabs the
    // other design would have given, on a control that also does everything in
    // between.
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
      oncursor={cursorFromExplore}
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
    <span class="brand" data-tauri-drag-region>Glooming</span>
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
        <!-- Keyed on the value: checking out another branch replays the arrival,
             which is the one moment this label has something to say. Everything
             below you may have changed, and the gloom did not. -->
        <!-- The GLOOM's branch, not the working tree's.
             A gloom is pinned to the versions it was read at, and the branch those
             versions came from is part of that reading — it does not change when
             you check something else out. Showing the live branch made the header
             report the state of your tree, which is a different tool's job and was
             a fact about *now* sitting in a window that is entirely about *then*. -->
        {#key doc?.branch ?? file.branch}
          <button
            class="branch"
            onclick={() => copyBranch(doc?.branch ?? file.branch ?? "")}
            class:off={!!offBranch}
            title={offBranch
              ? `This gloom was read on ${offBranch.gloom}. You are on ${offBranch.here}, so nothing can join it from here.`
              : "The branch this gloom was read on"}
          >
            ⑂ {copiedBranch && copiedBranch === (doc?.branch ?? file.branch)
              ? "copied ✓"
              : (doc?.branch ?? file.branch)}
          </button>
        {/key}
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
      <!-- Two actions, both about *this* gloom: find a file in the project, or
           open one from anywhere. Re-parse went — it re-read a file you had not
           edited, since lgtm never writes; and Library went with it, because the
           way to another gloom is ← Home, which is the first thing in this row and
           now looks like it. `⌘K` still opens the library from anywhere. -->
      <button
        class="btn"
        disabled={!!offBranch}
        onclick={async () => {
          if (await refuseOffBranch()) return;
          showFiles = true;
        }}
        title={offBranch
          ? `This gloom was read on ${offBranch.gloom}; you are on ${offBranch.here}`
          : "Find a file by name (⌘T)"}
      >
        Find…
      </button>
      <button
        class="btn primary"
        disabled={!!offBranch}
        onclick={pickFile}
        title={offBranch
          ? `This gloom was read on ${offBranch.gloom}; you are on ${offBranch.here}`
          : "Open a file from anywhere on disk (⌘⇧O)"}
      >
        Open file…
      </button>
    </div>
  {/if}

  <!-- Guarded on `file` as well as on the message: a notice is about the gloom on
       screen, and every path that closes one — Home, a delete from the library —
       has to leave it behind. -->
  {#if notice && file}
    <div class="notice" class:leaving={noticeLeaving}>
      <span class="mark" aria-hidden="true">⑂</span>
      <span>
        You are on
        <button class="bbadge" onclick={() => copyBranch(notice!.here)} title="Copy branch name">
          {copiedBranch === notice.here ? "copied ✓" : notice.here}
        </button>
        — check out
        <button class="bbadge" onclick={() => copyBranch(notice!.gloom)} title="Copy branch name">
          {copiedBranch === notice.gloom ? "copied ✓" : notice.gloom}
        </button>
        to add files to this gloom.
      </span>
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
    <!-- Home, rebuilt around what the app makes. It has three jobs, in this order:
         resume the gloom you were in, start one, and — for someone who has never
         seen the word — say what a gloom is. The old screen was a launcher: an
         icon, a product name and four buttons of equal weight, which told you what
         the app could do and nothing about what you were doing.
         `mockup/home.html` is the contract. -->
    <div class="home">
      <div class="masthead">
        <!-- Bloom, cosmos or aurora, rolled fresh each time you land here. -->
        <Sky />
        <h1>Glo<span>o</span>ming</h1>
        <p>
          Read a change until you can say <b>looks good to me</b>. Each reading is a
          <b>gloom</b> — one journey through the code, the files it led you through,
          and what you worked out on the way.
        </p>
      </div>

      <div class="homebody">
        <div class="hwrap">
          <section>
            <div class="hsec">
              <h2>Pick up where you left off</h2>
              <span class="rule"></span>
              <button class="more" onclick={() => (showLibrary = true)}>all glooms →</button>
            </div>
            {#if recents.length}
              <!-- `.rrow`, not `.grow`: the doc pane's wrapper is `.pane.grow`, so a
                   bare `.grow` rule was styling the right-hand pane as well — a
                   class-name collision that `svelte-check` cannot see, because both
                   are valid classes in the same component. -->
              <div class="rlist">
                {#each grouped as g (g.label)}
                  <div class="rbucket">{g.label}</div>
                  {#each g.items as d (d.id)}
                    <button class="rrow" onclick={() => openExisting(d)}>
                      <span class="rname" class:unnamed={d.title === d.filename}>{d.title}</span>
                      {#if d.label}
                        <span class="rtags">
                          {#each parseTags(d.label).slice(0, 3) as t (t)}
                            <span class="rtag" style="--hue:{tagHue(t)}">{t}</span>
                          {/each}
                        </span>
                      {/if}
                      <span class="rcount">
                        {d.fileCount}
                        {d.fileCount === 1 ? "file" : "files"}
                      </span>
                      <span class="rwhen">{when(d.updatedAt)}</span>
                    </button>
                  {/each}
                {/each}
              </div>
            {:else}
              <p class="noglooms">
                No glooms yet. Open a file and the reading starts — everything you
                write about it is kept with the version you read.
              </p>
            {/if}
          </section>

          <section>
            <div class="hsec"><h2>Start a gloom</h2><span class="rule"></span></div>
            <div class="start">
              <button class="go" onclick={() => (showFiles = true)}>
                Find a file… <kbd>⌘O</kbd>
              </button>
              {#if project}
                <span class="where">
                  <b>{project.name}</b>
                  <span>{project.path}</span>
                </span>
              {/if}
              <span class="alt">
                <button onclick={chooseProject}>
                  {project ? "Open a different folder…" : "Open a folder…"}
                </button>
                <button onclick={pickFile}>Anywhere on disk… ⌘⇧O</button>
                <button onclick={() => (showHelp = true)}>What it does ?</button>
              </span>
            </div>
          </section>

          <section>
            <div class="hsec"><h2>What a gloom is</h2><span class="rule"></span></div>
            <div class="idea">
              <div>
                <h3>One journey</h3>
                <p>
                  You open a file; the files it sends you to join it. That set is the
                  gloom — there is no group to create and manage.
                </p>
              </div>
              <div>
                <h3>Pinned in time</h3>
                <p>
                  Every file is kept as it was when you read it. If the code moves on,
                  lgtm says so and leaves your reading intact.
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

          <!-- Tags sit with the name, because they are part of what this gloom is
               called. Each carries its own hue, derived from its text — nobody
               picks a colour for a word they typed in two seconds, and a random
               one would make `retry` look different in two places. -->
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
      <!-- The code pane joins read mode's theme.
           Read mode is a theme, not a darker note: before this, turning it on in
           light mode left a dark reading pane beside an untouched white code
           pane — two surfaces disagreeing about whether the lights were out. The
           classes are the same two `DocPane` puts on itself, so one token set
           paints both. -->
      <div
        class="pane"
        class:dark={readingNow}
        class:reading-surface={readingNow}
        style:flex="0 0 {basis}%"
      >
        <!-- Nothing announces staleness over the code any more.
             A gloom holds the version it was read at, so "this differs from disk"
             is a fact about the world outside the reading — true, and not worth a
             bar across the top of the thing you are reading. It still shows where
             it costs nothing: the dot on the file button, and the words beside each
             file in ⌘⇧T. -->
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
            docId={doc?.id ?? null}
            {files}
            current={currentPath}
            {dirty}
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
          <kbd>⇧click</kbd> range · <kbd>/</kbd> find · <kbd>⌘⇧T</kbd> files ·
          <kbd>⌥1</kbd><kbd>⌥2</kbd> split · <kbd>⌘R</kbd> lgtm · <kbd>⌘E</kbd> edit ·
          <kbd>?</kbd> help
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
      {offBranch}
      {files}
      current={currentPath}
      {referenced}
      onpick={(p) => {
        showReadingFiles = false;
        switchTo(p);
      }}
      onremove={(p) => removeFile(p)}
      onadd={async () => {
        if (await refuseOffBranch()) return;
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
    color: var(--gloom-ink);
    /* The gloom's own teal, because this button is the gloom's files — it opens
       the shape of the change, and the shape is drawn in this colour. Among four
       neutral controls in a neutral row it was the one you hunt for most and the
       hardest to find; now it is the only coloured thing there.

       (It used to borrow `--code-bg` to look like the file it names, which was
       white, and once the header became a dark theme it stayed white while its
       label went near-white with everything else. A control takes its surface
       from the surface it is ON.) */
    background: color-mix(in srgb, var(--gloom) 20%, transparent);
    border: 1px solid color-mix(in srgb, var(--gloom) 45%, transparent);
    border-radius: 6px;
    cursor: pointer;
    box-shadow: var(--shadow);
  }
  .filebtn:hover {
    border-color: var(--gloom);
    background: color-mix(in srgb, var(--gloom) 30%, transparent);
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
    color: var(--gloom-ink);
    opacity: 0.75;
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
  /* Wandered off it.
     Amber said "something is wrong", and nothing is: the reading is intact and
     reads normally from here, it just cannot grow. So the chip goes *quiet* —
     neutral ink, a dashed edge — which is the same thing the disabled buttons
     beside it are saying, in the same language. A second colour would have been
     a third meaning in a row that already has two. */
  .branch.off {
    color: var(--fg-dim);
    background: color-mix(in srgb, white 7%, transparent);
    border-style: dashed;
    border-color: color-mix(in srgb, white 26%, transparent);
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

  /* A remark, in the gloom's own colour rather than the warning amber: nothing
     here has gone wrong. It arrives from just above and leaves on its own. */
  .notice {
    flex: none;
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 6px 14px;
    font-size: 12px;
    color: var(--gloom-deep);
    background: color-mix(in srgb, var(--gloom-deep) 10%, var(--bg));
    border-bottom: 1px solid color-mix(in srgb, var(--gloom-deep) 28%, transparent);
    animation: noticeIn var(--fast) var(--ease-out) both;
  }
  /* The names, as badges: the sentence is scaffolding, these are what you read —
     and what you click, since the reply to this notice is a checkout elsewhere. */
  .bbadge {
    font: inherit;
    font-family: var(--mono);
    font-size: 11px;
    color: inherit;
    background: color-mix(in srgb, var(--gloom-deep) 16%, transparent);
    border: 1px solid color-mix(in srgb, var(--gloom-deep) 30%, transparent);
    border-radius: 4px;
    padding: 1px 6px;
    cursor: pointer;
  }
  .bbadge:hover {
    background: color-mix(in srgb, var(--gloom-deep) 26%, transparent);
  }

  .notice .mark {
    font-family: var(--mono);
    opacity: 0.7;
  }
  .notice.leaving {
    animation: noticeOut 0.6s ease both;
  }
  @keyframes noticeIn {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
  }
  @keyframes noticeOut {
    to {
      opacity: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    /* It still appears and still goes; it just does not travel to get here. */
    .notice {
      animation: noticeFade var(--fast) ease both;
    }
    @keyframes noticeFade {
      from {
        opacity: 0;
      }
    }
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
  .hsec .more {
    font: inherit;
    font-size: 11px;
    color: var(--fg-dim);
    background: none;
    border: 0;
    padding: 0;
    cursor: pointer;
  }
  .hsec .more:hover {
    color: var(--accent);
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
  .rbucket {
    padding: 6px 12px;
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.11em;
    text-transform: uppercase;
    color: var(--fg-faint);
    background: var(--bg-inset);
    border-bottom: 1px solid var(--line-soft);
  }
  .rbucket:not(:first-child) {
    border-top: 1px solid var(--line-soft);
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
  .rname {
    flex: 1;
    min-width: 0;
    font-family: var(--serif);
    font-size: 14.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* Still called after its module: the same invitation the band gives. */
  .rname.unnamed {
    color: var(--fg-faint);
    font-style: italic;
  }
  .rtags {
    flex: none;
    display: flex;
    gap: 4px;
    min-width: 0;
  }
  .rtag {
    font-family: var(--mono);
    font-size: 9.5px;
    line-height: 1;
    padding: 2px 6px;
    border-radius: 999px;
    white-space: nowrap;
    /* On paper rather than ink, so the same hue needs a darker lightness to hold
       its own — one token pair, two surfaces. */
    color: oklch(0.45 0.12 var(--hue));
    background: oklch(0.45 0.12 var(--hue) / 0.12);
  }

  /* The origin filename was here and went: the name identifies the gloom, and a
     second identifier in every row is a column you read past. */
  .rcount,
  .rwhen {
    flex: none;
    text-align: right;
    font-family: var(--mono);
    font-size: 10px;
    color: var(--fg-faint);
  }
  .rcount {
    width: 52px;
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
  .start .go kbd {
    font-family: var(--mono);
    font-size: 10px;
    padding: 1px 5px;
    border-radius: 3px;
    background: color-mix(in srgb, white 14%, transparent);
    color: #b8f0e6;
  }
  .start .where {
    display: grid;
    gap: 2px;
    font-size: 12px;
  }
  .start .where b {
    font-family: var(--mono);
    font-size: 12px;
  }
  .start .where span {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--fg-faint);
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
