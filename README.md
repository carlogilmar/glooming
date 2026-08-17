<div align="center">

<img src="app-icon.png" alt="lgtm" width="140" />

# lgtm

**A desktop tool for reading code deeply.**

Open one source file. The window splits — source on the left, your explanation
on the right. lgtm parses the file and seeds the explanation with the module's
functions, their sizes, and what git knows about it. You fill in the prose.

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
| `⌘O` | open a file |
| `⌘K` | library — everything you've written |
| `⌘S` | force a save (autosave already runs 800ms after you stop typing) |
| `Esc` | clear the focused function |

Clicking a function focuses it. There are **four ways out** and they all work:
`Esc`, clicking the row again, clicking empty space in the code pane, or
clicking the hint pill at the bottom.

The code pane also has a **blame gutter** (only inside a git repo, and it only
shells out when you press the button), **font size** steppers, and a **soft
wrap** toggle.

### The doc is plain markdown

Your explanation is a normal markdown file with three extra fenced blocks. The
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

Everything after the **first** colon is prose, so explanations may contain
colons and backticks freely. An empty explanation renders as a ghost `explain…`
placeholder. A malformed block degrades to a plain code fence — it never
loses writing.

## Where your data lives

One SQLite file:

```
~/Library/Application Support/com.alertmedia.lgtm/lgtm.db
```

Each row is one *reading*: the file path, your markdown, and a **snapshot of the
source as it was when you read it** — so your prose can never silently drift
onto code that changed underneath it. Deleting a doc from the library asks first,
and never touches your source file.

## Project layout

```
mockup/index.html          standalone visual contract — no build, just open it
IMPLEMENTATION_PLAN.md     the reasoning behind every decision
CLAUDE.md                  the map, for humans and Claude sessions alike

src/                       SvelteKit frontend
  lib/components/          CodePane, DocPane, Divider, Library
  lib/stores/              theme, focus (shared by both panes)
  lib/markdownit.ts        the lgtm:* fence renderers
src-tauri/                 Rust + Tauri backend
  src/parse/elixir.rs      tree-sitter → Outline   ← the load-bearing file
  src/seed.rs              Outline → starter markdown
  src/reconcile.rs         doc + re-parsed source → merged doc
  src/git.rs               .git/HEAD read + lazy git blame + git log
```

Read `CLAUDE.md` for how it fits together, and `IMPLEMENTATION_PLAN.md` for why
each decision went the way it did.

## License

MIT
