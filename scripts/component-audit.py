#!/usr/bin/env python3
"""Every imported component must actually be rendered.

An import with no matching `<Component` in the markup is a feature that is wired
end to end and cannot be reached. `svelte-check` says nothing: the import is a
valid binding, the state that gates it is real, the button that sets that state
type-checks. Everything is correct except that nothing mounts.

This is not hypothetical. `HelpModal` was imported, `showHelp` was toggled by `?`
and by two buttons, and the `{#if showHelp}` block had been dropped — so pressing
`?` did nothing at all, through five commits, until someone went looking for a
shortcut they could not find.
"""
import os
import re
import sys

findings = []

for root, _, files in os.walk("src"):
    for name in sorted(files):
        if not name.endswith(".svelte"):
            continue
        path = os.path.join(root, name)
        text = open(path, encoding="utf-8").read()

        # Where the markup starts — an import cannot be "used" by the script block
        # referring to it, only by the template rendering it.
        for m in re.finditer(
            r'^\s*import\s+([A-Z][A-Za-z0-9_]*)\s+from\s+["\'][^"\']+\.svelte["\']',
            text,
            re.M,
        ):
            comp = m.group(1)
            # `<Comp`, `<Comp/>`, or dynamic `<svelte:component this={Comp}`.
            used = re.search(rf"<{re.escape(comp)}\b", text) or re.search(
                rf"this=\{{\s*{re.escape(comp)}\s*\}}", text
            )
            if not used:
                line = text[: m.start()].count("\n") + 1
                findings.append((path, line, comp))

for path, line, comp in findings:
    print(f"  UNMOUNTED  {os.path.basename(path)}:{line}  <{comp}> is imported but never rendered")

print(
    f"\n{len(findings)} imported component(s) never rendered"
    if findings
    else "\nEvery imported component is rendered."
)
sys.exit(1 if findings else 0)
