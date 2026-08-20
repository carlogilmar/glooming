<div align="center">

<img src="app-icon.png" alt="lgtm" width="140" />

# lgtm

**A desktop tool for reading code deeply.**

Open a source file. The window splits — source on the left, your explanation
on the right. lgtm parses the file and seeds the explanation with the module's
functions, their sizes, and what git knows about it. You fill in the prose.

Open another file and it joins the same note, because a change worth reviewing
rarely lives in one file.

*A file is done when you understand it well enough to say **looks good to me**.*

</div>

---

## What it does

- **Two panes, one selection.** Click a function in your explanation and the
  code pane scrolls to it, highlights every clause of it, tints its `@spec` in
  its own colour, and dims the rest of the file to 32% so nothing else competes.
- **Seeds the explanation for you.** Opening a file writes a starter doc: file
  stats, a treemap of function sizes, and every public and private function with
  an empty slot beside it. **Those empty slots are the point** — they show you
  what you haven't understood yet.
- **Never loses your writing.** When the file changes, lgtm merges rather than
  regenerates: prose stays keyed to `name/arity`, new functions append, deleted
  ones are struck through *but keep their explanation*.
- **Reads, never writes.** The left pane is read-only, permanently. lgtm never
  modifies your source.
- **One note, several files.** During a review you open file after file, and
  those files *are* the reading — there is no group to create and manage. Tabs
  show which files are in, which you have written about, and which you opened by
  accident; references may name any of them, and read mode swaps the code pane
  where your prose crosses a boundary.
- **No AI.** The explanation is yours. Writing it is the entire point —
  generating it would defeat the tool.

Elixir is the only language so far. The architecture assumes more will follow.

## Requirements

| Tool | Version | Why |
|---|---|---|
| [asdf](https://asdf-vm.com) | any | pins the toolchain via `.tool-versions` |
| Rust | 1.95.0 | Tauri backend, tree-sitter parsing |
| Node.js | 25.9.0 | frontend build |
| pnpm | 9.15.0 | **the only package manager** — a `package-lock.json` is a bug |
| Xcode Command Line Tools | — | macOS only; `xcode-select --install` |

## Getting started

```bash
git clone <repo-url> lgtm
cd lgtm

# Install the pinned toolchain. Plugins are one-time per machine.
asdf plugin add rust && asdf plugin add nodejs && asdf plugin add pnpm
asdf install

pnpm install          # JS deps; cargo fetches Rust deps on first build
pnpm tauri dev        # run the app
```

The first `pnpm tauri dev` compiles the Rust side from scratch — a few minutes.
After that it's seconds, and both Rust and Svelte hot-reload.

> **`Port 1420 is already in use`** means an orphaned Vite server survived a
> previous run: `lsof -ti:1420 | xargs kill -9`

## Everyday commands

```bash
pnpm tauri dev        # full app, hot reload
pnpm dev              # frontend only in a browser (panes render, IPC fails)
pnpm check            # svelte-check — must be clean
pnpm build            # frontend production build

cargo test   --manifest-path src-tauri/Cargo.toml    # parser, seeder, reconciler, DB
cargo clippy --manifest-path src-tauri/Cargo.toml
```

## Building the macOS app

```bash
pnpm tauri build
```

Produces, under `src-tauri/target/release/bundle/`:

- `macos/lgtm.app` — drag to `/Applications`
- `dmg/lgtm_0.1.0_aarch64.dmg` — the installer

The build is unsigned, so the first launch needs **right-click → Open** (or
`xattr -cr /Applications/lgtm.app`) to get past Gatekeeper.

To regenerate the icon set after editing `app-icon.png` (1024², RGBA):

```bash
pnpm tauri icon       # no arguments — app-icon.png is the default input
```

## Using it

| Key | Does |
|---|---|
| `⌘O` | open a single file with the system picker |
| `⌘T` | find a file by name in the open folder — or paste a path. With a reading open, this adds the file to it |
| `⌘K` | library — everything you've written |
| `⌘P` | jump to a function by name |
| `⌘S` | force a save (autosave already runs 800ms after you stop typing) |
| `[` `]` | previous / next function |
| `/` | search, marking every occurrence |
| `?` | what everything does |
| `Esc` | clear the focused function |

Clicking a function focuses it. There are **four ways out** and they all work:
`Esc`, clicking the row again, clicking empty space in the code pane, or
clicking the hint pill at the bottom.

The code pane also has **vim motions** (`j`/`k`, `gg`/`G`, `42G`, `H`/`M`/`L`,
`{`/`}`, `⌃d`/`⌃u`, `zz`, `yy`), **`/` search** that marks every occurrence in
the file, **font size** steppers, and a **blame gutter** that tints each line
with its author's colour — only inside a git repo, and it only shells out when
you press the button. Long lines always soft-wrap.

### Opening files

Pick a folder once — **Open a folder…** on the welcome screen — and then `⌘T`
finds anything in it by name. It matches the whole path, so `web/proc` and
`my_app/acc` both narrow, and build output and dependencies are never listed.
The folder is remembered and reopened next launch.

Pasting a full path into the same box works too, for files outside the project.

### A reading of several files

With a reading open, `⌘T` **adds** the file rather than starting over. Nothing is
seeded for it — your note stays yours — it simply widens what you can reference.

Tabs above the code carry the state: a green dot means your prose references that
file, amber that it has changed on disk since you read it, and hollow that you
opened it and never mentioned it. The `×` removes a file you opened by accident;
its snapshot leaves the reading, and neither your note nor the file on disk is
touched. To start a separate reading instead, go ← Home first.

References name a file when they need to and inherit one when they don't:

```markdown
`create_user/1` validates, then hands off.

Then `MyApp.Billing.charge/2` builds the invoice. `L25-29` is where it rounds.
```

The `L25-29` is billing's, because the sentence before it was — references resolve
in the order your prose takes, not by whichever tab is open. `billing.ex:25-29`
says it outright when you would rather be explicit. While editing, `/` offers
every function in the reading grouped by module, inserting bare names inside their
own file and module-qualified ones elsewhere.

### The doc is plain markdown

Your explanation is a normal markdown file with five extra fenced blocks. The
values live **in the text** — the file is the data, not a cache of one — so it
stays readable anywhere and you can edit anything you disagree with.

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

```lgtm:surface module=MyApp.Accounts
public:
  create_user/1 : 12
  get_user!/1   : 24
private:
  normalize/1   : 30 2 clauses
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

```lgtm:functions module=MyApp.Accounts
public:
  - create_user/1 : Entry point. Validates, then inserts.
  - get_user!/1   :
private:
  - normalize/1   : Trims and downcases the email.
```
````

They're seeded in that order — **how big is this, what's in it, what shape is
it, what does it touch**, and only then the block you write in. The first four
are generated and clicking anything in them focuses that function in the code;
`lgtm:functions` is yours, and it's the only one that gets reconciled when the
file changes.

Everything after the **first** colon is prose, so explanations may contain
colons and backticks freely. An empty explanation renders as a ghost `explain…`
placeholder. A malformed block degrades to a plain code fence — it never
loses writing.

## Where your data lives

One SQLite file:

```
~/Library/Application Support/com.alertmedia.lgtm/lgtm.db
```

Each row is one *reading*: your markdown, plus one row per file it covers holding
a **snapshot of that file as it was when you read it** — so your prose can never
silently drift onto code that changed underneath it, and each file can say for
itself whether it has moved. Deleting a reading from the library asks first, and
never touches your source files.

The file set lives in those rows rather than in the markdown, for the same reason
a single-file doc's path always did: the note has never declared which file it was
about. What the prose carries instead is module-qualified references, which tell a
reader which modules a reading covers without a manifest at the top.

## Project layout

```
mockup/*.html              standalone visual contracts — no build, just open them
IMPLEMENTATION_PLAN.md     the reasoning behind every decision
CLAUDE.md                  the map, for humans and Claude sessions alike

src/                       SvelteKit frontend
  lib/components/          CodePane, DocPane, Divider, Library, FileStrip, RefMenu
  lib/stores/              theme, focus (shared by both panes)
  lib/fileset.ts           the reading's file set: which file owns what
  lib/refs.ts              references in prose, and which file each one means
  lib/markdownit.ts        the lgtm:* fence renderers
  lib/{stats,surface,treemap,deps}.ts   one block each
src-tauri/                 Rust + Tauri backend
  src/parse/elixir.rs      tree-sitter → Outline   ← the load-bearing file
  src/seed.rs              Outline → starter markdown
  src/reconcile.rs         doc + re-parsed source → merged doc
  src/db/doc_files.rs      the files one reading covers
  src/git.rs               .git/HEAD read + lazy git blame + git log
```

Read `CLAUDE.md` for how it fits together, and `IMPLEMENTATION_PLAN.md` for why
each decision went the way it did.

## License

MIT
