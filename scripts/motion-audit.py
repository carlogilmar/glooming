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

# Inside @keyframes the movers are PROPERTY NAMES, so they are matched at the
# start of a declaration — `\b(bottom)\b` also matched `border-bottom-color`,
# which is a colour, and that false positive reported a fade as movement.
MOVERS = (
    r'(?:^|[;{\s])(transform|translate|scale|rotate|height|width'
    r'|top|left|right|bottom|margin|padding)(?:-[a-z]+)*\s*:'
)


def still_only_fades(s, name):
    """True if `@keyframes name` animates nothing that moves."""
    m = re.search(r'@keyframes\s+' + re.escape(name) + r'\s*\{', s)
    if not m:
        return False
    i, d = m.end(), 1
    while i < len(s) and d:
        d += 1 if s[i] == '{' else -1 if s[i] == '}' else 0
        i += 1
    return not re.search(MOVERS, s[m.end():i])


live, covered, files = [], {}, {}
for root, _, fs in os.walk('src'):
    for f in sorted(fs):
        if not f.endswith(('.svelte', '.css')):
            continue
        p = os.path.join(root, f)
        s = open(p).read()
        files[p] = s
        blocks = rm_blocks(s)
        inside = lambda i: any(a <= i < b for a, b in blocks)

        for m in re.finditer(r'animation(?:-name)?:\s*([\w-]+)([^;}]*)', s):
            name, rest, sel = m.group(1), m.group(2), selector_at(s, m.start())
            if inside(m.start()):
                # `none` covers a rule, and so does an animation that no longer
                # MOVES anything — one whose keyframes touch only opacity or
                # colour. Same principle already applied to transitions: reduced
                # motion means gentler, not nothing, and it is the better
                # override where the animation's end state is "gone". Deleting
                # such a rule outright would leave the thing it fades out
                # permanently on screen, which is worse than the motion was.
                if name == 'none' or still_only_fades(s, name):
                    for part in sel.split(','):
                        covered.setdefault(p, set()).add(part.strip())
            elif name != 'none':
                # A FINITE animation that only fades needs no rule at all — the
                # same exemption an opacity-only transition already gets, and for
                # the same reason: reduced motion drops movement, and there is
                # none here. `infinite` is excluded from the exemption, because an
                # ambient fade that never stops is exactly what the setting is
                # asking about even though nothing travels.
                if 'infinite' not in rest and still_only_fades(s, name):
                    continue
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

# ---------------------------------------------------------------------------
# An animation that leaves its own box needs a host that clips.
#
# This shipped, and it produced three symptoms none of which looked like the
# cause. The code pane's arrival sweep is a 45%-tall band translated from -120%
# to 320%, so at both ends it is entirely outside the element it decorates. With
# nothing clipping it, it swept over the pane header, the app header and the
# title strip — and, far worse, it gave `.app` scrollable height. `.app` is
# `overflow: hidden`, which clips but still makes a scroll container, and the
# browser scrolls one of those to reveal a focused or `scrollIntoView`-ed
# descendant. So selecting a function scrolled the whole window up and put the
# app header behind the traffic lights, while the footer changed width as a
# scrollbar came and went.
#
# The rule: a keyframe that translates more than 100% of its own size travels
# outside its box, so whatever hosts it must declare `overflow`. Both sweeps
# that predate this check clip themselves (`.gloombar`, `.masthead`); the one
# that did not was the one that broke.
def strip_comments(t):
    return re.sub(r'/\*.*?\*/', '', t, flags=re.S)

leaks = []
for p, raw in files.items():
    css = strip_comments(raw.split('<style>', 1)[1] if '<style>' in raw else raw)

    far = set()
    for m in re.finditer(r'@keyframes\s+([\w-]+)\s*\{((?:[^{}]|\{[^{}]*\})*)\}', css):
        pcts = [abs(float(x)) for x in re.findall(r'translate[XY]?\(\s*(-?[\d.]+)%', m.group(2))]
        if pcts and max(pcts) > 100:
            far.add(m.group(1))
    if not far:
        continue

    for sel, decl in re.findall(r'([^{}]+)\{([^{}]*)\}', css):
        used = [n for n in far if re.search(r'animation:[^;]*\b' + re.escape(n) + r'\b', decl)]
        if not used:
            continue
        for part in sel.split(','):
            part = part.strip()
            if not part or part.startswith('@'):
                continue
            # The pseudo travels; the element it is anchored to is what must clip.
            host = re.sub(r'::?(before|after)$', '', part).strip()
            base = host.split()[-1] if host.split() else host
            # `.gloombar.arriving` clips as `.gloombar` — take the first class.
            root = re.match(r'(\.[\w-]+|[\w-]+)', base)
            root = root.group(1) if root else base
            clips = re.search(
                r'(^|\})\s*[^{}]*' + re.escape(root) + r'[^{},]*\{[^{}]*overflow[^:]*:\s*(hidden|clip|auto|scroll)',
                css,
            )
            if not clips:
                leaks.append((p, used[0], part, root))

for p, n, sel, root in leaks:
    print(f"  LEAK {n:11} {sel[:52]}  {root} does not clip   {os.path.basename(p)}")
if leaks:
    print(f"{len(leaks)} animation(s) travel outside a box that nothing clips")
else:
    print("every animation that leaves its box has a host that clips")

sys.exit(1 if gaps or leaks else 0)
