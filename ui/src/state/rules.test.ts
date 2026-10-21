import { describe, expect, it } from 'vitest';
import { allowKeepingOne, groupRemovals, KEEP_ONE } from './rules';
import { Selection } from './selection';

const g = (...paths: string[]) => ({ files: paths.map((path) => ({ path })) });

describe('groupRemovals (SPEC rule 6)', () => {
  it('returns only groups with something to remove', () => {
    const groups = [g('a1', 'a2', 'a3'), g('b1', 'b2')];
    expect(groupRemovals(groups, (p) => p === 'a2' || p === 'a3')).toEqual([{ group: groups[0], remove: ['a2', 'a3'] }]);
  });

  it('refuses a selection that removes every copy of a group', () => {
    expect(() => groupRemovals([g('a1', 'a2')], () => true)).toThrow(KEEP_ONE);
  });
});

describe('allowKeepingOne', () => {
  it('ticks up to all but one file per group and counts the refusals', () => {
    const a = g('a1', 'a2', 'a3').files;
    const groupOf = new Map(a.map((f) => [f.path, a]));
    const sel = new Set<string>();
    expect(allowKeepingOne(['a1', 'a2', 'a3'], groupOf, (p) => sel.has(p))).toEqual({ ok: ['a1', 'a2'], refused: 1 });
    sel.add('a1');
    expect(allowKeepingOne(['a2', 'a3'], groupOf, (p) => sel.has(p))).toEqual({ ok: ['a2'], refused: 1 });
  });
});

describe('Selection', () => {
  it('tracks count and bytes and notifies only on change', () => {
    const s = new Selection();
    s.sizes.set('x', 10).set('y', 5);
    let calls = 0;
    s.subscribe(() => calls++);
    s.set(['x', 'y'], true);
    s.set(['x'], true);
    expect([s.count, s.bytes, calls]).toEqual([2, 15, 1]);
    s.set(['x'], false);
    expect([s.count, s.bytes, s.has('y')]).toEqual([1, 5, true]);
    s.clear();
    expect([s.count, s.bytes, calls]).toEqual([0, 0, 3]);
  });
});
