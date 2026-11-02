// The `/` menu's grammar.
//
// One thing is typed after a slash now — a reference — so the rule is short:
// the menu closes on whitespace, a newline or a backtick. A bare `/` followed by a
// space is a slash in ordinary prose and must stay one.
//
// It used to be longer. `/surface billing.ex` put a block in the note, and needed
// a space to be an argument separator once a command name matched. The blocks
// left the note when the code became live — the views beside the note are the
// live ones — and the commands went with them.
//
// Shared between `DocPane` (which decides when the menu stays open) and
// `RefMenu` (which decides what to show in it). One rule, or the two would
// disagree about what is still a live query.

/** Is this still a live query, or has the menu been typed out of? */
export function stillOpen(typed: string): boolean {
  return !/[\s`]/.test(typed);
}
