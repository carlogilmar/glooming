#!/usr/bin/env python3
"""An $effect must not both read and write the same $state.

Writing state that the same effect reads re-triggers it forever, pins the main
thread, and the window stops responding. `svelte-check` sees nothing wrong —
nothing is mistyped — so this is the only thing that catches it.

Only SYNCHRONOUS access counts. Svelte tracks the reads that happen while the
effect body runs, so state touched inside a `.then()`, a `setTimeout` or an event
listener is not a dependency and cannot loop — flagging those would make this cry
wolf on correct code, and a check people ignore is worse than no check.

Heuristic, deliberately: it flags direct reads and writes of a component's own
`$state` variables. Indirection through a function call is not detected. Prefer a
false positive here to a frozen UI.
"""
import os, re, sys

def strip_comments(src):
    """Blank out // and /* */ comments, preserving length so spans stay valid."""
    out, i, n = [], 0, len(src)
    while i < n:
        two = src[i:i + 2]
        if two == '//':
            j = src.find('\n', i)
            j = n if j == -1 else j
            out.append(' ' * (j - i))
            i = j
        elif two == '/*':
            j = src.find('*/', i + 2)
            j = n if j == -1 else j + 2
            out.append(''.join(c if c == '\n' else ' ' for c in src[i:j]))
            i = j
        else:
            out.append(src[i])
            i += 1
    return ''.join(out)


def match_block(s, open_at):
    """From the '(' of `$effect(`, return the span of the whole call."""
    i, d = open_at, 0
    while i < len(s):
        if s[i] == '(':
            d += 1
        elif s[i] == ')':
            d -= 1
            if d == 0:
                return open_at, i
        i += 1
    return open_at, len(s)

#: Callbacks whose reads are not tracked dependencies — either because they run
#: after the effect body has finished, or because `untrack` says so outright.
#: `untrack` is the sanctioned escape hatch for exactly the case this checks for
#: (restore some DOM state after a render, from state you must not depend on), so
#: treating a read inside it as a dependency would flag correct code — and a check
#: that cries wolf is one people learn to ignore.
DEFERRED = ('.then(', '.catch(', '.finally(', 'setTimeout(', 'setInterval(',
            'queueMicrotask(', 'requestAnimationFrame(', 'addEventListener(',
            'untrack(')


def deferred_spans(body):
    """Spans of every deferred-callback argument list inside `body`."""
    spans = []
    for marker in DEFERRED:
        start = 0
        while True:
            at = body.find(marker, start)
            if at == -1:
                break
            open_at = at + len(marker) - 1
            i, d = open_at, 0
            while i < len(body):
                if body[i] == '(':
                    d += 1
                elif body[i] == ')':
                    d -= 1
                    if d == 0:
                        break
                i += 1
            spans.append((open_at, i))
            start = at + len(marker)
    return spans


findings = []
for root, _, fs in os.walk('src'):
    for f in sorted(fs):
        if not f.endswith('.svelte'):
            continue
        p = os.path.join(root, f)
        s = open(p).read()

        states = set(re.findall(r'\blet\s+([A-Za-z_$][\w$]*)[^=\n]*=\s*\$state', s))
        if not states:
            continue

        for m in re.finditer(r'\$effect\s*\(', s):
            a, b = match_block(s, m.end() - 1)
            line = s[:m.start()].count('\n') + 1
            # Comments are stripped, or any comment that happens to use a state
            # variable's name reads as a dependency. That produced a false
            # positive on the word "steps" inside a sentence about steps.
            body = strip_comments(s[a:b])

            spans = deferred_spans(body)
            sync = lambda at: not any(a <= at < b for a, b in spans)

            for name in sorted(states):
                writes = [m.start() for m in re.finditer(
                    rf'(?<![\w.$]){re.escape(name)}\s*(?:=(?!=)|\+=|-=|\+\+|--)', body)]
                if not writes:
                    continue
                # Reads are occurrences that are not the assignment target. Blank
                # the write sites first so `x = x + 1` still counts as a read.
                blanked = re.sub(rf'(?<![\w.$]){re.escape(name)}\s*=(?!=)',
                                 lambda m: ' ' * len(m.group(0)), body)
                reads = [m.start() for m in re.finditer(
                    rf'(?<![\w.$]){re.escape(name)}(?![\w$]\s*=)', blanked)]

                # A loop needs BOTH to be tracked: a synchronous read to become a
                # dependency, and a write to invalidate it.
                if any(sync(r) for r in reads) and any(sync(w) for w in writes):
                    findings.append((p, line, name, len(writes), len(reads)))

for p, line, name, w, r in findings:
    print(f"  LOOP  {os.path.basename(p)}:{line}  ${{{name}}} written {w}×, read {r}× in one $effect")
print(f"\n{len(findings)} effect(s) that read and write the same state"
      if findings else "\nNo $effect reads and writes the same $state.")
sys.exit(1 if findings else 0)
