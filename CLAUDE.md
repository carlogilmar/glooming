# lgtm

A desktop tool for **reading a codebase deeply** — onboarding to one you do not
know yet is the case it is built for. You open a **project** and land on its
**Hub**: the project's glooms as a table of contents. Open a module — `⌘T`, the
file tree (`⌘B`), a reference — and the window splits: source on the left,
read-only and always live; on the right, the **Woop**, what the code shows you
(its moduledoc, its surface, the boundary it reaches through). Following a call
opens the next module, and your trail is walkable with `⌘[` / `⌘]`.

Writing is optional. When there is something worth keeping you start a
**gloom** — a note in the project's knowledge base, in your own words, whose
references name any function in the codebase by its full module name. The right
pane flips between the Woop and the Gloom (`⌘1` / `⌘2` / `⌘J`). Clicking a
reference focuses it in the code, and **glooming** (`▷ Gloom`, `⌘R`) walks the
code in the order your prose takes it, across modules — including ones nobody
opened. A project collects glooms, one per thing you worked out, arranged in the
Hub by topic.

A gloom stores your prose and nothing else. The code is **always read live**, so
a reference finds its function wherever it has moved, and strikes itself through
when it is gone.

Not every file is a module: a config script and a test suite get views of their
own, any other file opens as plain read-only text, and the Woop says *File not
supported* rather than showing empty boxes.

Elixir is the only language so far. The architecture assumes more will follow.

**The vocabulary** (UI words; the code keeps its older names): a **project** is a
folder you explore; a **gloom** is a note (`docs`, `Doc`, `reading` in code); to
**gloom** is to walk one in read mode, and **glooming** is doing it; the **Woop**
is the live views (`ExploreSections`, `explore`); the **Hub** is the project's
front page.

> The name is the goal: a file is done when you understand it well enough to
> say *looks good to me*.

> If you are a Claude session being onboarded: read this file end to end, then
> `IMPLEMENTATION_PLAN.md` for the *why* behind each decision. This file is the
> map; the plan is the reasoning. `README.md` is deliberately short — what it is,
> requirements, getting started — so the detail lives here and nowhere else.
>
> **The `mockup/*.html` files are the visual contracts** — standalone,
> dependency-free pages the real UI was built from. If a component and its
> mockup disagree, the mockup is right. Open them; they are interactive.

## What this is not

These are deliberate, and pushing back on them needs a real argument:

| Not doing | Why |
|---|---|
| Editing source code | lgtm reads. Your editor edits. The left pane is read-only, permanently. |
| Git diffs, PR fetching, branch checkout | Cut during design. You read committed files; lgtm just opens a path. |
| Call graphs, "who calls this" across the project | The project index knows which module lives in which file, and nothing more. Each file is still parsed alone; your prose does the joining. A reverse-call index is the obvious next step and has not been argued for yet. |
| Groups you create and manage | A gloom has no file set to manage. The modules it covers are whatever its prose names — derived, never stored. |
| Snapshots of the code | Cut on purpose when the project became the unit (see *The project is what you explore*). A knowledge base about live code decays, and the tool's job is to make that decay visible — struck-through references — rather than to freeze the code and drift silently away from what runs. |
| Plain line references (`L25-29`) | They cannot follow code that moves and cannot say they broke. Lines are written *inside a function* instead: `Billing.charge/2:3-6`. |
| Blocks in the note (`lgtm:*` fences) | Their values were text, which was right only while a snapshot guaranteed the code under them could not change. The live views beside the note do the job, and cannot disagree with the code. |
| Any AI/LLM involvement | The explanation is *yours*. Writing it is the point — generating it would defeat the tool. |
| `import` in the reach block | It puts functions in scope unqualified, so a bare `cast(...)` can't be told from a local call. Showing it with zero call sites would read as a bug. |

A few things were **built and then cut**, which is worth knowing so they don't
get rebuilt:

- **An arc diagram of the reading path**, drawn in a widened divider. It needed
  explaining, which meant it wasn't working. The same data as a plain two-axis
  chart is legible — parked as a possible view, not built.
- **A soft-wrap toggle.** Wrapping is now unconditional; there is no case where
  scrolling sideways to finish a line is the better trade.
- **A blame legend in the footer.** The gutter already names each author where
  the author changes, which is the same information where you are looking.

Git is touched in exactly two read-only places (see `src-tauri/src/git.rs`):
the branch label is a plain read of `.git/HEAD`, and `git blame` shells out
lazily, only when the button is pressed. Nothing ever mutates a repository.
(`git::history` — `git log --follow` for the cut stats block — is still there,
tested, and called by nothing.)

## Stack

Deliberately identical to Alexandria and Xray, so all three are one stack.

- **Frontend**: SvelteKit + `adapter-static` (SPA, **no SSR** — enforced by
  `src/routes/+layout.ts`), Svelte 5 runes (`$state` / `$derived` / `$effect`),
  Tailwind 4, TypeScript. `markdown-it` for the Gloom and the moduledoc,
  `highlight.js` (core build, Elixir only) for code. (`d3-hierarchy` drew the
  treemap and left with it.)
  `ignore` (Rust) walks the project with `.gitignore` honoured.
- **Backend**: Rust + Tauri 2, `sqlx` against SQLite, `tree-sitter` +
  `tree-sitter-elixir` for parsing. DB at
  `~/Library/Application Support/com.alertmedia.lgtm/lgtm.db`; migrations run
  at startup via `sqlx::migrate!("./migrations")`.
- **Toolchain**: pinned by `.tool-versions` (asdf) — rust 1.95.0, nodejs 25.9.0,
  pnpm 9.15.0. **pnpm only**; a `package-lock.json` or `yarn.lock` is a bug.

**The app is `Glooming`; the bundle ID stays `com.alertmedia.lgtm`.** `productName`
is what names the `.app`, the menu bar and the DMG, and it is now Glooming. The
identifier is deliberately untouched: it is what the database path is derived from
(`~/Library/Application Support/com.alertmedia.lgtm/lgtm.db`), so changing it would
orphan every gloom anyone has written. If it is ever renamed, that is a migration —
copy the directory, do not just rename the id.

The Cargo package and the npm package are still `lgtm`: they name the *source*, not
the product. The **binary** is `Glooming`, though, via `[[bin]]` — and that one is
not cosmetic. `tauri build` renames the binary to `productName` when it bundles, so
the `.app` was already right; but `tauri dev` runs the cargo binary directly, and
macOS labels the dock with the process name. Until the bin target was renamed the
dock said `lgtm` all day while the built app said Glooming. Tauri's own docs prefer
this over `mainBinaryName` for exactly that reason.

**The DMG is a drag-to-Applications window.** `bundle.macOS.dmg` sets a 620×420
volume window with the app at x=165 and the Applications alias at x=455 — the
layout every Mac user already knows, rather than the bundler's slightly wide 660×400
default. `minimumSystemVersion` is 11.0, which is the floor for the `color-mix()`
and `:has()` this UI leans on in WebKit.

**The background is generated, not drawn.** `scripts/dmg-background.mjs` writes
`src-tauri/dmg-background.png` — 620×420, ~9KB — and it has no dependencies,
because macOS ships neither ImageMagick nor PIL: it fills a Float64 buffer, then
emits the PNG itself with `zlib.deflateSync` and four chunks. The alternative was a
binary nobody can diff; this way changing the palette is changing a hex string.

**It is light, and that is not a preference.** Finder draws the icon labels —
*Glooming*, *Applications* — in black, and gives no way to change them: a
`.DS_Store`'s view options cover text size and label position, never colour. The
first version was an ink plate in `--gloom-bg`, which put black text on a near-black
ground and made both words unreadable. So the palette inverts: warm paper for the
ground, `--gloom-deep` (the on-paper teal the app already needed) for everything
drawn on it. Measured across the whole label band, the darkest pixel still gives
black text **17.3:1**.

What it draws is the app's own picture: the **strata** of the cut `shape.ts` at 9%
(thickness ∝ √lines; the script carries its own copy of the arithmetic), faded out through the middle so the two
icons sit on quiet ground, and a dashed teal arrow from the app to the Applications
alias. The plinths sit *below* the labels rather than through them — Finder puts a
name about 42px under an icon's centre, and a line across a word is worse than no
line. **Its geometry is tied to `bundle.macOS.dmg`** — the
arrow runs between x=165 and x=455 at y=210 because that is where the config puts
the icons. Move one and you must move the other, or the arrow points at nothing.

Regenerate after a palette change:

```bash
node scripts/dmg-background.mjs
``` The app icon is generated from `app-icon.png`
at the repo root (`pnpm tauri icon`, no arguments needed — that's the default
input name). Mobile icon output is deleted on purpose; this is a desktop app.

## Dev commands

> **If you are a Claude session: do not run the app.** `pnpm tauri dev` is mine,
> not yours — I run it and I verify the change myself. Starting it also takes
> port 1420, so an instance you left behind is one I have to go and kill before
> I can look at anything.
>
> What I want from you instead is everything that can be checked without a
> window, and then the hand-off:
>
> ```bash
> cargo test   --manifest-path src-tauri/Cargo.toml
> cargo clippy --manifest-path src-tauri/Cargo.toml
> pnpm check
> python3 scripts/motion-audit.py
> python3 scripts/effect-audit.py
> python3 scripts/component-audit.py
> ```
>
> Then tell me what to look at and what you could not check — "open a test file
> and the describes fold" is worth more than a screenshot I did not ask for.
> Where the logic is pure, probe it through `esbuild` or a throwaway `node -e`
> rather than reaching for the app; that is what the *Testing* section is about,
> and it catches more than clicking does.
>
> `pnpm tauri build` is different — that one is worth running when the build
> itself is the thing in question, because it fails in ways nothing else shows.

```bash
asdf install                 # first, always — a mismatched toolchain fails weirdly
pnpm install
pnpm tauri dev               # full app
pnpm dev                     # frontend only (browser, no IPC — panes render, commands fail)
pnpm check                   # svelte-check
pnpm build                   # frontend production build
pnpm tauri build             # .app / .dmg

cargo test   --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml
```

If `pnpm tauri dev` dies with **`Port 1420 is already in use`**, an orphaned
Vite server survived a previous run: `lsof -ti:1420 | xargs kill -9`.

**If you move or rename the checkout, delete `src-tauri/target`.** The failure it
otherwise gives names a path that no longer exists and a file nobody wrote:

```
failed to read plugin permissions: failed to read file
'/…/old-name/src-tauri/target/release/build/tauri-<hash>/out/permissions/app/autogenerated/commands/app_hide.toml'
```

`tauri`'s build script publishes its `OUT_DIR` to dependents as an **absolute
path** (`cargo:…PERMISSION_FILES_PATH=…`), and Cargo caches that metadata in
`target/<profile>/build/tauri-<hash>/output`. After a rename the script's *inputs*
are unchanged, so Cargo considers it fresh and **replays the old path** to this
crate's build script, which then reads a file under the previous directory name.
About 110 build-script caches carry the stale path; the tauri and
`tauri-plugin-*` ones are the ones that break.

The tell that this is a stale cache and not a code problem: **`pnpm tauri dev`
keeps working.** Dev builds with `--no-default-features`, which is a different
build unit whose script did re-run — so only `pnpm tauri build` fails, which
looks like a release-only bug and is not one.

`cargo clean -p tauri` is not enough (it leaves the plugin caches). The target
directory is a cache with nothing precious in it:

```bash
rm -rf src-tauri/target      # then build as usual; ~2 min from cold
```

## Repo layout

```
mockup/workspace.html          THE CONTRACT for the project model: home, exploring, writing, glooming
                               (its sidebar-of-glooms and split pane were since replaced — see the Hub, Woop and Gloom)
mockup/hub.html                the Hub: topics in columns, Recent, quick look (its modal state was cut)
mockup/broadway.html           the Broadway Woop: how start_link wires it, and the lifecycle graph
mockup/index.html              the original single-file contract — parser line numbers are pinned to it
mockup/deps.html               the reach diagram's visual contract
mockup/surface.html            the surface's visual contract
mockup/kinds.html              config / test / fallback — the non-module kinds
mockup/tests.html              navigating a suite: the two clicks, and three navigation candidates
mockup/reading.html            scroll-driven reading, first cut
mockup/authoring.html          read mode as shipped, plus the edge cases
mockup/motion.html             the animation contract, replayable
mockup/explore.html            the explore drawer: surface + reach beside the note
mockup/motion-audit.html       every motion finding, before and after
mockup/reading-theme.html      read mode as a theme: five palettes, five faces, measured
mockup/home.html               the home screen as it was before projects (history)
mockup/{group,files,stack,import}.html   history: the file set, the files modal, import — all cut
scripts/motion-audit.py        every animation vs its reduced-motion rule
scripts/effect-audit.py        $effects that read and write the same $state
scripts/component-audit.py     components imported but never rendered
app-icon.png                   icon source (1024², RGBA); also copied to static/
IMPLEMENTATION_PLAN.md         the v1 reasoning, plus §10: what the project model changed and why
README.md                      one paragraph, requirements, getting started — nothing else

src/
  app.css                      design tokens for light and dark; hljs → token mapping
  routes/
    +layout.svelte             theme init
    +page.svelte               shell: home, the band, the Hub or the workspace, file tree, index, trail, keys, autosave
  lib/
    components/
      CodePane.svelte          source, line numbers, blame, focus, search, vim motions, trail ← →
      DocPane.svelte           Woop | Gloom, preview/edit/read mode, references, the ambiguity chooser
      FontStepper.svelte       A− / A+, shared by both panes
      ExploreSections.svelte   the Woop: moduledoc, surface, reach — or settings, or the suite — per file kind
      Sky.svelte               the masthead's weather: bloom, cosmos or aurora, rolled per visit
      FilePalette.svelte       ⌘T — find a file in the project, or paste a path
      FileIcon.svelte          the tree's icons: folder, the Elixir drop, a page per kind
      FnPalette.svelte         ⌘P — jump to a function by name
      RefMenu.svelte           / — insert a reference to the module on screen or one of its functions
      HelpModal.svelte         ? — what everything does
      Divider.svelte           the draggable split between code and the right pane, double-click reset
      Hub.svelte               the project's front page and ⌘K: topics, Recent, filter, quick look, delete
    stores/
      theme.svelte.ts          ported from Alexandria; lgtm defaults to LIGHT
      focus.svelte.ts          which function is selected — shared by BOTH panes
      fontSize.svelte.ts       a remembered, clamped font size; one per pane
    explore.ts                 what the views show: surface rows, reach lines
    fileset.ts                 the index + loaded files: which module means what, how it is shown, the / vocabulary
    filetree.ts                the sidebar's file tree: rows, folding, mentioned files
    markdownit.ts              markdown-it + references (no blocks)
    refs.ts                    references in prose, resolved by name across the project
    slash.ts                   when the `/` menu is still a live query
    select.ts                  one signature → every span worth highlighting
    when.ts                    relative dates
    deps.ts                    draws the reach diagram (see mockup/deps.html)
    tags.ts                    a gloom's tags, in docs.label
    hub.ts                     what the Hub says about a gloom (by rendering it), grouping, moves
    moduledoc.ts               a module's @moduledoc, rendered for the Woop, with its headings
    broadway.ts                the Broadway Woop: wiring + lifecycle, hand-written SVG
    exporting.ts               a gloom's name → a filename (unused until export returns)
    ipc.ts                     typed wrappers over invoke()

src-tauri/
  src/
    parse/elixir.rs            tree-sitter → Outline   ← the load-bearing file
    parse/kinds.rs             config scripts and test suites
    parse/broadway.rs          `use Broadway`: start_link's options and the handle_batch clauses
    index.rs                   the project index: module names → files, by line scan
    git.rs                     .git/HEAD read + lazy blame (+ an unused log)
    db/{mod,models,docs}.rs    pool, serde shapes, gloom CRUD
    db/projects.rs             remembered folders and their gloom counts
    db/topics.rs               the Hub's topics, and arranging glooms in them
    commands/files.rs          open a file (always live), branch, blame
    commands/docs.rs           gloom CRUD, the new-gloom invitation, export_note (no UI yet)
    commands/hub.rs            the Hub's payload, topic CRUD, arrange
    commands/projects.rs       open a folder, walk every file in it, index its modules
  migrations/0001…0003         history
  migrations/0004_glooms_in_a_project.sql   drops snapshots and file sets; glooms belong to a project
  migrations/0005_topics.sql   topics, and a gloom's topic and place
  tests/pipeline.rs            end-to-end parse checks, pinned to the mockup
```

## Core concepts

### The project is what you explore; a gloom is a note in it

`mockup/workspace.html` is the contract for the model; `mockup/hub.html` for the
Hub. What was cut on the way here — snapshots, file sets, blocks, import — is
listed at the end of this section. In older prose below, "the reading" means the
gloom, and "the views" or "explore" means the Woop.

**The day it is built for.** You open Glooming and pick a project. You browse its
modules, follow a call into another one, and at some point start a gloom — *How
an invoice gets charged* — and write while you read. Tomorrow is a different
feature and a different gloom. Weeks later, opening `Billing` shows a teal dot in
the tree because a gloom mentions it, and a renamed function shows as a
struck-through reference in the note that explained it. A knowledge base about
live code decays; the tool makes the decay visible.

**Three things stand where a reading's file set used to:**

| | What | Stored? |
|---|---|---|
| the index | every module in the project and its file (`index.rs`, a line scan) | no — rebuilt on open and on window focus |
| the loaded files | files read this session, with outlines: on screen, walked through, named by a gloom's prose | no — a cache of live reads |
| the gloom | title, tags, markdown, `project_id` | yes, `docs` |

**The index is names only.** Parsing every file with tree-sitter to answer "which
file is `Weather.Pipeline` in" would be wasted work; a `defmodule` scan with
indentation-tracked nesting answers it exactly for formatted code. Only a file you
open, or one your prose names, is parsed in full. `namedPaths` finds the qualified
names in the markdown whose files are not loaded, and the shell loads them after
each edit (250ms debounce); the note re-renders when they land. Until then such a
reference renders *plain*, never struck through — "not read yet" is not "gone".

**Coming back to the window re-reads everything loaded and rebuilds the index**,
so an edit made in your editor is there when you return. Only files whose source
changed are replaced, so unchanged panes do not re-render.

**Glooms belong to a project.** `docs.project_id` has no `ON DELETE CASCADE`, and
`forget_project` refuses while glooms exist: tidying the recents list must never
be the gesture that deletes what you wrote. Home lists projects, with their gloom
counts; inside one, the Hub lists its glooms (below).

**The band is always there.** Inside a project the ink band under the header is
where you are: a gloom's name and tags, *The Hub*, or *Exploring — nothing is
saved*. It used to appear only once a gloom was open, so the way in to glooms
lived in the sidebar, which made the band a surprise rather than a place. A
`Glooms ▾` picker sat in it for a while and was replaced by `⌂ Hub` — anything
that hangs below the band must render *outside* it, because the band clips for its
opening sweep.

**Exploring needs no gloom.** With none open the right pane is the Woop alone and
there is no Woop | Gloom switch; the band says so, which is why the Woop carries no
*Start a gloom* bar. `DocPane`
stays mounted in that state (`gloom={false}`) because it owns the reach
diagram's hover and trace wiring. A new gloom is unnamed, opens with its name
field focused, and starts as the invitation (`INVITATION` in `commands/docs.rs`).
Opening a gloom lands the code pane on the first module its prose names; closing
one is done by opening another from the Hub. `close ×` was cut from the band
once the Hub became the way between glooms: closing to "no gloom" was a state
nobody went to on purpose.

**The band starts with its actions**: `⌂ Hub` filled in `--gloom` (hollowed while
you are on the Hub, so it reads "you are here" and a click goes back), then
`+ New` tinted in `--gloom-new`, a lilac that means nothing else in the chrome,
then a hairline and the name. They are the same in every gloom, so they get a
fixed place instead of drifting after a name of any length.

**The sidebar** (`⌘B`, or `‹` in its own header; hidden in read mode) is a **file
tree, the way an editor shows one**. It starts **collapsed** — the Hub and `⌘T` are
how most files are reached — and stays open once you open it (`sidebar` =
`shown`). Its right edge resizes it (160–520px,
remembered as `sidebarWidth`, double-click resets), and hiding it leaves a 30px
rail with a **Files** button (icon, the word set vertically) that brings it back.
The toggle was first a word in the app header, nowhere near the panel it acted
on, and then a bare 14px strip with a ›, which read as the pane's border. The tree shows every file, not only Elixir. It was a
namespace tree of modules first, and that was a partial map that looked complete:
reading a codebase means `mix.exs`, the config, the migrations and the README as
much as the modules. Folders first, then files, case-insensitive (`filetree.ts`,
pure and probed). Arriving at a file unfolds the way to it **once** (`reveal`, in
`showFile`) — it used to be forced open on every render, so a folder holding the
open file could not be folded: the click removed it and the next render put it back.

`project_files` walks with `.gitignore` honoured even outside a git checkout
(`require_git(false)`), dotfiles shown (`.formatter.exs` is part of a project) and
`SKIP` on top for build output, dependencies and OS litter. `⌘T` reads the same
list. The module index still scans only `.ex`/`.exs`.

**A file with no parser opens as plain text, read-only,** and the views say *File
not supported* rather than drawing an empty panel. `open_file` refuses a binary
file (a NUL in the first 8KB — git's own test) or anything over 2MB with a sentence
naming the file, because `stream did not contain valid UTF-8` says nothing to
someone who clicked a logo.

Icons (`FileIcon.svelte`) are deliberately few: a folder, the Elixir drop (solid
for a module, hollow for a script, with a check for a test), and a page carrying a
mark for config, docs, data, web, images and locks. What you scan a tree for is
*Elixir or not, test or not, config, docs or data* — an icon theme's hundred kinds
at 14px is colour noise. Hues come from existing tokens, never new ones.

A teal dot marks a file the open gloom names, and a hollow one a folded folder
with such a file inside — the old hollow-dot signal inverted, because the set is
now derived from what you wrote. A file's dot fills with one ring the moment your
prose first names it.

**The trail** is every module you opened, as back/forward (`⌘[` / `⌘]`, and ← →
in the code pane header). Read mode's file crossings do not add to it — scrolling
a note is not navigating.

#### References, across a whole codebase

| Written | Means |
|---|---|
| `` `MyApp.Billing.to_cents` `` | every arity — what `/` inserts |
| `` `MyApp.Billing.to_cents/1` `` | narrowed to one arity |
| `` `MyApp.Billing.charge/2:3-6` `` | lines 3–6 of the function, counted from its first line |
| `` `to_cents/1` `` | **not a reference** — plain inline code |
| `` `Weather.Pipeline` `` | the module itself — a step that opens it |

**The text stores the full name; the chip shows the shortest unique one.**
`moduleLabels` gives each module the shortest dot-suffix that names it uniquely in
the project, so `MyApp.Billing` shows as `Billing` while `Weather.Pipeline` and
`Ai.Pipeline` each show as themselves. Storing the short form would make a note
that is unique today ambiguous the day someone adds a second `Billing`; the full
name also makes an exported note resolve on a teammate's machine.

**Ambiguous is a third state**, beside resolved and broken. A typed
`Pipeline.run` with two Pipelines renders dashed amber; clicking it offers the
candidates and rewrites every occurrence of that text to the chosen full name.
`matchModules` takes an exact name outright, otherwise dot-boundary suffixes —
`Pipeline` never matches `ProcessingPipeline`.

**A reference always names its module; bare names are plain code.** They used to
resolve against the module the previous reference named — the "thread" — and
before that against an origin file, then every file in order. Both read well and
carried no information: `enabled?/2` alone does not say which file it is in,
moving a paragraph silently re-pointed it, and across a codebase `changeset/1`
exists in thirty modules. With the thread gone, a reference depends on nothing
but its own text, so the resolver is stateless and read mode walks the same path
wherever you stand.

**Writing about modules you have not opened is the normal case.** Every full name
tells the shell which file to read (`namedPaths`, from the index), so a note can
reference a dozen modules and read mode walks all of them without one being opened
first.

**Lines inside a function replace line ranges.** Counted from the first clause's
`def`, so edits elsewhere cannot move them; past the function's last line they
dangle, which is the one way they can still drift and it is visible.

**Read mode skips what cannot be walked.** Steps are `code.ref[data-line]`;
ambiguous and broken chips carry no `data-line`, so the walk passes them rather
than stopping or highlighting the wrong code.

**The `/` menu offers the module on screen, then its functions** — the module
itself first, as a step that opens it — and inserts the full module name with no
arity. It was every function in every file loaded this session, a set that made
sense when a gloom was a set of files and across a codebase was an accident of what
you had clicked. For anything else, type the full name.

#### Sharing

**Export and import are being redesigned as one feature, and neither has UI right
now.** Copy and Save… were taken off the Gloom's header to make room for that;
`export_note` (the one write outside the DB, to a path a save dialog chose) and
`exporting.ts` stay for it. What is settled: a gloom travels as its markdown,
verbatim — and with the full module name in every reference, it resolves against
any checkout of the same repository. Opening a shared `.md` into the current
project replaces the old import, without a file list or a branch check, because
there is no snapshot left to validate.
Storing glooms as files in the repo is open again now that the 42× snapshot
argument is gone, and deliberately deferred.

#### Built and then cut, 2026-11

So they are not rebuilt by accident:

- **Snapshots** (`docs.source`, `doc_files`), staleness, and the per-file dots.
- **The file set**: `add_doc_file` / `remove_doc_file`, the origin file,
  `for_path` and the "already part of a gloom" chooser, `⌘⇧T` and `FilesModal` /
  `shape.ts` (the portrait of a change could return as a project-level view).
- **The branch guard** (`branch_mismatch`, off-branch gating, the notice). The
  branch chip is now a plain, copyable label of the live branch.
- **Import** (`import.rs`, `ImportModal`, the template).
- **Seeding** (`seed.rs`): a new gloom is the invitation and nothing else.
- **Every `lgtm:*` block** and its renderer — `stats`, `surface`, `treemap`,
  `functions`, `settings`, `tests` — and the `/` block commands. `deps.ts`,
  `explore.ts` and the views' drawing code stay; they draw the live views. A
  fence in a note is now just code. Stats and treemap may come back as *views*.
- **Plain line references**, `L25-29` and `billing.ex:25-29`.
- **Bare-name references** and the thread that resolved them.
- **The sidebar's gloom list, the `Glooms ▾` picker, the library modal, the Hub
  as a modal, `close ×`** — the Hub replaced all of them.
- **The split between views and note**, its grip, and `Views | Both | Note` —
  replaced by Woop | Gloom.
- **Copy / Save…** on the note, pending export/import.
- **HTML5 drag and drop** in the Hub — Tauri's window takes the events.

### To the user, a reading is a **gloom**

The name is a UI label and nothing else: `Reading`, `ReadingFile` and
`open_reading` stay as they are, and `reading` in the frontend still also means
read mode. Renaming the internals was considered and declined as churn.

**A gloom is a note in the project's knowledge base** — one thing you worked out.
It gets its own band under the app header, and the band is **filled with
ink, not tinted**: `--gloom-bg` is a deep teal-black in both themes, with
`--gloom-ink` (near-white) on it. A masthead is the one element that can be dark in
either theme without looking like a mistake, and it gives the window a top edge to
hang from — which a 9% wash never did.

Teal because it is the one hue carrying no meaning in the chrome: not the accent,
which means *selected* in both panes; not `--read` gold, which is a mode. `--gloom`
itself is now the **bright** teal, and it survives as the accent *inside* the band —
the wordmark, the rule under the name while you type, the wash when it opens. On
ink it measures 7.6:1; on the old pale band the same colour was 4.2:1 and had to be
pushed darker to be legible at all. Four tokens, because a dark surface needs its
own ink and its own muted grey: `--gloom`, `--gloom-bg`, `--gloom-ink`,
`--gloom-dim`.

**One line: `⌂ Hub` and `+ New`, a hairline, the name and its tags, then the
wordmark at the right end.** A title is the first thing after the actions, not the
middle of the line — centring it needed a three-column grid and 56px of height to
hold a stacked caption, and bought nothing. A file count lived on the right
briefly and was cut, then the gloom's set of files went altogether.

**`GLOOM` is lettering, not a badge.** Condensed and tracked out at `0.28em` in
`--display`, it reads as the name of the thing you are in; a filled pill read as a
status. The stack names Oswald first, so it is used if you have it, and falls back
to the condensed faces macOS ships — the app is offline and its CSP blocks a
webfont. Dropping a woff2 into `static/` and `@font-face`-ing it is same-origin and
would work, if Oswald should be certain rather than likely. Tracking also adds its
gap after the *last* letter, so the word carries `margin-right: -0.28em` or it
floats off the edge.

**The name is near-white on the ink**, which is the only thing in either pane set
that way — a masthead's title, not a label. An unnamed gloom stays `--gloom-dim`
and italic: the white is what naming it earns. Measured on the band: **12.5:1**
light and **15.6:1** dark for the name, 7.6/9.5 for the wordmark, 6.2/7.8 for the
invitation. Hover is white at 8%, because on a dark surface "you can edit this" is
a lift rather than a tint, and a tint would have meant inventing a second colour.

**Renaming happens in place, not in a box.** A white input dropped into a tinted
band is a form appearing in the middle of the chrome. The field is transparent
with a `--gloom` rule under it, so the name looks like the line of prose it
already was and the rule is what says it is live.

**The name is set in a serif.** `--serif` is a system stack (`ui-serif` → New
York → Georgia); the app is offline and its CSP blocks a webfont, so nothing is
downloaded. In a tool made of monospace and UI sans, a serif reads as a *title*
rather than as one more label — it is the one line here you wrote as prose. The
same serif is used wherever a gloom's name is *shown* — the Hub's rows, its quick
look, a topic's heading — so it is recognisable as the same thing; filenames and
paths stay mono and sans, because they are data rather than something you wrote.

**A gloom has a name, and it is `docs.title`** — empty until you give it one. Click
the name to rename it, `↵` to commit, `esc` to throw the edit away.

**A new gloom opens with its name selected**, so naming it costs a sentence and
skipping it costs one key. That is the only moment it happens: on a gloom you
have already named, taking the caret is an interruption rather than an
invitation. There is deliberately no dialog on the way in — asking for a name
*before* you have read anything asks at the one moment you cannot answer, which
is the same reason there is no "new group?" prompt.

**An unnamed gloom shows as an invitation** — *Unnamed gloom*, dimmed and italic,
with *say what you are here to find out* beside it. Derived from the empty title
rather than stored as a flag: a flag would have to be maintained by every path that
writes a title, and this one cannot drift.

The rename takes the server's row but puts **your** markdown back — an autosave
may be in flight, and adopting the server's copy would lose the sentence you are
half-way through.

### The markdown IS the data

Still true of the note — it is plain markdown, readable anywhere, and nothing is
added on the way out — but it no longer carries blocks of values. What a module
*is* comes from the Woop, which read the code as it is now; what you
*understood* is the prose. The two cannot drift, because only one of them claims
to describe the code.

### A file is not always a module

`FileKind` decides what the Woop shows, because what suits one shape says nothing
about another:

| Kind | Detected by | What the Woop shows |
|---|---|---|
| `Module` | `defmodule` with `def`s | **moduledoc** + **surface** + **reach** |
| `Config` | `import Config` / any `config` call | **settings**, grouped by app, env vs literal |
| `Test` | `use …Case` in a module body | the **suite**, then its **describes** |
| `Plain` | an Elixir file with none of those | one line saying there is nothing to navigate by |
| — | not Elixir at all | *File not supported*; the source opens as plain text |

(The table once decided which blocks a doc was *seeded* with. Nothing is seeded
now; it decides what you navigate a file by, which is what it always described.)

The fallback is the point of the whole mechanism. A config file parsed as a
module has nothing to lay out, and **an empty panel reads as broken** — so `Plain`
gets one line saying nothing structural was recognised. No error, no empty boxes.

`Config` is decided before the module walk (a config script has no modules at
all); `Test` after, since a suite *is* a module, just one where surface and reach
say nothing useful.

**Every row carries a span, not a line.** That's what lets clicking a test,
describe, setup or setting select the *whole* thing in the code rather than
dropping a cursor on its opening line — the same thing clicking a function does.

**Keywords are click targets too.** In the code pane `markWord` wraps the first
`test` / `describe` / `setup` / `config` on a line, exactly as it wraps a
function's name on a `def` line. The negative lookahead matters twice: it stops
`setup` matching inside `setup_all`, and stops a test named `"the config test"`
having the word in its *name* wrapped instead of the keyword.

**The config view's finding is `env` vs literal.** `System.get_env` versus a
hardcoded string is the difference between a value you can change at deploy time
and one baked into the release; `fetch_env!` is marked `env!` because it crashes
on boot when unset. A literal whose *key* looks like a credential
(`SECRETISH` in `parse/kinds.rs`) is reported as `secret` **without its value** —
that it is hardcoded is the finding, and a value on screen is a value in a
screenshot.

**A test selects as a span; a describe is gone to.** A test row passes its span
to `selectFromExplore`; without it the click resolved a **function signature**
with `locate`, and a test's name is not one — so it returned null and fell through
to `gotoLine`, a one-line cursor on the `test "…" do` line.

A describe deliberately does **not** select. Selecting forty lines dims nothing
useful and claims you are reading all of them, when what you are doing is
travelling to one of the tests inside — so it lands as a line cursor, which is
the word this app already has for a position, in `--read` gold rather than the
accent. `oncursor` is its own callback for that reason, and it clears the
function selection first: leaving it up would dim the file around a function you
are no longer looking at.

**A test's `@tag` is an `@spec`.** It came out of building the mockup. `line` is
the `test` call, so a skipped test's `@tag :skip` sat *outside* the span and got
dimmed one line above the test it is the most important fact about. Folding it
into the span is wrong too — it is not the body. It is exactly what an `@spec` is
to a function, so `TestCase` carries a `tag_range` beside `line`/`end_line`, and
it travels in `focus`'s **`spec` slot**: the violet `--mark` treatment, selected
with the test, distinct from it, and no new row state to compete with the five
already there. Stacked tags are one range, because they are one thing to a reader.

**Assertions are marked by kind, not shaded by count.** `count_asserts` returned
a `u32` on the grounds that "a one-assertion test looks different from a
five-assertion one". It does — but what it looks different *about* is the
author's style: one `assert {:ok, %User{email: ^e, id: id}} = …` checks four
things and drew palest, while six `assert` lines poking one field drew darkest.
The shade reported style as though it were thoroughness, which is precisely the
confident wrong answer the `+?` two elements away exists to avoid.

`Assertion { line, kind }` replaces it — `assert`, `error` (`refute`,
`assert_raise`), `message` (`assert_receive`) — classified on the **call name
alone**, which is why those are the three. `assert {:error, cs} = …` stays plain:
the pattern is in the arguments, so recognising it means matching text, and the
house rule is that the uncertain answer is drawn plain rather than confidently
amber. `asserts` survives as `assertions.len()`, so nothing reading the count
broke while the shading it fed was retired.

The marks are drawn in the code pane **only inside the selection**. Across a
whole suite they are a wall, and worse, a sixth row state arguing with `hit`,
`head`, `cursor`, `spec` and the blame tint — which is what got the sticky
function header deleted. Inside the selected test they are not a state at all:
they are the shape of the thing you already chose to look at. The gutter is
absolutely positioned in the 14px the line number already reserves, not a flex
column, or it would push the source over in *every* file to make room for
something only a suite ever draws.

**`@moduletag` is not the next test's tag.** `attribute_tag` matched `tag` and
`moduletag` and the caller could not tell them apart, so a `@moduletag
:integration` at the top of a file was pushed onto the pending list and became
the **first test's** tag. It has its own field now. This was invisible until a
tag started drawing its own span.

**The suite view's finding is that setup stacks.** A test starts from module
`setup_all` + module `setup` + its describe's `setup`, which can be a hundred
lines apart, so each describe shows the accumulated context its tests can
destructure. `provides` is a best-effort read of the block's *last expression*
(`%{user: user}`, `{:ok, repo: repo}`); a named callback (`setup :put_user`)
lives elsewhere in the file, so its keys are `None` — **unknown, which is not
the same as "provides nothing"** — and the UI shows `+?` rather than guessing.

### How the right pane got to Woop | Gloom

Three shapes, each replacing the last; *Woop and Gloom* (below) is the current one.

1. **One column**: the views above the note in one scroll — scroll up to look
   something up, down to write. It replaced a fixed 270px drawer with its own
   scrollbar and a collapse toggle, and the toggle was the tell: a band
   permanently taking room from the pane you write in, and **two scroll regions in
   one column, so neither can be scrolled past**. That lesson stands.
2. **A split you own**: a grip between views and note, draggable, collapsible at
   either end with ⌥1 / ⌥2, remembered per gloom. On a long surface the note had
   started a screen and a half down; the split made the height yours.
3. **Woop | Gloom**: one at a time. Each half of the split was too short to do its
   job, and the grip was one more thing to manage.

What survived all three:

- **The Woop is warm.** A grey mix read as *dirty paper*; a teal wash read as a
  green panel dropped into a warm room, arguing with the gold `▷ Gloom` and the
  header. Both failed by being a different **hue** where what was needed was a
  different **level**: the Woop is `--read` at 9% over the paper, more of the
  pane's own warm chrome. Teal stays where it means something — the boundary's
  outside, the gloom band.
- **`body` is the note's scroller**, not the pane. Read mode's lead-in, tail, band
  and step tests all measure `body`, so they describe the thing being read.
- **Nothing collapses with a transition** — see *Motion means something*.

### The Woop's sections

What you navigate the file on screen by (`ExploreSections.svelte`): for a module,
its moduledoc, its **surface** and its **reach**; for a config, its settings; for a
suite, its describes. They follow the file — every file behaves the same, and
nothing has to be seeded.

**These are not blocks.** They read the live `Outline`; nothing in `explore.ts`
parses a fence. They describe the code as it is *now*.

**`explore.ts` sorts by `(name, arity)` explicitly**, comparing the parts rather
than `localeCompare` on the whole signature — which once gave
`get_user_by_email/1, get_user!/1, get_user/1` against the Rust seeder's
`get_user/1, get_user!/1`. The Rust test that pinned the same order went with
`seed.rs`, so nothing pins it now — probe `surfaceOf` if you touch it.

**The surface is a table, capped at six rows.** Two columns — the name, and the
line it is on — with tabular figures so the digits stack, which is the whole
reason it is a table and not a list. Past six rows it scrolls, so a 40-function
module costs exactly what a 3-function one does and the reach sits at a constant
offset. **No badges**: `default args` and `3 clauses` are true, but at forty rows
they turn a catalog you scan into a wall of annotations — they are in the row's
`title`, and the arity in the name already hints at the first.

**The diagram is inline, not behind a button.** An earlier version put it in an
overlay reached by `⌥R`, on the argument that it needs 2.6–6.4× the room a list
does. In a scrolling region that argument evaporates: room is free below the fold.
A map you have to remember to open is a map you do not use.

**Isolating works from both sides.** Clicking an outside function asks *where is
this used*; clicking a **local** one asks *what does this touch* — the boundary
holds that one function and the outside column holds only what it reaches. Only one
can be open at a time: they are two answers to the same picture. A left-hand click
also selects the function in the code, because that is the other half of what you
meant by clicking it.

**Clicking an outside function isolates the boundary on it.** The module is
redrawn with **only the functions that call it** inside, and only the lines that
reach it — *where is this used* is the question you have the moment your eye lands
on `Repo.insert/1`, and two lit lines inside twelve grey rows do not answer it.
Isolating narrows **both** columns: eleven other names still listed beside the
answer would be pointing somewhere else.

The layout is **re-run**, not hidden with CSS: hiding the other rows leaves the
survivors at their old `y`, so every line points into a gap where something used
to be. `renderDeps` takes an `isolate` argument and filters `locals` *before*
laying out — and filters the edges to that one target.

**Narrowing lights the lines — and everything else it drew.** The travelling dash
only runs on `svg.focusing .edge.lit`, so an isolated view drew its connections
*dead*. Isolating is a selection, so it wears the selected state — `focusing` on
the svg, `lit` on every edge, and `lit` on the `.fn` / `.rfn` groups too, or
`focusing` would dim the names that *are* the answer to 30%.

**Three ways out, like the focus pill's four**: a `× whole boundary` button in the
diagram's own top-right corner, clicking the isolated function again, and clicking
empty space inside the diagram. The button is on the *picture* — an exit you have
to go and find is what makes a filtered view feel like a trap. The empty-space exit
is scoped to `.reachwrap`, so a click on the jump button is not silently an exit.

**Reach is the cross-file navigator.** A call landing in a module the project index
knows is a **jump**: a named control under the diagram, `open billing.ex ↗`, opens
it and focuses that function. The click on the dot answers "who calls this", the
question you ask *first*; a button that names the file it will open is more
discoverable than an invisible click target ever was. A call landing outside the
project — `Repo` from a dependency — says so.

**Hidden in read mode.** `DocPane` renders the sections as a snippet and reports
the mode with `onreading` rather than sharing it, because the pane owns it and two
owners would eventually disagree with the button.

**The suite list has no inner scroll, and that is the correction of a
correction.** The first fix for a sixty-test suite was the surface's six-row cap.
But the cap exists so a long surface cannot bury the **reach below it**, and
nothing sits below the describes — so a 57-test suite became a 236px porthole
*inside* a region that already scrolls: two scroll regions in one column, the
thing this pane exists to avoid. The list fills the Woop and scrolls with it.

`overflow: clip` on the list, not `hidden`. Both clip children to the rounded
corner, but `hidden` makes the list a scroll container, and a sticky describe
header inside a box that cannot scroll is pinned to something that never moves.
Written correctly and visible never, which is the `overflow-x` bug's exact shape.

**A describe is a section, and it folds.** With nine describes the headers *are*
the map, so each carries its test count and a chevron. Folding is `display`, not
height — no animated collapse is right, because it has to move everything below
it. The toggle is a **sibling** of the header button, never nested in it. Fold
state is session state keyed by file: a suite that reopens with half its describes
hidden looks broken rather than tidy.

**Module scope is stated once.** `suiteScope` is the stack at the top; a describe
shows only its `adds`. `async: false` gets a colour rather than a dimmed label: it
says the suite touches shared state, the most reliably reviewable fact in a test
file.

**A named setup's keys are unknown, and that is not the same as none.** A *named*
callback (`setup :put_user`) is defined elsewhere in the file, so `testsOf` returns
the keys it **can** see plus an `unknown` flag, and the section shows `:user +?` —
here is what I know, and there is more. An earlier version collapsed the whole list
to null the moment anything was unreadable, and treated a named callback as
readable besides. The probe caught both.

### The reach diagram

`mockup/deps.html` is the contract; `deps.ts` is the port. The concept is a
**boundary, not a graph** — a node-link diagram of one file's dependencies is
always a star (one hub, N leaves, identical topology every time), so it carries
no information. Instead the module is a closed shape with its functions inside
in source order, and the only lines drawn are the ones that **pierce it**.

Functions that reach nothing are drawn grey with no line. That silence is the
finding, not an empty state.

Layout is deterministic: outside anchors are ordered by the **barycentre** of
their callers, which is the standard crossing-reduction heuristic and gives zero
crossings on typical modules. No force simulation — the same module must always
render the same picture.

**Both columns size the diagram.** The viewBox height is
`max(localsH, stackH)`, and each column keeps its natural spacing while being
centred in whatever height the other one forced. Sizing from the local functions
alone meant a small file with many aliases produced a taller right-hand stack
than the viewBox, and centring it then pushed the top *and* the bottom outside —
where SVG silently clips, with nothing on screen to say it had happened. Centring
each column separately also stops five functions being stretched thin across a
diagram made tall by a long alias list.

The left column comes from the outline's functions and the edges from its deps —
both live, so the picture is always the module as it is on disk.

Parser notes, all confirmed by dumping the tree rather than guessing:

| Written | Grammar |
|---|---|
| `alias MyApp.Repo` | `args[ alias ]` |
| `alias MyApp.{User, Profile}` | `args[ dot[ alias, tuple[alias, …] ] ]` |
| `alias X.Y, as: S` | `args[ alias, keywords[ pair ] ]` |
| `Repo.insert(cs)` | `call[ dot[ alias, identifier ], arguments ]` |
| `%User{}` | `map[ struct[ alias ] ]` |

**A function's identity is not its label, and the diagram has to match on the
identity.** A function with default arguments displays as `decode_message/1..2`
(`displaySig`), while Rust records a call site's caller as `Def::signature()` —
plain `name/arity`, the top one. `deps.ts` keyed its local functions by the *label*,
so every edge whose caller had default args failed its lookup, was dropped by
`if (!local) continue`, and the module it called was drawn in the outside column
**with no line at all**: a dependency that looks unused while the call sits in the
source. Locals now carry `key` (identity, what edges and hovers match) beside `sig`
(the label, what is drawn and what `focus.sig` matches). `paintFunction` narrows a
display sig to its identity for the same reason.

The two are the same string for every function without default arguments, which is
why one field did the work of two until a file turned up that had them.

**The pipe shifts arity.** `attrs |> Repo.insert()` is written with no arguments
but calls `insert/1`. `call_arity` adds one when the call is the *right* operand
of a `|>`; reporting `insert/0` would name a function that does not exist.
`a_pipe_adds_one_to_the_arity` pins it.

**Only aliased modules count.** A bare `String.trim/1` or `Enum.map/2` is a
call, not a declared dependency — the `alias` list at the top of the file is
what the author chose to depend on, and drowning it in stdlib noise buries the
one thing worth seeing. `only_aliased_modules_are_dependencies` pins it.

`import` is deliberately **not** covered either: it puts functions in scope
unqualified, so a bare `cast(...)` is indistinguishable from a local call
without a symbol table for the imported module. Showing it with zero call sites
would read as a bug.

### A gloom is about live code

It used to be pinned: every file it touched was snapshotted, the pane showed the
snapshot, and a changed file said so. That was right for reviewing one version of
a change and wrong for a knowledge base, where the point is that next month's
reading of a gloom describes next month's code. The database stays — it holds the
glooms, the projects and the topics — but it is no longer the reason a gloom is a
row rather than a file; see *Sharing* above.

### Motion means something

`mockup/motion.html` is the contract — standalone and replayable, with a reduced
motion toggle, because a motion spec you cannot re-watch is not a spec.

**Easing is chosen by direction, not by taste.** Three curves, copied verbatim
from the animation-audit catalog rather than tuned by hand — a curve you invent is
a curve nobody can check:

| Token | | For |
|---|---|---|
| `--ease-out` | `cubic-bezier(0.23, 1, 0.32, 1)` | entering or exiting |
| `--ease-in-out` | `cubic-bezier(0.77, 0, 0.175, 1)` | moving on screen |
| `--ease-drawer` | `cubic-bezier(0.32, 0.72, 0, 1)` | a drawer or sheet |

Plain `ease` is correct for hover and colour, and plain `linear` for constant
motion like the reach trace — those are not oversights. **`ease-in` is never
correct for UI**: it starts slow, delaying the exact moment you are watching.

One `--ease` used to do all four jobs. Splitting it was the highest-leverage
finding of the motion audit, because a token change improves every site at once.

**Cadence is a design token, like a colour.** `2.1s` had already been typed out
in four places by hand and a fifth copy was one edit from drifting, so it lives
in `app.css` with the rest, in **two tiers**:

| Token | | Means |
|---|---|---|
| `--slow` | 2.1s | ambient — Xray's `tmPulse` beat: the focus pulses, the orb |
| `--calm` | 1.9s | ambient — the treemap's top three, kept distinct |
| `--fast` | 0.18s | transient — a thing arrived |
| `--wipe` | 0.34s | transient — a thing arrived somewhere long |
| `--trace` | 0.62s | transient — a connection being drawn |
| `--ring` | 0.7s | transient — one pulse, then nothing |
| `--greet` | 1.35s | transient — a session announcing itself |

**Nothing added since the first pass is ambient**, and that is deliberate rather
than incidental: the app's budget for infinite motion is already spent, and more
of it is the single easiest way to make this worse. `--trace` borrows the beat
`focus.leaving` uses for the read-mode crossfade, so a connection being drawn and
a selection handing over feel like one tempo.

Each animation carries information rather than decorating:

- **The reach diagram traces outward.** `reachHosts()` walks the whole pane
  rather than the note, because the Woop's diagram (`.lgtm-deps` markup, a name
  kept from when it was a block) sits *outside* `.doc` — looking only inside the
  note rendered the trace and the arrival ring every time and ran them never, the
  `overflow-x` bug's exact shape in a different place. The pointer wiring lives on
  `.panebody` for the same reason, which is also why `DocPane` stays mounted with
  no gloom open.

  A static boundary shows you *that* `create_user/1` reaches `Repo.insert/1`,
  never that the call goes **out**. A travelling `stroke-dashoffset` does. The far dot then takes one ring on arrival,
  fired on a 260ms timer so it reads as the *consequence* of the dash getting
  there — both at once is the static picture again with extra noise. The ring is
  its own `<circle>` scaled by `transform` on a `fill-box` origin, because `r` is
  not reliably animatable and this way the renderer supplies no geometry for it.
- **A new gloom points at its blank page.** 260ms after it opens, a ring expands
  from the **invitation** — not the pane. "A gloom arrived" is decoration; you
  know, you just made it. The invitation is found *structurally*: the last
  paragraph, containing nothing but emphasis. That is what `INVITATION` is, and
  **the moment you type a word it stops matching** — so nothing has to remember
  whether a gloom is "fresh", and the nudge only ever appears on one you have not
  written in.
- **A reference arrives; it never fades in.** The rule from the read-mode notes
  still holds — transitioning the chip's *fill* left two references looking
  current at once. So the outgoing chip loses its state in the same frame and only
  the **incoming** one animates: an underline wiping in from the left. Hover's
  border is suppressed on an active chip, or one word gets two underlines.
- **The code wipes in, top down — as a fade, never a slide.** The refusal here
  was against motion in the code pane's *text*, and it narrowed rather than held:
  a **fade** costs nothing, because the glyphs never leave the position you are
  about to read them in. A *slide* is what the refusal was actually about — text
  that has to settle before your eye can land on it. So the line numbers, the
  blame gutter and the source all arrive on one beat, and only the gutter (which
  is chrome) travels the 4px. It runs at `--wipe` and 26ms a row, not `--fast` and
  16ms: those are calibrated for a short catalog you scan, and the same values
  down a whole pane read as a flicker rather than a sweep. The last row lands at
  652ms. `backwards`, not `both`: the
  fill has to hold the from-state through the stagger delay and then get out of
  the way, or it pins `opacity` and outranks `.row.hit .ln`, which is a
  declaration and loses to an animation. No gate is needed, because `CodePane` is
  keyed on the path — a mount *is* a new file.
- **The boundary assembles in reading order.** The shape, then what is inside it
  top to bottom, then what lies beyond, and the lines last — a connection cannot
  arrive before both of its ends exist. `deps.ts` carries the order as `--i` on
  the markup, because it is the only place that knows the drawing order; a pierce
  shares its function's index and a kind label shares its module's, or one label
  arrives as two. Every edge shares the last index, so they draw as one gesture.
  Opacity only: the parts are laid out against each other, and sliding them in
  from an offset would put a function somewhere its own line does not reach.
- **The directory arrives in order.** `--i` on the surface table's `<tr>` in
  `ExploreSections`, gated on the file's *path* so it plays once per file rather than on every render — staggered
  16ms in CSS, and **capped at row 12** — 200 functions would otherwise cascade
  for 3.4s, and waiting on an animation to look a name up is worse than no
  animation. Both columns share the index so they arrive together.
- **A gloom announces which one it is.** One breath of `--gloom` across the
  **whole band** when it opens, 180ms after the band lands so it reads as the
  *consequence* of arriving rather than as a second thing happening at once — the
  reach block's arrival-ring timing argument, reused. Lighting only the title read
  as a note about that one word; the band is what means "this session". Opacity
  only, so nothing moves while it happens — every label in there is text you might
  already be reading. Once, then nothing: a session starting is an event, not a
  state that needs holding.

  It runs at `--greet`, its own token, and **not** at `--ring`: that is the beat of
  a hit landing, and a greeting at that speed reads as a flicker — as though
  something went wrong. The curve is asymmetric too, up in 22% and down over the
  rest, because a symmetric fade is a flash where a long tail is something
  settling.

  **On ink, a wash does not read.** The flat overlay that worked on the pale band
  moved the band's luminance by 4 points and was, in practice, invisible. Light
  *moving across* a dark surface is what a dark surface can show, so the greeting
  is a **sweep**: one pass of `--gloom` from left to right, `-120%` to `320%` of a
  45%-wide pane — off one edge to off the other, computed rather than eyeballed —
  with a brief overall lift under it so the band is not unlit while the sweep is
  still at the left-hand end. Peak is 16 points of luminance, four times the wash.
  Under reduced motion the sweep hands its job to the lift and simply stops
  travelling.
- **A gloom settles in, and its name lands.** The band arrives from 6px above —
  it is the first thing that says *which* journey you are in, so the rest of the
  window reads as its contents rather than as a new screen. It is keyed on the
  doc id rather than gated by a class and a timer: a remount replays the arrival
  exactly once per gloom and never while you type in it. Naming one then draws a
  single wipe under the title in `--gloom` and lets it go — the reference chip's
  underline idiom, for the same reason, and it does not persist, because a
  permanent underline is a decoration rather than an event. Diffed against the
  previous title, so it fires on the transition and a gloom you named last week
  opens silently.
- **The dot earns its colour.** The file tree diffs `referenced` against the
  previous set, so a file's dot rings on the *transition* rather than the state —
  and the first paint of an already-written gloom is taken silently, because that
  is not an achievement. A state change you caused is the one place a small reward is
  honest.
- **The trigger band flashes per step.** It showed you *where* the hand-over
  happens but never *that* it happened, which is the difference between "the code
  changed on its own" and "I did that by scrolling". Only on an actual step
  change; every scroll frame would be noise, and noise is what gets animation
  switched off.

**Arrival animations are gated on `opened`, not on render.** `html` is derived
from `markdown`, so it re-renders on every keystroke — an unguarded arrival
replays while you type. The shell bumps `opened` only when the *doc id* changes,
and `DocPane` puts `.arriving` on the container for one beat.

**Restarting a CSS animation needs a forced reflow** — remove the class, read
`offsetWidth`, add it back. Without the read the browser coalesces both changes
into one frame and nothing runs. That single awkwardness is the whole reason an
animation library looks tempting; it is three lines, and it appears in
`arriveAt`, `pulseInvitation` and the band flash.

**Reduced motion: the motion stops, the meaning survives.** Every rule keeps its
*end* state — the ring becomes a static outline, the underline stays drawn, the
focus bar keeps the bright end of its pulse — and the trace hands direction over
to an **arrowhead** that is invisible the rest of the time. Honouring the setting
by deleting the signal would be worse than ignoring it.

**Nothing collapses with a transition.** The explore drawer — long since replaced by
one scrolling column — used to animate its `height`, and there is no version of
that which is right: collapsing a panel has to make what is below it move up, so
`height` and `grid-template-rows: 0fr→1fr` both go through layout, and `transform`
would leave a hole. The only composited option is to overlay the code, which is
the design that was cut for covering the thing you are reading. The audit finding
outlived the panel: **the answer to "how should this collapse" was to not have a
collapsible panel.**

The same reasoning retired `transition: height` on read mode's lead/tail spacers.
Their whole job is to be invisible, so animating them reflowed the doc for nobody.

**A flash on a repeating event is a transition, never keyframes.** Keyframes
restart from zero; a transition retargets from wherever it is. Read mode's band
fires on every step, so scrolling quickly used to restart it over and over — and
the `remove → reflow → add` dance needed to replay it *was* the symptom. It now
snaps the opacity up for 40ms and lets a slow transition carry it down.

**Popovers scale from what spawned them.** The `/` menu grows out of the caret via
`transform-origin`, flipping to `bottom left` when the menu flips above the line,
or it would grow away from the thing that opened it. The files modal is the
exception the rule names: a modal appears centred, so `transform-origin: center` is
correct there. Both set a `mounted` class one frame after mount, because an element
rendered already-open has no state to transition *from*.

`scripts/motion-audit.py` enforces this, and it compares **selector text**, not
just animation names. Inside `@keyframes` the movers are matched as *property
names* at the start of a declaration, because `\b(bottom)\b` also matched
`border-bottom-color` — a colour — and reported a pure fade as movement — **per selector part on both sides**, since a grouped rule
needs every part overridden and comparing a whole comma-joined selector against a
set of parts matched nothing at all. A **finite** animation that only fades is exempt outright — the same exemption an
opacity-only transition already gets, and for the same reason: reduced motion drops
movement, and there is none. `infinite` is excluded from that, because an ambient
fade that never stops is exactly what the setting is asking about even though
nothing travels. It also accepts a reduced-motion override that **still fades** — an `animation` whose
keyframes touch only opacity or colour — on the same grounds it already accepts an
opacity-only transition: where an animation's end state is *gone*, replacing it
with `animation: none` leaves the thing it was fading out permanently on screen,
which is worse than the motion was. It also covers **transitions that move
something** — added
after three keyframe animations became transitions and silently left the audit's
field of view. Opacity and colour transitions are exempt, since reduced motion
keeps those, and an override counts if it *stops the movement*, whether by
`transition: none` or by transitioning opacity alone. The second form is the better
one: reduced motion means gentler, not nothing. That is not pedantry: on its first run it found three rules
that were written, looked correct, and could never apply — each had a *shorter*
selector than the rule it meant to override (`.edge.lit` against
`svg.focusing .edge.lit`) and lost on specificity. It also found `hintIn` had
never been covered at all. Run it after touching any animation:

```bash
python3 scripts/motion-audit.py     # exits non-zero on an uncovered animation
```

**What was refused** is in `mockup/motion.html`'s last section, and matters more
than the six above because these are the things a library makes easy: counting
the stats up (it delays reading the number, which was the whole point of the
block), any *sliding* of the code pane's text (that surface is for
reading: the arrival fades it and travels only the gutter, and the focus bar
breathes in the gutter for the same reason — nothing the eye is about to land on
has to settle first),
route transitions (there is one route), and easing scroll position (read mode
already drives the code pane *from* scroll; animating scroll from scroll fights
the trackpad). And the library itself: all six are `class` + `@keyframes`, none
interpolate arbitrary values, none need a timeline or physics.

### Focus is one shared store

`focus.svelte.ts` is what makes the two panes feel like one selection instead
of two coincidences. The doc pane sets it; the code pane reacts.

A selection is **several spans, not one**, because an Elixir function rarely is:

- **every clause** of the chosen `name/arity` → primary highlight
- **other arities of the same name** → dimmer "related" tint (they are one
  function to a reader, even though the BEAM treats them as separate)
- **its `@spec`** → violet (`--mark`), because the contract is not the body

The `def` line of each clause breathes an accent bar at 2.1s — Xray's `tmPulse`
cadence — and the rest of the file **dims to 32%** (Xray's `.focusing` idiom).

There are **four ways out** and all four ship together: `Esc`, re-clicking the
selected row, clicking empty space in the code pane, and clicking the hint
pill. One is never enough; the pill exists so the exit is discoverable.

Surface rows and diagram functions are the same gesture — both carry `data-sig`, and
`DocPane.select()` handles either.

**Dimming yields to file-wide views.** `.focusing` (the dim-to-32%) is right for
"show me this one function" and wrong for any question about the whole file, so
it is suppressed while blame or a search is active — see `dimming` in
`CodePane.svelte`. The selection keeps its own highlight and the pill still says
where you are; it just stops hiding the author colours or the matches you asked
for. Without this, turning on blame with a function selected leaves 68% of the
authors invisible.

### Home is a list of projects

A gloom only means something inside the codebase it was written about, so home
lists **projects** — name, path, gloom count, when you were last in it — and each
project's glooms live in its Hub. One way in (**Open a folder…**), then three
sentences on what a gloom is.

The masthead keeps its weather — `Sky.svelte`, bloom / cosmos / aurora, rolled per
visit — and it is still the only ambient motion outside the two already budgeted.
It stops when the window is hidden, cleans up on unmount, and under reduced motion
composes one frame and holds it.

`--gloom-deep` came out of the old home screen: the bright teal is for ink surfaces
and measures 2.2:1 on paper; the deep one is 5.2:1, and it is what the project
counts, the Hub's headings and selection, and the tree's dots use.

### The app header is a small theme

`.apphead` redefines the neutral tokens for its own subtree — `--fg`, `--fg-dim`,
`--line`, `--bg-raised`, `--accent` — over `--head-bg`, a step lighter than the
gloom band under it. Every control in the row adapts without knowing it is on a
dark strip, which is read mode's trick at the scale of one row, and the two bars
stack as a single masthead: actions above, the name of the journey below. Measured
on the light head: ink 10.9:1, dim 7.5:1, the teal accent 6.5:1.

**The row is the project's**: `← Home`, the project's name, the **live branch**,
the save state, and *Find a module or file… ⌘T*. Everything about a gloom is in the
band below it.

**The branch is the working tree's, live** — a plain read of `.git/HEAD`, re-read
when the window regains focus. While a gloom was pinned to the versions it was read
at, the header showed *the gloom's* branch, and a guard refused references off it;
with the code always live, the branch you are on is the one true answer. It carries
the accent, is copyable (a branch name is something you are about to type somewhere
else), and is keyed on its own value so it plays its arrival when it changes.

**`← Home` is a way out and looks like one** — filled, bolder, first in the row.
Re-parse, the library button, the file button and the staleness notices have all
left this row on the way here; there is nothing to re-parse when nothing is
snapshotted.

### The code pane shows what you selected, and nothing else

**The sticky function header was removed.** It pinned the enclosing signature to
the top of the pane while you scrolled inside a long function — which sounds
useful and read as a *fourth selection state*: a highlighted bar naming something
that was not what you had selected, in a pane whose whole job is to show what you
selected. `topLine` survives; it drives the vim motions.

**A run of lines is selectable, three ways.** `⇧`-click extends from the review
cursor (or the top of the current selection); dragging the **line numbers** sweeps
a range, and the gutter carries `cursor: ns-resize` to say so; and **an ordinary
text selection across several lines is one too**.

**Ask the rows which of them the range touches**, not the range which rows its
endpoints are in. A drag downward routinely ends with `focusNode` set to a
*container* — `.code`, or a row element with a child offset — and
`closest(".row")` on a container is `null`, so the whole gesture was discarded and
the feature looked dead. `range.intersectsNode(row)` does not care where the
endpoints landed. A selection stopping at the very start of a row does not include
it, which is what the highlight on screen already shows.

That third one matters more than it sounds. Dragging over the code selects text —
that has to keep working, you copy from this pane — but a drag crossing three lines
is unmistakably *"these three lines"*, and making you repeat the gesture in the
gutter to say so would be the tool arguing with you. The native selection is read
at `mouseup` and mapped onto the same `focus` range everything else uses: the text
stays highlighted for the copy, and the file dims around the lines it spans.

**`⇧` is handled at `mousedown` *and* on the click.** Mousedown is where it belongs:
by the time the click arrives the browser has already extended its own text
selection over the gesture, which is both ambiguous and — because `onCodeClick`
opens by ignoring clicks that ended a text selection — swallowed outright. But a
shift-click landing on a child element, or a `preventDefault` the platform decides
to ignore, can arrive as a click and nothing else, so handling it in one place made
the gesture work or not depending on where in a line you clicked. `extendTo` runs
from both and the `painted` guard makes the repeat free.

**The native text selection wears the same colour as a selected row.** Dragging over
the code produces both highlights — the browser's on the glyphs, ours on the rows it
spans — and in two different colours that reads as two selections arguing. `.code
::selection` is `--accent` at 13% over the code surface, which is exactly what
`.row.hit` paints, so the overlap is invisible and a part-line selection still shows.
Opaque on purpose: a translucent one stacks with the row tint and prints twice as
strong.

**A drag selects with `focus.select`, never `focus.set`.** `set` toggles — clicking
the already-selected thing clears it, which is right for a function and fatal for a
drag: a pointer moving inside one line fires dozens of mousemoves carrying the same
range, so the selection flickered on and off and mostly landed off. The store says
as much in its own comment: `select` is for "go here", where going where you already
are must never mean going nowhere. A `painted` guard skips the repeats as well, and
resets on mousedown so a fresh drag can re-paint a range that is already up.

A gutter drag sets a `dragged` flag for the same family of reasons — the click
that ends it would otherwise drop the range and set the cursor instead. It
goes through `focus.set` like every other selection, so the dimming, the hint pill
and the doc pane treat it as one thing. Dragging is in the gutter on purpose: the
source is selectable text — deliberately, you copy from it — so a drag across the
glyphs has to keep meaning "select this text", and the line numbers are the one
column nobody copies.

### The moduledoc leads the Woop

A module's `@moduledoc` is rendered first in the Woop (`lib/moduledoc.ts`), above
the surface: it is the author telling you what the module is for, which is what to
read before its functions. The parser already hands it over as markdown with the
fencing and indent stripped.

- **A plain markdown-it, not the note's.** The note's renderer turns inline code
  naming a function into a reference chip; in someone else's moduledoc that would
  invite a click nothing on this side answers.
- **Doctests are highlighted.** `iex>` examples sit in *indented* code blocks,
  which markdown-it renders without calling `highlight`; `code_block` is routed
  through it, and unlabelled code in an Elixir module is Elixir.
- **Its own height, folded.** 260px and scrolling inside, so a long moduledoc does
  not push the surface and the reach a screen down; a fade says there is more, and
  `Show all` lets it run full length (the Woop scrolls then, and the navigation bar
  sticks to its top).
- **A navigation bar when there are two or more headings** (h1–h3), lighting the
  section you are in. Ids are prefixed and de-duplicated — two `## Examples` are
  common.
- `L3–20` on the section's rule selects the moduledoc in the code.
- **`@moduledoc false` is said, not rendered**: hiding a module from the docs is a
  decision, and it marks the module as internal.

### Behaviour views: the Broadway Woop

`mockup/broadway.html` is the contract. A module that says `use Broadway` keeps
its moduledoc, surface and reach, and gets two views below them. It is the first
of a family: `ModuleInfo.behaviour` is a tagged enum (`Behaviour::Broadway`), so
GenServer, Oban or Phoenix is one more variant and one more renderer.

**Read statically, in Rust** (`parse/broadway.rs`), and only what the tree says.
The options are taken when they are a literal keyword list passed to
`Broadway.start_link/2`; built anywhere else, `runtime_options` carries the
expression and the view says so. Each value is a `KnobValue`: a literal, an
app-env read (`Application.get_env/3`, `fetch_env!`, `System.get_env`) with its
default, or `Runtime`, carried as its source text — never guessed.

**Four inks for where a value comes from**, and none of them is `+?`, which said
nothing. Written in the code; from app env, showing its default; **not set —
Broadway's default** (`DEFAULTS` in `broadway.ts`: producer and batcher
concurrency 1, demand 10, batch size 100, timeout 1s), in a quieter ink with
lighter squares; and **computed at runtime**, named by what computes it
(`batch_partition_count(…)`, `…processor_concurrency`) with no squares, because
the number is not in the code. Processor concurrency left out is "2 × cores" with
no squares: `System.schedulers_online() * 2` describes the machine, and sixteen
squares would describe a laptop. A field name is cut from its START, keeping the
end — `weather_processing_producer_concurrency` and `…_processor_concurrency`
share a prefix, and cut from the end both read `weather_process…`. `__MODULE__` resolves to the
module. **Comments are named nodes**, so they are filtered out of every child list:
the real Weather producer's tuple has one, and reading the second child as its
options read the comment instead.

**How start_link wires it** — producer by its module name (that is what tells SQS
from Kafka from a custom one; its options are "N options ›", one hover), the
processor group and each batcher with **concurrency drawn as that many process
squares** (capped at 100), the bucket's size and ⏱ timeout, and the context as the
map it is: a chip per entry, coloured by where its value comes from. Labels are one
or two words; sentences were tried and read as clutter.

**The lifecycle** — Broadway's callbacks as nodes, its routes as edges, ending in
the only two outcomes a message has, **✓ acked** and **✗ failed**. The producer; a
dashed *processors* group around `prepare_messages` and `handle_message`; a dashed
*batchers* group around one `handle_batch` per batcher (the clause that names it,
or the catch-all); `handle_failed`; the outcomes. One failed edge leaves each
**group**, not one node: any callback in a stage can fail a message, `:default`'s
included. A callback the module does not define is drawn **ghosted** — the
lifecycle is Broadway's, not the module's. Two things were tried and cut: a flow
chart unrolling `handle_message`'s own calls (`Decoder.decode`, `Customers.fetch_many`
— specific to one codebase, a second, worse copy of any long callback), and
`Message.put_data` / `put_batcher` chips on the nodes.

**Interaction.** Hover tints the element's lines in the code with `focus.peek` — a
store field that selects, dims and scrolls nothing, so moving across a diagram
never rearranges the code. Click selects: a callback by its signature (every
clause), anything else by its lines. A node's outgoing edges trace once on hover,
and the arrowhead turns blue **when the line arrives** (`arrived`, set at 620ms) —
turning it with the draw coloured the head before the line had left. A stage's
squares cascade as the pointer enters it. Markers are in user units, or a lit
line's head grows with its stroke. The outcomes carry no `data-lines`: they are
where a message ends, not code.

### Woop and Gloom: the right pane shows one at a time

**The names.** The note is a **Gloom**; the live views of the code on screen —
surface, reach, settings, suite — are its **Woop**, after Wooper. One syllable
each, a pair of Pokémon-ish words: the Woop is what the code shows you, the Gloom
is what you make of it. (It was *Blossoom* — Gloom evolves into Bellossom — for
one change.) UI words only — the code still says `explore`,
`ExploreSections`, `doc`, as `reading` survived the first rename.

**One at a time, not a split.** `Woop | Gloom` sits first in the pane's
header; **`⌘1` switches between them** (`flip` in `DocPane`, called by the shell,
which owns the window keys). Two sides, and the switch on screen always says which
one you are on, so one key does it — `⌘2` and a `⌘J` flip existed and were
removed, and before that ⌥1 / ⌥2 collapsed a split's ends. From the Hub, `⌘1`
returns to the gloom on the side you left it rather than flipping too: you cannot
see the pane from the Hub, so a flip there would land somewhere unpredictable. The choice is remembered per
gloom (`gloomSide:<id>`). Edit and read mode always land on the Gloom; picking
the Woop while reading leaves read mode. Exploring with no gloom open, there
is only a Woop and no switch.

This replaced **the split you own** (below), and then a short-lived
`Views | Both | Note` control on top of it. In use the two halves were each too
short to do their job and the grip was one more thing to manage; two names made
the choice clearer than a seam did. The grip, its per-gloom height and ⌥1/⌥2 as
"collapse an end" are gone.

**The header belongs to the side on screen.** On the Gloom: `▷ Gloom`, Edit /
Preview, A−/A+. On the Woop, nothing but the switch: it said "of PipelineSelector ·
live from disk" for a while, which repeated the code pane's own header. The note's
controls never sit over the views — above the reach diagram they read as acting
on the diagram.

**Copy and Save… are gone for now**, ahead of export/import being designed as one
thing. `export_note`, `ipc.exportNote` and `exporting.ts` stay: they are what the
export will be built on.

The active half of the switch carries its side's hue pulled 20% toward the ink —
the pure tokens on their own tint were 3.9:1 and 4.3:1 on light paper.

### The Gloom's header is controls, nothing else

It used to open with `impact_stage.ex.md` — a filename for a file that does not
exist, since the note is a row in a database and the `.md` was an invention. The
gloom's name is in the band above it and the source file's name is on the pane
beside it, so a third name was the only thing that row was adding. On the Gloom
side it is `Woop | Gloom`, then `▷ Gloom`, then Preview/Edit and A−/A+.

**Preview/Edit is one button, not a segmented control.** You are always in exactly
one of the two modes, so half of that control's width was spent telling you where
you already are. The label is what the click *does*: `Edit` while previewing,
`Preview` while editing.

**`▷ Gloom` comes first after the switch**: it is the one control here that changes
what the whole window does, and among right-aligned buttons it read as the third of
them.

**The font stepper is last, as in the code pane.** The same control in the same
place on both sides of the window.

**For when export returns — the parts that were already argued:**

- **It hands over exactly what is in the editor**: no front matter, no generated
  title, nothing added on the way out. That is the only version that keeps "the
  markdown IS the data" true.
- **Saving goes through `export_note`, not `tauri-plugin-fs`.** The plugin would put
  a general "write files" capability into the app — with a scope to get wrong — to
  serve one gesture whose destination the user has already named in a native
  dialog. The command writes that text to that path and can do nothing else. It is
  the only place the app writes outside its own database, and the rule it does not
  break is the important one: **it never writes into a repository it read from.**
  `dialog:allow-save` is the only capability it needs.
- **`exportFilename`** turns a name written as prose (`Accounts — the write path`)
  into a filename: diacritics folded rather than dropped (`Añadir usuario` →
  `anadir-usuario`, not `a-adir-usuario`), the stem capped at 60 characters, and
  `gloom.md` when nothing survives.
- **A confirmation lands on the control, not in a toast**, and replaces it rather
  than sitting beside it, or the row shifts every time you use it.

### Six keys, and `?` lists only them

| Key | Does |
|---|---|
| `⌘K` | the Hub (again: back to what was open) |
| `⌘B` | the file tree — the sidebar toggle of VS Code, Zed and Sublime; it was `⌘\` |
| `⌘N` | a new gloom, from anywhere in a project |
| `⌘1` | Woop ↔ Gloom |
| `⌘E` | edit ↔ preview |
| `⌘R` | reading mode |

All on ⌘, the Mac convention for app commands. **Not Option**: ⌥+letter types a
character on a Mac layout (⌥E starts an accent), so in the note editor it would
insert symbols. **Not Ctrl**: ⌃+letter is Cocoa text editing in every field (⌃A,
⌃E, ⌃K…). The rest still work — `⌘T`/`⌘O` find, `⌘P` functions, `⌘[`/`⌘]` the
trail, `⌘S`, the code pane's vim motions — but `?` shows only the six. It was a
ten-section manual; what you open `?` for is the key you forgot.

### The two mode toggles are keys as well as buttons

`⌘R` for read mode and `⌘E` for edit/preview. Both live in `DocPane` as exported
functions the shell calls, rather than a second keydown listener inside the pane —
the shell already owns the window's handler for `⌘T`, `⌘K` and the rest, and two
listeners would eventually disagree with the buttons about which mode is on.

`⌘R` is `preventDefault`ed before anything else can see it: in a dev build it
would reload the webview.

**Entering read mode is gated; leaving it never is.** The conditions — a
reference that resolves, not editing, at least one step — decide whether there is anything to
walk. They were applied to the toggle in *both* directions, so anything that
emptied `steps` mid-read locked you in: the button disappeared and `⌘R` was refused
by the same test. `Esc` exits too, for the same reason the focus pill exists — a
mode needs an escape that does not depend on finding the control that started it,
and clearing the focus underneath read mode would do nothing visible anyway, since
the next scroll frame sets it again.

`toggleEdit` leaves read mode on the way in, because scroll-driven selection while
you type is chaos — the same rule the Edit button already followed.

Adding them exposed the real problem, which was the status bar. It advertised
`j k gg G vim`, which tells anyone who does not think of themselves as a vim user
to skip the line. It names the keys you need instead, and now reads
`⌘T find · ⌘[⌘] trail · ⌘B sidebar · ⌘J woop/gloom · ⌘R glooming · ⌘E edit · ? help`.

### Lists are rows

A reading is usually a numbered list of steps, so **a list item is a row**, not a
line of prose with a bullet in front of it — it is the unit read mode walks, and
it should look like a unit. Ordinary markdown throughout; there is nothing new to
type, and the text still reads correctly pasted into a PR comment.

- The marker sits in the gutter the row's own left padding leaves, so a wrapped
  second line aligns with the first instead of tucking under the number.
- **Both marker offsets are in `em`.** `calc(2.5px + 0.825em)` centres the bullet
  on the first line, and `line-height: 2.01` on a `0.82em` number gives it the
  same `1.65em` line box the content has. The doc font is adjustable, so a
  constant here is only correct at 14px — the same trap `caretXY` avoids.
- **Depth shows in the marker, not only the indent**: filled square, hollow
  circle, then a dash. Three levels is more than any reading needs.
- Hover lifts the row. The reference inside stays the click target — making the
  whole row clickable would take text selection away from a surface you edit.

**A re-render wipes the step marks, and only a cross-file reference re-renders.**
`md` depends on `current`, so crossing a file rebuilds the whole note's DOM. The
scroll handler does not fire — nothing scrolled — and `activeStep` has not changed,
so `.now` was never re-applied: the step you had just walked into dropped to 45%
and stayed there, while `.here` returned on the next frame and tinted it. A dimmed
row with a highlight on it, which is how the bug was spotted. `markSteps` now
restores both marks after every render, and it never showed up on a same-file
reference, because that is the only case that does not re-render.

**The current chip went the same way, for longer.** The render effect reset
`activeRef` to null, so a reference into another file lost its colour the moment it
became current — in a list naming one module per line, every step. It is now
restored from `activeStop` after each render while reading, and the chip is marked
there directly, since the selection effect may already have run for that render.

**The highlight follows the reading line; the step follows the references.** Two
rates, because they answer two questions — *where am I reading* and *what should
the code be showing*. A step only changes where a reference is, so between two
referencing paragraphs the tint used to sit on the last one while the trigger was
three paragraphs further down: you were reading one block and a different one was
lit. `.here` is the block the line is actually inside, recomputed every scroll
frame; `.now` stays on the step, which is what drives the code and what must not
dim. When the line is inside a step the two coincide, which is the common case.

Innermost wins, by taking the last block in document order that contains the line —
a parent `li` is listed before the `p` inside it. The loop **breaks** once a block
starts below the line: tops are non-decreasing in document order, and without that
it reads every block's geometry on every frame.

**Every step is a row, paragraph or list item.** A list item got a tinted row with
an accent rule down its left edge and a plain paragraph got nothing but a change of
opacity — so a note written as prose showed a flicker as the dimming swapped and no
mark of where you were. The step is the unit read mode walks whatever shape it
takes. The padding and the transparent left border sit on **every** step, not only
the current one, so the geometry never moves and only the colours change; a
negative margin keeps the text's left edge where it was, and the measure grows by
the padding it gained, or a doc with no lists in it would rewrap on entering read
mode.

**Read mode marks the row, not just the block.** In a loose list the step is the
inner `<p>`, so without this the number in the gutter stayed bright while the text
beside it dimmed. The enclosing `li` gets `.steprow`, and `markNow` — one helper,
because scrolling and clicking both set this and were already required never to
disagree — puts `.now` on the block and on its row. Dimming then lives on the row
with `li.steprow .step { opacity: 1 }` beside it, or the two multiply to 0.2 and
the text nearly vanishes.

### The app is Glooming; to gloom is to read

The window title and the brand in the title strip say **Glooming**, set in
`--display` — the same wordmark home carries, so the two agree.

**The read-mode button is `▷ Gloom`, and `▶ Glooming` while it runs** — the verb
when it is something you can do, its present continuous while you are doing it;
the trigger band's tag says *Glooming* too. *Gloom* is the verb: to explore a
codebase by walking your own prose through it. It was `▷ LGTM` while a gloom was
a review of one change, named for the verdict at the end of the pass; once the
project became the unit there is no change to approve, and the verb that names
what the pass *does* replaced the one that named what it concluded. `lgtm`
survives in the repo, the crate, the bundle id and the internals (`reading`,
`toggleRead`), which were not renamed — churn, not clarity.

### Reading: the doc drives the code

`▷ Gloom` in the Gloom's header, shown when the note has a reference to walk. A
gloom walks every module its prose names, swapping the code pane where the prose
crosses a boundary and reading files nobody opened.

The geometry scrollytelling wants was already here: the doc is the text, the
code pane is the sticky graphic, and `focus` is the graphic's state. So this is
one wire — doc scroll position → focus — not a rewrite.

**There is no new syntax.** Inline code naming a function by its full module name
becomes a reference (`refs.ts`) — with or without an arity, with or without lines
inside it. That keeps the markdown portable: paste a gloom into a PR comment and it
still reads correctly, which inventing `{{create_user/1}}` would have destroyed. Anything
that doesn't look like a signature — `` `nil` ``, `` `{:ok, user}` `` — stays
ordinary inline code.

Four rules, each of which replaced something that read worse:

- **The reading advances by reference, not by block.** A paragraph often names
  three functions in the order the code runs them, and one step per block meant the
  first won and the other two were scenery: you read "…then `changeset/2`, then
  `Repo.insert/1`" while the code sat on `normalize/1` until the paragraph ended.
  Each reference is now a **stop**, triggered by its own chip crossing the line —
  so the code changes as you reach the *sentence* that names it, and the block
  stays highlighted throughout. `.mention` is gone with the old idea: a second
  reference in a paragraph is the next thing the reading walks, not scenery.

  Blocks are still what gets *marked* — `.step` for the dimming, `.now` on the
  block owning the current stop, `.here` on the block under the line. They are
  simply no longer the unit of advance. `stopOwner` maps each reference to its
  innermost block, which is where the old innermost-owner rule now lives: a loose
  list's `<li><p>ref</p></li>` still resolves to the `p`, so a three-item list is
  three rows and not six.

  **A stop fires at its own position, not its chip's.** Two references on one
  line of text cross the trigger in the same frame, so the later one won and the
  first was never shown — even on entry, where `startAtFirstStop` selected it and
  the very next scroll event skipped past. `stopPositions` pushes the chips' tops
  apart to at least `STOP_GAP` (24px, under one line of read-mode prose, so
  references on successive lines fire where they sit), and the tail grows by the
  drift that leaves at the end. Entering, clicking and `↓`/`↑` all go through
  `goToStop`, which **steers** for 700ms: a smooth scroll passes over every stop
  between start and landing, and letting those fire swapped files on the way.
  `↓`/`↑` (and `j`/`k`) step exactly one reference — the guarantee scrolling can
  only approximate — and the code pane's own cursor keys stand down meanwhile.

  A click still lands on the chip that was clicked (`stops.indexOf(chip)`) rather
  than the first one carrying that signature, and still scrolls the *block* to the
  line — a chip alone would put one sentence at the trigger and leave its paragraph
  half off the top.

  "Innermost" is load-bearing, and it took two ordinary markdown shapes to find
  out. A **loose** list — blank lines between items — wraps each item's text in a
  `<p>`, so `li` *and* `p` both matched: a three-item list became **six** steps
  and every second scroll did nothing. A **nested** list has the same problem from
  the other direction, the parent `li` containing the child's reference. Taking
  the last carrier in document order that contains a reference fixes both, and
  handles them together: a parent keeps whatever it holds directly and the child
  keeps its own. Verified against all five shapes in a synthetic-tree probe.

  The consequence in the DOM: `.mention` now means "not this block's step" rather
  than "not the first reference", and the scroll handler reads
  `code.ref[data-line]:not(.mention)`. A reference is a mention from outside and
  a lead from inside.
- **The lead-in sits at the top of the doc, not before the first step.** Both put
  the same pixels on screen, and only one of them looks right: immediately before
  the step it opened a hole between your title and the paragraph under it, which
  reads as a *missing section* — a gap between two things looks like something was
  removed from between them. At the top it is the note starting lower down the
  page, which is what it always was.
- **The band is placed every frame, not once.** A single measurement can be taken
  before the pane has its height — and when it is, the band lands just under the
  header and stays there for the whole session, reading as a stray rule at the top
  of the page rather than as an instrument. `placeBand()` is two rects; running it
  per scroll frame is cheaper than being wrong, and it re-runs after the frame
  that read mode's own type change lays out. It is *meant* to stay still: the
  trigger is a fixed height and the text moves past it.
- **The tail is measured from the last stop to the end of the content**, not from
  the height of the block it sits in. Those agree only when the last reference is
  the first thing in the last block; when a closing paragraph names *two*
  functions, the second sits near its bottom, the block height wildly overstates
  what is below the chip, and the tail comes out short by the difference — on a
  800px pane with a 120px final paragraph, 56px short. The symptom is precise and
  easy to misread: every reference works except the last one, and adding any
  paragraph after it makes the problem vanish.
- **Entering read mode begins the reading.** At rest the note sat above the trigger
  with nothing selected, on the grounds that a reading has not begun until you
  scroll. That is right for *scrolling past* the first paragraph and wrong for
  *pressing the button*: pressing it is the decision, so the code pane should be
  showing the first thing your prose talks about by the time you look left.
  `startAtFirstStop` scrolls the first stop's **block** to the line, not the chip —
  putting one sentence at the trigger with its paragraph half off the top is the
  mistake `alignReading` already avoids — and then **selects it outright** rather
  than waiting for the scroll to say so. `applyStop` is split out of the scroll
  handler for exactly that: a smooth scroll of nearly zero fires no scroll event at
  all, so the reading opened with the band in place and the code pane still at rest.
- **A lead-in *and* a tail, both measured in JS.** The trigger sits 38% down the
  pane, which on a tall window is *below* the first paragraph or two at rest —
  so the reading would open at step 2, and how far in depended on the monitor.
  The tail is the same bug at the other end: the last step can only fire if at
  least `(1 - TRIGGER)` of a pane sits below its top at maximum scroll, so a
  fixed fraction silently fails above ~500px of pane. Both are computed from
  the measured pane height in `sizeLead()`; neither is a constant.
- **At rest the reading has not begun.** Before the first paragraph crosses, the
  file sits undimmed with nothing selected, rather than pre-armed on step one.
- **The overlap belongs in the code, not the prose.** The reference chips
  originally transitioned their fill, so the outgoing one was still blue while
  the incoming one lit and two references looked current at once — a glitch,
  not a crossfade. Chips hand over instantly; only the code pane overlaps.
- **One marking mechanism.** A chip is marked by the same `focus.sig` effect
  that marks table rows and treemap tiles, not by the scroll handler. Two
  mechanisms meant clicking a reference selected the code but left the chip
  unmarked, and scrolling could disagree with clicking about which was current.
- **Read mode is a theme, not a darker note.** `.dark` plus `.reading-surface`,
  on **both panes** — an earlier version themed only the doc pane, which in light
  mode left a dark reading surface beside an untouched white code pane: two
  surfaces disagreeing about whether the lights were out. The shell puts the same
  two classes on the code pane's wrapper, so one token set paints both, and both
  fade over the same 0.3s.

  The palette is **Nocturne**, chosen in `mockup/reading-theme.html` — which draws
  both panes at once and measures contrast live, because a palette that looks nice
  and cannot ship should say so before it is built. Five were compared; this one
  won on two grounds. It is the gloom band's teal carried into the reading, so
  read mode inside a gloom is one identity rather than two unrelated dark modes;
  and its accent clears **9.6:1** on the prose, the best of the five, which matters
  because the accent is what the current step is drawn in. Measured: prose 14.4:1,
  code 13.2:1, keywords 8.2:1, strings 11.0:1, and a dimmed step still at ~6.5:1.

  **The faint tokens had to be lifted, and the dim softened.** On a near-black teal
  the first cut put comments and line numbers at 3.8:1, and the code pane dims
  everything that is not the current function *on top of that* — so at 32% they
  composited to about 1.4:1, which is not quiet, it is gone. `--fg-faint`,
  `--fg-dim`, `--syn-com` and `--syn-doc` are all a step brighter here than the
  palette started, and read mode dims to **45%** rather than 32%: the file you are
  not focused on is still the thing you are reading *around*. Dimmed now: body
  3.6:1, comment 2.4:1, line numbers 2.2:1.

  **`--accent` and the syntax colours are overridden here, deliberately.** The old
  rule — inherit them from `.dark`, so "current" means one thing everywhere —
  survives *within* a theme, and read mode is now its own theme: current is teal in
  the prose and teal in the code, which is the consistency the rule was protecting.

  **The type changes too, because that is most of what reading is.** `--book` is
  Iowan Old Style — a *book* face, drawn for text at reading sizes, where
  `ui-serif`/New York is a system UI face and Georgia was cut for 1996 screens.
  Every fallback ships with macOS; the CSP blocks a webfont anyway. Line-height
  goes to 1.75, the measure to 62ch (a book face fits the same 62–66 characters in
  less room than a sans), and the size is `calc(var(--doc-font) * 1.14)` — derived,
  not fixed, so the A−/A+ stepper still works and the default 14px lands at the
  16px the mockup was judged at. Blocks keep their absolute sizes and their mono.

  (An earlier version reused dark mode outright, which is why the dark tokens in
  `app.css` are scoped to `.dark` and not `html.dark` — worth keeping regardless,
  since it means any subtree can be themed.)
- **Read mode has its own colour, not the accent.** `--read` is gold, and the
  button is filled while active, so a mode never looks like a *selected thing*.
  `--read-ink` is the label on the fill and has to flip per theme: the light gold
  is dark enough to need white, the dark gold bright enough to need ink. All four
  states clear 4.5:1 — the first pairing I tried was 3.7:1 at 11px.
- **A click moves the reading, not just the code.** Selecting anything the prose
  mentions scrolls its paragraph up to the trigger. Without that, a click leaves
  the doc where it was — you are reading paragraph two while the code shows step
  four — and the next scroll event snaps back to two. `alignReading()`.
- **A chip's identity is its element, not its name.** The same function can be
  referenced from five paragraphs, so `code.ref` carries a per-render
  `data-ref` index and the current one is tracked by that. Keying on the
  signature lit every duplicate at once, and clicking the third mention scrolled
  back to the *first* — `alignReading` now prefers the paragraph the click was
  actually in. Table rows and tiles are still matched by name, because there is
  only ever one row per signature.
- **The caret lands after the reference, and that needs `tick()`.** The textarea is
  `bind:value`, so assigning `markdown` re-writes `.value` — and writing a
  textarea's value resets its selection to 0. A `queueMicrotask` can run *before*
  Svelte flushes that write, so the caret was set and then thrown away, and every
  insert returned you to the top of the note. `await tick()` resolves after the DOM
  update, which is the only moment the range sticks. A multi-line insert also
  bounces focus, because a browser will not scroll to a caret it did not move
  itself.
- **`/` inserts a reference while editing** (`RefMenu.svelte`). It only opens at
  a word boundary, so `lib/my_app` and `2026/08` stay prose, and a space or a
  backtick closes it. Locating the caret in a textarea needs a hidden mirror div
  matching **every** metric that affects glyph position — font, size, line
  height, padding, width, wrap — since any drift puts the menu somewhere
  unrelated to what you are typing. Those declarations are shared between
  `.mirror` and `.raw` in one rule for exactly that reason.
- **The menu flips above the caret when it will not fit below.** Near the end of
  a long note there is no room underneath, and the pane's `overflow: auto` clips
  whatever hangs past it — the menu rendered every time and was visible never.
  Flipping is done by anchoring `bottom` instead of `top`, so the menu's height
  never has to be measured; it only flips when there is genuinely more room the
  other way, or a short pane would bounce it into even less. `x` is clamped for
  the same reason at the right-hand edge.
- **The crossfade.** The outgoing ranges linger 620ms (`focus.leaving`) while
  the incoming ones arrive. That overlap is what makes a jump read as a
  connection instead of a cut, and it is the whole reason the feature feels like
  anything.

A dangling reference — the function has since been deleted — renders struck
through and unclickable: never quietly lose the fact that the code moved.

`mockup/reading.html` and `mockup/authoring.html` are the contracts. An earlier
version put a "reading path" arc diagram in the divider; it was cut because it
needed explaining, which meant it wasn't working. The same data as a plain
two-axis chart (step across, file position down) is legible, and is parked as a
possible view rather than built.

### Projects are a path, and an index derived from it

A project is picked explicitly with **Open a folder…** and remembered in a
`projects` table, most recent first, with the glooms that belong to it. `⌘T`
searches its files by name.

Still deliberately thin. There is **no index on disk and no notion of
membership**. The module index (`project_index`) and the file walk
(`project_files`) are rebuilt in memory when the project opens and whenever the
window regains focus. Both take milliseconds on a large app, and caching either would be
inventing a staleness problem to solve in a tree that changes while you work.

Two things it does *not* do, both on purpose:

- **No root inference.** An earlier plan walked up for `mix.exs`. Explicit is
  predictable, and predictable beats clever for something used fifty times a day.
- **No git.** Listing changed files was discussed and cut: the tool is about
  opening files to read, and knowing what changed is done elsewhere.

`ignore::WalkBuilder` honours `.gitignore` (even outside a git checkout), with
`SKIP` on top — `_build`, `deps`, `node_modules`, `.git`, `.elixir_ls`,
`.lexical`, `cover`, `.hex`, `.mix`, `.DS_Store`, `erl_crash.dump` — because a
fresh checkout may not have them ignored yet. Every other file is listed, in the
palette as in the tree; only the module index is limited to `.ex`/`.exs`.

**Matching ranks filename hits above directory hits**, otherwise everything
under a `processor/` directory outranks `processor.ex` itself. Substring first,
then subsequence over the whole relative path, so `myacc` finds
`my_app/accounts.ex`.

`⌘O` opens the same palette as `⌘T`. It is the muscle memory for "open", and what
you almost always want to open is a file in the project you are already in — so
the system picker moved to `⌘⇧O`, for the rarer case of a file outside it. Two keys
for one action is deliberate: `⌘T` is advertised in the status bar as "find",
`⌘O` is what fingers reach for, and neither being wrong is worth more than the
saved binding. Comparing `e.key` **lowercased** matters — `⇧O` arrives as `"O"`,
so the old `=== "o"` meant the shifted form silently did nothing.

`⌘T` also absorbed `⌘L`: if what you have typed looks like a path rather than a
search, `↵` opens it directly. A path from a stack trace is often outside the
project, and two near-identical dialogs for "open a file by naming it" was one
too many.

### Tags

A gloom's name says what you were asking; a **tag** says what kind of thing it was —
`perf`, `retry`, `PR 412` — which is how you find it again a month later among two
hundred.

**They live in `docs.label`**, comma-separated. The first migration called that
column "a free field: `PR #412`, `claude-generated`", and this is what it was free
for. A table of its own buys nothing: tags here are for *finding* a gloom, not for
reporting on, so there is nothing to join.

**Each tag's colour comes from its own text.** `tagHue` hashes the string, so
`retry` is the same hue in the gloom band, the Hub's filters and its quick look —
for free, and forever. Nobody picks a colour for a word they typed in two seconds, and a
random one would make one gloom look like two.

**A tag you cannot search by is decoration**, so the Hub's filter matches tags
alongside the name and the prose (`matches` in `hub.ts`), and each tag is a
one-click filter at the top of the Hub. `db::docs::list` does the same in SQL,
pinned by `search_matches_the_name_the_tags_and_the_prose`, though nothing calls
it since the library went.

Added and removed in the band, beside the name, because they are part of what the
gloom is *called*. `parseTags` drops blanks and duplicates and preserves order —
the order you typed them reads as importance.

### The Hub

`mockup/hub.html` is the contract. A project's front page and its table of
contents: **opening a project lands on it**, in the space the two empty panes used
to take (`hubFull` — no gloom and no file open). Once you are reading, `⌂ Hub` in
the band or `⌘K` brings the **same page** back in place of the two panes
(`hubPage`); opening a gloom or any file leaves it, and esc or `← Back` returns
to what was open, which stayed open behind it. It replaced the library, so it
also searches prose and deletes.

**Its masthead is the top of the page, not a toolbar.** A large centred `HUB` in
the display face, the project and its counts under it, then the controls and
tags, all inside the one scroll — so it reads as a page you are on, and gets out
of the way once you are into the topics. It was a 13px label at the start of a
toolbar row and read as one more control.

**It closes the file tree while it is up**, and leaving restores it — the index is
the whole page, and a tree beside it was a second way in competing with the first.
Not persisted, so `sidebar` in storage keeps meaning what you chose. The pane's
keys work from it too: `⌘E` / `⌘R` leave the Hub, let the pane mount, then act
(`onPane`), and `⌘1` goes back to the gloom — the pane is not mounted under the Hub, so they used to
do nothing there. `⌘N` starts a gloom from anywhere.

**It never shows before its glooms have loaded.** `enterProject` fetches the
index, the files and the Hub together and sets `project` last; setting it first
showed *"Nothing written about this project yet"* for the moment the glooms took
to arrive.

**It was a modal first, and that was cut.** A sheet over the code made the index
a detour you had to dismiss, where it is a place you go to — and the sidebar, the
band and the trail are all still there around the page, so nothing about where
you were is lost by going.

**Topics are ordered sections; a gloom sits in exactly one**, or in Unsorted
(`topic_id` NULL), at a `position` within it. One place per gloom because a table
of contents needs one place and an order per entry — tags already cover "belongs
to several", and a gloom under three topics would be read three times by the
newcomer the order is for. New glooms land at the top of Unsorted. Unsorted is
shown only while it holds something; with every gloom filed it would be a
heading over nothing. Removing a
topic moves its glooms to Unsorted in the same transaction; it never deletes one.
`forget_project` takes a project's (necessarily empty) topics with it.

**The order is the reading order**, numbered across the whole Hub, topic to
topic; Unsorted has no numbers because it is not on the path yet. A drag sends the
**whole arrangement** (`arrange_hub`) in one transaction rather than a move: a
drag changes several places at once and the Hub already holds the result. It does
**not touch `updated_at`** — filing a note is not writing it, and Recent is the
order you wrote in. Pinned by
`arranging_orders_topics_and_glooms_without_touching_updated_at`.

**Dragging is pointer events, not HTML5 drag and drop.** DnD was built first and
did nothing in the app: Tauri's window handles files dropped onto it and takes the
drag events before the page sees them. Pointer events are the page's own. A press
becomes a drag after 5px, so a click still opens the row, and the click ending a
real drag is swallowed; the target is found with `elementFromPoint` against
`data-gloom` / `data-topic`, and the ghost under the pointer is
`pointer-events: none` or it would be what that finds. Esc cancels; the index
scrolls when the pointer nears its top or bottom edge.

**It is an index, so it is dense.** One 26px line per gloom, topics flowing in
columns (`auto-fill, minmax(330px, 1fr)`), so thirty glooms fit on a screen. The
first mockup gave each gloom a two-line card and was a scroll at thirty. Tags are
their **words**, small and grey — they were coloured dots first, and nobody could
read them without a legend. Health shows **only when something is wrong** (`⚠ 2`);
a tick on every row would be a wall. The detail — the walk, what is broken, the
opening paragraph, Open / ▷ Gloom / Delete — is a **quick look** (Space or `i`).

**`Topics | Recent`**, remembered per project (`hubView:<id>`): Recent is every
gloom newest first, the topic named on the row, no numbers and no dragging.

**What the Hub says is derived by rendering the note** (`factsOf` in `hub.ts`),
with the note's own markdown-it and resolver: broken = `code.ref.broken`,
ambiguous = `code.ref.ambiguous`, the walk = the resolved chips' files in order.
A second parser for "is this broken" would be a second opinion, and the day the two
disagreed the Hub would call a gloom healthy while the note struck it through. A
gloom naming a file not yet loaded is **checking**, never healthy — "not read yet"
is not "fine". The shell reads every named file while the Hub is shown and adopts
them in **one** assignment; a file at a time re-rendered every pane once per file.

**The Hub's keys are offered by the shell's handler**, not a second listener:
`hubRef.key(e)` runs first while the Hub is on screen and returns whether it used
the key, so ↑/↓, Space, ↵, `/` and esc mean the Hub's things there and the code
pane's elsewhere. `▷ Gloom` from the Hub opens the gloom and *retries* entering read
mode for up to 3s (`toggleRead` returns whether it is on), because there is nothing
to walk until the note has rendered and its named files have loaded.

**Deleting asks first** (in the quick look), and says what is lost: the note, and
only the note — no code is touched, because a gloom never holds any. It closes the
gloom if it is the one open, or the next autosave would write to a row that no
longer exists (`saving_to_a_deleted_doc_is_an_error_not_a_silent_no_op`).

## Known quirks and hard-won lessons

**tree-sitter-elixir has no `function_definition` node.** Everything is a
`call`: `def foo(x) do … end` is a call to the identifier `def` whose arguments
contain another call (`foo(x)`). Worse, the grammar names only *some* children
— a call exposes `target` as a field, but its argument list is an **unnamed**
`arguments` child. `child_by_field_name("arguments")` returns `None` and the
parser silently finds nothing. All accessors in `parse/elixir.rs` are therefore
kind-based (`child_of_kind`) with field lookups as fallback. If you add a
language, dump the tree first — don't trust intuition about node shapes.

**Serde casing is load-bearing.** `ipc.ts` reads `endLine` / `minArity` /
`clauseRanges` / `specRange`. Rust fields are snake case, so
`#[serde(rename_all = "camelCase")]` on the `parse` structs is what makes focus
cover the whole function body instead of just the `def` line. This shipped
broken once. `tests/pipeline.rs::the_wire_format_is_camel_case` pins it.

**Attributes stack in any order.** `@doc` above `@spec` above `def` is normal,
as is the reverse. `preceding_attrs` walks back through *consecutive* attribute
siblings and stops at the first non-attribute — checking only the immediate
previous sibling misses half of them.

**`data-tauri-drag-region` needs an explicit permission.** `core:default`
includes `core:window:default`, which is a *read-only query* set —
`allow-start-dragging` is not in it, so drags are silently denied by the ACL
and the window cannot be moved. `capabilities/default.json` grants it
explicitly. Also, only the element carrying the attribute is draggable, so the
brand and the spacer carry it too. (Alexandria never hit this: it uses
`titleBarStyle: "Transparent"`, which keeps a native title bar underneath.
lgtm uses `"Overlay"`, which does not.)

**The titlebar is two rows.** The window strip shares space with the macOS
traffic lights (hence 84px of left padding) and holds only the brand and theme
toggle. Everything about the open file lives in the app header below it.

**Don't pad panes with `60vh` to make scroll-into-view work.**
`scrollIntoView({block: "center"})` clamps at the document end by itself. The
padding was visible as dead space below the last line.

**Soft wrap is unconditional** — there is no toggle. It needs
`overflow-wrap: anywhere`, not `break-word`: a long string or URL has no break
opportunity and will still force a horizontal scrollbar, which defeats the
point. Line numbers and the blame gutter need `align-self: flex-start` so they
stay level with the *first* visual line of a wrapped row.

**The blame tint must lose to every selection state.** `.row.authored` is
deliberately one rule at two classes' specificity, with per-theme strength in
the `--who-l` / `--who-a` tokens. A `html.dark .row.authored` override would
carry an extra element selector and silently outrank focus, spec, doc and
cursor — so in dark mode the blame tint would win. Every selection state is
prefixed `.code ` to sit above it.

**Multi-clause functions collapse to one row.** Elixir routinely spreads a
function across several `def`s; the outline reports one `name/arity` entry with
a `clauses` count, `clause_ranges` for every clause, and jumps to the first.
Default arguments produce an arity *range* (`search/1..2`) whose identity is the
top arity. The surface row's tooltip spells this out (`default args`,
`3 clauses`) — the earlier `search/1..2 ·3` was unreadable.

**Count assertions over the whole `test` call, not its do-block.** A one-liner
written `test "x", do: assert(y)` has no `do_block` at all, so walking only the
block reported zero assertions — which quietly made every one-line test look
untested in the strip shading.

**Word matching needs the negative lookahead.** `markWord` in `CodePane` wraps
`test` / `describe` / `setup` / `config` and function names, and `(?![\w!?])` is
what stops `setup` matching inside `setup_all`, `get_user` inside `get_user!`,
and a test named `"the config test"` having the word in its *name* wrapped
instead of its keyword.

**Read mode needs a lead-in *and* a tail, both measured.** The trigger sits at a
fraction of the pane, so on a tall window the first paragraphs are already past
it at rest, and at the other end the last step can only fire if `(1 - TRIGGER)`
of a pane sits below its top. Fixed fractions fail silently at both ends and the
failure looks like nothing at all. One `TRIGGER` constant drives the lead-in,
the tail, the band position and the step test.

**`overflow-x: auto` clips vertically too.** A file strip (since cut) scrolled
sideways when a reading had many tabs, and the remove confirmation was an
absolutely-positioned popover *inside* it. Per the CSS Overflow spec, `visible` on one axis computes to
`auto` when the other is not `visible` — so the strip clips at its own 32px and a
popover starting 37px down never appears. It rendered correctly every time and was
never once visible, which is the worst kind of bug: nothing to see, nothing in the
console. The same shape has come back twice since — the band's `overflow: hidden`
is why the Hub renders nothing below it, and why the old picker sat outside it.
Anything positioned relative to a clipping box has the same problem.

**Don't nest a button inside a button.** A tab's `×` was once a
`<span role="button">` inside the tab's `<button>`, which is invalid HTML and needs
`stopPropagation` to behave. Make it a sibling. A Hub row is a `div` for the same
reason: its `i` is a button.

**An imported component must actually be rendered.** `HelpModal` was imported,
`showHelp` was toggled by `?` and by two buttons, and the `{#if showHelp}` block
had been dropped in an unrelated rewire of the shell markup — so `?` did nothing
at all for **five commits**, until someone went looking for a shortcut and could
not find the list.

Nothing catches this on its own. The import is a valid binding, the state is real,
the buttons that set it type-check, and `pnpm check` is clean: everything is
correct except that nothing mounts. A feature wired end to end and unreachable
looks exactly like a feature that works, right up to the moment you use it.

```bash
python3 scripts/component-audit.py   # exits non-zero on an unrendered import
```

**An `$effect` must never read and write the same `$state`.** The write
re-triggers the effect, which writes again — the main thread is pinned and the
window stops responding to anything. This shipped: the file strip's "dot earns its
colour" effect read `seen` and `earned` while assigning both, and because the
strip only mounts once a reading has **two** files, the app froze the moment a
second file was added. Nothing was mistyped, so `pnpm check` was clean throughout.

The rule it leaves behind: inside an effect, read reactive state or write it,
never both. Bookkeeping that only the effect itself consults — a "previous value"
— should be a plain `let`, not `$state`. Only what the template renders needs to
be reactive.

Only *synchronous* access counts: state touched inside a `.then()`, a `setTimeout`
or an event listener is not a tracked dependency and cannot loop, which is why the
`recentProjects` effect in `+page.svelte` is correct despite looking similar.

**The same loop can run through the DOM instead of the signal graph, and nothing
catches that.** The effect that computes `steps` was made to restore read mode's
`.now` / `.here` marks after a render — which meant reading `activeStep`. Every
scroll step then re-ran it; each run rebuilt `steps` as a fresh array, which re-ran
the measuring effect, which resized the lead and tail spacers, which moved the
content under the reader, which fired the scroll handler again. Main thread pinned,
and the symptom was "read mode will not exit" — because nothing responded at all,
not because the exit was broken. `effect-audit.py` is blind to it: the read and the
write are *different* `$state`s.

The rule it leaves behind: **an effect that writes state must not track the state
its own side effects depend on.** Wrap the restore in `untrack`. And when a bug
presents as a control that does nothing, check for a pinned thread before
suspecting the control.

`scripts/effect-audit.py` catches the simpler form, and knows that difference. It
also treats a read inside `untrack(...)` as untracked — that is the sanctioned
escape hatch for precisely this situation (restore some DOM state after a render,
from state the effect must not depend on), so flagging it would be crying wolf. It also strips
comments before matching — a comment using a state variable's own name read as a
dependency and flagged a correct effect, and a check that cries wolf is one people
learn to ignore:

```bash
python3 scripts/effect-audit.py    # exits non-zero on a read-write loop
```

**A sole `{@html}` owns its parent's children.** Svelte 5 updates an `{@html}`
that is an element's only child with `parent.innerHTML = …`, which deletes
anything else put in that element by hand. Read mode's lead-in and tail spacers
were inserted into `.doc` that way, so every re-render — and a reference into
another file re-renders the note — wiped them: the page jumped by the lead's
height, the scroll clamped, and the reading fell back to the paragraph's first
reference, so a second reference in a paragraph was never reached. They are
Svelte-owned siblings of `.doc` now. Measuring them by collapsing to zero had the
same effect on its own (a shorter document clamps `scrollTop`), so `sizeLead`
subtracts the lead's height instead.

**`db` is `pub`** solely so `tests/pipeline.rs` can construct `FileHistory`.

**HTML5 drag and drop does not reach the page.** Tauri's window handles files
dropped onto it and takes the drag events first, so `draggable` rows did nothing.
Drag with pointer events (the Hub does) — or set `dragDropEnabled: false` on the
window, which would cost dropping files onto the app.

**Measure without collapsing.** Setting a spacer to `0px` to read the layout under
it shrinks the document, the browser clamps `scrollTop` to the shorter page, and
restoring the height does not restore the scroll. Read mode's spacers are measured
by subtracting their own height (`sizeLead`).

## Testing

~83 Rust tests cover the parser (modules, configs, test suites), the project
index and walk, the git parsing, and the DB layer (projects, glooms, topics and
arranging them). `tests/pipeline.rs` runs the whole
chain against `tests/fixtures/accounts.ex`, which is the exact file
`mockup/index.html` draws — asserting the click-target line numbers
`12, 20, 24, 30, 36`, the full body spans `12–17, 20–22, 24–26, 36–41`, and the
camelCase wire format. **If the parser drifts from the mockup, that test fails
first.**

There are no frontend tests, and `pnpm check` must be clean. That makes three
habits load-bearing rather than optional:

- **Dump the tree before writing a parser.** Every Elixir extraction in this
  repo was written against a printed syntax tree, and every one of them would
  have been wrong from intuition. `child_by_field_name` in particular returns
  `None` for children the grammar leaves unnamed.
- **Check layout maths with a throwaway `node -e`.** The treemap's cell sizes (when
  there was one), the reach diagram's crossing count, read mode's stop spacing, the read mode lead-in and tail — all of them
  had arithmetic bugs that a script caught in seconds and eyeballing would not
  have. The read-mode tail was silently short on any pane above ~500px; nothing
  looked broken, the last step just never fired.
- **Probe frontend logic through `esbuild`, not the app.** There is no test
  runner, but `src/lib` is plain TypeScript and bundles for node in one command:

  ```bash
  node_modules/.pnpm/node_modules/.bin/esbuild probe.ts --bundle \
    --platform=node --format=esm --alias:'$lib=./src/lib' --outfile=probe.mjs
  node probe.mjs
  ```

  That is how the reference resolver's threading and the rendered `data-path`
  contract were checked — order-dependent resolution across four files is not
  something to verify by clicking. Worth reaching for whenever the logic is
  pure and the alternative is guessing.

## Conventions

- Match the surrounding code. Comments explain *why*, not *what* — the existing
  comments are the tone to aim for, and several of them record a bug that was
  actually shipped. Keep those.
- **Mock it before building it.** Every visual block here started as a
  standalone `mockup/*.html`, and the two that were argued about in prose first
  (the reach diagram, the reading spine) both changed shape once drawn. A mockup
  settles in an afternoon what a discussion cannot.
- **Say "unknown" rather than guessing.** A named setup callback's context keys,
  a dangling reference, an `import`'s call sites — where the parser cannot know,
  the UI shows `+?` or strikes the row through. A confident wrong answer is
  worse than a visible gap, and the gaps are the tool's whole method.
- Rust owns parsing; the frontend never sees a syntax tree, only an `Outline`.
- Commands in `commands/` stay thin; logic lives in `db/`, `parse/`, `index.rs`,
  `git.rs`.
- **Each pane owns its font size, under its own key** (`codeFontSize`,
  `docFontSize`) via one shared `fontSize()` store. `--doc-font` sits on the doc
  pane's scroll container so **both** the rendered prose and the markdown editor
  read it — the editor at `0.89` of it, the ratio their two defaults already had
  (12.5px mono against 14px prose), so stepping keeps the relationship instead of
  converging on one number. Every block keeps absolute sizes, because a table
  that reflows when you nudge the reading size is a data display behaving like
  text.

  Two consequences worth not breaking: `.mirror` and `.raw` share one
  declaration block because the `/` menu's caret measurement needs their metrics
  identical, and `caretXY` derives its line offset from the measured marker
  height rather than a constant, which would drift the moment the size changed.
- **Prose is capped, pictures are not.** `.doc` fills its pane; only the text
  elements carry a `max-width` for a readable measure. Capping the whole doc
  left dead space beside every diagram, so widening the window bought nothing.
- **The doc pane has three levels, not two.** `--doc-raised` is its header and
  section frames — a step *up* from `--doc-bg`, and warm, because `--bg-raised` is
  a cool near-white and against warm paper it reads as a colour mistake rather
  than a level. All three surfaces define it (light, `.dark`, `.reading-surface`).
  The header was the same colour as the prose under it, which made it read as the
  document's first line rather than as chrome; its controls now sit on the paper
  colour with borders of their own, so they read as controls. `.read` and `.warn`
  are excluded from that rule by `:not()` — they carry their own fill and a
  component-scoped selector would outrank it.
- Design tokens live in `app.css`. Never hard-code a color in a component —
  both themes come from one place, and the two panes are deliberately different
  surfaces (`--code-bg` cool screen, `--doc-bg` warm paper).
- Theme is class-based and **scoped to `.dark`, not `html.dark`** — so any
  subtree can opt into the dark palette, which is how read mode works. Adding a
  `html.dark X` descendant rule breaks that and also outranks selection states
  (see the blame-tint quirk). Preference stored under `theme`
  as `light | dark | system`, cycled in that order. lgtm defaults to **light**;
  Alexandria defaults to system. That is the only intentional difference.
