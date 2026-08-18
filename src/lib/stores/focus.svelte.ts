// What is being read right now. Shared by both panes: the doc pane sets the
// function selection, the code pane sets the line cursor, and both panes react
// to both. Keeping it in one store is what makes the two sides feel like one
// selection rather than two coincidences.
//
// There are two independent selections, because they answer different questions:
//
//   · a FUNCTION selection — "show me this whole thing" — is several spans, as
//     an Elixir function rarely is one: every clause, the sibling arities, its
//     @spec and its @doc. It dims the rest of the file.
//   · a LINE cursor — "I am reviewing this line" — is one line, moves with the
//     arrow keys, and deliberately does NOT dim anything, because reading line
//     by line is exactly when you want the surrounding context.
//
// They compose: focus a function, then walk its body a line at a time.

export interface Span {
  start: number;
  end: number;
}

const inside = (spans: Span[], n: number) => spans.some((s) => n >= s.start && n <= s.end);

class FocusStore {
  // ---- function selection ----
  sig = $state("");
  ranges = $state<Span[]>([]);
  related = $state<Span[]>([]);
  spec = $state<Span | null>(null);
  doc = $state<Span | null>(null);

  // ---- line cursor ----
  cursorLine = $state<number | null>(null);

  // ---- scroll-driven reading ----
  /** True while the doc's scroll position is driving the selection. */
  reading = $state(false);
  /**
   * The ranges being left behind, held for a beat so the outgoing highlight
   * overlaps the incoming one. Seeing both for a moment is what makes a jump
   * legible instead of a cut.
   */
  leaving = $state<Span[]>([]);
  private leaveTimer: ReturnType<typeof setTimeout> | null = null;

  get active(): boolean {
    return this.ranges.length > 0;
  }

  /** Anything selected at all — the hint pill shows for either. */
  get anything(): boolean {
    return this.active || this.cursorLine !== null;
  }

  /**
   * Where to scroll. The top of the whole unit, so the doc and spec above a
   * function are on screen with it rather than just off the top edge.
   */
  get line(): number | null {
    if (!this.ranges.length) return null;
    return Math.min(
      this.ranges[0].start,
      this.spec?.start ?? Infinity,
      this.doc?.start ?? Infinity,
    );
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

  isDoc(n: number): boolean {
    return !!this.doc && n >= this.doc.start && n <= this.doc.end;
  }

  isLeaving(n: number): boolean {
    return !this.contains(n) && inside(this.leaving, n);
  }

  /**
   * Move to a step of a reading. Unlike `set` it never toggles — scrolling back
   * onto a step you have already seen must show it again — and it stashes the
   * outgoing ranges for the crossfade.
   */
  step(sig: string, ranges: Span[], spec: Span | null = null, doc: Span | null = null) {
    if (this.ranges.length && sig !== this.sig) {
      this.leaving = this.ranges;
      if (this.leaveTimer) clearTimeout(this.leaveTimer);
      this.leaveTimer = setTimeout(() => (this.leaving = []), 620);
    }
    this.select(sig, ranges, [], spec, doc);
  }

  /** Back to the resting state: the reading has not begun. */
  rest() {
    if (this.leaveTimer) clearTimeout(this.leaveTimer);
    this.leaving = [];
    this.clearFunction();
  }

  /** First line of any clause — the lines that get the breathing bar. */
  isHead(n: number): boolean {
    return this.ranges.some((r) => r.start === n);
  }

  isTail(n: number): boolean {
    return this.ranges.some((r) => r.end === n && r.end !== r.start);
  }

  /** Clicking the already-selected thing toggles focus off. */
  set(
    sig: string,
    ranges: Span[],
    related: Span[] = [],
    spec: Span | null = null,
    doc: Span | null = null,
  ) {
    if (this.sig === sig && this.active) {
      this.clearFunction();
      return;
    }
    this.select(sig, ranges, related, spec, doc);
  }

  /**
   * Select without toggling — for navigation (`[`/`]`, the palette), where
   * "go here" must never mean "go nowhere" just because you were already there.
   */
  select(
    sig: string,
    ranges: Span[],
    related: Span[] = [],
    spec: Span | null = null,
    doc: Span | null = null,
  ) {
    this.sig = sig;
    this.ranges = ranges;
    this.related = related;
    this.spec = spec;
    this.doc = doc;
  }

  /** Click a line to review it; click the same line again to drop the cursor. */
  setCursor(n: number) {
    this.cursorLine = this.cursorLine === n ? null : n;
  }

  /**
   * Step the cursor. With no cursor yet, an arrow key starts it at the top of
   * the focused function (or line 1), so ↓ always does something sensible.
   */
  moveCursor(delta: number, total: number) {
    const from = this.cursorLine ?? (this.ranges[0]?.start ?? 1) - delta;
    this.cursorLine = Math.min(Math.max(from + delta, 1), Math.max(total, 1));
  }

  /** Absolute jump — `42G`, `gg`, `H`. Never toggles; "go here" means go here. */
  gotoLine(n: number, total: number) {
    this.cursorLine = Math.min(Math.max(n, 1), Math.max(total, 1));
  }

  clearFunction() {
    this.sig = "";
    this.ranges = [];
    this.related = [];
    this.spec = null;
    this.doc = null;
  }

  /** Esc: drop everything, so one keypress always gets you back to plain code. */
  clear() {
    if (this.leaveTimer) clearTimeout(this.leaveTimer);
    this.leaving = [];
    this.clearFunction();
    this.cursorLine = null;
  }
}

export const focus = new FocusStore();
