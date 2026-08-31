# lgtm

A desktop tool for **reading code deeply**. You open one source file; the window
splits with the source on the left and your explanation on the right. lgtm
parses the file and seeds the explanation with what it found — its size and
history, its surface, what it reaches outside itself — and you fill in the prose. Clicking anything in the explanation focuses
it in the code, and with `▷ Read` on, **scrolling the explanation walks the
code in the order your prose takes it**. The explanation is saved and comes back
next time.

Not every file is a module: a config script and a test suite get blocks of their
own, and anything unrecognised gets a blank page rather than empty ones.

A reading is not always one file either. With a reading open, opening another
file **joins it** — the files you open during a review are the set — and
references may name any of them.

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
| Call graphs, cross-module analysis, "who calls this" | A reading may cover several files, but lgtm never reasons *across* them. It parses each one and lets your prose do the joining. |
| Groups you create and manage | There is no gesture for making one. You open files during a review and those files are the reading; the `×` on a tab undoes an accident. Anything more is a project manager, not a reading tool. |
| Reconciling a changed file into an existing gloom | lgtm reads committed code, not code in progress. A gloom is a reading of a particular version; merging a changed file into it produces a note half of which describes lines that no longer exist. Open the new version as a new gloom — the old reading stays intact beside it. |
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
mockup/group.html              a reading across several files
mockup/motion.html             the animation contract, replayable
mockup/explore.html            the explore drawer: surface + reach beside the note
mockup/files.html              ten mixed-kind files: the files modal, and the drawer per kind
mockup/motion-audit.html       every motion finding, before and after
mockup/reading-theme.html      read mode as a theme: five palettes, five faces, measured
mockup/home.html               the home screen rebuilt around glooms
mockup/stack.html              the files modal as the shape of the change
scripts/motion-audit.py        every animation vs its reduced-motion rule
scripts/effect-audit.py        $effects that read and write the same $state
scripts/component-audit.py     components imported but never rendered
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
      FontStepper.svelte       A− / A+, shared by both panes
      ExploreSections.svelte   surface table + the boundary, above the note in one scroll
      Sky.svelte               the masthead's weather: bloom, cosmos or aurora, rolled per visit
      FilesModal.svelte        ⌘⇧T — the reading's files: switch, add, remove
      FilePalette.svelte       ⌘T — find a file, or add one to the reading
      FnPalette.svelte         ⌘P — jump to a function by name
      RefMenu.svelte           / — insert a reference, from any file in the reading
      HelpModal.svelte         ? — what everything does
      Divider.svelte           draggable split, double-click reset
      Library.svelte           saved docs: search, sort, folder grouping, keyboard nav, delete
    stores/
      theme.svelte.ts          ported from Alexandria; lgtm defaults to LIGHT
      focus.svelte.ts          which function is selected — shared by BOTH panes
      fontSize.svelte.ts       a remembered, clamped font size; one per pane
    explore.ts                 what the drawer shows: surface rows, reach lines
    fileset.ts                 the reading's file set: which file owns what
    markdownit.ts              markdown-it + the lgtm:* fence renderers
    lgtmBlock.ts               parses the functions block
    refs.ts                    references in prose, and which file each one means
    slash.ts                   the `/` menu's grammar: commands and their arguments
    select.ts                  one signature → every span worth highlighting
    when.ts                    relative dates (the library's ages)
    treemap.ts                 parses + draws the treemap block
    stats.ts                   parses + draws the stats block
    deps.ts                    parses + draws the reach block (see mockup/deps.html)
    shape.ts                   the gloom as layers: thickness ∝ size (see mockup/stack.html)
    surface.ts                 parses + draws the surface block (see mockup/surface.html)
    settings.ts                parses + draws the config settings block
    tests.ts                   parses + draws the test suite block
    ipc.ts                     typed wrappers over invoke()

src-tauri/
  src/
    parse/elixir.rs            tree-sitter → Outline   ← the load-bearing file
    parse/kinds.rs             config scripts and test suites
    seed.rs                    Outline + source + git history → starter markdown
    git.rs                     .git/HEAD read + lazy blame + log for stats
    db/{mod,models,docs}.rs    pool, serde shapes, doc CRUD
    db/doc_files.rs            the files one reading covers
    commands/{files,docs}.rs   IPC surface (blame lives in files.rs)
    commands/projects.rs       open a folder, walk it for Elixir files
    db/projects.rs             remembered folders, most recent first
  migrations/0001_initial.sql
  migrations/0002_projects.sql
  migrations/0003_doc_files.sql
  tests/pipeline.rs            end-to-end, pinned to the mockup
```

## Core concepts

### To the user, a reading is a **gloom**

The name is a UI label and nothing else: `Reading`, `ReadingFile` and
`open_reading` stay as they are, and `reading` in the frontend still also means
read mode. Renaming the internals was considered and declined as churn.

**A gloom is one revision journey** — your note, and the files it led you
through. It gets its own band under the app header, and the band is **filled with
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

**One line: the name at the left, the wordmark at the right end of it.** A title is
the first thing on its line, not the middle of it — centring it needed a
three-column grid and 56px of height to hold a stacked caption, and bought nothing.
A file count lived on the right briefly and was cut: the file button in the row
above already says it, and the same fact in two places is one place too many.

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

**The name is centred, and set in a serif.** `--serif` is a system stack (`ui-serif`
→ New York → Georgia); the app is offline and its CSP blocks a webfont, so nothing
is downloaded. In a tool made of monospace and UI sans, a serif reads as a *title*
rather than as one more label — it is the one line here you wrote as prose. The
band is `grid-template-columns: 1fr auto 1fr`, not flex, so the name sits in the
middle of the **window**: with flex it would drift as the count went from "1 file"
to "12 files", and a title that moves is not a title. The same serif is used
wherever a gloom's name is *shown* — the library, the welcome recents — so it is
recognisable as the same thing; filenames and paths stay mono and sans, because
they are data rather than something you wrote.

**A gloom has a name, and it is `docs.title`.** That column has said "seeded from
the module name, editable" since the first migration and `save_doc` always took a
title — nothing had ever done the editing. Click the name to rename it, `↵` to
commit, `esc` to throw the edit away.

**A new gloom opens with its name selected**, so naming it costs a sentence and
skipping it costs one key. That is the only moment it happens: on a gloom you
have already named, taking the caret is an interruption rather than an
invitation. There is deliberately no dialog on the way in — asking for a name
*before* you have read anything asks at the one moment you cannot answer, which
is the same reason there is no "new group?" prompt.

**An untouched name shows as an invitation**, dimmed with a line saying what it
is for. `seededTitle` is *derived and compared* rather than stored as a flag: a
flag would have to be maintained by every path that writes a title, and this one
cannot drift. Same idiom as the Explain invitation and the hollow file dot.

The rename takes the server's row but puts **your** markdown back — an autosave
may be in flight, and adopting the server's copy would lose the sentence you are
half-way through. Same one-payload habit as the file mutations.

### A doc is a *reading*, not a note

One row in `docs` = one file + the markdown you wrote about it. It stores a
**snapshot of the source** (`source` + `source_sha`), so your prose can never
silently drift onto code that changed underneath it. Opening a path that
already has docs offers the existing one rather than starting a duplicate.

`branch` and `label` are inert metadata — they exist so two readings of the
same file are distinguishable. They never drive file loading.

### A reading can cover several files

A change worth reviewing rarely lives in one file, so a doc is one note over a
**set** of files. The set accumulates: with a reading open, opening a file joins
it. There is deliberately **no gesture for creating a group** — the files you
open during a review *are* the group, and asking "same reading or a new one?"
fifty times a day would cost more than the occasional stray file. A stray file
is undone by the `×` on its tab; the way to start a separate reading is
← Home, then open.

**The file set lives in rows, not in the markdown.** This is the one place the
"markdown IS the data" rule does not apply, and the reason is consistency, not
convenience: a single-file doc's `path` has always been a column, so the note has
never declared which file it was about. Putting a *set* of paths into the prose
would be a new inconsistency rather than a preserved principle. Portability is
already covered — the module-qualified references in the prose tell a reader
which modules a reading covers, inline and in context, which beats a list at the
top. An earlier design had an `lgtm:files` block; it was cut for exactly this,
and with it went the question of what happens when the block and the DB disagree.

`docs.path` stays the **origin**: the file the reading was seeded from, whose
module owns `lgtm:functions`, what the library groups by, and the one file that
cannot be removed. Everything else in `doc_files` joined later.

**One snapshot per file, so staleness is per-file.** A single `docs.source` could
only ever say "something changed". `docs.source` is still maintained for the
origin, because the library and the chooser read it — Both are written once, when the file
joins, and never again.

**One gloom, one branch.** A file may only join a gloom from the branch the gloom
was read on. A gloom is a reading of *one version* of a change; a file snapshotted
from another branch is a second version of the world sitting silently in the same
note, with line numbers your prose will describe as though they were the same —
and nothing later can untangle that, so `add_doc_file` refuses at the door with a
sentence that says what to do (*check out `main` to keep going, or start a new
gloom for `fix/x`*).

`branch_mismatch` is deliberately **narrow**: it refuses only when it is sure. Both
sides must be in a repository and the **same** repository — a file from another
checkout has branch names that mean nothing here — and both branch names must be
readable. A detached HEAD, a repo with no commits, a file outside git: all pass,
because *"I cannot tell"* is not *"no"*. A false refusal blocks work; a false pass
costs one confusing note. Pinned by `a_gloom_without_a_branch_accepts_anything` and
`a_file_outside_git_is_not_refused`.

**The live branch has to be asked for, not remembered.** `ReadingFile.branch` is
sampled when the reading is *built*, so it says where you were when you opened the
gloom — and the case this guard exists for is checking out another branch in a
terminal afterwards, which nothing in the app has any reason to notice. The first
cut compared against that stale value and blocked nothing at all. `branch_of` is a
command of its own (one read of `.git/HEAD`), re-asked when the reading changes,
whenever the window comes back to the front — you switched branches in the terminal
you just came from — and **awaited at the gesture itself**, because a check only as
fresh as the last focus is not fresh enough for the one action it guards.

**The UI stops offering what the guard refuses.** A rule enforced only in Rust is a
rule you learn by tripping over it, so `offBranch` — `docs.branch` against the live
branch — disables *Find…* and *Open file…*, short-circuits `⌘T`,
`⌘O` and `⌘⇧O` into the same explanation, replaces the files modal's **Add a file**
row with why it is missing, and turns the branch chip **quiet** — neutral ink, dashed
edge. Amber said *something is wrong*, and nothing is: the reading is intact and
reads normally from here, it simply cannot grow. Quiet says the same thing the
disabled buttons beside it are saying, in the same language, and avoids a third
meaning in a row that already has two. The chip still names only the gloom's
branch, because that is the fact it is for; where *you* are is in its tooltip and
in the banner. One sentence, at whichever gesture you reached
for.

**And the banner says what to do rather than what happened.** Opening a gloom whose
files are not on the current branch used to report *"Not on disk any more: … —
showing the snapshot"*, which is true, already handled, and not news: the snapshot
is the whole design. It now says, in one line, *"You are on `fix/x` — check
out `main` to add files to this gloom."* — and it says it as a **notice**, not as
the error banner: teal rather than amber, arriving from just above, holding long
enough to read twice, and leaving on its own after 5.8s.

That split is the point. The banner is for things that went wrong and stay wrong
until you deal with them — a failed save, a missing file — so it waits to be
dismissed. Being on another branch is a *state* you are standing in, visible in the
chip and in the disabled buttons, and a strip that stays across the top saying so
becomes furniture within a minute. `noticeLeaving` gives it a fade rather than a
disappearance; under reduced motion it still appears and still goes, it just does
not travel to get here.

**The notice is structured, not a sentence.** `{ here, gloom }` rather than
pre-formatted text, so the two branch names can be drawn as **badges** — the
sentence is scaffolding, the names are what you read — and so they can be *copied*.

**Every branch name in the app is a copy button**, the chip included. The reply to
this notice is a `git checkout` in another window, so the name is something you are
about to type somewhere else. The confirmation lands on the name itself (`copied ✓`
for 1.4s): a toast for two words would be louder than the thing it confirms. Same trigger, useful sentence, and the same
sentence every refused gesture gives — a banner is a glance, and the reassurance
that the reading still works belongs in `?`.

Opening an existing gloom on the wrong branch is **not** restricted — the reading
is made of snapshots and shows the same thing whatever you have checked out. It is
only *adding* that mixes versions.

**Adding a file seeds nothing.** No blocks, no headings, no edit to your prose.
The only thing it changes is what `/` can offer you. That is the whole feature:
by the second file the note is yours, and generating more of it would be writing
your reading for you.

**References answer "which file", and half of them don't say.** So they resolve
in **document order**, each threading the current file forward:

```markdown
Then `MyApp.Billing.charge/2` builds the invoice.   → billing.ex
`L25-29` is where it rounds.                        → still billing.ex
```

`refs.ts` is therefore a stateful resolver, reset once per render by a `md.render`
override. Order-dependence is the point: it means an unqualified name resolves
the way the sentence reads, and — this is what matters — it depends **only** on
document order, never on which tab happens to be open. Keying off the current tab
would make read mode walk a different path depending on where you were standing
when you started scrolling. Search order for a bare name is *threaded file →
origin → the rest in strip order*.

**The arity is optional, and the `/` menu leaves it out.** A reference without one
means *every* arity, which is already how selection thinks: the focus store tints
sibling arities as "related" on the grounds that they are one function to a
reader. So `get_user` selects `get_user/1` and `get_user/2` together, `get_user/1`
is the narrowing, and `search/1..2` — never a readable thing to have in the middle
of a sentence — is retired as a reference form.

A name-only reference therefore carries **several spans, not one**. The chip
holds `data-ranges="20-22,28-29"`, because two arities can sit either side of an
unrelated function and one enclosing `min..max` range would light that function up
too. `data-line` / `data-end` remain the scroll target and the line count.

| Written | Means |
|---|---|
| `` `Billing.to_cents` `` | every arity; names its own module, and moves the thread |
| `` `Billing.to_cents/1` `` | the same, narrowed to one arity |
| `` `to_cents` `` | every arity, in the threaded module |
| `` `to_cents/1` `` | the same, narrowed |
| `` `L25-29` `` | plain lines in the threaded file |
| `` `billing.ex:25-29` `` | plain lines, said out loud about another file |

**A module is named by its last segment.** `ImpactPipeline.Shared.AlertImpact.SingleTarget.foo`
is unreadable mid-sentence, and the prefix is identical for every module in the
reading — so it carries no information exactly where it costs the most. The last
segment *is* the module's name; everything before it is where the file lives.

`findModule` resolves an exact name first, then any **dot-boundary** suffix, so
`SingleTarget`, `AlertImpact.SingleTarget` and the full path all reach the same
module and you can write as much of it as you feel like. The dot is what stops
`Target` matching `SingleTarget`.

**Shortened only where the short form is unique.** Two files whose modules both
end in `.Worker` would give `Worker.run` two meanings, so `moduleLabels` leaves
*those* fully qualified and shortens everything else. Ambiguity here is rare;
resolving it silently the wrong way would not be.

The same rule applies wherever a module name is **printed** — block headers, the
reach diagram's boundary and its outside labels — with the full name on a
`title`/`<title>`. The `module=` in the block *text* stays the full name: that is
what locates the file, and shortening it there would break the mapping. In the
reach diagram the kind label's offset is computed from the *drawn* width, not the
full name's, or it lands in the middle of nowhere.

**Two forms are allowed to fail silently, and both have to be.** Making the arity
optional widened what *looks* like a reference straight into ordinary prose, so
`resolveRef` returns `null` — plain inline code, no strikethrough — rather than
`"dangling"` in exactly two cases:

- **A bare name that resolves to nothing.** Prose about Elixir is full of
  lowercase words in backticks that are not functions: `attrs`, `opts`, `conn`,
  `config`, `path`. Striking them through would ruin half of what you write.
- **A qualified name whose module is not in the reading.** `String.trim`,
  `Enum.map`, `GenServer.call` are prose about code outside the reading, not
  broken links into it. Before the arity became optional these did not match at
  all; afterwards every mention of the standard library struck itself through.
  `knowsModule` is the gate.

Everything explicit still dangles visibly: a missing function in a module you *do*
have, and anything carrying an arity. The trade on the second rule is that
removing a file makes its references go quiet rather than break loudly — that case
has its own signals (the `×` spells out what it does, and the strip shows the
set), whereas a struck-through `String.trim` has none and just looks like a bug.

**Every reference the `/` menu inserts names its module. Always** — from the
first file onward, not only once a reading covers several.

That rule was arrived at twice, and the second time corrected an inconsistency.
`seed.rs` already qualifies the references it generates on the grounds that
**generated text is written once and never revisited**, so it has to stay correct
as the reading grows. An inserted reference is no more revisited than a seeded
one: you open accounts.ex, insert six bare references, then open billing.ex — and
those six are now resolved by *search order* rather than by what they say.
Qualifying only above two modules made the seeder and the menu disagree about the
same durability argument.

It is also what makes a note stand on its own where it matters most: pasted into a
PR comment, with no file strip beside it to explain itself.

The label is the module's own name, **never the tab that happens to be open**. An
earlier version keyed off the tab, so looking at billing.ex and picking `charge/2`
inserted a bare name: the same keystroke produced different text depending on
where you were standing. The tab still *ranks* the menu, because you are usually
writing about what you are looking at — that is a preference, not a meaning. The
footer spells out the exact text `↵` will insert, since the row shows a bare
signature and what lands is neither bare nor arity-bearing.

A bare name still *resolves* (threaded file → origin → the rest), because prose
you typed yourself may well be loose and old docs are full of it. It is simply not
something the menu will ever produce.

**Blocks find their own file from the `module=` they already carried.** That
attribute existed for readability; in a multi-file reading it becomes the thing
that locates the right outline, so `lgtm:functions module=MyApp.Billing` keeps
working while you are standing in accounts.ex. Every block renderer's output is
tagged with `data-path` by one helper in `markdownit.ts` rather than by threading
a path through six signatures — which is safe only because each of them returns a
string starting with `<div`.

**Crossing a file is a different transition from moving within one.** The 620ms
crossfade — outgoing ranges lingering under incoming ones — is what makes a jump
read as a connection instead of a cut. Across two different files there is
nothing shared to fade between, so the same treatment reads as a glitch. The pane
dips instead and **the code pane's own header lights up in read-mode gold**, and
`focus.step` takes a `path` so it can skip the crossfade. `focus.path` exists for
that one decision.

It is full-strength `--read` at the peak, not a mix into the header's own surface:
mixed down it becomes a beige wash, which is a different signal. The controls in
that header fade out for the announcement and come back as the gold recedes —
`A−`/`A+` on a gold fill is a control wearing someone else's colour, and recolouring
them would mean maintaining a second set of states for a moment nobody clicks in.
It also leaves the filename alone on the fill, which is the message.

Naming the landing used to be a gold pill floating over the code. The filename was
on screen the whole time, two inches away — so the pill was a second copy of a fact
rather than a signal about it, and it covered the first line of the file to tell you
which file it was. Lighting the header points at where the answer already lives. No
gate is needed: `CodePane` is keyed on the path, so a mount *is* a new file.

**A pane must paint its own surface *and* its own text, not merely define the
tokens.** Two bugs of one shape, found a day apart:

- `body` sets `color: var(--fg)` **once**, and what descends from it is the
  *computed* colour. Redefining `--fg` on an ancestor of the code pane therefore
  changed the token and repainted nothing: every untokenised glyph stayed
  near-black on read mode's near-black surface. `.panebody` in `CodePane` now
  re-declares `color: var(--fg)`. The doc pane never had it, because its
  `.panebody` always set `color: var(--doc-fg)`.
- The same thing with the background, one level up. The swap dip faded
  `.codewrap` to nothing over a `.pane` that set `--code-bg` and never used it, so
  every cross-file jump revealed the app's `--bg` — a white flash, and in read mode
  a white flash through a dark theme. `.pane:not(.grow)` now paints
  `var(--code-bg)`.

Defining a token is not using it, and a theme swap only repaints what some rule
actually reads. Both bugs were invisible until a *subtree* was themed differently
from the document, which is exactly what read mode does.

**The dip goes to a tenth, not to nothing.** At zero the pane is blank for 130ms
and reads as a *load*; a deep dip with 4px of downward travel reads as the same
pane changing what it shows. The arriving file then plays its own gutter cascade
for free, because `CodePane` is keyed on the path and a mount is a new file.

**CodePane is keyed on the path.** Switching file remounts it, deliberately:
blame and a search belong to the file they were run against, and carrying either
across a switch would attribute one file's authors to another.

**The hollow dot is the finding.** A tab's dot is green when your prose
references that file, amber when it has changed on disk, and hollow when you
opened it and never mentioned it. Opening a file to check something and never
coming back to it is the normal accident of a review, and the hollow dot is the
file-level version of an Explain section still showing its invitation. `DocPane` reports the
referenced paths by walking `code.ref[data-path]` after each render — the DOM
already knows, so nothing is recomputed.

**`for_path` matches the whole set, not just the origin.** You open billing.ex on
Tuesday as part of a reading of accounts.ex, and on Wednesday you open billing.ex
first. Matching only `docs.path` would hide that reading and quietly start a
duplicate. Choosing it also lands you on the file you arrived by, not on the
origin — you asked for that file; the reading is only how you are going to read
it.

The strip only appears at two files or more, and `vocabulary()` inserts bare
names inside their own file, so **a single-file reading looks and behaves exactly
as it always did**.

### The markdown IS the data

This is the load-bearing principle for every `lgtm:*` block. Blocks are written
out **with their values already in them** at seed time; renderers only format
what the text says. Nothing is recomputed at render time.

That buys three things: the doc is readable as plain text anywhere, it survives
being pasted into a PR comment, and you can hand-edit anything you disagree
with. The live `Outline` is consulted for exactly **one** thing — the line
number a row or tile jumps to.

The one deliberate exception is **which files a reading covers**, which lives in
`doc_files` rather than in the prose — see above for why that is the consistent
choice rather than a violation.

A block whose body is empty renders as a short "re-seed this doc, or write
`…` style rows here" hint, never as a blank box.

### A file is not always a module

`FileKind` decides which blocks a doc gets, because the blocks that suit one
shape say nothing about another:

| Kind | Detected by | What the drawer shows |
|---|---|---|
| `Module` | `defmodule` with `def`s | **surface** + **reaches** |
| `Config` | `import Config` / any `config` call | **settings**, grouped by app, env vs literal |
| `Test` | `use …Case` in a module body | the **suite**, then its **describes** |
| `Plain` | anything else | one line saying there is nothing to navigate by |

**This table used to decide which blocks a doc got *seeded* with. Now it decides
what you navigate a file by** — which is the job it was always describing. No kind
seeds a block: every doc is a title, a summary and a blank page.

The fallback is still the point of the whole mechanism. A config file parsed as a
module has nothing to lay out, and **an empty panel reads as broken** — so `Plain`
gets one line saying nothing structural was recognised, in the drawer *and* in the
seeded note. No error, no empty boxes.

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

**No block is ever rewritten after it is written.** A block is a snapshot of what
the code was when you read it, and a gloom holds the source that produced it — the
two cannot drift, because neither one changes.

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

| Tag | File kind | Renderer | Seeded? |
|---|---|---|---|
| `lgtm:stats` | all | `stats.ts` — key-agnostic, formats whatever keys it finds | yes |
| `lgtm:surface` | module | `surface.ts` — two scrolling columns, sorted by name | yes |
| `lgtm:treemap` | module | `treemap.ts` — squarified, top 3 labelled | **no** — write it when you want it |
| `lgtm:functions` | module | `markdownit.ts` — a prose slot per function | **no** — write it when you want it |
| `lgtm:deps` | module | `deps.ts` — the boundary, omitted when nothing is reached | when it reaches something |
| `lgtm:settings` | config | `settings.ts` — grouped by app, env vs literal | yes |
| `lgtm:tests` | test | `tests.ts` — describes, setups, assertion strips | yes |

`lgtm:stats` serving four file kinds is why it renders whatever keys the text
carries instead of expecting a fixed set — one renderer, no per-kind variants.

### The right pane is a split you own

The pane has two jobs that want the same height, and for a while it was one
column — surface, boundary, then your note — so you could scroll up to look
something up without leaving what you were writing. A day of real use found the
cost: on a module with a long surface and a busy boundary the note starts a screen
and a half down, and **writing** becomes the thing you travel to.

So the height is yours. A **grip** between the two regions: drag it anywhere,
collapse either side to nothing, double-click to reset, `⌥1` / `⌥2` for the two
ends from the keyboard.

**This is not the drawer that was cut.** That one was *fixed* — a band permanently
taking 270px from the pane you write in, with a collapse toggle as the workaround,
and the toggle was the tell. A band you can drag to zero and abolish with one click
costs you nothing. What the earlier argument got right and still holds: two scroll
regions in one column is a real cost, and the collapse is what pays it.

**Its two ends are the tabs the alternative would have been.** Tabs (`Explore` /
`Gloom`) were mocked beside it in `mockup/panes.html` and lose on two counts: a map
behind a mode change is a map you forget — the argument that already killed the
`⌥R` reach overlay — and the two jobs genuinely overlap, since writing *"…then it
hands off to `changeset/2`"* is the moment you want the surface in view. A split
does everything tabs do and everything in between; if it turns out nobody ever
sits between the ends, the drag can be deleted and nothing is lost.

**Read mode and edit mode both take the whole pane.** Neither is navigating. That
was mocked as its own arrangement and folded in here instead of shipped alone.

**The navigation half is warm, and it took two wrong answers to get there.** A grey
mix (`--doc-raised` toward `--doc-line`) read as *dirty paper*. A teal wash read as a
green panel dropped into a warm room, arguing with the gold `▷ LGTM` sits in and with
the pane header above it. Both failed the same way: they made it a different **hue**
where what it needed was a different **level**.

This pane is warm paper and warm chrome. The navigation half is simply more of the
chrome — `--read` at 9%, one step deeper than the header, 1.12:1 against the note
(1.16:1 dark) — and the grip carries the same warmth at 15%, so it belongs to the
half it controls. Teal stays where it means something: the boundary's outside, the
gloom band, the file button.

**The grip is a seam, not a header.** At 28px with a label it was a third band
between two others, in a tone close enough to the navigation half that the eye could
not tell which side it belonged to. It is 16px now and *deeper than both* — `--read`
at 26%, 1.29:1 against the navigation above it and 1.45:1 against the paper below —
which is what a seam is. The label went with the height: a divider that has to be
captioned is not dividing anything. `GRIP_H` in the script must match its CSS height,
or the drag clamps against the wrong maximum.

**The labelled rule at the end of the sections is gone.** It said *"your note"*
across the seam; the grip says it four pixels away. The same fact in two places is
one place too many, and the grip's version also moves.

**The split is remembered per gloom** (`gloomSplit:<id>` in `localStorage`): a
config script and a forty-function module want different balances, and a split you
re-make every time is one you stop using.

**`body` is the note's scroller**, not the pane. Read mode's lead-in, tail, band and
step tests all measure `body`, so pointing it at the region that actually holds the
prose keeps every one of those measurements describing the thing being read. The
grip lives outside both scrollers, or it would scroll away from the split it
controls.

### The right pane was one column

Surface, then the boundary diagram, then your note — **one scrollbar**, scroll
down to write and up to look something up. `ExploreSections.svelte` is rendered as
a *snippet* inside `DocPane`'s own scroll container, which is what makes it one
region rather than two.

This replaced a fixed-height drawer with its own scrollbar and a collapse toggle,
and both parts of that were wrong:

- **A band permanently taking 270px from the pane you write in** costs you the
  thing that pane is for. Collapsing it was the workaround, which is the tell.
- **Two scroll regions in one column means neither can be scrolled past.** Every
  gesture needs you to first decide which one you are in.

Being able to scroll away is also what makes the **diagram affordable inline**,
and inline is the only place it does its job: it is a map you glance at while
reading the file. Behind a button — which is where it briefly was — it is a thing
you remember to open, which is to say a thing you don't.

Two consequences:

- The scroll container is still `DocPane`'s, so read mode measures the element it
  always has. The sections are `display: none` while reading, so the lead-in sees
  the same geometry it did before they existed — and if they were ever visible the
  lead would compute to `0`, which is correct rather than broken: the sections have
  already pushed the first step past the trigger.
- **The cross-file jump moved onto the diagram.** Clicking an outside function
  whose module is also under review switches file and focuses it. It used to live
  in a separate reaches list; the picture can carry it, and the same fact in two
  places is one place too many.

### The files modal is the shape of the change

`⌘⇧T` was a list: one row per file, a dot, the state in words. It worked, and it
said nothing about the *change* — ten filenames tell you what you have open, never
what you are looking at. So the picker became a **portrait**, drawn from
`mockup/stack.html`: one layer per file, **thickness ∝ size**, the grain inside it
the functions it is made of, pale until your prose has named it. The same click
answers both questions at once — which file, and which part of this is big.

**One claim, so everything else stayed out.** No threads to the modules a file
reaches (a portrait is not a dependency list), no reading path, no line-count
captions, no per-setting marks. The things that survived are the ones the list was
already saying: filled or dashed is written-about, amber is stale or gone, a
heavier border is the file you are in, `×` on hover removes, and the origin has no
`×` because it is what the reading is anchored to.

**The name column is measured, not fixed.** 138px fitted `AllTargets` and clipped
`ImpactPipelineTelemetryReporter`, so `labelWidth` sizes it from the longest name in
the gloom, clamped between 110 and 300px, and the plates start after it. The modal
itself went to `min(860px, 94vw)`: a picture with a name column and a readable grain
needs the room, and a list never did.

**The hover path comes from the layer, not the cursor.** `cursor` indexes the
*filtered* hits, for the keyboard; a layer knows its index into `files`. Using one
for the other showed the wrong path — or none at all — the moment a filter was
typed. And the path is trimmed from the left in JS (`tailPath`) rather than by CSS,
because a right-to-left ellipsis needs `direction: rtl`, which reorders the segments
of anything with punctuation in it.

**Geometry lives in `shape.ts`, and is pure.** Both bugs found while drawing it
were sums rather than taste, and both are now probed headlessly rather than clicked
at:

- **Marks ran past the plate that owned them.** Widths were shared as
  `(len/total) × width` and the gaps were *added afterwards*, so a module with
  seven private helpers drew its last mark outside its own layer. Padding and gaps
  come off the width first.
- **A floor ate the proportion.** Sharing a fixed height between twelve files put
  every band within 4px of the minimum: still a usable list, no longer saying
  anything about size. The proportional pool **grows with the set** — 18px floor
  plus 40px a file — which holds the tallest at ~2.3× the shortest at twelve files
  against a √ ratio of 3.6×, and the body scrolls for the rest. The floor is there
  because this is a *picker*: no layer may be too small to click.

**√lines, not lines.** Linear draws a 26-line config at a fifth of a 342-line
module and the layer effectively disappears.

**Filtering dims rather than removes.** Proportion is the claim the picture makes,
and a stack that re-proportions itself as you type would lie about it. `↑↓` still
walk only the matches.

**The footer says one thing at a time**: the path you are pointing at (truncated
from the left, because the end identifies it), or why nothing can be added from
this branch, or what removing would cost. The branch refusal here is **one line and
names no branches** — as badges, a real name like `feature/impact-retry-fix` wrapped
the footer into three lines of chrome, and the names are already in the header chip
and the notice. In a footer the only thing worth saying is that the gloom cannot
grow from where you are. The path is there because it is the one
fact a module name cannot carry, and a footer can hold a long one where a row
cannot.

### The files modal replaced the tab strip

`⌘⇧T`, or the file button in the app header. `FilesModal.svelte` — the Library
idiom scoped to one reading: filter, `↑↓↵`, grouped by directory with sticky
headers, `×` or `⌫` to remove with an inline confirm, and an **Add a file** row.

The strip was cut on a measurement, not a preference: **ten filenames need about
1200px of tabs and the left pane has about 750**, so three of ten were off-screen
at exactly the size a real review is. Its whole job was "which files, and which am
I in", and it stopped doing that job precisely when it mattered.

What made it affordable to lose is that **navigation had already moved**: the
note's references, and the drawer's reaches list. Switching file is no longer the
main way you get around, so it can cost a keystroke.

**The button that opens it is the only coloured thing in that row** — the gloom's
own teal, because it opens the *shape of the change* and the shape is drawn in that
colour. Among four neutral controls it was the one you reach for most and the
hardest to find.

The header button is **136px, constant whatever the file count** — the filename,
the count, and one dot for the current file's state. Hollow still means "your prose
has not mentioned this", and the *earned* animation moved here with it: the moment
your prose first names something in the file you are looking at, the dot fills and
one ring leaves. `⌘T` adds a file, `⌘⇧T` manages the ones already in — the two
halves of the same idea, paired on purpose.

The state each row shows is said in **words** as well as a dot — *referenced in
your note*, *not mentioned yet*, *changed on disk*. A strip had five pixels for
that; a ten-row list has room to spell it out.

### Navigation lives beside the note, not in it

**A seeded module doc is a title, the moduledoc, and a blank page.** Not one
fence. `a_seeded_module_doc_carries_no_blocks` pins it.

Everything that used to be generated into it now lives **above** the note in the
same scroll (`ExploreSections.svelte`), showing the surface and the reach of
whatever file is on screen.

That is not tidying. `lgtm:surface` and `lgtm:deps` are what you **navigate** by,
and in the note they were pinned to a position in a narrative — so they only ever
served the file the reading started from. Open a second file and it had none,
which is exactly what made a multi-file reading uneven. Up there they follow the
tab, so **every file behaves the same and nothing has to be seeded**.

`lgtm:stats` left for a different reason and is *not* up there: size and history
are not consulted while navigating, they are context you want
**recorded**. `/stats` puts them where your prose wants them, and they travel with
the text into a PR comment.

**These sections are not blocks.** They read the live `Outline`; nothing in
`explore.ts` parses a fence. A block in the note is a snapshot of what the code
was when you read it; the sections are what the code is *now*. Both are useful and
they are not the same thing, so "the markdown IS the data" does not apply out
there.

**Which means there are two sorters again** — the exact situation that once gave
`get_user_by_email/1, get_user!/1, get_user/1` in the renderer against
`get_user/1, get_user!/1` in the seeder. `explore.ts` therefore sorts by
`(name, arity)` explicitly, comparing the parts rather than `localeCompare` on the
whole signature, and **both sides pin the same literal**:
`the_surface_order_is_pinned_for_the_drawer_to_match` in Rust (the name predates
the drawer's removal), and the same string
in the `explore.ts` probe. If either changes its mind, one of the two fails.

**The surface is a table, capped at six rows.** Two columns — the name, and the
line it is on — with tabular figures so the digits stack, which is the whole
reason it is a table and not a list. Past six rows the column scrolls, so a
40-function module costs exactly what a 3-function one does (166px either way,
measured) and the note sits at a constant offset. **No badges**: `default args`
and `3 clauses` are true, but at forty rows they turn a catalog you scan into a
wall of annotations — they moved to the row's `title`, and the arity in the name
already hints at the first.

**The diagram is inline, not behind a button.** An earlier version put it in an
overlay reached by `⌥R`, on the argument that it needs 2.6–6.4× the room a list
does. In one scrolling column that argument evaporates: room is free below the
fold. A map you have to remember to open is a map you do not use.

**Clicking an outside function isolates the boundary on it.** The module is
redrawn with **only the functions that call it** inside, and only the lines that
reach it. That is the question you actually have the moment your eye lands on
`Repo.insert/1` — *where is this used* — and two lit lines inside twelve grey rows
do not really answer it.

**Three ways out, like the focus pill's four**: a `× whole boundary` button in the
diagram's own top-right corner, clicking the isolated function again, and clicking
empty space inside the diagram. The button is on the *picture*, not under it — the
diagram is where you are looking, and an exit you have to go and find is what makes
a filtered view feel like a trap. The empty-space exit is scoped to `.reachwrap`, so
a click on the hint below it, or on the jump button, is not silently an exit.

The layout is **re-run**, not hidden with CSS: hiding the other rows leaves the
survivors at their old `y`, so the box keeps its full height and every line points
into a gap where something used to be. `renderDeps` therefore takes an `isolate`
argument and filters `locals` *before* laying out — and filters the edges to that
one target, because keeping a caller's other calls puts back most of what
isolating just removed.

**Isolating narrows both columns.** The outside stack was left whole at first, on
the grounds that it is what you pick the next question from — but a boundary drawn
to answer *"who calls `Repo.insert/1`"* with eleven other names still listed beside
it is answering the question while pointing somewhere else. Picking the next
question is what leaving the isolate is for, and there are three ways to do that.

A stale block that names a caller the outline no longer has says so rather than
drawing an empty box.

**Reach is the cross-file navigator**, and it is the best thing to come out of
this. A call landing in a module that is *also* under review is a **jump**: one
click switches file and focuses that function, which is what following a flow
across files actually means. Calls landing outside the reading say so — and that
marks the edge of what you are reviewing, which is worth seeing.

**The jump moved off the click and onto a named control**, under the diagram,
reading `open billing.ex ↗`. "Who calls this" is the question you ask *first*, so
the click answers that one; and a button that names the file it will open is more
discoverable than an invisible click target on a dot ever was.

**Hidden in read mode.** `DocPane` renders the sections as a snippet and reports
the mode with `onreading` rather than sharing it, because the pane owns it and two owners would eventually disagree with
the button. Read mode is being walked *through* something; a reference panel above
it is noise.

**A named setup's keys are unknown, and that is not the same as none.** A test
starts from module `setup_all` + module `setup` + its describe's `setup`, and a
*named* callback (`setup :put_user`) is defined elsewhere in the file — so its keys
cannot be read from the describe. `testsOf` therefore returns the keys it **can**
see plus a separate `unknown` flag, and the section shows `:user +?`: here is what
I know, and there is more. An earlier version collapsed the whole list to null the
moment anything was unreadable, which threw away the keys it *had* found — and it
got the condition backwards as well, treating a named callback as readable. The
probe caught both.

The blocks themselves are untouched — all five still render, `lgtm:functions`
still renders, and `/` inserts any of them. What changed is that **nothing puts one
in your note but you**.

### What a seeded module doc actually is

````markdown
# MyApp.Accounts

> Reads and writes for the `users` table.

## Explain

_The code is on the left — write what you make of it here. Press `/` while
editing to reference any function in this reading; once your prose names one,
`▷ Read` will walk the code in the order you wrote it._
````

That is the whole thing. One heading, and an invitation under it.
`a_seeded_module_doc_carries_no_blocks` pins that there is not a single fence.

### The blocks, when you ask for one

`/stats`, `/surface`, `/deps`, `/treemap` — for the file you are looking at, or
`/surface impact_stage.ex` for any other file in the reading. Generated by
`block_for` in `commands/docs.rs`, which is in Rust even though the frontend holds
the outline: **the block's order has to come from one place**, and surface was once
generated in both `seed.rs` and the renderer and they disagreed.

`stats` is the only one needing more than the outline — the line counts come from
the source and the history from git, so it re-reads both rather than trusting
numbers the frontend happens to be holding.

What each one writes:

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

**The space is the interesting part.** The menu closes on whitespace, and has to:
a bare `/` followed by a space is a slash in ordinary prose and must stay one. So
the rule became *a space is an argument separator only once a command name
matches* — which keeps the protection and buys the argument. `stillOpen` in
`slash.ts` owns it, shared between `DocPane` deciding whether to stay open and
`RefMenu` deciding what to show, or the two would disagree about what is still a
live query. A command needs **two** characters before it matches, so `/s` still
filters functions on the way to `/smembers`.

Naming a file **replaces** the function list rather than appending to it — you are
choosing a target, not a reference, and showing both would be two menus in one.
Only files with a module are offered, since `block_for` would refuse a config
script and offering something that fails is worse than not offering it. The
directory is shown beside a filename **only when two files in the reading share
it** — three pipelines each with a `config.ex` is the normal case, and this is the
one place lgtm lets you choose between them by hand.

Insert one where you want it, delete it when you don't. That is the whole point of
not seeding them: a block you asked for beats three you scroll past.

**Two blocks are renderable and deliberately not seeded.** Both were
seeded once and both were cut for the same reason — a generated section above the
part you write in has to earn that space every single time you open a file:

- `lgtm:treemap`. Function sizes answer a question you ask *occasionally*: "is
  anything in here disproportionate?"
- `lgtm:functions`. It was a **second listing of the names `lgtm:surface` already
  gives you**, and its one unique offering — a prose slot per function — turned
  out to be the wrong shape. An explanation follows the path through a module.
  An alphabetical index does not.

Both generators (`treemap_block`, `functions_block`) stay, stay `pub`, and stay
tested against their own output rather than against the seeder's, because a block
whose only producer is a hand-typed guess drifts away from its renderer. Write
either fence yourself when you want it.

**A block is never regenerated, and neither is the source under it.** A gloom holds
the version it was read at, so a block written on Tuesday still describes the code
the pane is showing. What tells you the world moved on is the file's own "changed
on disk" row — and, for the code you *wrote about*, the dangling reference.

**Private helpers are not seeded anywhere.** They are in `lgtm:surface` (it is a
directory, so it lists everything) and one `/` away when your explanation needs to
reach one. Notes used to carry a **generated list of every private helper** as
inline references, specifically so `▷ Read` did something on a file you had not
written a word about. `/` does that job better: you reference the functions your
explanation actually reaches, in the order your prose takes them. **A list of
things you did not choose was never really a reading.**

Two consequences worth stating rather than leaving to be discovered:

- **`▷ Read` stays hidden until you write the first reference** (the button is
  gated on `steps.length`). The invitation text names both `/` and `▷ Read` for
  exactly this reason — otherwise it reads as a missing feature rather than a
  hidden one.
- **A function's `@doc` is no longer copied into the doc.** It used to become the
  starting prose of its row in `lgtm:functions`. It is still right there in the
  code pane, styled as prose — and not duplicating what the source already says
  is the better default.

**Every seed ends with `## Explain` and an invitation**, never a bare heading — an
empty section reads as something missing. The config, test and plain seeds get a
shorter invitation that does not promise `/` or `▷ Read`, since neither does
anything without functions to reference.

### The treemap

**Not seeded** — see above; you write the fence when you want it. Function sizes
as area, the one view the table can't give you: *is anything in here
disproportionate?* Squarified `d3-hierarchy` layout, drawn to an SVG string
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

### A gloom is pinned to the versions it was read at

**The pane shows the snapshot, not the file.** `read_one` returns
`doc_files.source`, and the outline is parsed from *that* — so a line number in
your prose means next month what it meant today, and the surface, the reach and
every reference stay consistent with what is on screen.

Disk is read for exactly one thing: whether it has moved on. That is worth a row
above the code — *"the code I am reading is no longer the code that runs"* is
precisely what a reader needs to know — but it is **a statement, not an offer**.

**There is no reconciliation, no re-snapshot and no re-parse.** All three were
built and all three are gone, along with `reconcile.rs`, `resnapshot_doc_file`,
`reconcile_doc` and the `reparse` command. The argument that removed them:

- **lgtm reads committed code, not code in progress.** A gloom is a reading of a
  particular version. Merging a changed file into it produces a note half of which
  describes lines that no longer exist, and no merge algorithm can tell you which
  half.
- **The alternative is one gesture.** ← Home, open the file, and you have a new
  gloom of the new version — with the old reading still intact beside it in the
  library, which is what you want when the question is "what changed".
- **Reconciliation was already vestigial.** It only ever touched `lgtm:functions`,
  which stopped being seeded, so on nearly every doc it did nothing at all.

What survives is the *signal*, in two places: a file that has changed on disk says
so above the code and on its dot in the files modal, and a **dangling reference**
strikes through in the prose where you wrote it. Both put the fact where you are
looking without pretending the tool can fix it.

`a_gloom_keeps_the_source_it_was_read_at` pins the premise.

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

- **The reach block traces outward**, wherever it is drawn. `reachHosts()` walks
  the whole scroll container rather than the note, because the sections above the
  note render the same `.lgtm-deps` markup and sit *outside* `.doc` — so the trace
  and the arrival ring were rendered every time and could never run, which is the
  `overflow-x` bug's exact shape in a different place. The pointer wiring moved up
  to `.panebody` with it, and every host is lit, not the first: a hand-written
  `/deps` block and a section's diagram can both be on screen.

  A static boundary shows you *that* `create_user/1` reaches `Repo.insert/1`,
  never that the call goes **out**. A travelling `stroke-dashoffset` does. The far dot then takes one ring on arrival,
  fired on a 260ms timer so it reads as the *consequence* of the dash getting
  there — both at once is the static picture again with extra noise. The ring is
  its own `<circle>` scaled by `transform` on a `fill-box` origin, because `r` is
  not reliably animatable and this way the renderer supplies no geometry for it.
- **Opening a file points at the blank page.** The pane rises, then 260ms later a
  ring expands from the **Explain invitation** — not the pane. "A file arrived" is
  decoration; you know, you just opened it. The invitation is found
  *structurally*: the last paragraph of the doc, containing nothing but emphasis.
  That is what `seed.rs` writes, and **the moment you type a word it stops
  matching** — so nothing has to remember whether a doc is "fresh", and the nudge
  only ever appears on a doc you have not written in.
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
- **The directory arrives in order.** `--i` per row — in `surface.ts` for the
  block and on the surface table's `<tr>` in `ExploreSections`, which gates it on
  the *path* so it plays once per file rather than on every render — staggered
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
- **The dot earns its colour.** The file button diffs `referenced` against the
  previous set, so this fires on the *transition* rather than the state — and the
  first paint of an already-written note is seeded silently, because that is not
  an achievement. A state change you caused is the one place a small reward is
  honest.
- **The trigger band flashes per step.** It showed you *where* the hand-over
  happens but never *that* it happened, which is the difference between "the code
  changed on its own" and "I did that by scrolling". Only on an actual step
  change; every scroll frame would be noise, and noise is what gets animation
  switched off.

**Arrival animations are gated on `opened`, not on render.** `html` is derived
from `markdown`, so it re-renders on every keystroke — an unguarded stagger
re-cascades the surface block while you type. The shell bumps `opened` only when
the *doc id* changes (adding a file replaces `doc` too, and re-cascading because
you opened one more file is exactly that restlessness), and `DocPane` puts
`.arriving` on the container for one beat.

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

**Nothing collapses with a transition.** The explore drawer — since replaced by
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

Treemap tiles and table rows are the same gesture — both carry `data-sig`, and
`DocPane.select()` handles either.

**Dimming yields to file-wide views.** `.focusing` (the dim-to-32%) is right for
"show me this one function" and wrong for any question about the whole file, so
it is suppressed while blame or a search is active — see `dimming` in
`CodePane.svelte`. The selection keeps its own highlight and the pill still says
where you are; it just stops hiding the author colours or the matches you asked
for. Without this, turning on blame with a function selected leaves 68% of the
authors invisible.

### Home is about glooms, not about the app

`mockup/home.html` is the contract. Three bands, in the order you need them:

1. **An ink masthead** — `--gloom-bg`, the wordmark in `--display`, and one sentence
   defining the word for someone who has never seen it. It plays the same accent
   sweep the gloom band does: same surface, same greeting.
2. **The glooms you were in** — as *cards*, not filenames in a list: the name you
   gave it, and badges for the files it covers. Nothing else. A first cut carried a
   file count, an age and a branch as well, and all three answered questions you are
   not asking while scanning this screen — *which one was I in?* The name answers
   that; the badges say what it reached; the count is already in the badges (`+2`);
   and the age only ordered the list, which the order of the list does. A card still
   called after its module is greyed and italic, the same invitation the band gives.
3. **One way to start**, with the current project beside it and the rarer routes as
   quiet text. The old screen offered four buttons of equal weight, which is four
   decisions before you have started.

Then three sentences on what a gloom is. The features are discoverable; the idea is
not, and nothing else in the app has room to say it.

**The masthead has weather, and it is the only ambient motion outside the two
already budgeted.** `Sky.svelte` draws one of three on a canvas — **bloom** (rings
opening from a point, which is where the *name* comes from: the swell when water
first hits coffee grounds), **cosmos** (parallax drift with a faint twinkle), or
**aurora** (three sine bands under soft gradients) — and **which one is rolled when
home appears**. A surface you land on a dozen times a day should not be identical a
dozen times a day, and since none of the three says anything, none can say the wrong
thing by being picked.

Home is the one place this is affordable: you are not reading code on it and nothing
here has to be tracked. Everywhere else the rule stands.

A fourth candidate, **constellation**, is kept in the mockup and was cut: it draws
lines between dots, and the reach diagram is a real graph — teaching the eye that
teal lines between dots are ornamental is the one thing worth not doing.

Three things it does that a decorative canvas usually forgets: it **stops when the
window is hidden**, it **disconnects its observer and cancels its frame on unmount**,
and under `prefers-reduced-motion` it composes **one** frame and holds it — the
picture survives, the movement does not. That last one is the same rule the CSS
follows; `scripts/motion-audit.py` cannot see canvas, so it is written here instead.

**`--gloom-deep` came out of this.** The bright teal is for ink surfaces and
measures **2.2:1** on a card — the home screen needed the same hue at a weight that
survives on paper (5.2:1 light; in dark it *is* the bright one). Measured on the
rest: hero title 12.5:1, hero prose 6.2:1, card name 17.2:1, meta 6.1:1, the start
button 10.9:1.

### The app header is a small theme

`.apphead` redefines the neutral tokens for its own subtree — `--fg`, `--fg-dim`,
`--line`, `--bg-raised`, `--accent` — over `--head-bg`, a step lighter than the
gloom band under it. Every control in the row adapts without knowing it is on a
dark strip, which is read mode's trick at the scale of one row, and the two bars
stack as a single masthead: actions above, the name of the journey below. Measured
on the light head: ink 10.9:1, dim 7.5:1, the teal accent 6.5:1.

**The header shows the GLOOM's branch, not the working tree's.** A gloom is pinned
to the versions it was read at, and the branch those versions came from is part of
that reading — it does not change when you check something else out. Showing the
live branch made the header report the state of your tree, which is a different
tool's job and was a fact about *now* sitting in a window entirely about *then*.
**Nothing announces staleness over the code.** A bar above the pane said it for a
while, in two wordings, and both were the same mistake: a gloom holds the version it
was read at, so *"this differs from disk"* is a fact about the world **outside** the
reading — true, and not worth a row across the top of the thing you are reading. It
still shows where it costs nothing: the dot on the file button, and the words beside
each file in `⌘⇧T` (*changed on disk*).

**The branch is context, not decoration.** It was a grey pill among grey pills, and
it names the versions everything under it is showing. It carries the accent now, and
is keyed on its own value so it plays its arrival when it changes — which, since it
is the gloom's branch, happens when you open a different gloom.

**`← Home` is a way out and now looks like one** — filled, bolder, first in the
row. **Re-parse is gone**: it re-read a file you had not edited, since lgtm never
writes, and the two cases that matter have their own controls (*Code changed —
reconcile* in the note, and the amber staleness row above a joined file). Both of
those have since gone the same way — see *A gloom is pinned to the versions it was
read at*.
**Library is gone from this row too** — the way to another gloom is ← Home, and
`⌘K` still opens the library from anywhere. What is left is about *this* gloom:
where you are, which files it covers, and two ways to add one.

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

### The note's header is controls, nothing else

It used to open with `impact_stage.ex.md` — a filename for a file that does not
exist, since the note is a row in a database and the `.md` was an invention. The
gloom's name is in the band above it and the source file's name is on the pane
beside it, so the third name was the only thing that row was adding.

**Preview/Edit is one button, not a segmented control.** You are always in exactly
one of the two modes, so half of that control's width was spent telling you where
you already are. The label is what the click *does*: `Edit` while previewing,
`Preview` while editing.

**`▷ Read` is on the left**, where the row starts: it is the one control here that
changes what the whole window does, and among four right-aligned buttons it read as
the third of them.

**The font stepper is last, as in the code pane.** The same control in the same
place on both sides of the window; it had drifted into the middle of this one.

### The two mode toggles are keys as well as buttons

`⌘R` for read mode and `⌘E` for edit/preview. Both live in `DocPane` as exported
functions the shell calls, rather than a second keydown listener inside the pane —
the shell already owns the window's handler for `⌘T`, `⌘K` and the rest, and two
listeners would eventually disagree with the buttons about which mode is on.

`⌘R` is `preventDefault`ed before anything else can see it: in a dev build it
would reload the webview.

**Entering read mode is gated; leaving it never is.** The conditions — a module in
the reading, not editing, at least one step — decide whether there is anything to
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
to skip the line — while `↑`/`↓` for the review cursor and `⌘T` to add a file were
only mentioned inside `?`. It now reads
`↑↓ lines · [ ] fns · / find · ⌘T add a file · ⌘R read · ⌘E edit · ? help`.

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

### The app is Glooming; the button is LGTM

The window title and the brand in the title strip say **Glooming**, set in
`--display` — the same wordmark home carries, so the two agree. `lgtm` survives
where it always meant the most: as the *phrase*, on the button that ends a reading.

`▷ Read` is now **`▷ LGTM`**, and the rename is not decoration. That button starts
the pass where you scroll your own note and watch the code follow it — which is
exactly the pass at the end of which you either understand the change or find the
sentence where you stopped understanding it. Naming it for the verdict names the
thing it is for. The repo, the crate and the bundle id are untouched.

### Reading: the doc drives the code

`▷ Read` in the doc pane header, shown when there is a module anywhere in the
reading — a config or a test suite on its own is a directory, not a narrative, so
there is nothing to walk. A reading of several files walks all of them, swapping
the code pane where the prose crosses a boundary.

The geometry scrollytelling wants was already here: the doc is the text, the
code pane is the sticky graphic, and `focus` is the graphic's state. So this is
one wire — doc scroll position → focus — not a rewrite.

**There is no new syntax.** Inline code naming a function in the reading becomes a
reference (`refs.ts`) — with or without an arity, bare or module-qualified — and
`L30-34` points at plain lines. That keeps the
markdown portable: paste a reading into a PR comment and it still reads
correctly, which inventing `{{create_user/1}}` would have destroyed. Anything
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
through and unclickable. Same principle as the functions block keeping
struck-through prose: never quietly lose the fact that the code moved.

`mockup/reading.html` and `mockup/authoring.html` are the contracts. An earlier
version put a "reading path" arc diagram in the divider; it was cut because it
needed explaining, which meant it wasn't working. The same data as a plain
two-axis chart (step across, file position down) is legible, and is parked as a
possible `lgtm:path` block rather than built.

### Projects are a path, nothing more

`⌘T` searches the open folder by name; the folder is picked explicitly with
**Open a folder…** and remembered in a `projects` table, most recent first, so
the last one is reopened next launch.

Deliberately thin. There is **no index on disk, no scan at startup and no notion
of membership** — the walk runs when the palette opens, takes milliseconds even
on a large app, and caching it would be inventing a staleness problem to solve
in a tree that changes while you work.

Two things it does *not* do, both on purpose:

- **No root inference.** An earlier plan walked up for `mix.exs`. Explicit is
  predictable, and predictable beats clever for something used fifty times a day.
- **No git.** Listing changed files was discussed and cut: the tool is about
  opening files to read, and knowing what changed is done elsewhere.

`ignore::WalkBuilder` honours `.gitignore`, with `_build`, `deps`,
`node_modules`, `.git`, `.elixir_ls` and `cover` skipped explicitly on top —
a fresh checkout may not have them ignored yet. Only `.ex`/`.exs` are listed:
lgtm can open anything, but the picker offers what it actually reads.

**Matching ranks filename hits above directory hits**, otherwise everything
under a `processor/` directory outranks `processor.ex` itself. Substring first,
then subsequence over the whole relative path, so `myacc` finds
`my_app/accounts.ex`.

`⌘O` opens the same palette as `⌘T`. It is the muscle memory for "open", and what
you almost always want to open is a file in the project you are already in — so
the system picker moved to `⌘⇧O`, for the rarer case of a file outside it. Two keys
for one action is deliberate: `⌘T` is advertised in the status bar as "add a file",
`⌘O` is what fingers reach for, and neither being wrong is worth more than the
saved binding. Comparing `e.key` **lowercased** matters — `⇧O` arrives as `"O"`,
so the old `=== "o"` meant the shifted form silently did nothing.

`⌘T` also absorbed `⌘L`: if what you have typed looks like a path rather than a
search, `↵` opens it directly. A path from a stack trace is often outside the
project, and two near-identical dialogs for "open a file by naming it" was one
too many.

### The library

`⌘K`. Built for a few hundred docs, not a few: search over title/filename/path/
branch, three orderings (Recent / Name / Folder), sticky folder-group headers,
and `↑`/`↓`/`↵` keyboard navigation that crosses group boundaries in visual
order.

**Deleting reports back.** `Library` refreshes its own list, but the welcome
screen's recents is a *cached query over the same table* and nothing it reads
changes when a row goes away — so deleted readings kept appearing under "pick up
where you left off". The `ondelete` callback exists for that, and it also closes
the reading if the deleted one is the one on screen: leaving it open means the
next autosave writes to a row that no longer exists, and the failure surfaces as
a "not found" banner seconds later with nothing to connect it to. Pinned by
`saving_to_a_deleted_doc_is_an_error_not_a_silent_no_op` — an UPDATE matching no
rows must not report success.

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
in `seed.rs` only, now that nothing re-writes a block after it is written. `lgtm:surface` is sorted in `seed.rs` *only*, and the renderer
preserves the text's order — because two sorters did disagree there (Rust orders
by `(name, arity)`, JS `localeCompare` reorders punctuation). If you add a
sorted block, pick one of these two shapes deliberately.

**Tests must scope assertions to one block.** Several blocks list every function
name, so `md.lines().find(|l| l.contains("get_user!/1"))` finds the surface row,
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

**`overflow-x: auto` clips vertically too.** The file strip scrolls sideways when
a reading has many tabs, and the remove confirmation was an absolutely-positioned
popover *inside* it. Per the CSS Overflow spec, `visible` on one axis computes to
`auto` when the other is not `visible` — so the strip clips at its own 32px and a
popover starting 37px down never appears. It rendered correctly every time and was
never once visible, which is the worst kind of bug: nothing to see, nothing in the
console. The confirmation is now a row *below* the strip, outside the scroll
container. Anything positioned relative to a tab has the same problem.

**Don't nest a button inside a button.** The tab's `×` was a `<span role="button">`
inside the `<button class="tab">`, which is invalid HTML and needs
`stopPropagation` to behave. It is now a sibling button positioned over the tab's
right edge — valid, and the propagation question disappears.

**Adding a file to a reading must not re-snapshot it.** Opening the same file
twice mid-review is completely normal, so `doc_files::add` is idempotent and
returns the existing row. Re-snapshotting would silently discard the staleness
you were about to be told about. `adding_twice_keeps_the_first_snapshot` pins it.

**A mutation returns the whole reading.** `add_doc_file`, `remove_doc_file` and
`resnapshot_doc_file` all return a `Reading`, so the UI replaces state rather
than patching it — the same one-payload habit as `open_file`. The one thing the
frontend must *not* adopt is the markdown: an autosave may be in flight, and
taking the server's copy would lose the sentence you are mid-way through. Every
call site saves `markdown` and puts it back.

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

**`db` is `pub`** solely so `tests/pipeline.rs` can construct `FileHistory`.

## Testing

~90 Rust tests cover the parser (modules, configs, test suites), the seeder, the
git parsing and the DB layer. `tests/pipeline.rs` runs the whole
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
- **Check layout maths with a throwaway `node -e`.** The treemap's cell sizes,
  the reach block's crossing count, the read mode lead-in and tail — all of them
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
- Commands in `commands/` stay thin; logic lives in `db/`, `parse/`, `seed.rs`,
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
