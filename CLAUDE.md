# lgtm

A desktop tool for **reading code deeply**. You open one source file; the window
splits with the source on the left and your explanation on the right. lgtm
parses the file and seeds the explanation with what it found — its size and
history, its surface, a treemap of function sizes, what it reaches outside
itself — and you fill in the prose. Clicking anything in the explanation focuses
it in the code, and with `▷ Read` on, **scrolling the explanation walks the
code in the order your prose takes it**. The explanation is saved and comes back
next time.

Not every file is a module: a config script and a test suite get blocks of their
own, and anything unrecognised gets a blank page rather than empty ones.

Elixir is the only language so far. The architecture assumes more will follow.

> The name is the goal: a file is done when you understand it well enough to
> say *looks good to me*.

> If you are a Claude session being onboarded: read this file end to end, then
> `IMPLEMENTATION_PLAN.md` for the *why* behind each decision. This file is the
> map; the plan is the reasoning. `README.md` is the user-facing version.
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
| Multi-file projects, call graphs, cross-module navigation | One file at a time. v2 at the earliest. |
| Any AI/LLM involvement | The explanation is *yours*. Writing it is the point — generating it would defeat the tool. |
| `import` in the reach block | It puts functions in scope unqualified, so a bare `cast(...)` can't be told from a local call. Showing it with zero call sites would read as a bug. |

A few things were **built and then cut**, which is worth knowing so they don't
get rebuilt:

- **An arc diagram of the reading path**, drawn in a widened divider. It needed
  explaining, which meant it wasn't working. The same data as a plain two-axis
  chart is legible — parked as a possible `lgtm:path` block, not built.
- **A soft-wrap toggle.** Wrapping is now unconditional; there is no case where
  scrolling sideways to finish a line is the better trade.
- **A blame legend in the footer.** The gutter already names each author where
  the author changes, which is the same information where you are looking.

Git is touched in exactly three read-only places (see `src-tauri/src/git.rs`):
the branch label is a plain read of `.git/HEAD`; `git log --follow` runs once at
seed time for the stats block; and `git blame` shells out lazily, only when the
button is pressed. Nothing ever mutates a repository.

## Stack

Deliberately identical to Alexandria and Xray, so all three are one stack.

- **Frontend**: SvelteKit + `adapter-static` (SPA, **no SSR** — enforced by
  `src/routes/+layout.ts`), Svelte 5 runes (`$state` / `$derived` / `$effect`),
  Tailwind 4, TypeScript. `markdown-it` for the doc pane, `highlight.js`
  (core build, Elixir only) for the code pane, `d3-hierarchy` for the treemap.
- **Backend**: Rust + Tauri 2, `sqlx` against SQLite, `tree-sitter` +
  `tree-sitter-elixir` for parsing. DB at
  `~/Library/Application Support/com.alertmedia.lgtm/lgtm.db`; migrations run
  at startup via `sqlx::migrate!("./migrations")`.
- **Toolchain**: pinned by `.tool-versions` (asdf) — rust 1.95.0, nodejs 25.9.0,
  pnpm 9.15.0. **pnpm only**; a `package-lock.json` or `yarn.lock` is a bug.

Bundle ID `com.alertmedia.lgtm`. The app icon is generated from `app-icon.png`
at the repo root (`pnpm tauri icon`, no arguments needed — that's the default
input name). Mobile icon output is deleted on purpose; this is a desktop app.

## Dev commands

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

## Repo layout

```
mockup/index.html              standalone visual contract — no build, just open it
mockup/deps.html               the reach block's visual contract
mockup/surface.html            the surface block's visual contract
mockup/kinds.html              config / test / fallback — the non-module kinds
mockup/reading.html            scroll-driven reading, first cut
mockup/authoring.html          read mode as shipped, plus the edge cases
app-icon.png                   icon source (1024², RGBA); also copied to static/
IMPLEMENTATION_PLAN.md         the reasoning behind every decision here
README.md                      user-facing: clone, install, run, build

src/
  app.css                      design tokens for light and dark; hljs → token mapping
  routes/
    +layout.svelte             theme init
    +page.svelte               shell: titlebar, app header, split, status bar, autosave
  lib/
    components/
      CodePane.svelte          source, line numbers, blame, focus, search, vim motions, font size
      DocPane.svelte           preview/edit/read, block styling, row wiring, treemap tooltip
      FnPalette.svelte         ⌘P — jump to a function by name
      RefMenu.svelte           / — insert a reference while editing
      HelpModal.svelte         ? — what everything does
      Divider.svelte           draggable split, double-click reset
      Library.svelte           saved docs: search, sort, folder grouping, keyboard nav, delete
    stores/
      theme.svelte.ts          ported from Alexandria; lgtm defaults to LIGHT
      focus.svelte.ts          which function is selected — shared by BOTH panes
    markdownit.ts              markdown-it + the lgtm:* fence renderers
    lgtmBlock.ts               parses the functions block (mirrors reconcile.rs)
    refs.ts                    recognises `create_user/1` / `L30-34` in prose
    select.ts                  one signature → every span worth highlighting
    when.ts                    relative dates, shared so two views can't disagree
    treemap.ts                 parses + draws the treemap block
    stats.ts                   parses + draws the stats block
    deps.ts                    parses + draws the reach block (see mockup/deps.html)
    surface.ts                 parses + draws the surface block (see mockup/surface.html)
    settings.ts                parses + draws the config settings block
    tests.ts                   parses + draws the test suite block
    ipc.ts                     typed wrappers over invoke()

src-tauri/
  src/
    parse/elixir.rs            tree-sitter → Outline   ← the load-bearing file
    parse/kinds.rs             config scripts and test suites
    seed.rs                    Outline + source + git history → starter markdown
    reconcile.rs               doc + re-parsed source → merged doc
    git.rs                     .git/HEAD read + lazy blame + log for stats
    db/{mod,models,docs}.rs    pool, serde shapes, doc CRUD
    commands/{files,docs}.rs   IPC surface (blame lives in files.rs)
  migrations/0001_initial.sql
  tests/pipeline.rs            end-to-end, pinned to the mockup
```

## Core concepts

### A doc is a *reading*, not a note

One row in `docs` = one file + the markdown you wrote about it. It stores a
**snapshot of the source** (`source` + `source_sha`), so your prose can never
silently drift onto code that changed underneath it. Opening a path that
already has docs offers the existing one rather than starting a duplicate.

`branch` and `label` are inert metadata — they exist so two readings of the
same file are distinguishable. They never drive file loading.

### The markdown IS the data

This is the load-bearing principle for every `lgtm:*` block. Blocks are written
out **with their values already in them** at seed time; renderers only format
what the text says. Nothing is recomputed at render time.

That buys three things: the doc is readable as plain text anywhere, it survives
being pasted into a PR comment, and you can hand-edit anything you disagree
with. The live `Outline` is consulted for exactly **one** thing — the line
number a row or tile jumps to.

A block whose body is empty renders as a short "re-seed this doc, or write
`…` style rows here" hint, never as a blank box.

### A file is not always a module

`FileKind` decides which blocks a doc gets, because the blocks that suit one
shape say nothing about another:

| Kind | Detected by | Blocks |
|---|---|---|
| `Module` | `defmodule` with `def`s | stats, surface, treemap, deps, functions |
| `Config` | `import Config` / any `config` call | stats, **settings** |
| `Test` | `use …Case` in a module body | stats, **tests** |
| `Plain` | anything else | stats only, plus a note saying why |

The fallback is the point of the whole mechanism. A config file parsed as a
module produces four empty blocks, and **an empty block reads as broken** — so
`Plain` writes a title, the size, and one italic line explaining that nothing
structural was recognised. No error, no empty boxes.

`Config` is decided before the module walk (a config script has no modules at
all); `Test` after, since a suite *is* a module, just one where surface, treemap
and reach say nothing useful.

**Every block carries a span, not a line.** `12-40` in the text collapses to
`12` when a block is one line. That's what lets clicking a test, describe,
setup or setting select the *whole* block in the code rather than dropping a
cursor on its opening line — the same thing clicking a function does.

**Keywords are click targets too.** In the code pane `markWord` wraps the first
`test` / `describe` / `setup` / `config` on a line, exactly as it wraps a
function's name on a `def` line. The negative lookahead matters twice: it stops
`setup` matching inside `setup_all`, and stops a test named `"the config test"`
having the word in its *name* wrapped instead of the keyword.

**Only `lgtm:functions` is reconciled**, and only module docs have one — so a
config or test doc passes through `reconcile_markdown` verbatim. Pinned by
`reconciling_a_config_doc_changes_nothing`.

**The config block's finding is `env` vs literal.** `System.get_env` versus a
hardcoded string is the difference between a value you can change at deploy time
and one baked into the release; `fetch_env!` is marked `env!` because it crashes
on boot when unset. A literal whose *key* looks like a credential
(`SECRETISH` in `parse/kinds.rs`) is reported as `secret` **without its value** —
that it is hardcoded is the finding, but docs get pasted into PR comments.

**The test block's finding is that setup stacks.** A test starts from module
`setup_all` + module `setup` + its describe's `setup`, which can be a hundred
lines apart, so each describe shows the accumulated context its tests can
destructure. `provides` is a best-effort read of the block's *last expression*
(`%{user: user}`, `{:ok, repo: repo}`); a named callback (`setup :put_user`)
lives elsewhere in the file, so its keys are `None` — **unknown, which is not
the same as "provides nothing"** — and the UI shows `+?` rather than guessing.

### Every block at a glance

| Tag | File kind | Renderer | Reconciled? |
|---|---|---|---|
| `lgtm:stats` | all | `stats.ts` — key-agnostic, formats whatever keys it finds | no |
| `lgtm:surface` | module | `surface.ts` — two scrolling columns, sorted by name | no |
| `lgtm:treemap` | module | `treemap.ts` — squarified, top 3 labelled | no |
| `lgtm:deps` | module | `deps.ts` — the boundary, omitted when nothing is reached | no |
| `lgtm:functions` | module | `markdownit.ts` — the public surface, **where you write** | **yes** |
| `lgtm:settings` | config | `settings.ts` — grouped by app, env vs literal | no |
| `lgtm:tests` | test | `tests.ts` — describes, setups, assertion strips | no |

`lgtm:stats` serving four file kinds is why it renders whatever keys the text
carries instead of expecting a fixed set — one renderer, no per-kind variants.

### The five module blocks

````markdown
```lgtm:stats
lines: 42
code: 33
public: 3
private: 2
commits: 14
authors: Carlo Padilla, Jane Rivera
created: 2025-02-14
updated: 2026-08-10
```

```lgtm:treemap
  changeset/2   : 6 private
  create_user/1 : 6 public
  normalize/1   : 4 private
```

```lgtm:deps module=MyApp.Accounts
  MyApp.Repo : app
    insert/1 : create_user/1
    get/2    : get_user/1
```

```lgtm:surface module=MyApp.Accounts
public:
  create_user/1 : 12
  get_user!/1   : 24
private:
  normalize/1   : 30 2 clauses
```

```lgtm:functions module=MyApp.Accounts
public:
  - create_user/1 : Entry point. Validates, then inserts.
  - get_user!/1   :
```
````

…and under `## Notes`, the private helpers as **prose**:

```markdown
Private helpers, in source order:

- `normalize/1` —
- `changeset/2` —
```

Seed order is **stats → surface → treemap → deps → functions**, under the
headings *(none)* → Surface → Shape → Reach → Explain: how big is this, what's
in it, what shape is it, what does it touch, and only then the block you write
in. `lgtm:deps` is omitted entirely when a module reaches nothing.

The directory comes **before** the pictures on purpose: names are what you
orient by, and both the treemap and the reach diagram read better once you
already know what the names are.

**`lgtm:functions` is the public surface; private helpers are prose.** The block
carries what the module *offers*; the helpers go under Notes as a list of inline
references. That is not just tidiness — it means a freshly seeded doc **already
has a reading**, so `▷ Read` does something on a file you haven't written a word
about. The em dash after each name is the gap you write into.

The trade: prose isn't reconciled, so a new private helper won't appear in that
list on its own (re-seed for that), and a deleted one renders struck through
rather than being removed. Nothing is lost silently either way.

Reconcile's rule is **never add a group the block doesn't have, never drop one
it does** — older docs still carrying a `private:` section keep it and keep it
maintained, because dropping it would take their prose with it. Pinned by
`group_tests`.

**`lgtm:surface` and `lgtm:functions` are not redundant.** Surface is the
*directory* — sorted by name, two scrolling columns, for getting somewhere.
Functions is where *you write*; its rows carry your prose and its gaps are the
nudge. Only `lgtm:functions` is reconciled, and only it should ever be.

**Surface is sorted in exactly one place — `seed.rs`.** The renderer preserves
the order the text gives it. An earlier version sorted in both, and they
disagreed: Rust orders by `(name, arity)` giving `get_user/1, get_user!/1`,
while JS `localeCompare` on the full signature reorders punctuation and gave
`get_user_by_email/1, get_user!/1, get_user/1` — so the text and the picture
showed different orders. One sorter also means a row you move by hand stays put.

Everything after the **first** colon is prose, so explanations may contain
colons and backticks freely. An empty explanation is legal and renders as a
ghost `explain…` placeholder — **those gaps are the entire nudge of the tool**.
A malformed block degrades to a plain code fence; it must never lose writing.

Rendering is a `md.renderer.rules.fence` override in `markdownit.ts`, the same
approach Alexandria uses for mermaid. New block types register against the same
hook — that's why the `lgtm:` prefix exists.

**The functions grammar is parsed in two places** (`lgtmBlock.ts` for rendering,
`reconcile.rs` for merging). They must agree. That duplication is why the
grammar is kept this simple.

### The treemap

Function sizes as area — the one view the table can't give you: *is anything in
here disproportionate?* Squarified `d3-hierarchy` layout, drawn to an SVG string
during the synchronous markdown pass.

Deliberate choices, each of which replaced something that read worse:

- **No header, legend or footer.** Chrome was eating the space the chart needed.
- **Only the top 3 are labelled**, with the line count as the primary label and
  the name below it when the square has room. Numbers on every cell turned it
  into a wall of digits.
- **The top 3 breathe** (`brightness` + `saturate`, staggered 0.25s). A pulsing
  outline — Alexandria's treatment — was invisible at this size.
- **Corner radius scales with the cell** (`min(bw,bh) * 0.12`, capped at 5). A
  fixed `rx` turns small cells into lozenges.
- **Tooltip is rendered by DocPane, not `<title>`.** Native tooltips are slow to
  appear and unstyleable; tiles carry `data-tip` and the pane draws a pill that
  follows the pointer.
- Public is green, private is amber — the same colours the functions table uses.

### The reach block

`mockup/deps.html` is the contract; `deps.ts` is the port. The concept is a
**boundary, not a graph** — a node-link diagram of one file's dependencies is
always a star (one hub, N leaves, identical topology every time), so it carries
no information. Instead the module is a closed shape with its functions inside
in source order, and the only lines drawn are the ones that **pierce it**.

Functions that reach nothing are drawn grey with no line. That silence is the
finding, not an empty state.

Layout is deterministic: outside anchors are ordered by the **barycentre** of
their callers, which is the standard crossing-reduction heuristic and gives zero
crossings on typical modules. No force simulation — the same doc must always
render the same picture.

**Both columns size the diagram.** The viewBox height is
`max(localsH, stackH)`, and each column keeps its natural spacing while being
centred in whatever height the other one forced. Sizing from the local functions
alone meant a small file with many aliases produced a taller right-hand stack
than the viewBox, and centring it then pushed the top *and* the bottom outside —
where SVG silently clips, with nothing on screen to say it had happened. Centring
each column separately also stops five functions being stretched thin across a
diagram made tall by a long alias list.

The left column comes from the **outline**, not the block. The block records
edges; listing every local function a fourth time would be noise. So a doc whose
file has moved renders the outside column and a note rather than the boundary.

Parser notes, all confirmed by dumping the tree rather than guessing:

| Written | Grammar |
|---|---|
| `alias MyApp.Repo` | `args[ alias ]` |
| `alias MyApp.{User, Profile}` | `args[ dot[ alias, tuple[alias, …] ] ]` |
| `alias X.Y, as: S` | `args[ alias, keywords[ pair ] ]` |
| `Repo.insert(cs)` | `call[ dot[ alias, identifier ], arguments ]` |
| `%User{}` | `map[ struct[ alias ] ]` |

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

### Reconciliation: prose is never discarded

When the file changes, `reconcile.rs` merges the doc with a fresh parse:
prose is kept keyed by `name/arity`, new functions append with empty slots,
vanished ones are struck through **but keep their explanation**, and prose
follows a function across a `def`↔`defp` change. A rename reads as a delete
plus an add — the old prose survives struck through rather than silently
re-attaching to a function it wasn't written about.

Only `lgtm:functions` is reconciled. The stats, treemap and deps blocks are
left alone; re-seed the doc if you want them refreshed.

If you touch this file, the tests in it are the specification.

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

Treemap tiles and table rows are the same gesture — both carry `data-sig`, and
`DocPane.select()` handles either.

**Dimming yields to file-wide views.** `.focusing` (the dim-to-32%) is right for
"show me this one function" and wrong for any question about the whole file, so
it is suppressed while blame or a search is active — see `dimming` in
`CodePane.svelte`. The selection keeps its own highlight and the pill still says
where you are; it just stops hiding the author colours or the matches you asked
for. Without this, turning on blame with a function selected leaves 68% of the
authors invisible.

### Reading: the doc drives the code

`▷ Read` in the doc pane header, **modules only** — a config or a test suite is
a directory, not a narrative, so there is nothing to walk.

The geometry scrollytelling wants was already here: the doc is the text, the
code pane is the sticky graphic, and `focus` is the graphic's state. So this is
one wire — doc scroll position → focus — not a rewrite.

**There is no new syntax.** Inline code naming a function in this file becomes a
reference (`refs.ts`), and `L30-34` points at plain lines. That keeps the
markdown portable: paste a reading into a PR comment and it still reads
correctly, which inventing `{{create_user/1}}` would have destroyed. Anything
that doesn't look like a signature — `` `nil` ``, `` `{:ok, user}` `` — stays
ordinary inline code.

Four rules, each of which replaced something that read worse:

- **One step per paragraph.** The first reference in a block is its step; later
  mentions stay clickable but don't re-trigger. Without this a paragraph naming
  three functions fires three code scrolls inside ~60px of scrolling.
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
- **The room goes dark, in its own colours.** Read mode puts *two* classes on
  the doc pane: `.dark` for the semantic colours, and `.reading-surface` for a
  set of **warm** neutrals. That is a third surface on purpose — the doc pane is
  warm paper in light mode, so its lights-out form should still be warm, and it
  has to be tellable from the app's own cool dark at a glance (`#1b1714` warm vs
  `#191b21` cool). Only the neutrals are overridden, so `--accent`, `--pub`,
  `--priv` and `--mark` keep their dark values and "current"/"public"/"private"
  mean the same thing in both panes. Body text lands at 14.3:1.

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
- **`/` inserts a reference while editing** (`RefMenu.svelte`). It only opens at
  a word boundary, so `lib/my_app` and `2026/08` stay prose, and a space or a
  backtick closes it. Locating the caret in a textarea needs a hidden mirror div
  matching **every** metric that affects glyph position — font, size, line
  height, padding, width, wrap — since any drift puts the menu somewhere
  unrelated to what you are typing. Those declarations are shared between
  `.mirror` and `.raw` in one rule for exactly that reason.
- **The crossfade.** The outgoing ranges linger 620ms (`focus.leaving`) while
  the incoming ones arrive. That overlap is what makes a jump read as a
  connection instead of a cut, and it is the whole reason the feature feels like
  anything.

A dangling reference — the function has since been deleted — renders struck
through and unclickable. Same principle as the functions block keeping
struck-through prose: never quietly lose the fact that the code moved.

`mockup/reading.html` and `mockup/authoring.html` are the contracts. An earlier
version put a "reading path" arc diagram in the divider; it was cut because it
needed explaining, which meant it wasn't working. The same data as a plain
two-axis chart (step across, file position down) is legible, and is parked as a
possible `lgtm:path` block rather than built.

### The library

`⌘K`. Built for a few hundred docs, not a few: search over title/filename/path/
branch, three orderings (Recent / Name / Folder), sticky folder-group headers,
and `↑`/`↓`/`↵` keyboard navigation that crosses group boundaries in visual
order.

**Deleting asks first.** The row turns amber and states exactly what is lost
(the markdown and the source snapshot), that it cannot be undone, and — in
green — that the source file on disk is untouched. The `×` only appears on the
hovered/cursored row, so a destructive control is never sitting under an idle
cursor. There is no soft delete; if that ever matters, add `deleted_at` rather
than a confirmation dialog.

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

**Two blocks sort, and they follow opposite rules.** `lgtm:functions` is sorted
in *both* `seed.rs` and `reconcile.rs`, which must agree or reconciling a doc
reshuffles it. `lgtm:surface` is sorted in `seed.rs` *only*, and the renderer
preserves the text's order — because two sorters did disagree there (Rust orders
by `(name, arity)`, JS `localeCompare` reorders punctuation). If you add a
sorted block, pick one of these two shapes deliberately.

**Tests must scope assertions to one block.** Four blocks list every function
name, so `md.lines().find(|l| l.contains("get_user!/1"))` finds the treemap row,
not the table row. Split on the fence first — see `functions_block_of` in
`seed.rs`'s tests and `block_of` in `kind_tests`.

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
top arity. Badges spell this out (`default args`, `3 clauses`) — the earlier
`search/1..2 ·3` was unreadable.

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

**`db` is `pub`** solely so `tests/pipeline.rs` can construct `FileHistory`.

## Testing

~90 Rust tests cover the parser (modules, configs, test suites), the seeder, the
reconciler, git parsing and the DB layer. `tests/pipeline.rs` runs the whole
chain against `tests/fixtures/accounts.ex`, which is the exact file
`mockup/index.html` draws — asserting the click-target line numbers
`12, 20, 24, 30, 36`, the full body spans `12–17, 20–22, 24–26, 36–41`, and the
camelCase wire format. **If the parser drifts from the mockup, that test fails
first.**

There are no frontend tests, and `pnpm check` must be clean. That makes two
habits load-bearing rather than optional:

- **Dump the tree before writing a parser.** Every Elixir extraction in this
  repo was written against a printed syntax tree, and every one of them would
  have been wrong from intuition. `child_by_field_name` in particular returns
  `None` for children the grammar leaves unnamed.
- **Check layout maths with a throwaway `node -e`.** The treemap's cell sizes,
  the reach block's crossing count, the read mode lead-in and tail — all of them
  had arithmetic bugs that a script caught in seconds and eyeballing would not
  have. The read-mode tail was silently short on any pane above ~500px; nothing
  looked broken, the last step just never fired.

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
- Commands in `commands/` stay thin; logic lives in `db/`, `parse/`, `seed.rs`,
  `reconcile.rs`, `git.rs`.
- **Prose is capped, pictures are not.** `.doc` fills its pane; only the text
  elements carry a `max-width` for a readable measure. Capping the whole doc
  left dead space beside every diagram, so widening the window bought nothing.
- Design tokens live in `app.css`. Never hard-code a color in a component —
  both themes come from one place, and the two panes are deliberately different
  surfaces (`--code-bg` cool screen, `--doc-bg` warm paper).
- Theme is class-based and **scoped to `.dark`, not `html.dark`** — so any
  subtree can opt into the dark palette, which is how read mode works. Adding a
  `html.dark X` descendant rule breaks that and also outranks selection states
  (see the blame-tint quirk). Preference stored under `theme`
  as `light | dark | system`, cycled in that order. lgtm defaults to **light**;
  Alexandria defaults to system. That is the only intentional difference.
