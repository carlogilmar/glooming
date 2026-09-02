// Turning a gloom's name into a filename.
//
// A gloom is named as prose — `Accounts — the write path`, `Retry storm, PR 412`
// — and a filename is not prose. This is the one bit of that translation with
// edge cases worth pinning: em dashes, accents, a name that is entirely
// punctuation, and a name long enough to be unreasonable as a file.

/** How long a stem is allowed to get. Past this it stops being a name. */
const MAX = 60;

/**
 * `Accounts — the write path` → `accounts-the-write-path.md`
 *
 * Diacritics are folded rather than dropped: `Añadir usuario` should be
 * `anadir-usuario`, not `a-adir-usuario`, which is what stripping non-ASCII
 * without decomposing first gives you.
 *
 * An unnamed gloom, or one whose name survives none of this, falls back to
 * `gloom.md` — the save dialog is where the user names it anyway, so the
 * suggestion only has to be reasonable, never clever.
 */
export function exportFilename(title: string): string {
  const stem = (title ?? "")
    .normalize("NFD")
    // Combining marks, once the base letters have been split off them.
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    // A `.md` the user typed in the name is not part of the name.
    .replace(/\.md$/, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, MAX)
    // Slicing can leave a trailing dash where a word was cut.
    .replace(/-+$/, "");

  return `${stem || "gloom"}.md`;
}
