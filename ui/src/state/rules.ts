// Keep-one rules (SPEC safety rule 6, similar images "keep at least one"). The shell checks again; this stops the UI
// from ever sending a selection that removes every copy of a group.

export const KEEP_ONE = 'Keep at least one copy of every file. Pick which copy stays, then try again.';

/** The paths to remove, one list per group. Throws if any group would lose every file. */
export function groupRemovals(groups: readonly { files: { path: string }[] }[], removing: (path: string) => boolean): string[][] {
  const out: string[][] = [];
  for (const group of groups) {
    const remove = group.files.filter((f) => removing(f.path)).map((f) => f.path);
    if (!remove.length) continue;
    if (remove.length >= group.files.length) throw new Error(KEEP_ONE);
    out.push(remove);
  }
  return out;
}

/** Of `ids` the user wants ticked, the ones that still leave at least one file unticked in their group. */
export function allowKeepingOne(ids: readonly string[], groupOf: ReadonlyMap<string, readonly { path: string }[]>, isSelected: (path: string) => boolean): { ok: string[]; refused: number } {
  const taken = new Map<readonly { path: string }[], number>();
  const ok: string[] = [];
  let refused = 0;
  for (const id of ids) {
    const g = groupOf.get(id);
    if (!g || isSelected(id)) continue;
    const n = taken.get(g) ?? g.filter((f) => isSelected(f.path)).length;
    if (n + 1 >= g.length) {
      refused++;
      continue;
    }
    taken.set(g, n + 1);
    ok.push(id);
  }
  return { ok, refused };
}
