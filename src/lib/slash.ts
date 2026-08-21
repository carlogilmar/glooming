// The `/` menu's grammar.
//
// Two things are typed after a slash, and they need different rules:
//
//   /to_cents              a reference — one token, no spaces
//   /surface billing.ex    a command with an argument
//
// The menu closes on a space, deliberately: a bare `/` followed by a space is a
// slash in ordinary prose and must stay one. But `/surface billing.ex` needs that
// space, so the rule is **a space is an argument separator only once a command
// name matches** — which keeps the protection and buys the argument.
//
// Shared between `DocPane` (which decides when the menu stays open) and
// `RefMenu` (which decides what to show in it). One parser, or the two would
// disagree about what is still a live query.

import type { ReadingFile } from "$lib/ipc";

export type BlockKind = "stats" | "surface" | "deps" | "treemap";

export interface SlashCommand {
  kind: BlockKind;
  name: string;
  /** One line, shown beside the name in the menu. */
  what: string;
}

/**
 * The blocks you can put in the note, and nothing is put there for you.
 *
 * `stats` is first because it is the one with nowhere else to live: the explore
 * drawer carries surface and reach, but size and history are not something you
 * navigate by — they are context you want *recorded*, and a note is where a
 * record belongs. The other three are for when your explanation deliberately
 * wants a reader to see a file's shape at that point in the prose.
 */
export const COMMANDS: SlashCommand[] = [
  { kind: "stats", name: "stats", what: "size, authors, first and last touched" },
  { kind: "surface", name: "surface", what: "directory of every function" },
  { kind: "deps", name: "deps", what: "the boundary, and what pierces it" },
  { kind: "treemap", name: "treemap", what: "function sizes as area" },
];

/**
 * How much of a command name has to be typed before it counts as matched.
 *
 * Two, so a single letter still filters functions — which is what the menu is
 * usually open for — and so `/s` does not become a command the moment you reach
 * for `/smembers`.
 */
const MIN = 2;

export function matchCommand(token: string): SlashCommand | null {
  const t = token.toLowerCase();
  if (t.length < MIN) return null;
  return COMMANDS.find((c) => c.name.startsWith(t)) ?? null;
}

export interface Slash {
  /** The first token — a command name, or the start of a function name. */
  token: string;
  /** Everything after the first space, or null when no space has been typed. */
  arg: string | null;
  /** The command `token` matches, if any. */
  command: SlashCommand | null;
}

export function parseSlash(typed: string): Slash {
  const at = typed.indexOf(" ");
  const token = at === -1 ? typed : typed.slice(0, at);
  return {
    token,
    arg: at === -1 ? null : typed.slice(at + 1),
    command: matchCommand(token),
  };
}

/** Is this still a live query, or has the menu been typed out of? */
export function stillOpen(typed: string): boolean {
  // A newline or a backtick always ends it — a fence cannot span the trigger.
  if (/[\n\r`]/.test(typed)) return false;
  if (!/\s/.test(typed)) return true;
  // Past here a space has been typed, so only a matched command may continue.
  return !!parseSlash(typed).command;
}

export interface FileOption {
  path: string;
  filename: string;
  /** Directory, shown only to tell two files of the same name apart. */
  dir: string;
  module: string;
}

/**
 * The files a block can be generated for: the ones with a module in them.
 *
 * A config script or a test suite has none, and `block_for` would refuse — better
 * not to offer it than to offer it and fail.
 */
export function blockTargets(files: ReadingFile[]): FileOption[] {
  return files.flatMap((f) => {
    const module = f.outline?.modules?.[0]?.name;
    if (!module) return [];
    return [
      {
        path: f.path,
        filename: f.filename,
        dir: f.path.slice(0, f.path.length - f.filename.length).replace(/\/$/, ""),
        module,
      },
    ];
  });
}
