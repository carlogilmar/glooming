#!/usr/bin/env python3
"""Every animation — and every transition that MOVES something — must have an
exactly-matching reduced-motion rule.

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
    starts = [
        s.rfind('}', 0, brace) + 1,
        s.rfind('{', 0, brace) + 1,
        s.rfind('*/', 0, brace) + 2,
        # In a .svelte file the markup above `<style>` is full of braces from
        # Svelte expressions, so the first rule in the block would otherwise
        # capture half a template as its selector.
        s.rfind('<style>', 0, brace) + len('<style>'),
    ]
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
                # Per selector PART, on both sides. A grouped rule needs every
                # part overridden — covering three of four leaves one animation
                # running under reduced motion, and comparing the whole comma
                # -joined string against a set of parts never matched at all.
                for part in sel.split(','):
                    live.append((p, name, part.strip()))

        # Transitions count too, but only the ones that MOVE something. Reduced
        # motion means "drop movement, keep opacity and colour" — so a fading
        # scrim needs no rule, and a scaling popover does. Missed on the first
        # pass, which mattered the moment three keyframe animations became
        # transitions and left the audit's field of view.
        for m in re.finditer(r'transition:\s*([^;]+);', s):
            decl, sel = m.group(1), selector_at(s, m.start())
            if inside(m.start()):
                # An override covers the rule if it stops the MOVEMENT — either
                # `transition: none`, or one that only transitions opacity or
                # colour. The second form is the better override: reduced motion
                # means gentler, not nothing, and an element that pops in with no
                # transition at all is harder to follow than one that fades.
                still_moves = re.search(
                    r'\b(transform|height|width|top|left|right|bottom|margin|padding)\b', decl
                )
                if not still_moves:
                    for part in sel.split(','):
                        covered.setdefault(p, set()).add(part.strip())
                continue
            moves = re.search(
                r'\b(transform|height|width|top|left|right|bottom|margin|padding)\b', decl
            )
            if moves:
                for part in sel.split(','):
                    live.append((p, 'transition:' + moves.group(1), part.strip()))
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
