import { memo, useDeferredValue, useMemo, useState } from 'react';
import { api } from '../api/client';
import type { Move } from '../api/generated/Move';
import { Button, Icon, NumberField } from '../design/components';
import { ResultList, type RowProps } from '../design/ResultList';
import { ago, baseName, humanSize, plural } from '../format';
import { act, alert, confirm, exportCsv, refreshUndo, report, reveal, startScan, unpinned, updateSettings, useApp } from '../state/store';
import { FilterField, ListFooter, matches, ScreenHeader, showDetails, TabPlaceholder, useTabState } from './common';

type Row = { head: true; cat: string; n: number; bytes: number } | { head?: false; m: Move };

const SortRow = memo(function SortRow({ row }: RowProps<Row>) {
  if (row.head) return <span className="row-head">{row.cat.replace('/', ' › ')} · {plural(row.n, 'file')} · {humanSize(row.bytes)}</span>;
  return (
    <>
      <span className="row-main">{baseName(row.m.src)}</span>
      <span className="row-meta"><Icon name="arrowSort" size={16} /> {row.m.category.replace('/', ' › ')}</span>
      <span className="row-size">{humanSize(row.m.size)}</span>
    </>
  );
});

const rowId = (r: Row) => (r.head ? `head:${r.cat}` : r.m.src);
const isHead = (r: Row) => !!r.head;

export function Sort() {
  const plan = useApp((s) => s.results.sort);
  const sel = useApp((s) => s.sel.sort);
  const settings = useApp((s) => s.settings);
  const presets = useApp((s) => s.presets);
  const undo = useApp((s) => s.undo);
  const folder = useApp((s) => s.folder);
  const busy = useApp((s) => s.busy || s.status === 'scanning');
  const state = useTabState(plan?.moves.length);
  const [q, setQ] = useState('');
  const query = useDeferredValue(q);

  const { rows, visible } = useMemo(() => {
    const by = new Map<string, Move[]>();
    for (const m of plan?.moves ?? []) {
      if (!matches(query, baseName(m.src))) continue;
      const list = by.get(m.category);
      if (list) list.push(m);
      else by.set(m.category, [m]);
    }
    const rows: Row[] = [];
    const visible: Move[] = [];
    for (const cat of [...by.keys()].sort()) {
      const ms = by.get(cat) ?? [];
      rows.push({ head: true, cat, n: ms.length, bytes: ms.reduce((a, m) => a + m.size, 0) }, ...ms.map((m) => ({ m })));
      visible.push(...ms);
    }
    return { rows, visible };
  }, [plan, query]);

  const selectAll = () => sel.set(visible.map((m) => m.src), true);

  const sortNow = () => {
    const keep = new Set(unpinned(sel.list()));
    const moves = (plan?.moves ?? []).filter((m) => keep.has(m.src));
    const n = moves.length;
    void act({
      title: 'Sort files?',
      body: `Move ${plural(n, 'file')} into type folders inside ${folder}? Nothing is deleted. You can undo this with "Undo last sort".`,
      confirmLabel: `Sort ${plural(n, 'file')}`,
      run: (root) => api.applySort(root, moves, settings?.recentHours ?? 24),
      done: (k) => `Moved ${plural(k, 'file')}.`,
    });
  };

  const undoNow = async () => {
    const info = await api.peekUndo().catch(() => null);
    if (!info) return void alert('Undo', 'Nothing to undo.');
    const when = info.ageSeconds === null ? '' : ` (${ago(info.ageSeconds)})`;
    const name = baseName(info.folder) || info.folder;
    if (!(await confirm({ title: 'Undo last sort?', body: `Undo the sort of ${plural(info.count, 'file')} in ${name}${when}? The files go back where they were.`, confirmLabel: `Undo ${plural(info.count, 'file')}` }))) return;
    try {
      const r = await api.undoLastSort();
      report(`Restored ${plural(r.count, 'file')}.`, r.errors);
    } catch (e) {
      await alert('Something went wrong', typeof e === 'string' ? e : 'The sort could not be undone.');
    }
    await refreshUndo();
    if (folder) await startScan();
  };

  return (
    <div className="screen">
      <ScreenHeader title="Sort by type" subtitle="Moves loose files into folders by type (Images, Documents, …). Nothing is deleted.">
        <Button icon="arrowExportLtr" disabled={!visible.length} onClick={() => void exportCsv('sort', { kind: 'sort', moves: visible })}>Export CSV</Button>
      </ScreenHeader>
      {undo && undo.count > 0 && (
        <div className="banner">
          <Icon name="arrowUndo" />
          <p>Your last sort moved {plural(undo.count, 'file')}{undo.ageSeconds !== null && ` ${ago(undo.ageSeconds)}`}.</p>
          <Button onClick={() => void undoNow()} disabled={busy}>Undo last sort</Button>
        </div>
      )}
      <div className="toolbar">
        <div className="field">
          <label className="field-label" htmlFor="preset">Sort rules</label>
          <select id="preset" className="input select" value={settings?.activePreset ?? ''} onChange={(e) => void updateSettings({ activePreset: e.target.value || null })}>
            <option value="">Standard (by file type)</option>
            {presets.map((p) => <option key={p.id} value={p.id}>{p.name}{p.builtin ? '' : ' (yours)'}</option>)}
          </select>
        </div>
        {settings && (
          <NumberField label="Leave files touched in the last (hours)" value={settings.recentHours} min={0} max={72} hint="0 turns it off" onCommit={(n) => void updateSettings({ recentHours: n })} />
        )}
        {folder && <Button icon="arrowClockwise" onClick={() => void startScan()} disabled={busy}>Scan again</Button>}
        {state === 'ready' && <FilterField value={q} onChange={setQ} />}
      </div>
      <p className="hint">
        Only loose files directly in this folder are sorted. Shortcuts and config files stay. Changes to the rules apply on the next scan.
        {plan && plan.recentLeft > 0 && ` ${plural(plan.recentLeft, 'recent file')} left alone.`}
      </p>
      {state !== 'ready' ? (
        <TabPlaceholder state={state} what="files to sort" emptyTitle="Nothing to sort. Nice." />
      ) : (
        <>
          <div className="list-card">
            {rows.length ? (
              <ResultList
                rows={rows}
                rowId={rowId}
                isHeader={isHead}
                selection={sel}
                onSelectAll={selectAll}
                Row={SortRow}
                label="Files to sort"
                onOpen={(r) => !r.head && showDetails({ path: r.m.src, size: r.m.size }, <><dt>Goes to</dt><dd className="path">{r.m.dst}</dd></>)}
                onReveal={(r) => !r.head && reveal(r.m.src)}
              />
            ) : <p className="no-match">No file names match “{q}”.</p>}
          </div>
          <ListFooter selection={sel} noun="file" action={(n) => (n ? `Sort ${plural(n, 'file')}` : 'Sort files')} onAction={sortNow} onSelectAll={selectAll} />
        </>
      )}
    </div>
  );
}
