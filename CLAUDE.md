# lgtm

A desktop tool for **reading code deeply**. You open one source file; the window
splits with the source on the left and your explanation on the right. lgtm
parses the file, seeds the explanation with the module's stats, a treemap of
function sizes, and its public and private functions, and you fill in the prose.
Clicking a function in the explanation focuses it in the code. The explanation
is saved and comes back next time.

Elixir is the only language so far. The architecture assumes more will follow.

> The name is the goal: a file is done when you understand it well enough to
> say *looks good to me*.

> If you are a Claude session being onboarded: read this file end to end, then
> `IMPLEMENTATION_PLAN.md` for the *why* behind each decision. This file is the
> map; the plan is the reasoning. `README.md` is the user-facing version.
> `mockup/index.html` is the visual contract — a standalone, dependency-free
> HTML mockup that the real UI was built from.

## What this is not

These are deliberate, and pushing back on them needs a real argument:

| Not doing | Why |
|---|---|
| Editing source code | lgtm reads. Your editor edits. The left pane is read-only, permanently. |
| Git diffs, PR fetching, branch checkout | Cut during design. You read committed files; lgtm just opens a path. |
| Multi-file projects, call graphs, cross-module navigation | One file at a time. v2 at the earliest. |
| Any AI/LLM involvement | The explanation is *yours*. Writing it is the point — generating it would defeat the tool. |

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
      DocPane.svelte           edit/preview toggle, block styling, row wiring, treemap tooltip
      Divider.svelte           draggable split, double-click reset
      Library.svelte           saved docs: search, sort, folder grouping, keyboard nav, delete
    stores/
      theme.svelte.ts          ported from Alexandria; lgtm defaults to LIGHT
      focus.svelte.ts          which function is selected — shared by BOTH panes
    markdownit.ts              markdown-it + the lgtm:* fence renderers
    lgtmBlock.ts               parses the functions block (mirrors reconcile.rs)
    treemap.ts                 parses + draws the treemap block
    stats.ts                   parses + draws the stats block
    ipc.ts                     typed wrappers over invoke()

src-tauri/
  src/
    parse/elixir.rs            tree-sitter → Outline   ← the load-bearing file
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

### The three blocks

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

```lgtm:functions module=MyApp.Accounts
public:
  - create_user/1 : Entry point. Validates, then inserts.
  - get_user!/1   :
private:
  - normalize/1   : Trims and downcases the email.
```
````

Seed order is **stats → treemap → functions**: how big is this, what shape is
it, and only then what's in it.

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

### Reconciliation: prose is never discarded

When the file changes, `reconcile.rs` merges the doc with a fresh parse:
prose is kept keyed by `name/arity`, new functions append with empty slots,
vanished ones are struck through **but keep their explanation**, and prose
follows a function across a `def`↔`defp` change. A rename reads as a delete
plus an add — the old prose survives struck through rather than silently
re-attaching to a function it wasn't written about.

Only `lgtm:functions` is reconciled. The stats and treemap blocks are left
alone; re-seed the doc if you want them refreshed.

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

**Table order is alphabetical, in two places.** `seed.rs` and `reconcile.rs`
must sort identically, or reconciling a doc silently reshuffles the table back
into source order.

**Tests must scope assertions to one block.** Three blocks now list every
function name, so `md.lines().find(|l| l.contains("get_user!/1"))` finds the
treemap row, not the table row. Split on the fence first — see
`functions_block_of` in `seed.rs`'s tests.

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

**`db` is `pub`** solely so `tests/pipeline.rs` can construct `FileHistory`.

## Testing

`cargo test` covers the parser, seeder, reconciler and DB layer.
`tests/pipeline.rs` runs the whole chain against `tests/fixtures/accounts.ex`,
which is the exact file `mockup/index.html` draws — including asserting the
click-target line numbers `12, 20, 24, 30, 36` and the full body spans
`12–17, 20–22, 24–26, 36–41`. **If the parser drifts from the mockup, that test
fails first.**

There are no frontend tests. `pnpm check` must be clean. For anything with
layout maths (the treemap especially), a throwaway `node -e` script that runs
the same d3 call and prints cell sizes is worth more than eyeballing it.

## Conventions

- Match the surrounding code. Comments explain *why*, not *what* — the existing
  comments are the tone to aim for.
- Rust owns parsing; the frontend never sees a syntax tree, only an `Outline`.
- Commands in `commands/` stay thin; logic lives in `db/`, `parse/`, `seed.rs`,
  `reconcile.rs`, `git.rs`.
- Design tokens live in `app.css`. Never hard-code a color in a component —
  both themes come from one place, and the two panes are deliberately different
  surfaces (`--code-bg` cool screen, `--doc-bg` warm paper).
- Theme is class-based (`.dark` on `<html>`), preference stored under `theme`
  as `light | dark | system`, cycled in that order. lgtm defaults to **light**;
  Alexandria defaults to system. That is the only intentional difference.
