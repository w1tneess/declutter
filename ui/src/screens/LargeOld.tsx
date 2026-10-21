import { memo, useDeferredValue, useMemo, useState } from 'react';
import { api } from '../api/client';
import type { FileRef } from '../api/generated/FileRef';
import type { Leftover } from '../api/generated/Leftover';
import type { SizeEntry } from '../api/generated/SizeEntry';
import { Badge, Button, Card, NumberField, Segmented } from '../design/components';
import { ResultList, ROW_H, type RowProps } from '../design/ResultList';
import { baseName, daysOld, humanSize, parentRel, plural, relPath } from '../format';
import { act, binWording, exportCsv, reveal, startScan, unpinned, updateSettings, useApp } from '../state/store';
import { FilterField, ListFooter, matches, ScreenHeader, showDetails, TabPlaceholder, useTabState } from './common';

type Row =
  | { t: 'head'; text: string; note?: string }
  | { t: 'left'; x: Leftover }
  | { t: 'file'; f: FileRef };

const LargeRow = memo(function LargeRow({ row }: RowProps<Row>) {
  const folder = useApp((s) => s.folder);
  if (row.t === 'head')
    return (
      <span className="row-head row-head-wrap">
        <strong>{row.text}</strong>
        {row.note && ` ${row.note}`}
      </span>
    );
  const f = row.t === 'left' ? row.x : row.f;
  return (
    <>
      <span className="row-main">{baseName(f.path)}</span>
      <span className="row-meta">{parentRel(f.path, folder) || 'Chosen folder'}</span>
      {row.t === 'left' && <Badge tone="warning">{row.x.reason}</Badge>}
      <span className="row-meta row-age">{plural(daysOld(f.mtime), 'day')} old</span>
      <span className="row-size">{humanSize(f.size)}</span>
    </>
  );
});

const rowId = (r: Row) => (r.t === 'head' ? `head:${r.text}` : r.t === 'left' ? r.x.path : r.f.path);
const isHead = (r: Row) => r.t === 'head';
const rowHeight = (r: Row) => (r.t === 'head' && r.note ? 64 : ROW_H);

function Bars({ title, entries, label }: { title: string; entries: SizeEntry[]; label: (e: SizeEntry) => string }) {
  const top = entries.slice(0, 8);
  const max = Math.max(1, ...top.map((e) => e.bytes));
  return (
    <Card className="bars">
      <h2 className="card-title">{title}</h2>
      <ul>
        {top.map((e) => (
          <li key={e.name}>
            <span className="bar-label">{label(e)}</span>
            <span className="bar-value">{humanSize(e.bytes)} · {plural(e.files, 'file')}</span>
            <span className="bar-track" aria-hidden="true"><span className="bar-fill" style={{ transform: `scaleX(${e.bytes / max})` }} /></span>
          </li>
        ))}
      </ul>
    </Card>
  );
}

export function LargeOld() {
  const large = useApp((s) => s.results.largeOld);
  const leftovers = useApp((s) => s.results.leftovers);
  const breakdown = useApp((s) => s.results.breakdown);
  const sel = useApp((s) => s.sel.largeOld);
  const settings = useApp((s) => s.settings);
  const folder = useApp((s) => s.folder);
  const busy = useApp((s) => s.busy || s.status === 'scanning');
  const state = useTabState(large && leftovers ? large.length + leftovers.length : undefined);
  const [view, setView] = useState<'files' | 'space'>('files');
  const [q, setQ] = useState('');
  const query = useDeferredValue(q);

  const { rows, shownLarge } = useMemo(() => {
    const left = (leftovers ?? []).filter((x) => matches(query, baseName(x.path)));
    const mine = new Set((leftovers ?? []).map((x) => x.path));
    const shownLarge = (large ?? []).filter((f) => !mine.has(f.path) && matches(query, baseName(f.path)));
    const rows: Row[] = [];
    if (left.length) rows.push({ t: 'head', text: `Likely leftovers (${left.length}).`, note: 'Look at each one before you tick it. Nothing is ticked.' }, ...left.map((x) => ({ t: 'left' as const, x })));
    if (left.length && shownLarge.length) rows.push({ t: 'head', text: 'Large and old files' });
    rows.push(...shownLarge.map((f) => ({ t: 'file' as const, f })));
    return { rows, shownLarge };
  }, [large, leftovers, query]);

  // "Select all" ticks large and old files only; likely leftovers are ticked one by one.
  const selectAll = () => sel.set(shownLarge.map((f) => f.path), true);

  const remove = () => {
    const paths = unpinned(sel.list());
    void act({
      title: 'Send to Recycle Bin?',
      body: binWording(paths.length),
      confirmLabel: `Send ${plural(paths.length, 'item')}`,
      run: (root) => api.trashPaths(root, paths),
      done: (k) => `Sent ${plural(k, 'item')} to the Recycle Bin. Restore them from the Bin if needed.`,
    });
  };

  return (
    <div className="screen">
      <ScreenHeader title="Large & old files" subtitle="Big or long-untouched files. Nothing is selected for you, so tick what you want gone. Double-click a file to see it in Explorer.">
        <Button icon="arrowExportLtr" disabled={!shownLarge.length} onClick={() => void exportCsv('large-old', { kind: 'largeOld', files: shownLarge })}>Export CSV</Button>
      </ScreenHeader>
      <div className="toolbar">
        {settings && (
          <>
            <NumberField label="Bigger than (MB)" value={settings.minMb} min={0} onCommit={(n) => void updateSettings({ minMb: n })} />
            <NumberField label="Older than (days)" value={settings.olderDays} min={0} onCommit={(n) => void updateSettings({ olderDays: n })} />
          </>
        )}
        {folder && <Button icon="arrowClockwise" onClick={() => void startScan()} disabled={busy}>Scan again</Button>}
        <span className="toolbar-gap" />
        {breakdown && <Segmented label="Show" value={view} onChange={setView} options={[{ value: 'files', label: 'Files' }, { value: 'space', label: 'Where the space goes' }]} />}
      </div>
      <p className="hint">0 turns a filter off. Both filters must match. New filter values apply on the next scan.</p>
      {view === 'space' && breakdown ? (
        <div className="bars-grid">
          <Bars title="Biggest folders" entries={breakdown.folders} label={(e) => relPath(e.name, folder)} />
          <Bars title="By file type" entries={breakdown.types} label={(e) => e.name} />
        </div>
      ) : state !== 'ready' ? (
        <TabPlaceholder state={state} what="large and old files" />
      ) : (
        <>
          <div className="toolbar"><FilterField value={q} onChange={setQ} /></div>
          <div className="list-card">
            {rows.length ? (
              <ResultList
                rows={rows}
                rowId={rowId}
                isHeader={isHead}
                selection={sel}
                onSelectAll={selectAll}
                Row={LargeRow}
                rowHeight={rowHeight}
                label="Large and old files"
                onOpen={(r) => r.t !== 'head' && showDetails(r.t === 'left' ? r.x : r.f, r.t === 'left' ? <><dt>Why it is listed</dt><dd>{r.x.reason}</dd></> : undefined)}
                onReveal={(r) => r.t !== 'head' && reveal(r.t === 'left' ? r.x.path : r.f.path)}
              />
            ) : <p className="no-match">No file names match “{q}”.</p>}
          </div>
          <ListFooter selection={sel} action="Send to Recycle Bin" onAction={remove} onSelectAll={selectAll} />
        </>
      )}
    </div>
  );
}
