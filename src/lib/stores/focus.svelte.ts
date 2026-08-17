// Which function is being read. Shared by both panes: the doc pane sets it,
// the code pane reacts. Keeping it in one store is what makes the two sides
// feel like one selection rather than two coincidences.
//
// A selection is not one line span but several, because an Elixir function
// rarely is:
//   · every clause of the chosen name/arity   → primary highlight
//   · the same name at other arities          → related, dimmer highlight
//   · its @spec                               → its own color, read alongside

export interface Span {
  start: number;
  end: number;
}

const inside = (spans: Span[], n: number) => spans.some((s) => n >= s.start && n <= s.end);

class FocusStore {
  sig = $state("");
  ranges = $state<Span[]>([]);
  related = $state<Span[]>([]);
  spec = $state<Span | null>(null);

  get active(): boolean {
    return this.ranges.length > 0;
  }

  /** Where to scroll: the first clause, or the @spec just above it. */
  get line(): number | null {
    if (!this.ranges.length) return null;
    const first = this.ranges[0].start;
    return this.spec && this.spec.start < first ? this.spec.start : first;
  }

  /** Total lines covered, so the hint pill can say how much you're reading. */
  get lineCount(): number {
    return this.ranges.reduce((n, r) => n + (r.end - r.start + 1), 0);
  }

  get clauseCount(): number {
    return this.ranges.length;
  }

  contains(n: number): boolean {
    return inside(this.ranges, n);
  }

  isRelated(n: number): boolean {
    return !this.contains(n) && inside(this.related, n);
  }

  isSpec(n: number): boolean {
    return !!this.spec && n >= this.spec.start && n <= this.spec.end;
  }

  /** First line of any clause — the lines that get the breathing bar. */
  isHead(n: number): boolean {
    return this.ranges.some((r) => r.start === n);
  }

  isTail(n: number): boolean {
    return this.ranges.some((r) => r.end === n && r.end !== r.start);
  }

  set(sig: string, ranges: Span[], related: Span[] = [], spec: Span | null = null) {
    // Clicking the already-selected row toggles focus off.
    if (this.sig === sig && this.active) {
      this.clear();
      return;
    }
    this.sig = sig;
    this.ranges = ranges;
    this.related = related;
    this.spec = spec;
  }

  clear() {
    this.sig = "";
    this.ranges = [];
    this.related = [];
    this.spec = null;
  }
}

export const focus = new FocusStore();
