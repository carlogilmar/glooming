// Which function is being read. Shared by both panes: the doc pane sets it,
// the code pane reacts to it. Keeping it in one store is what makes the two
// sides feel like one selection rather than two coincidences.

class FocusStore {
  /** First line of the focused definition, or null when nothing is focused. */
  line = $state<number | null>(null);
  endLine = $state<number | null>(null);
  sig = $state("");

  get active(): boolean {
    return this.line !== null;
  }

  /** Is line `n` inside the focused definition? */
  contains(n: number): boolean {
    if (this.line === null) return false;
    return n >= this.line && n <= (this.endLine ?? this.line);
  }

  set(sig: string, line: number, endLine: number) {
    // Clicking the already-focused row toggles focus off.
    if (this.line === line) {
      this.clear();
      return;
    }
    this.sig = sig;
    this.line = line;
    this.endLine = endLine;
  }

  clear() {
    this.line = null;
    this.endLine = null;
    this.sig = "";
  }
}

export const focus = new FocusStore();
