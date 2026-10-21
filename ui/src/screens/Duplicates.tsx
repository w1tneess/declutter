import { memo, useDeferredValue, useMemo, useState } from 'react';
import { api } from '../api/client';
import type { DupGroup } from '../api/generated/DupGroup';
import type { FileRef } from '../api/generated/FileRef';
import type { Redownload } from '../api/generated/Redownload';
import { Badge, Button } from '../design/components';
import { ResultList, ROW_H, type RowProps } from '../design/ResultList';
import { baseName, dateText, humanSize, parentRel, plural, relPath } from '../format';
import { useSelectionVersion } from '../state/selection';
import { groupRemovals } from '../state/rules';
import { act, alert, binWording, exportCsv, reveal, unpinned, useApp } from '../state/store';
import { FilterField, ListFooter, matches, ScreenHeader, showDetails, TabPlaceholder, useTabState } from './common';

type Row =
  | { t: 'group'; g: DupGroup; i: number }
  | { t: 'file'; f: FileRef; g: DupGroup }
  | { t: 'section'; n: number }
  | { t: 'redl'; p: Redownload };

const sizePair = (a: number, b: number) => (humanSize(a) !== humanSize(b) ? `${humanSize(a)} vs ${humanSize(b)}` : `${a.toLocaleString('en-US')} B vs ${b.toLocaleString('en-US')} B`);

const DupRow = memo(function DupRow({ row }: RowProps<Row>) {
  const keep = useApp((s) => s.keep);
  const folder = useApp((s) => s.folder);
  switch (row.t) {
    case 'group':
      return <span className="row-head">{baseName(row.g.files[0]?.path ?? '')} · {row.g.files.length} copies · {humanSize(row.g.size)} each</span>;
    case 'section':
      return (
        <span className="row-head row-head-wrap">
          <strong>Possible re-downloads: {row.n}.</strong> Same folder and type, named like a copy, almost the same size, but not identical. Nothing is selected: look at each pair. The original always stays.
        </span>
      );
    case 'redl':
      return (
        <>
          <span className="row-main">{baseName(row.p.copy.path)}</span>
          <Badge>Numbered copy · {sizePair(row.p.copy.size, row.p.original.size)}</Badge>
          <span className="row-meta">Original (stays): {baseName(row.p.original.path)}</span>
        </>
      );
    case 'file': {
      const kept = keep.has(row.f.path);
      return (
        <>
          <span className="row-main">{relPath(row.f.path, folder)}</span>
          <span className="row-meta">{dateText(row.f.mtime)}</span>
          {kept ? <Badge tone="accent">Keep</Badge> : <span className="row-action" data-action="keep">Keep this one</span>}
        </>
      );
    }
  }
});

const rowId = (r: Row) => (r.t === 'file' ? r.f.path : r.t === 'redl' ? r.p.copy.path : r.t === 'group' ? `group:${r.i}` : 'section');
const isHead = (r: Row) => r.t === 'group' || r.t === 'section';
const rowHeight = (r: Row) => (r.t === 'section' ? 72 : ROW_H);

export function Duplicates() {
  const groups = useApp((s) => s.results.duplicates);
  const redl = useApp((s) => s.results.redownloads);
  const sel = useApp((s) => s.sel.duplicates);
  const keep = useApp((s) => s.keep);
  const folder = useApp((s) => s.folder);
  useSelectionVersion(keep);
  const state = useTabState(groups && redl ? groups.length + redl.length : undefined);
  const [q, setQ] = useState('');
  const query = useDeferredValue(q);

  const { rows, shownGroups, shownPairs } = useMemo(() => {
    const rows: Row[] = [];
    const shownGroups: DupGroup[] = [];
    const shownPairs: Redownload[] = [];
    (groups ?? []).forEach((g, i) => {
      if (!matches(query, ...g.files.map((f) => baseName(f.path)))) return;
      shownGroups.push(g);
      rows.push({ t: 'group', g, i }, ...g.files.map((f) => ({ t: 'file' as const, f, g })));
    });
    for (const p of redl ?? []) if (matches(query, baseName(p.copy.path), baseName(p.original.path))) shownPairs.push(p);
    if (shownPairs.length) rows.push({ t: 'section', n: shownPairs.length }, ...shownPairs.map((p) => ({ t: 'redl' as const, p })));
    return { rows, shownGroups, shownPairs };
  }, [groups, redl, query]);

  const selectable = (r: Row) => r.t === 'redl' || (r.t === 'file' && !keep.has(r.f.path));
  // "Select all" covers exact duplicates only; re-downloads are ticked one by one.
  const selectAll = () => sel.set(shownGroups.flatMap((g) => g.files.filter((f) => !keep.has(f.path)).map((f) => f.path)), true);

  const setKeeper = (r: Row) => {
    if (r.t !== 'file') return;
    keep.set(r.g.files.map((f) => f.path).filter((p) => p !== r.f.path), false);
    keep.set([r.f.path], true);
    sel.set([r.f.path], false);
  };

  const remove = async () => {
    const going = new Set(unpinned(sel.list()));
    let removals;
    try {
      removals = groupRemovals(groups ?? [], (p) => going.has(p) && !keep.has(p));
    } catch (e) {
      return void alert('Keep one copy', (e as Error).message);
    }
    const pairs = (redl ?? []).filter((p) => going.has(p.copy.path));
    const n = removals.reduce((a, r) => a + r.remove.length, 0) + pairs.length;
    if (!n) return;
    await act({
      title: 'Send to Recycle Bin?',
      body: binWording(n),
      confirmLabel: `Send ${plural(n, 'item')}`,
      run: async (root) => {
        const a = removals.length ? await api.trashDuplicates(root, removals) : { count: 0, errors: [] };
        const b = pairs.length ? await api.trashRedownloads(root, pairs, removals.flatMap((r) => r.remove)) : { count: 0, errors: [] };
        return { count: a.count + b.count, errors: [...a.errors, ...b.errors] };
      },
      done: (k) => `Sent ${plural(k, 'item')} to the Recycle Bin. Restore them from the Bin if needed.`,
    });
  };

  return (
    <div className="screen">
      <ScreenHeader title="Duplicates" subtitle="Files with identical content. The oldest copy is kept; pick a different one if you prefer (click “Keep this one” or press K). Double-click a file to see it in Explorer.">
        <Button icon="arrowExportLtr" disabled={!shownGroups.length} onClick={() => void exportCsv('duplicates', { kind: 'duplicates', groups: shownGroups })}>Export CSV</Button>
      </ScreenHeader>
      {state === 'ready' && <div className="toolbar"><FilterField value={q} onChange={setQ} /></div>}
      {state !== 'ready' ? (
        <TabPlaceholder state={state} what="duplicates" />
      ) : (
        <>
          <div className="list-card">
            {rows.length ? (
              <ResultList
                rows={rows}
                rowId={rowId}
                isHeader={isHead}
                selectable={selectable}
                selection={sel}
                onSelectAll={selectAll}
                Row={DupRow}
                rowHeight={rowHeight}
                label="Duplicate files"
                keyActions={{ k: 'keep' }}
                onAction={(a, r) => a === 'keep' && setKeeper(r)}
                onOpen={(r) => {
                  if (r.t === 'file') showDetails(r.f, <><dt>Folder</dt><dd className="path">{parentRel(r.f.path, folder) || 'The chosen folder'}</dd></>);
                  if (r.t === 'redl') showDetails(r.p.copy, <><dt>Original (stays)</dt><dd className="path">{r.p.original.path}</dd></>);
                }}
                onReveal={(r) => (r.t === 'file' ? reveal(r.f.path) : r.t === 'redl' && reveal(r.p.copy.path))}
              />
            ) : <p className="no-match">No file names match “{q}”.</p>}
          </div>
          <ListFooter selection={sel} action="Send to Recycle Bin" onAction={() => void remove()} onSelectAll={selectAll} />
        </>
      )}
    </div>
  );
}
