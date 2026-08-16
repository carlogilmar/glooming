# lgtm — Implementation Plan

> A desktop tool for **reading code deeply**. Open one source file, get a parsed
> outline of it, and write an explanation next to it in extended markdown.
> Elixir first; the architecture assumes more languages later.

Status: plan. Nothing implemented yet. The validated UI mockup lives at
`mockup/index.html` and is the visual contract for Milestones 1–8.

---

## 1. What this is

You open a code file. The window splits: **source on the left, your explanation
on the right**. lgtm parses the file, seeds the explanation with the module's
public and private functions, and you fill in the prose. Clicking a function in
the explanation focuses it in the source. The explanation is saved and comes
back next time.

That's the entire v1. It is a *reading* tool, not an editor and not a linter.

### Non-goals (deliberate, revisit later)

| Not doing | Why |
|---|---|
| Editing source code | lgtm reads. Your editor edits. |
| Git diffs, PR fetching, branch checkout | Cut in design. You read committed files; lgtm just opens a path. |
| Multi-file projects, call graphs, cross-module nav | v2 at the earliest. One file at a time. |
| Languages beyond Elixir | Python/JS come after the Elixir loop feels right. |
| Any AI/LLM involvement | The explanation is *yours*. That's the point of writing it. |

### Settled decisions

All confirmed before implementation started:

1. **The doc stores a snapshot of the source.** `docs.source` holds the file's
   text as read. A doc is a permanent *record of a reading* — prose can never
   silently drift onto code that changed underneath it.
2. **One module per file.** The Elixir community standard, so the seeder assumes
   it. `Outline` still returns a `Vec<ModuleInfo>` (the parser reports what it
   finds), but the UI seeds from the first module and shows a quiet notice if a
   file contains more. Multi-module handling is a later concern, not a v1 one.
3. **Reopening a path offers the existing doc** via a small chooser
   ("1 existing doc — branch `main`, 2 months ago"), so the library doesn't fill
   with near-duplicates.
4. **Doc title is seeded from the module name and is editable** afterwards.
5. **Branch is prefilled by reading `.git/HEAD`** — a plain file read, no git
   binary, no network. Missing `.git` → null, nothing breaks.
6. **Blame shells out to `git blame --line-porcelain`**, read-only and lazily on
   first press of the Blame button. The button is hidden when there's no `.git`.
   This is the only place git is invoked.

---

## 2. Stack

Same as Alexandria and Xray, matched version-for-version so nothing here is a
new thing to learn.

**Frontend** — SvelteKit 2 + Svelte 5 (runes), `adapter-static`, Vite 6,
Tailwind 4, TypeScript. `markdown-it` 14 for rendering, `highlight.js` 11
(core build, Elixir registered) for the code pane.

**Backend** — Tauri 2, Rust 2021. `sqlx` 0.8 (sqlite, macros, migrate),
`serde`, `thiserror`, `dirs`, `tokio`. Plugins: `tauri-plugin-dialog` (file
picker), `tauri-plugin-opener`, `tauri-plugin-clipboard-manager`.

**New to this project** — `tree-sitter` + `tree-sitter-elixir`. Pin exact
versions at implementation time; the Rust binding's API shifts between minors.

**Toolchain** — pinned with **asdf**, identical to Alexandria and Xray, so all
three projects share one installed toolchain. `.tool-versions` is committed at
the repo root:

```
rust 1.95.0
nodejs 25.9.0
pnpm 9.15.0
```

**pnpm is the only package manager.** No npm, no yarn — a stray `package-lock.json`
or `yarn.lock` should be treated as a bug.

```bash
# once per machine (plugins are shared across all three projects)
asdf plugin add rust && asdf plugin add nodejs && asdf plugin add pnpm
asdf install                 # reads .tool-versions

# setup
pnpm install                 # JS deps; cargo fetches Rust deps on first build

# develop — hot-reloading Svelte + Rust rebuilds
pnpm tauri dev

# checks
pnpm check                   # svelte-check
cargo check  --manifest-path src-tauri/Cargo.toml
cargo test   --manifest-path src-tauri/Cargo.toml   # the parser tests (M3, M9)
cargo clippy --manifest-path src-tauri/Cargo.toml

# ship a .app / .dmg
pnpm tauri build
```

`asdf install` must be run in the repo root before anything else — a mismatched
Node or Rust is the single most common way a Tauri build fails confusingly.

**Storage** — one SQLite file at
`~/Library/Application Support/com.alertmedia.lgtm/lgtm.db`, migrations in
`src-tauri/migrations/` run on startup via `sqlx::migrate!`, exactly as
`alexandria/src-tauri/src/db/mod.rs` does it.

### Layout

```
src/
  routes/+page.svelte            app shell: titlebar, split, status bar
  lib/
    components/
      CodePane.svelte            source + line numbers + blame gutter + focus
      DocPane.svelte             edit/preview toggle, autosave
      Divider.svelte             draggable split
      FocusHint.svelte           the "esc to exit" pill
      Library.svelte             list of saved docs (M11)
    stores/
      theme.svelte.ts            ported from Alexandria, default light
      doc.svelte.ts              current doc + dirty tracking + autosave
      focus.svelte.ts            which function is selected, shared by panes
    markdownit.ts                markdown-it instance + lgtm:* fence renderers
    lgtmBlock.ts                 parse/serialize the lgtm:functions body
    ipc.ts                       typed wrappers over invoke()
src-tauri/
  src/
    commands/{mod,files,docs,blame}.rs
    db/{mod,models}.rs
    parse/{mod,elixir}.rs        tree-sitter → Outline
    error.rs  lib.rs  main.rs
  migrations/0001_initial.sql
```

---

## 3. The core data flow

```
  ┌──────────┐   pick path    ┌─────────────┐
  │ Svelte   │───────────────▶│ Rust        │
  │ frontend │                │             │
  │          │◀───────────────│ read file   │──▶ source text + sha256
  │          │  OpenedFile    │ parse       │──▶ Outline (module + functions)
  │          │                │ query db    │──▶ existing docs for this path
  └────┬─────┘                └─────────────┘
       │
       │ no existing doc → seed markdown from Outline
       │ existing doc    → load its markdown + snapshot
       ▼
  render: markdown-it, with ```lgtm:functions rendered as the block component
       │
       │ you type ──▶ 800ms debounce ──▶ save_doc(id, markdown)
       ▼
  SQLite
```

Parsing happens **once per open** (and on explicit re-parse). It is not
incremental and does not need to be — tree-sitter on a 500-line file is
sub-millisecond, and nothing edits the source.

### The parse contract

Rust owns parsing; the frontend never sees a syntax tree. One serde struct
crosses the boundary:

```rust
pub struct Outline {
    pub lang: String,             // "elixir"
    pub modules: Vec<ModuleInfo>,
}
pub struct ModuleInfo {
    pub name: String,             // "MyApp.Accounts"
    pub line: u32,                // 1-based
    pub doc: Option<String>,      // @moduledoc text
    pub functions: Vec<FnInfo>,
}
pub struct FnInfo {
    pub name: String,             // "create_user"
    pub arity: u8,
    pub visibility: Visibility,   // Public | Private
    pub line: u32,                // first clause, 1-based
    pub end_line: u32,            // matching `end` (or same line if one-liner)
    pub clauses: u8,              // >1 for multi-clause functions
    pub doc: Option<String>,      // @doc text
}
```

`clauses` exists because of `normalize/1` in the mockup: Elixir routinely
defines one function across several clauses. The block shows **one row per
`name/arity`**, jumps to the first clause, and shows a `·2` badge when
`clauses > 1`.

**Elixir grammar note that will bite you:** `tree-sitter-elixir` has no
`function_definition` node. Everything is a `call` — `def foo(x) do … end` is a
call to the identifier `def` with a do-block argument. So the query is roughly:

```scheme
(call target: (identifier) @kw
      (arguments (call target: (identifier) @fn_name (arguments) @args))
      (do_block)? @body)
(#match? @kw "^(def|defp|defmacro|defmacrop)$")
```

with arity from counting `@args` children, and defaults (`\\`) meaning one
definition yields several arities — collapse those into a single row labelled
`f/1..2`. Budget real time for the query; it is the only genuinely fiddly part
of this project. Write it against fixture files with tests, not by eyeballing
the app.

---

## 4. The extended markdown block

One block type in v1: `lgtm:functions`.

````markdown
```lgtm:functions module=MyApp.Accounts
public:
  - create_user/1 : Entry point. Normalizes, validates, inserts.
  - get_user/1    : Plain fetch by id. Returns `nil` when missing.
  - get_user!/1   :
private:
  - normalize/1   : Trims and downcases the email.
  - changeset/2   : Cast + validate. The one place field rules live.
```
````

**Grammar** — deliberately forgiving, because you type it by hand:

- Info string: `lgtm:functions` plus optional `key=value` pairs.
- Body: `public:` / `private:` group headers; under each, `- name/arity : prose`.
- Everything after the **first** ` : ` is prose, and may contain colons,
  backticks, inline code. An empty explanation is legal and renders as the
  ghost "explain…" placeholder — that gap is the tool's nudge.
- Anything unparseable renders as a plain code fence rather than throwing. A
  malformed block must never lose your writing.

**Rendering** — a `md.renderer.rules.fence` override in `markdownit.ts`,
matching Alexandria's mermaid/cards approach: intercept the fence, build the
block's HTML by hand, emit rows carrying `data-line` so the click handler can
reach the code pane. Prose runs through `md.renderInline()` so backticks work.

The parsed shape is shared by the renderer and the reconciler:

```ts
type FnEntry = { name: string; arity: number; visibility: "public" | "private";
                 prose: string; line?: number; clauses?: number };
```

Adding `lgtm:callgraph`, `lgtm:flow`, etc. later means registering another
renderer against the same fence hook. That extensibility is the reason for the
`lgtm:` prefix.

---

## 5. Schema

```sql
-- 0001_initial.sql
CREATE TABLE docs (
  id          INTEGER PRIMARY KEY,
  path        TEXT NOT NULL,          -- absolute path as opened
  filename    TEXT NOT NULL,          -- denormalized for the library list
  lang        TEXT NOT NULL,          -- "elixir"
  title       TEXT NOT NULL,          -- module name, editable
  branch      TEXT,                   -- label, prefilled from .git/HEAD
  label       TEXT,                   -- free field: "PR #412", "claude-generated"
  markdown    TEXT NOT NULL,          -- the whole doc, extended syntax and all
  source      TEXT NOT NULL,          -- snapshot of the file when read
  source_sha  TEXT NOT NULL,          -- sha256 of `source`
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);
CREATE INDEX docs_path_idx       ON docs(path);
CREATE INDEX docs_updated_at_idx ON docs(updated_at DESC);
```

One table. No repos, no reviews, no git state. `branch` and `label` are inert
metadata — they exist so you can tell two readings of the same file apart.

**Branch prefill is a file read, not a git command.** Walk up from the file's
directory to the nearest `.git/`, read `HEAD`, take the text after
`ref: refs/heads/`. No git binary, no libgit2, no network. Missing `.git` →
`branch` is null and nothing breaks.

---

## 6. IPC surface

Every command returns `AppResult<T>` and is thin — logic lives in `db/` and
`parse/`, same as Alexandria.

| Command | In | Out |
|---|---|---|
| `open_file` | `path` | `OpenedFile { source, sha, outline, branch, existing: Vec<DocSummary> }` |
| `pick_file` | — | `Option<String>` (dialog plugin) |
| `create_doc` | `path, lang, title, branch, markdown, source, sha` | `Doc` |
| `save_doc` | `id, markdown, title?, branch?, label?` | `()` |
| `load_doc` | `id` | `Doc` |
| `list_docs` | `query?, limit` | `Vec<DocSummary>` |
| `delete_doc` | `id` | `()` |
| `reparse` | `path` | `Outline` + fresh `source`/`sha` |
| `blame_file` | `path` | `Vec<BlameLine { author, when, sha }>` (M10) |

`open_file` returning everything in one round trip is intentional: one invoke,
one render, no waterfall.

---

## 7. Milestones

Each is a working app and roughly one commit. Order is chosen so that something
runnable exists from M1 and every later milestone has something to demo on.

**M0 — Scaffold.** Commit `.tool-versions` (already present) and run
`asdf install`. `pnpm create tauri-app` (SvelteKit + TS), Tailwind 4, static
adapter, plugins wired, `pnpm check` / `cargo check` / `pnpm tauri dev` all
green. Bundle id `com.alertmedia.lgtm`. A README with the §2 commands.

**M1 — Read a file and show it.** `pick_file` + `open_file` (source only, no
parse). CodePane renders lines with numbers and hljs Elixir highlighting.
*Done when:* you can open any `.ex` and read it.

**M2 — The shell.** Titlebar, split layout, status bar, **theme store ported
from Alexandria** (light default, `.dark` on `<html>`, `light→dark→system`
cycle), **draggable divider** (20–80% clamp, double-click resets to 52%),
distinct doc-pane surface. This is `mockup/index.html`'s CSS becoming real
components. Cheap, and every later milestone is nicer to build against it.

**M3 — Parse Elixir.** `parse/elixir.rs`, the tree-sitter query, `Outline`.
Pure Rust, **unit-tested against fixture files** — multi-clause functions,
default args, one-line `do:`, nested modules, `defmacro`, a file with no module.
No UI yet. This is the milestone most likely to take twice as long as expected;
that's fine, it's the foundation.

**M4 — Database + seeding.** Migration, `db/`, doc CRUD. Opening a file with no
existing doc generates the seeded markdown from the `Outline` and inserts it.
Doc pane shows the raw markdown in a textarea. *Done when:* open a file, quit,
reopen, your text is still there.

**M5 — Render the block.** `markdownit.ts` + `lgtmBlock.ts` + the fence
renderer. Preview shows the styled public/private block from the mockup.
Edit/preview toggle.

**M6 — Autosave.** 800ms debounce, dirty dot in the pane header, `Saved ✓` in
the titlebar. Save on blur and on window close too.

**M7 — Linking and focus.** The interaction the whole tool exists for: click a
function → scroll + highlight through its matching `end`, one-shot flash, slow
2.1s breathing bar on the `def` line, **everything else dimmed to 32%**, doc row
pulses in sync with a nudging caret. Exit via **Esc / re-click the row / click
empty code / click the hint pill**, with the `Reading create_user/1 · esc to
exit` pill visible while focused. All four exits ship together — the mockup
proved one isn't enough.

**M8 — Polish pass.** Empty state before a file is open, error states
(unreadable file, parse failure, non-Elixir file), keyboard shortcuts
(`⌘O` open, `⌘S` force save, `⌘E` toggle edit/preview).

At M8 the mockup is fully real. Everything below is new ground.

**M9 — Staleness and reconcile.** Compare `source_sha` against the file on disk
when a doc is opened. If it changed: show a `code changed` marker with a
**Reconcile** action that re-parses and merges — prose kept keyed by
`name/arity`, new functions appended to their group, vanished ones struck
through rather than deleted. *Never silently discard writing.* Test this with
fixtures: added fn, removed fn, renamed fn, arity change.

**M10 — Blame gutter.** `git blame --line-porcelain <path>`, parsed in Rust,
**lazily on first press of the Blame button** — not on open. Author + relative
age printed only when the author changes, per-author color bar. Hide the button
when there's no `.git`. This is the one place git re-enters, read-only and
optional.

**M11 — Library.** A sidebar (or `⌘K` palette) listing saved docs by recency
with search over title/path/branch. Opening a path that already has docs shows
the chooser from §1. Without this, docs are unreachable after M6 and the tool
only works for one sitting.

**M12 — Export.** Copy a doc to the clipboard as plain markdown with the
`lgtm:functions` block flattened into a normal list — so it can be pasted into
a PR comment. Pairs with your existing `pr-description` skill.

### Then, and only then

Second block type (`lgtm:callgraph` or `lgtm:flow`), then a second language.
Python is the better second target than JS: tree-sitter-python is stable, and
`class`/`def`/`_private` maps cleanly onto the same `Outline` with no new
concepts. If the `Outline` struct survives Python unchanged, the abstraction
was right.

---

## 8. Risks

**The tree-sitter query is the whole project's load-bearing wall.** If M3 is
wrong, everything above it is wrong. Mitigation: fixtures and unit tests in
Rust, before any UI touches it.

**Seeded-doc churn.** If reconcile is annoying, you'll stop reopening docs and
lgtm becomes write-only. M9 deserves more care than its size suggests.

**Scope creep toward "a second editor."** The moment lgtm can edit source, it's
competing with your editor and loses. The left pane stays read-only.

**Prose keyed by `name/arity` breaks on rename.** A renamed function looks like
delete + add, and your explanation strands on the deleted one. Acceptable in
v1 (struck through, still readable, copy-pasteable). Fuzzy rename detection is
a v2 idea, not a v1 requirement.

---

## 9. Open questions

None blocking. `mockup/index.html` is committed on purpose — it is the visual
contract for M1–M8 and is referenced throughout this document.
