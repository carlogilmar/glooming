// A remembered font size, shared by both panes.
//
// The code pane had this inline; the doc pane wanted the same thing. Two copies
// of "clamp, round to a half point, persist" is two places for them to drift,
// so it lives here once and each pane supplies its own storage key.

const MIN = 10;
const MAX = 22;

export interface FontSize {
  readonly value: number;
  readonly min: number;
  readonly max: number;
  load(): void;
  set(next: number): void;
  reset(): void;
}

export function fontSize(key: string, initial: number): FontSize {
  let size = $state(initial);

  return {
    get value() {
      return size;
    },
    get min() {
      return MIN;
    },
    get max() {
      return MAX;
    },
    /** Read the stored preference. Called from an effect, so it runs client-side. */
    load() {
      const stored = parseFloat(localStorage.getItem(key) ?? "");
      if (stored >= MIN && stored <= MAX) size = stored;
    },
    set(next: number) {
      // Half points: fine enough to tune, coarse enough that two presses are
      // visibly different.
      size = Math.min(MAX, Math.max(MIN, Math.round(next * 2) / 2));
      localStorage.setItem(key, String(size));
    },
    reset() {
      this.set(initial);
    },
  };
}
