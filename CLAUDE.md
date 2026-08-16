# lgtm

A desktop tool for **reading code deeply**. You open one source file; the window
splits with the source on the left and your explanation on the right. lgtm
parses the file, seeds the explanation with the module's public and private
functions, and you fill in the prose. Clicking a function in the explanation
focuses it in the code. The explanation is saved and comes back next time.

Elixir is the only language so far. The architecture assumes more will follow.

> The name is the goal: a file is done when you understand it well enough to
> say *looks good to me*.

> If you are a Claude session being onboarded: read this file end to end, then
> `IMPLEMENTATION_PLAN.md` for the *why* behind each decision. This file is the
> map; the plan is the reasoning. `mockup/index.html` is the visual contract —
> a standalone, dependency-free HTML mockup that the real UI was built from.

## What this is not

These are deliberate, and pushing back on them needs a real argument:

| Not doing | Why |
|---|---|
| Editing source code | lgtm reads. Your editor edits. The left pane is read-only, permanently. |
| Git diffs, PR fetching, branch checkout | Cut during design. You read committed files; lgtm just opens a path. |
| Multi-file projects, call graphs, cross-module navigation | One file at a time. v2 at the earliest. |
| Any AI/LLM involvement | The explanation is *yours*. Writing it is the point — generating it would defeat the tool. |

Git is touched in exactly two read-only places (see `src-tauri/src/git.rs`):
the branch label is a plain read of `.git/HEAD`, and blame shells out to
`git blame` lazily, only when the button is pressed.

## Stack

Deliberately identical to Alexandria and Xray, so all three are one stack.

- **Frontend**: SvelteKit + `adapter-static` (SPA, **no SSR** — enforced by
  `src/routes/+layout.ts`), Svelte 5 runes (`$state` / `$derived` / `$effect`),
  Tailwind 4, TypeScript. `markdown-it` for the doc pane, `highlight.js`
  (core build, Elixir only) for the code pane.
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
app-icon.png                   icon source (1024², RGBA)
IMPLEMENTATION_PLAN.md         the reasoning behind every decision here

src/
  app.css                      design tokens for light and dark; hljs → token mapping
  routes/
    +layout.svelte             theme init
    +page.svelte               shell: titlebar, app header, split, status bar, autosave
  lib/
    components/
      CodePane.svelte          source, line numbers, blame gutter, focus, font sizing
      DocPane.svelte           edit/preview toggle, block styling, row wiring
      Divider.svelte           draggable split, double-click reset
      Library.svelte           saved docs, search, delete
    stores/
      theme.svelte.ts          ported from Alexandria; lgtm defaults to LIGHT
      focus.svelte.ts          which function is selected — shared by BOTH panes
    markdownit.ts              markdown-it + the lgtm:functions fence renderer
    lgtmBlock.ts               parses the block body (mirrors reconcile.rs)
    ipc.ts                     typed wrappers over invoke()

src-tauri/
  src/
    parse/elixir.rs            tree-sitter → Outline   ← the load-bearing file
    seed.rs                    Outline → starter markdown
    reconcile.rs               doc + re-parsed source → merged doc
    git.rs                     .git/HEAD read + lazy git blame
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

### The extended markdown block

````markdown
```lgtm:functions module=MyApp.Accounts
public:
  - create_user/1 : Entry point. Validates, then inserts.
  - get_user!/1   :
private:
  - normalize/1   : Trims and downcases the email.
```
````

Everything after the **first** colon is prose, so explanations may contain
colons and backticks freely. An empty explanation is legal and renders as a
ghost `explain…` placeholder — **those gaps are the entire nudge of the tool**.
A malformed block degrades to a plain code fence; it must never lose writing.

Rendering is a `md.renderer.rules.fence` override in `markdownit.ts`, the same
approach Alexandria uses for mermaid. New block types (`lgtm:callgraph`, …)
register against the same hook — that's why the `lgtm:` prefix exists.

**The grammar is parsed in two places** (`lgtmBlock.ts` for rendering,
`reconcile.rs` for merging). They must agree. That duplication is why the
grammar is kept this simple.

### Reconciliation: prose is never discarded

When the file changes, `reconcile.rs` merges the doc with a fresh parse:
prose is kept keyed by `name/arity`, new functions append with empty slots,
vanished ones are struck through **but keep their explanation**, and prose
follows a function across a `def`↔`defp` change. A rename reads as a delete
plus an add — the old prose survives struck through rather than silently
re-attaching to a function it wasn't written about.

If you touch this file, the tests in it are the specification.

### Focus is one shared store

`focus.svelte.ts` is what makes the two panes feel like one selection instead
of two coincidences. The doc pane sets it; the code pane reacts. Selecting a
function highlights its whole body (header through `end`), breathes an accent
bar on the `def` line at 2.1s — Xray's `tmPulse` cadence — and **dims the rest
of the file to 32%** (Xray's `.focusing` idiom).

There are **four ways out** and all four ship together: `Esc`, re-clicking the
selected row, clicking empty space in the code pane, and clicking the hint
pill. One is never enough; the pill exists so the exit is discoverable.

## Known quirks and hard-won lessons

**tree-sitter-elixir has no `function_definition` node.** Everything is a
`call`: `def foo(x) do … end` is a call to the identifier `def` whose arguments
contain another call (`foo(x)`). Worse, the grammar names only *some* children
— a call exposes `target` as a field, but its argument list is an **unnamed**
`arguments` child. `child_by_field_name("arguments")` returns `None` and the
parser silently finds nothing. All accessors in `parse/elixir.rs` are therefore
kind-based (`child_of_kind`) with field lookups as fallback. If you add a
language, dump the tree first — don't trust intuition about node shapes.

**Serde casing is load-bearing.** `ipc.ts` reads `endLine` / `minArity`. Rust
fields are snake case, so `#[serde(rename_all = "camelCase")]` on the `parse`
structs is what makes focus cover the whole function body instead of just the
`def` line. `tests/pipeline.rs::the_wire_format_is_camel_case` pins it.

**`data-tauri-drag-region` needs an explicit permission.** `core:default`
includes `core:window:default`, which is a *read-only query* set —
`allow-start-dragging` is not in it, so drags are silently denied by the ACL
and the window cannot be moved. `capabilities/default.json` grants it
explicitly. Also, only the element carrying the attribute is draggable, so the
brand and the spacer carry it too. (Alexandria never hit this: it uses
`titleBarStyle: "Transparent"`, which keeps a native title bar underneath.
lgtm uses `"Overlay"`, which does not.)

**Don't pad panes with `60vh` to make scroll-into-view work.**
`scrollIntoView({block: "center"})` clamps at the document end by itself. The
padding was visible as dead space below the last line.

**Multi-clause functions collapse to one row.** Elixir routinely spreads a
function across several `def`s; the outline reports one `name/arity` entry with
a `clauses` count and jumps to the first clause. Default arguments produce an
arity *range* (`search/1..2`) whose identity is the top arity.

## Testing

`cargo test` covers the parser, seeder, reconciler and DB layer.
`tests/pipeline.rs` runs the whole chain against `tests/fixtures/accounts.ex`,
which is the exact file `mockup/index.html` draws — including asserting the
click-target line numbers `12, 20, 24, 30, 36`. **If the parser drifts from the
mockup, that test fails first.**

There are no frontend tests. `pnpm check` must be clean.

## Conventions

- Match the surrounding code. Comments explain *why*, not *what* — the existing
  comments are the tone to aim for.
- Rust owns parsing; the frontend never sees a syntax tree, only an `Outline`.
- Commands in `commands/` stay thin; logic lives in `db/`, `parse/`, `seed.rs`,
  `reconcile.rs`.
- Design tokens live in `app.css`. Never hard-code a color in a component —
  both themes come from one place, and the two panes are deliberately different
  surfaces (`--code-bg` cool screen, `--doc-bg` warm paper).
- Theme is class-based (`.dark` on `<html>`), preference stored under `theme`
  as `light | dark | system`, cycled in that order. lgtm defaults to **light**;
  Alexandria defaults to system. That is the only intentional difference.
