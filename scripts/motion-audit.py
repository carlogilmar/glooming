#!/usr/bin/env python3
"""Every animation must have an exactly-matching reduced-motion rule.

Selector text is compared, not just the animation name — because a rule written
with a shorter selector than the one it means to override loses on specificity
and silently does nothing. That is exactly the bug this found the first time it
ran, three times over.
"""
import os, re, sys

def rm_blocks(s):
    out = []
    for m in re.finditer(r'@media \(prefers-reduced-motion: reduce\)\s*\{', s):
        i, d = m.end(), 1
        while i < len(s) and d:
            d += 1 if s[i] == '{' else -1 if s[i] == '}' else 0
            i += 1
        out.append((m.start(), i))
    return out

def selector_at(s, at):
    """The selector of the rule containing `at`, with comments stripped."""
    brace = s.rfind('{', 0, at)
    starts = [s.rfind('}', 0, brace) + 1, s.rfind('{', 0, brace) + 1, s.rfind('*/', 0, brace) + 2]
    head = s[max(starts):brace]
    head = re.sub(r'/\*.*?\*/', ' ', head, flags=re.S)
    return ' '.join(head.split())

live, covered, files = [], {}, {}
for root, _, fs in os.walk('src'):
    for f in sorted(fs):
        if not f.endswith(('.svelte', '.css')):
            continue
        p = os.path.join(root, f)
        s = open(p).read()
        blocks = rm_blocks(s)
        inside = lambda i: any(a <= i < b for a, b in blocks)

        for m in re.finditer(r'animation(?:-name)?:\s*([\w-]+)', s):
            name, sel = m.group(1), selector_at(s, m.start())
            if inside(m.start()):
                if name == 'none':
                    for part in sel.split(','):
                        covered.setdefault(p, set()).add(part.strip())
            elif name != 'none':
                live.append((p, name, sel))
        # `display: none` on a decorative ::after counts as neutralising it.
        for m in re.finditer(r'display:\s*none', s):
            if inside(m.start()):
                for part in selector_at(s, m.start()).split(','):
                    covered.setdefault(p, set()).add(part.strip())

gaps = [(p, n, sel) for p, n, sel in live if sel not in covered.get(p, set())]
for p, n, sel in gaps:
    print(f"  GAP  {n:11} {sel[:70]}   {os.path.basename(p)}")
print(f"\n{len(live)} animations, {len(gaps)} without an exactly-matching reduced-motion rule")
sys.exit(1 if gaps else 0)
