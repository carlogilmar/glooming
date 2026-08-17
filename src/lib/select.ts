// Resolving a signature to everything worth highlighting.
//
// Shared by both panes, because selecting a function has to mean the same thing
// whether you clicked its row in the explanation or its name in the code. The
// unit is not "the body" — it's the body plus the two things that describe it:
//
//   ranges   every clause of this exact name/arity
//   related  the same name at other arities (one function to a reader, even
//            though the BEAM treats them as separate)
//   spec     its @spec — the contract, not the body
//   doc      its @doc — the prose the author already wrote

import type { ModuleInfo, Range } from "$lib/ipc";

export interface Selection {
  ranges: Range[];
  related: Range[];
  spec: Range | null;
  doc: Range | null;
}

/** `search/1..2` → name `search`, arity 2. The top arity is the identity. */
export function splitSig(sig: string): { name: string; arity: number } {
  const bare = sig.replace(/~~/g, "");
  const slash = bare.lastIndexOf("/");
  const name = slash === -1 ? bare : bare.slice(0, slash);
  const arity = parseInt(bare.slice(slash + 1).split("..").pop() ?? "0", 10) || 0;
  return { name, arity };
}

export function locate(sig: string, module: ModuleInfo | null): Selection | null {
  const fns = module?.functions ?? [];
  const { name, arity } = splitSig(sig);

  const hit = fns.find((f) => f.name === name && f.arity === arity);
  if (!hit) return null;

  return {
    ranges: hit.clauseRanges?.length ? hit.clauseRanges : [{ start: hit.line, end: hit.endLine }],
    related: fns.filter((f) => f.name === name && f.arity !== arity).flatMap((f) => f.clauseRanges ?? []),
    spec: hit.specRange,
    doc: hit.docRange,
  };
}

/** How a function is written in a block: `create_user/1`, or `search/1..2`. */
export function displaySig(f: { name: string; arity: number; minArity: number }): string {
  return f.minArity < f.arity ? `${f.name}/${f.minArity}..${f.arity}` : `${f.name}/${f.arity}`;
}
