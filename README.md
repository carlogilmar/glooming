<div align="center">

<img src="app-icon.png" alt="Glooming" width="140" />

# Glooming

**A desktop tool for reading a codebase deeply.**

Open a project. Browse it, follow a call from one module into the next, and see
each module's shape beside its source. When something is worth keeping, write it
down — a **gloom** — in your own words, with references into the code. Then
**gloom** it: scroll your note and watch the code walk itself in the order your
prose takes it.

*A module is done when you understand it well enough to say **looks good to me**.*

</div>

---

## What it does

- **Onboarding to an unfamiliar codebase is the case it is built for.** You open a
  project, not a file; every module in it is one click or one `⌘T` away.
- **The Hub is a project's front page.** Its glooms as a table of contents:
  topics in reading order, one line per gloom, drag to arrange, `Recent` for
  everything newest first, a filter over names, tags and prose, and a quick look
  (Space) at any gloom without opening it. It flags a gloom whose references no
  longer match the code.
- **Two panes.** The source on the left — read-only, always live from disk. On the
  right, one of two things, switched with `⌘1` / `⌘2` / `⌘J`:
  - the **Woop** — what the code shows you: the module's rendered `@moduledoc`,
    its **surface** (every function and the line it is on), and its **reach**, the
    module drawn as a closed shape with lines only where a call leaves it. A
    config script shows its settings; a test suite its describes and tests.
  - the **Gloom** — your note.
- **References name the code, not a line.** Inline code naming a function becomes
  a chip: `` `ImpactPipeline.Weather.Pipeline.handle_message/3` `` shows as
  `Weather.Pipeline.handle_message/3`, and clicking it opens and selects the
  function. Lines are written *inside* a function (`…process_through_stages/2:3-7`)
  so an edit elsewhere cannot move them. A reference whose function is gone is
  struck through rather than quietly wrong.
- **Glooming.** `⌘R` (`▷ Gloom`) turns the window into a reading surface: scroll
  the note, or press `↓`, and the code pane follows your references one by one,
  opening each module as your prose reaches it — even ones you never opened.
- **Nothing is copied out of your code.** A gloom stores your prose and nothing
  else, so it always describes the code as it is now, and the decay of an old
  explanation is visible instead of silent.
- **Reads, never writes.** The left pane is read-only, permanently. Glooming never
  modifies your repository.
- **No AI.** The explanation is yours. Writing it is the point — generating it
  would defeat the tool.

Elixir is the only language so far. Any other file opens as plain, read-only
text. The architecture assumes more languages will follow.

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
git clone <repo-url> glooming
cd glooming

# Install the pinned toolchain. Plugins are one-time per machine.
asdf plugin add rust && asdf plugin add nodejs && asdf plugin add pnpm
asdf install

pnpm install          # JS deps; cargo fetches Rust deps on first build
pnpm tauri dev        # run the app
```

The first `pnpm tauri dev` compiles the Rust side from scratch — a few minutes.
After that a Svelte change hot-reloads in about a second; a Rust change rebuilds
and restarts the app.

> **`Port 1420 is already in use`** means an orphaned Vite server survived a
> previous run: `lsof -ti:1420 | xargs kill -9`

## Everyday commands

```bash
pnpm tauri dev        # full app, hot reload
pnpm dev              # frontend only in a browser (panes render, IPC fails)
pnpm check            # svelte-check — must be clean
pnpm build            # frontend production build

cargo test   --manifest-path src-tauri/Cargo.toml    # parser, index, DB
cargo clippy --manifest-path src-tauri/Cargo.toml

python3 scripts/motion-audit.py     # every animation has a reduced-motion rule
python3 scripts/effect-audit.py     # no $effect reads and writes the same $state
python3 scripts/component-audit.py  # every imported component is rendered
```

## Building the macOS app

```bash
pnpm tauri build
```

Produces, under `src-tauri/target/release/bundle/`:

- `macos/Glooming.app` — drag to `/Applications`
- `dmg/Glooming_0.1.0_aarch64.dmg` — the installer: the usual
  drag-the-app-onto-Applications window, on a generated backdrop
  (`node scripts/dmg-background.mjs` rebuilds it)

The build is unsigned, so the first launch needs **right-click → Open** (or
`xattr -cr /Applications/Glooming.app`) to get past Gatekeeper.

To regenerate the icon set after editing `app-icon.png` (1024², RGBA):

```bash
pnpm tauri icon       # no arguments — app-icon.png is the default input
```

## Using it

### A day of it

1. **Open a folder…** on the home screen. The project is remembered, with its
   glooms, and listed there from then on.
2. You land on the **Hub**. With nothing written yet it says so; otherwise it is
   your table of contents.
3. Find a module — `⌘T`, the **Files** rail on the left (`⌘\`), or a reference in
   a gloom. The Woop shows what it is for, what it offers and what it reaches.
   Click an outside function in the reach diagram to see who calls it, and open it
   from there.
4. When you have worked something out, **`+ New`** in the band starts a gloom.
   Name it, tag it, write. `⌘2` / `⌘1` (or `⌘J`) switch between your Gloom and
   the Woop.
5. **`▷ Gloom`** (`⌘R`) and read it back with the code following along. Where you
   lose the thread is where your understanding stops.
6. Back in the Hub (`⌘K`), drag it into a topic. Tomorrow is a different feature
   and a different gloom.

### Writing references

| You write | It means |
|---|---|
| `` `MyApp.Billing.to_cents` `` | the function, every arity — what `/` inserts |
| `` `MyApp.Billing.to_cents/1` `` | exactly one arity |
| `` `MyApp.Billing.charge/2:3-6` `` | lines 3–6 *of that function*, counted from its first line |
| `` `MyApp.Billing` `` | the module itself — a step that opens it |
| `` `to_cents/1` ``, `` `String.trim` `` | plain code: no module, or a module outside the project |

- **Always the full module name in the text.** The chip shows the shortest name
  that is unique in the project — `Weather.Pipeline` and `Ai.Pipeline` keep their
  first part, `CommonStages` stands alone — with the full name on hover.
- **A dashed amber chip is ambiguous**: `Pipeline.run` when four modules end in
  `Pipeline`. Click it to choose; the text is rewritten to the full name.
- **`/` while editing** offers the module on screen and its functions. For any
  other module, type the full name — the note reads that file without your
  opening it.

A gloom is plain markdown. A list with one reference per item reads as a
step-by-step walk, and read mode treats each item as a row.

### Keys

| Key | Does |
|---|---|
| `⌘K` | the Hub |
| `⌘T` / `⌘O` | find a file in the project, or paste a path |
| `⌘⇧O` | the system picker, for a file outside the project |
| `⌘\` | the file tree |
| `⌘[` `⌘]` | back / forward along the modules you opened |
| `⌘1` `⌘2` `⌘J` | the Woop, the Gloom, flip between them |
| `⌘R` | glooming — read mode on / off |
| `↓` `↑` | while glooming: the next / previous reference |
| `⌘E` | edit / preview the gloom |
| `⌘S` | force a save (autosave runs 800ms after you stop typing) |
| `⌘P` | jump to a function by name |
| `[` `]` | previous / next function in the code |
| `?` | what everything does |
| `Esc` | clear the selection; leaves read mode |

The code pane also has **vim motions** (`j`/`k`, `gg`/`G`, `42G`, `H`/`M`/`L`,
`{`/`}`, `⌃d`/`⌃u`, `zz`, `yy`), **`/` search** that marks every occurrence, a
font stepper, and a **blame gutter** — only inside a git repo, and it only shells
out to `git blame` when you press the button.

Clicking a function focuses it: every clause highlighted, its `@spec` in its own
colour, the rest of the file dimmed. There are four ways out — `Esc`, clicking it
again, clicking empty space in the code, or the hint pill at the bottom.

Motion is used sparingly and always to say something — the reach diagram traces a
call *outward*, a reference's underline wipes in as it becomes current — and all
of it respects `prefers-reduced-motion`: the movement stops, the meaning stays.

## Where your data lives

One SQLite file:

```
~/Library/Application Support/com.alertmedia.lgtm/lgtm.db
```

It holds the projects you have opened, their topics, and their glooms — your
prose, its name and tags, and where it sits in the Hub. **No code is ever stored**:
every file is read live from disk, every time. The bundle identifier stays
`com.alertmedia.lgtm` across the rename to Glooming, because the database path is
derived from it.

Forgetting a project from the home screen is refused while it has glooms, and
deleting a gloom asks first — and never touches your source.

## Project layout

```
mockup/*.html              standalone visual contracts — no build, just open them
CLAUDE.md                  the map: how it fits together, and why
IMPLEMENTATION_PLAN.md     the original v1 reasoning (predates projects)

src/                       SvelteKit frontend
  routes/+page.svelte      the shell: home, the Hub, the workspace, the keys
  lib/components/          CodePane, DocPane (Woop | Gloom), ExploreSections,
                           Hub, FilePalette, RefMenu, HelpModal, …
  lib/refs.ts              references in prose, resolved by name across the project
  lib/hub.ts               what the Hub says about a gloom
  lib/moduledoc.ts         a module's @moduledoc, rendered for the Woop
  lib/deps.ts              the reach diagram
src-tauri/                 Rust + Tauri backend
  src/parse/elixir.rs      tree-sitter → Outline   ← the load-bearing file
  src/index.rs             the project index: module names → files
  src/db/                  projects, glooms, topics
```

## License

MIT
