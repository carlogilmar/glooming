// What the explore drawer shows for the file in front of you.
//
// The drawer is **not** a block. It reads the live `Outline` rather than rendering
// text out of the note — which is the whole point of moving surface and reach out
// there. A block in the note is a snapshot of what the code was when you read it;
// the drawer is what the code is now, for whichever tab is open. Both are useful
// and they are not the same thing.
//
// So "the markdown IS the data" does not apply here. Nothing below parses a fence.

import type {
  AssertKind,
  Assertion,
  ConfigGroup,
  Dep,
  Describe,
  FnInfo,
  ModuleInfo,
  Outline,
  Range,
  ReadingFile,
  Setting,
  SetupInfo,
  TestInfo,
} from "$lib/ipc";
import { displaySig } from "$lib/select";
import { shortModule } from "$lib/fileset";

export interface SurfaceRow {
  sig: string;
  /** `create_user` — what you scan for. */
  name: string;
  /** `/1`, `/1..2` — dimmed beside the name, since the name is what you read. */
  arity: string;
  line: number;
  /**
   * `default args`, `3 clauses`.
   *
   * No longer rendered as badges: at forty rows they turned a catalog you scan
   * into a wall of annotations. Kept because they become the row's `title`, where
   * they cost nothing until you ask.
   */
  flags: string[];
}

/**
 * Sort by `(name, arity)`, exactly as `seed.rs` does.
 *
 * This is the one place the drawer could drift from the seeder, and there is
 * history: surface was once sorted in Rust *and* in the renderer, and the two
 * disagreed — Rust ordered by `(name, arity)` giving `get_user/1, get_user!/1`,
 * while JS `localeCompare` on the whole signature reordered punctuation and gave
 * `get_user_by_email/1, get_user!/1, get_user/1`. Comparing the parts, in the
 * same order, with the arity numeric, is what keeps the drawer and a `/surface`
 * block agreeing about the order of the same functions.
 *
 * `sorts_the_way_rust_does` pins it against the pipeline fixture.
 */
export function byNameThenArity(a: FnInfo, b: FnInfo): number {
  if (a.name !== b.name) return a.name < b.name ? -1 : 1;
  return a.arity - b.arity;
}

export function surfaceOf(module: ModuleInfo | null, kind: "public" | "private"): SurfaceRow[] {
  return (module?.functions ?? [])
    .filter((f) => f.visibility === kind)
    .slice()
    .sort(byNameThenArity)
    .map((f) => {
      const flags: string[] = [];
      if (f.minArity < f.arity) flags.push("default args");
      if (f.clauses > 1) flags.push(`${f.clauses} clauses`);
      const sig = displaySig(f);
      const slash = sig.indexOf("/");
      return {
        sig,
        name: slash === -1 ? sig : sig.slice(0, slash),
        arity: slash === -1 ? "" : sig.slice(slash),
        line: f.line,
        flags,
      };
    });
}

export interface ReachLine {
  /** `Read.zrange/3` — the module's own name, not the path to it. */
  to: string;
  /** Local functions that make the call. */
  from: string[];
  /**
   * The file in this reading that the call lands in, when there is one.
   *
   * This is what makes the list a navigator rather than a readout: a reached
   * module that is *also* under review means one click follows the call across
   * the boundary. Null marks the edge of what you are reviewing, which is worth
   * seeing too.
   */
  path: string | null;
  filename: string | null;
  /** Line to focus in that file. Null when the function is not found there. */
  line: number | null;
}

/** Every call that leaves this module, one line each, in the block's own order. */
export function reachOf(deps: Dep[], files: ReadingFile[]): ReachLine[] {
  const out: ReachLine[] = [];

  for (const dep of deps) {
    // `dep.module` is the full alias as written; the reading knows its files by
    // module name, so match on the last segment the way references do.
    const short = shortModule(dep.module);
    const target =
      files.find((f) => f.outline?.modules?.some((m) => shortModule(m.name) === short)) ?? null;

    for (const fn of dep.functions) {
      const hit = target?.outline?.modules
        ?.flatMap((m) => m.functions)
        .find((f) => displaySig(f) === fn.name || `${f.name}/${f.arity}` === fn.name);

      out.push({
        to: `${short}.${fn.name}`,
        from: fn.callers,
        path: hit ? (target?.path ?? null) : null,
        filename: hit ? (target?.filename ?? null) : null,
        line: hit?.line ?? null,
      });
    }
  }

  return out;
}

/** The one-line summary the collapsed drawer carries. */
export function summarise(module: ModuleInfo | null, reach: ReachLine[]): string {
  if (!module) return "nothing structural in this file";
  const pub = module.functions.filter((f) => f.visibility === "public").length;
  const priv = module.functions.length - pub;
  const mods = new Set(reach.map((r) => r.to.split(".")[0])).size;
  return (
    `${pub} public · ${priv} private · ` +
    (mods ? `reaches ${mods} module${mods === 1 ? "" : "s"}` : "reaches nothing")
  );
}

// ---- the other kinds -------------------------------------------------------
//
// The drawer is per FileKind, which is that table finally doing one job instead
// of two: it used to decide which blocks a doc got *seeded* with, and now it
// decides what you navigate a file by. A config script has settings, a suite has
// describes, and neither has functions — so neither ever had a surface worth
// showing, which is why the note carried their blocks for so long.

export interface SettingRow {
  key: string;
  line: number;
  /** How the value arrives: this is the whole finding of a config block. */
  kind: "env" | "env!" | "secret" | "literal";
  /** The env var, or the literal — never the value behind a `secret`. */
  value: string;
}

export interface SettingGroup {
  app: string;
  target: string | null;
  line: number;
  rows: SettingRow[];
}

/**
 * A config script's groups, flattened just enough to render as rows.
 *
 * `env` versus literal is the point: `System.get_env` is a value you can change
 * at deploy time, a hardcoded string is one baked into the release, and `env!`
 * crashes on boot when unset. A literal whose *key* looks like a credential comes
 * back as `secret` **without its value** — that it is hardcoded is the finding,
 * and a note gets pasted into PR comments.
 */
export function settingsOf(groups: ConfigGroup[]): SettingGroup[] {
  const value = (s: Setting): SettingRow => {
    switch (s.source.kind) {
      case "env":
        return {
          key: s.key,
          line: s.line,
          kind: s.source.required ? "env!" : "env",
          value: s.source.var,
        };
      case "secret":
        return { key: s.key, line: s.line, kind: "secret", value: "" };
      default:
        return { key: s.key, line: s.line, kind: "literal", value: s.source.value };
    }
  };

  return groups.map((g) => ({
    app: g.app,
    target: g.target,
    line: g.line,
    rows: g.settings.map(value),
  }));
}

export interface TestRow {
  name: string;
  line: number;
  /** So clicking the row selects the whole test, not its opening line. */
  endLine: number;
  /** Every assertion with its line and kind — the spine, and the code gutter. */
  assertions: Assertion[];
  /** The `@tag`s above it, carried like a function's `@spec`. */
  tagRange: Range | null;
  tags: string[];
  skipped: boolean;
}

/**
 * What every test in the file starts from, stated **once**.
 *
 * The old view repeated the module's keys on every describe row, right under a
 * band that had already listed them — the same fact in two places, and the
 * repetition grew with the number of describes. So module scope is the stack,
 * and a describe shows only its own delta.
 */
export interface SuiteScope {
  /** The module-scope blocks themselves, so each one is clickable. */
  setups: SetupInfo[];
  keys: string[];
  named: string[];
  /** Something in scope contributes keys that cannot be read from here. */
  unknown: boolean;
}

export interface DescribeGroup {
  name: string;
  line: number;
  /** The container's span. A describe is *jumped to*, never selected — but the
   *  end is what tells you how much of the file it covers. */
  endLine: number;
  /** This describe's OWN setups. Module scope is in `SuiteScope`, not here. */
  setups: SetupInfo[];
  /**
   * What this describe adds on top of module scope.
   *
   * A **named** callback (`setup :put_user`) is defined elsewhere in the file, so
   * its keys are unknowable — and unknown is not the same as "provides nothing".
   * Kept separate from the keys rather than collapsing the whole list to null,
   * so `:user +?` can say *both* things: here is what I know, and there is more.
   */
  adds: { keys: string[]; named: string[]; unknown: boolean };
  tests: TestRow[];
}

/** The keys and named callbacks a run of setup blocks contributes. */
function scopeOf(from: SetupInfo[]): { keys: string[]; named: string[]; unknown: boolean } {
  const keys: string[] = [];
  const named: string[] = [];
  let unknown = false;
  for (const s of from) {
    if (s.named) {
      named.push(s.named);
      unknown = true;
    } else if (s.provides === null) {
      unknown = true;
    } else {
      for (const k of s.provides) if (!keys.includes(k)) keys.push(k);
    }
  }
  return { keys, named, unknown };
}

export function suiteScope(tests: TestInfo | null): SuiteScope {
  const setups = tests?.setups ?? [];
  return { setups, ...scopeOf(setups) };
}

/**
 * A suite's describes, each with what it adds to the context its tests get.
 *
 * A test starts from module `setup_all` + module `setup` + its describe's
 * `setup`, which can be a hundred lines apart. `suiteScope` states the first two
 * once; this states the third.
 */
export function testsOf(tests: TestInfo | null): DescribeGroup[] {
  if (!tests) return [];

  return tests.describes.map((d: Describe) => ({
    name: d.name ?? "(no describe)",
    line: d.line,
    endLine: d.endLine,
    setups: d.setups,
    adds: scopeOf(d.setups),
    tests: d.tests.map((t) => ({
      name: t.name,
      line: t.line,
      endLine: t.endLine,
      assertions: t.assertions ?? [],
      tagRange: t.tagRange ?? null,
      tags: t.tags,
      skipped: t.skipped,
    })),
  }));
}

/**
 * Every assertion in the file, one per line, for the code pane's gutter.
 *
 * A line can carry two calls (`assert foo(bar) == refute_baz()` is contrived,
 * but `assert_raise E, fn -> assert x end` is not), and a row has one gutter —
 * so the more informative kind wins. `message` over `error` over `assert`,
 * because that is the order of how much they narrow down what the test does.
 */
export function assertionLines(outline: Outline | null | undefined): Map<number, AssertKind> {
  const rank: Record<AssertKind, number> = { assert: 0, error: 1, message: 2 };
  const out = new Map<number, AssertKind>();
  for (const d of outline?.tests?.describes ?? []) {
    for (const t of d.tests) {
      for (const a of t.assertions ?? []) {
        const had = out.get(a.line);
        if (!had || rank[a.kind] > rank[had]) out.set(a.line, a.kind);
      }
    }
  }
  return out;
}

/** The collapsed line, for whichever kind the file is. */
export function summariseFile(file: ReadingFile | null, reach: ReachLine[]): string {
  const kind = file?.outline?.kind;

  if (kind === "config") {
    const groups = file?.outline?.config?.groups ?? [];
    const all = groups.flatMap((g) => g.settings);
    const env = all.filter((s) => s.source.kind === "env").length;
    return (
      `${groups.length} group${groups.length === 1 ? "" : "s"} · ` +
      `${all.length} setting${all.length === 1 ? "" : "s"} · ${env} from env`
    );
  }

  if (kind === "test") {
    const d = file?.outline?.tests?.describes ?? [];
    const all = d.flatMap((x) => x.tests);
    // "How many assertions" was never a reviewable number. "Does this suite
    // check failure at all" is, so that is what the line says.
    const err = all.filter((t) =>
      (t.assertions ?? []).some((a) => a.kind === "error"),
    ).length;
    return (
      `${d.length} describe${d.length === 1 ? "" : "s"} · ` +
      `${all.length} test${all.length === 1 ? "" : "s"} · ` +
      (err ? `${err} checking failure` : "none checking failure")
    );
  }

  if (kind === "module") return summarise(moduleOfFile(file), reach);
  return "nothing structural recognised";
}

function moduleOfFile(file: ReadingFile | null): ModuleInfo | null {
  return file?.outline?.modules?.[0] ?? null;
}

/**
 * The outline written back out as a `lgtm:deps` block body.
 *
 * `renderDeps` takes block *text*, because in the note that is what it gets, and
 * the drawer should not need a round trip to Rust just to open a diagram. So the
 * live deps are serialised into the same grammar `seed.rs`'s `deps_block` writes.
 *
 * Two levels by indent, everything after the first colon is the value — the
 * format is deliberately this simple precisely because it is parsed in more than
 * one place. Padding matches the seeder so a diagram drawn from here and one
 * drawn from a pasted block are the same picture.
 */
export function seedDepsBlock(module: ModuleInfo): string {
  let out = `\`\`\`lgtm:deps module=${module.name}\n`;
  for (const dep of module.deps) {
    out += `  ${dep.module} : ${dep.kind}\n`;
    const width = Math.max(0, ...dep.functions.map((f) => f.name.length));
    for (const fn of dep.functions) {
      out += `    ${fn.name.padEnd(width)} : ${fn.callers.join(", ")}\n`;
    }
  }
  return out + "```\n";
}
