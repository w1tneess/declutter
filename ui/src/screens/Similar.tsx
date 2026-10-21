import { memo, useDeferredValue, useMemo, useState } from 'react';
import { api } from '../api/client';
import type { SimilarFile } from '../api/generated/SimilarFile';
import type { SimilarGroup } from '../api/generated/SimilarGroup';
import { Badge, Button, Icon } from '../design/components';
import { ResultList, type RowProps } from '../design/ResultList';
import { baseName, humanSize, plural, relPath } from '../format';
import { allowKeepingOne, groupRemovals } from '../state/rules';
import { alert, exportCsv, go, recycleSelected, reveal, toast, unpinned, useApp } from '../state/store';
import { FilterField, ListFooter, matches, ScreenHeader, showDetails, TabPlaceholder, useTabState } from './common';

type Row = { head: true; n: number; i: number } | { head?: false; f: SimilarFile; first: boolean };

const likeness = (d: number) => (d <= 4 ? 'Nearly identical' : d <= 10 ? 'Very similar' : 'Similar');

const SimilarRow = memo(function SimilarRow({ row }: RowProps<Row>) {
  const folder = useApp((s) => s.folder);
  if (row.head) return <span className="row-head">Group {row.i + 1} · {plural(row.n, 'similar picture')}</span>;
  return (
    <>
      <span className="row-main">{relPath(row.f.path, folder)}</span>
      <span className="row-meta">{row.f.width} × {row.f.height}</span>
      <span className="row-size">{humanSize(row.f.size)}</span>
      {row.first ? <Badge>First in group</Badge> : <Badge>{likeness(row.f.distance)} · difference {row.f.distance}</Badge>}
    </>
  );
});

const rowId = (r: Row) => (r.head ? `group:${r.i}` : r.f.path);
const isHead = (r: Row) => !!r.head;

export function Similar() {
  const groups = useApp((s) => s.results.similar);
  const sel = useApp((s) => s.sel.similar);
  const asked = useApp((s) => s.similarImages);
  const status = useApp((s) => s.status);
  const state = useTabState(groups?.length);
  const [q, setQ] = useState('');
  const query = useDeferredValue(q);

  const { rows, shown, groupOf } = useMemo(() => {
    const rows: Row[] = [];
    const shown: SimilarGroup[] = [];
    const groupOf = new Map<string, SimilarFile[]>();
    (groups ?? []).forEach((g, i) => {
      g.files.forEach((f) => groupOf.set(f.path, g.files));
      if (!matches(query, ...g.files.map((f) => baseName(f.path)))) return;
      shown.push(g);
      rows.push({ head: true, n: g.files.length, i }, ...g.files.map((f, k) => ({ f, first: k === 0 })));
    });
    return { rows, shown, groupOf };
  }, [groups, query]);

  // Never preselected, and every group keeps at least one picture.
  const tick = (ids: string[], on: boolean) => {
    if (!on) return sel.set(ids, false);
    const { ok, refused } = allowKeepingOne(ids, groupOf, (p) => sel.has(p));
    sel.set(ok, true);
    if (refused) toast('Keep at least one picture in each group.');
  };
  const selectAll = () => tick(shown.flatMap((g) => g.files.slice(1).map((f) => f.path)), true);

  const remove = async () => {
    const going = new Set(unpinned(sel.list()));
    let removals;
    try {
      removals = groupRemovals(groups ?? [], (p) => going.has(p));
    } catch {
      return void alert('Keep one picture', 'Keep at least one picture in each group, then try again.');
    }
    await recycleSelected(removals.flat().length, (root) => api.trashSimilar(root, removals));
  };

  if (!asked && state !== 'ready' && status !== 'scanning')
    return (
      <div className="screen">
        <ScreenHeader title="Similar images" subtitle="Pictures that look alike: bursts, edits and resized copies. Nothing is selected for you." />
        <div className="empty">
          <span className="empty-icon"><Icon name="imageMultiple" /></span>
          <p className="empty-title">Similar pictures are not part of this scan.</p>
          <p className="empty-body">Turn on “Also look for similar pictures” on Home, then scan. It takes a little longer.</p>
          <Button variant="primary" onClick={() => go('home')}>Go to Home</Button>
        </div>
      </div>
    );

  return (
    <div className="screen">
      <ScreenHeader title="Similar images" subtitle="Pictures that look alike: bursts, edits and resized copies. Nothing is selected for you, and each group keeps at least one. Double-click a picture to see it in Explorer.">
        <Button icon="arrowExportLtr" disabled={!shown.length} onClick={() => void exportCsv('similar', { kind: 'similar', groups: shown })}>Export CSV</Button>
      </ScreenHeader>
      {state === 'ready' && <div className="toolbar"><FilterField value={q} onChange={setQ} /></div>}
      {state !== 'ready' ? (
        <TabPlaceholder state={state} what="similar pictures" />
      ) : (
        <>
          <div className="similar-layout">
            <div className="list-card">
              {rows.length ? (
                <ResultList
                  rows={rows}
                  rowId={rowId}
                  isHeader={isHead}
                  selection={sel}
                  onToggle={tick}
                  onSelectAll={selectAll}
                  Row={SimilarRow}
                  label="Similar pictures"
                  onOpen={(r) => !r.head && showDetails(r.f, <><dt>Dimensions</dt><dd>{r.f.width} × {r.f.height}</dd></>)}
                  onReveal={(r) => !r.head && reveal(r.f.path)}
                />
              ) : <p className="no-match">No file names match “{q}”.</p>}
            </div>
            <aside className="preview" aria-label="Preview">
              <span className="preview-frame"><Icon name="image" size={20} /></span>
              <span className="preview-frame"><Icon name="image" size={20} /></span>
              <p>Side-by-side previews are coming in a later version. For now, double-click a picture to compare it in Explorer.</p>
            </aside>
          </div>
          <ListFooter selection={sel} action="Send to Recycle Bin" onAction={() => void remove()} onSelectAll={selectAll} />
        </>
      )}
    </div>
  );
}
