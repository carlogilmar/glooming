// Tags on a gloom.
//
// They live in `docs.label`, which the first migration called "a free field:
// `PR #412`, `claude-generated`" — this is what it was free for. A
// comma-separated list rather than a table of its own, because tags here are for
// *finding* a gloom again, not for reporting on: there is nothing to join, and a
// migration buys nothing the string does not already give.
//
// Order is preserved (the order you typed them reads as importance), duplicates
// are dropped, and the whole thing is trimmed — a stray comma should not become
// an empty chip nobody can click off.

export function parseTags(label: string | null | undefined): string[] {
  if (!label) return [];
  const out: string[] = [];
  for (const raw of label.split(",")) {
    const t = raw.trim();
    if (t && !out.some((x) => x.toLowerCase() === t.toLowerCase())) out.push(t);
  }
  return out;
}

export function joinTags(tags: string[]): string {
  return tags.join(", ");
}

/** Add one, keeping the list a set. Returns the same array when nothing changed. */
export function withTag(tags: string[], tag: string): string[] {
  const t = tag.trim().replace(/,/g, " ").replace(/\s+/g, " ");
  if (!t) return tags;
  if (tags.some((x) => x.toLowerCase() === t.toLowerCase())) return tags;
  return [...tags, t];
}

export function withoutTag(tags: string[], tag: string): string[] {
  return tags.filter((x) => x.toLowerCase() !== tag.toLowerCase());
}

/**
 * A stable colour per tag, derived from the tag itself.
 *
 * Nobody picks a colour for a tag — that is a preference dialog for a thing you
 * typed in two seconds — and a random one would make the same tag look different
 * in two places. A hash of the text gives `retry` the same hue everywhere, for
 * free, and forever.
 */
export function tagHue(tag: string): number {
  let h = 2166136261;
  for (const c of tag.toLowerCase()) {
    h ^= c.charCodeAt(0);
    h = Math.imul(h, 16777619);
  }
  return (h >>> 0) % 360;
}
