import { memo, useDeferredValue, useMemo, useState } from 'react';
import { api } from '../api/client';
import { Button, Icon } from '../design/components';
import { ResultList, type RowProps } from '../design/ResultList';
import { plural, relPath } from '../format';
import { act, binWording, exportCsv, reveal, unpinned, useApp } from '../state/store';
import { FilterField, ListFooter, matches, ScreenHeader, showDetails, TabPlaceholder, useTabState } from './common';

const FolderRow = memo(function FolderRow({ row }: RowProps<string>) {
  const folder = useApp((s) => s.folder);
  return (
    <>
      <Icon name="folder" size={16} />
      <span className="row-main">{relPath(row, folder)}</span>
    </>
  );
});

const rowId = (r: string) => r;

export function EmptyFolders() {
  const dirs = useApp((s) => s.results.emptyDirs);
  const sel = useApp((s) => s.sel.empty);
  const state = useTabState(dirs?.length);
  const [q, setQ] = useState('');
  const query = useDeferredValue(q);
  const rows = useMemo(() => (dirs ?? []).filter((d) => matches(query, d)), [dirs, query]);
  const selectAll = () => sel.set(rows, true);

  const remove = () => {
    const paths = unpinned(sel.list());
    void act({
      title: 'Send to Recycle Bin?',
      body: binWording(paths.length),
      confirmLabel: `Send ${plural(paths.length, 'item')}`,
      run: (root) => api.trashEmptyDirs(root, paths),
      done: (k) => `Sent ${plural(k, 'item')} to the Recycle Bin. Restore them from the Bin if needed.`,
    });
  };

  return (
    <div className="screen">
      <ScreenHeader title="Empty folders" subtitle="Folders with no files inside, however deep. Double-click one to see it in Explorer.">
        <Button icon="arrowExportLtr" disabled={!rows.length} onClick={() => void exportCsv('empty-folders', { kind: 'emptyDirs', dirs: rows })}>Export CSV</Button>
      </ScreenHeader>
      {state !== 'ready' ? (
        <TabPlaceholder state={state} what="empty folders" />
      ) : (
        <>
          <div className="toolbar"><FilterField value={q} onChange={setQ} /></div>
          <div className="list-card">
            {rows.length ? (
              <ResultList rows={rows} rowId={rowId} selection={sel} onSelectAll={selectAll} Row={FolderRow} label="Empty folders" onOpen={(r) => showDetails({ path: r })} onReveal={reveal} />
            ) : <p className="no-match">No folder names match “{q}”.</p>}
          </div>
          <ListFooter selection={sel} noun="folder" showSize={false} action="Send to Recycle Bin" onAction={remove} onSelectAll={selectAll} />
        </>
      )}
    </div>
  );
}
