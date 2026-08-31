<div align="center">

<img src="app-icon.png" alt="Glooming" width="140" />

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
- **A gloom is one revision journey.** Your note, and the files it led you
  through. It gets a name — click it in the teal band under the header, or type
  one the moment a gloom is created — so a week later you know why you opened
  four files, not just which ones.
- **Nothing is written for you.** Opening a file writes a title, the module's own
  `@moduledoc`, and a blank page. The explanation is the part you came to do, and a
  generated first draft would only give you something to skim.
- **Explore above the note, in the same scroll.** What to navigate the current
  file by sits over your writing, and it changes with the kind of file: a module's
  **surface** — a two-column table of names and line numbers, six rows before it
  scrolls — and its **reaches**, a config's **settings** (marked env, `env!`,
  secret or literal), a suite's **describes** with the context each one
  accumulates. One scrollbar: scroll down to write, up to look something up.
- **Ten files is normal.** `⌘⇧T` lists the reading's files — filter, `↑↓↵` to
  switch, `×` to remove one you opened by accident. `⌘T` adds another.
- **See the boundary.** Below the surface, the module is drawn as a closed shape
  with lines only where a call leaves it. Functions that reach nothing stay quiet —
  that silence is the finding.
- **Follow a call across files.** When a reached module is also one of the files
  you are reviewing, clicking its function in the diagram switches file and
  focuses it.
- **A gloom is pinned to the version you read.** Each file is stored with the
  source as it was when you opened it, and that is what the pane shows — so a line
  number in your prose means the same thing next month. If a file moves on, lgtm
  says so and leaves the reading alone: there is no merge, no re-parse. To read the
  new version, start a new gloom; the old one stays beside it in the library, which
  is what you want when the question is *what changed*.
- **Read mode has its own theme.** `⌘R` and the whole window changes: both panes
  go to Nocturne — a deep teal dark, the same family as the gloom band — and your
  prose is set in Iowan Old Style at a book's line-height and measure. Scrolling
  the note then walks the code in the order your sentences take it.
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

cargo test   --manifest-path src-tauri/Cargo.toml    # parser, seeder, DB
cargo clippy --manifest-path src-tauri/Cargo.toml

python3 scripts/motion-audit.py   # every animation has a reduced-motion rule
python3 scripts/effect-audit.py   # no $effect reads and writes the same $state
python3 scripts/component-audit.py  # every imported component is rendered
```

## Building the macOS app

```bash
pnpm tauri build
```

Produces, under `src-tauri/target/release/bundle/`:

- `macos/Glooming.app` — drag to `/Applications`
- `dmg/Glooming_0.1.0_aarch64.dmg` — the installer, which opens as the usual
  drag-the-app-onto-Applications window

The build is unsigned, so the first launch needs **right-click → Open** (or
`xattr -cr /Applications/Glooming.app`) to get past Gatekeeper.

Your glooms live at `~/Library/Application Support/com.alertmedia.lgtm/lgtm.db` —
the bundle identifier is deliberately unchanged by the rename, so renaming the app
does not orphan anything you have written.

To regenerate the icon set after editing `app-icon.png` (1024², RGBA):

```bash
pnpm tauri icon       # no arguments — app-icon.png is the default input
```

## Using it

| Key | Does |
|---|---|
| `⌘O` | find a file by name in the open folder — same as `⌘T` |
| `⌘⇧O` | the system picker, for a file outside the project |
| `⌘T` | find a file by name in the open folder — or paste a path. With a reading open, this adds the file to it |
| `⌘K` | library — everything you've written |
| `⌘P` | jump to a function by name |
| `⌘S` | force a save (autosave already runs 800ms after you stop typing) |
| `⌘R` | read mode on / off |
| `⌘E` | edit / preview the note |
| `[` `]` | previous / next function |
| `/` | search, marking every occurrence |
| `?` | what everything does |
| `Esc` | clear the focused function |

Clicking a function focuses it. There are **four ways out** and they all work:
`Esc`, clicking the row again, clicking empty space in the code pane, or
clicking the hint pill at the bottom.

Motion is used sparingly and always to say something: the reach diagram traces a
call *outward* along its edge, opening a file rings the empty Explain section
rather than the whole pane, a file's tab dot fills the moment your prose first
mentions it, and read mode's trigger band flashes as each paragraph hands over.
All of it respects `prefers-reduced-motion` — the movement stops, the meaning
stays.

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

References name a module when they need to and inherit one when they don't, and
**the arity is optional**:

```markdown
`create_user` validates, then hands off.

Then `Billing.charge` builds the invoice. `L25-29` is where it rounds.
```

The `L25-29` is billing's, because the sentence before it was — references resolve
in the order your prose takes, not by whichever tab is open. `billing.ex:25-29`
says it outright when you would rather be explicit.

Leaving the arity off means *every* arity: `get_user` selects `get_user/1` and
`get_user/2` together, which is usually what you meant. Write `get_user/2` when
you mean exactly one.

While editing, `/` offers every function in the reading grouped by module, and the
footer shows the exact text you are about to get. **`/surface`** drops a directory
of the file you are looking at into the note — every function, sorted, with line
numbers — and **`/surface impact_stage.ex`** does it for any other file in the
reading. Add one where you want it and delete it when you don't; only the file the
reading started from gets blocks written for it up front. **Every reference it inserts is
module-qualified**, from the first file onward — a one-file reading becomes a
multi-file one the moment you open another, and the references you already wrote
have to still mean what they said.

A module is written by its **last segment**: `SingleTarget.foo`, not
`ImpactPipeline.Shared.AlertImpact.SingleTarget.foo` — the prefix is the same for
every module in the reading, so it says nothing where it costs the most. Any
longer piece of the path still resolves if you prefer to write it, and if two
modules in a reading share a last segment, both keep their full names rather than
becoming ambiguous.

Two things deliberately stay plain prose rather than becoming broken references:
a bare word that names no function (so `attrs` and `opts` are safe), and a
qualified name from a module outside the reading (so `String.trim` is too).
Anything explicit — a missing function in one of *your* modules, or any reference
carrying an arity — still renders struck through, because code moving out from
under your explanation is worth being told about.

### The doc is plain markdown

Your explanation is a normal markdown file with a few extra fenced blocks. The
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

```lgtm:deps module=MyApp.Accounts
  MyApp.Repo : app
    insert/1 : create_user/1
    get/2    : get_user/1
```

````

They're seeded in that order — **how big is this, what's in it, what does it
touch** — and then a `## Explain` heading with nothing under it. All three are
click targets: anything in them focuses that function in the code.

Two more blocks render but aren't seeded, because a generated section above the
part you write in has to earn that space every time you open a file:

- **`lgtm:treemap`** draws function sizes as area. It answers a question you ask
  occasionally — "is anything in here disproportionate?"
- **`lgtm:functions`** gives every function a prose slot of its own — an
  alphabetical index, which turned out to be the wrong shape for an explanation
  that follows a path through a module.

Write either fence yourself when you want it.

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
  lib/components/          CodePane, DocPane, ExploreSections, FilesModal, Library
  lib/stores/              theme, focus (shared by both panes)
  lib/fileset.ts           the reading's file set: which file owns what
  lib/refs.ts              references in prose, and which file each one means
  lib/markdownit.ts        the lgtm:* fence renderers
  lib/{stats,surface,treemap,deps}.ts   one block each
src-tauri/                 Rust + Tauri backend
  src/parse/elixir.rs      tree-sitter → Outline   ← the load-bearing file
  src/seed.rs              Outline → starter markdown
  src/db/doc_files.rs      the files one reading covers
  src/git.rs               .git/HEAD read + lazy git blame + git log
```

Read `CLAUDE.md` for how it fits together, and `IMPLEMENTATION_PLAN.md` for why
each decision went the way it did.

## License

MIT
