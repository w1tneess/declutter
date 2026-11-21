import type { ReactNode } from 'react';
import { Button, EmptyState, Icon, ProgressBar } from '../design/components';
import { dateText, humanSize, plural } from '../format';
import { useSelectionVersion, type Selection } from '../state/selection';
import { alert, go, reveal, useApp } from '../state/store';

export function ScreenHeader({ title, subtitle, children }: { title: string; subtitle: ReactNode; children?: ReactNode }) {
  return (
    <header className="screen-head">
      <div>
        <h1 className="title">{title}</h1>
        <p className="subtitle">{subtitle}</p>
      </div>
      {children && <div className="screen-head-actions">{children}</div>}
    </header>
  );
}

export type TabState = 'none' | 'pending' | 'stopped' | 'empty' | 'ready';

/** `count` is undefined until the tab's ScanEvent has arrived. */
export function useTabState(count: number | undefined): TabState {
  const status = useApp((s) => s.status);
  if (count !== undefined) return count ? 'ready' : 'empty';
  if (status === 'scanning' || status === 'cancelling') return 'pending';
  if (status === 'cancelled' || status === 'error') return 'stopped';
  return 'none';
}

/** Everything a tab shows when it has no rows to list. */
export function TabPlaceholder({ state, what, emptyTitle = 'Nothing to clean. Nice.', emptyBody }: { state: TabState; what: string; emptyTitle?: string; emptyBody?: string }) {
  if (state === 'pending')
    return (
      <div className="empty">
        <p className="empty-title">Looking for {what}…</p>
        <p className="empty-body">Results appear here as soon as this part of the scan is done.</p>
        <div className="empty-progress"><ProgressBar value={null} label={`Looking for ${what}`} /></div>
      </div>
    );
  if (state === 'empty') return <EmptyState title={emptyTitle}>{emptyBody}</EmptyState>;
  const stopped = state === 'stopped';
  return (
    <EmptyState icon={stopped ? 'warning' : 'search'} title={stopped ? 'The scan stopped before this part.' : 'Scan a folder on Home first.'} action={<Button variant="primary" onClick={() => go('home')}>Go to Home</Button>}>
      {stopped ? 'Scan again to see these results.' : 'Pick a folder, press Scan, then come back here.'}
    </EmptyState>
  );
}

/** "Selected: N items, X GB", Select all / none and the one action button of a tab. */
export function ListFooter({ selection, noun = 'item', showSize = true, action, onAction, onSelectAll, extra }: {
  selection: Selection;
  noun?: string;
  showSize?: boolean;
  action: string | ((n: number) => string);
  onAction: () => void;
  onSelectAll: () => void;
  extra?: ReactNode;
}) {
  useSelectionVersion(selection);
  const busy = useApp((s) => s.busy || s.status === 'scanning' || s.status === 'cancelling');
  const n = selection.count;
  return (
    <div className="list-foot">
      <Button variant="subtle" onClick={onSelectAll}>Select all</Button>
      <Button variant="subtle" onClick={() => selection.clear()}>Select none</Button>
      <p className="sel-label" aria-live="polite">
        Selected: {plural(n, noun)}
        {showSize && n > 0 && `, ${humanSize(selection.bytes)}`}
      </p>
      {extra}
      <Button variant="primary" large disabled={!n || busy} onClick={onAction}>{typeof action === 'string' ? action : action(n)}</Button>
    </div>
  );
}

export function FilterField({ value, onChange, label = 'Filter by name' }: { value: string; onChange: (v: string) => void; label?: string }) {
  return (
    <div className="filter">
      <Icon name="search" size={16} />
      <input type="search" className="input" data-filter aria-label={label} placeholder={label} value={value} onChange={(e) => onChange(e.target.value)} />
    </div>
  );
}

export function showDetails(f: { path: string; size?: number; mtime?: number }, extra?: ReactNode) {
  void alert(
    f.path.slice(f.path.lastIndexOf('\\') + 1) || f.path,
    <>
      <dl className="details">
        <dt>Location</dt>
        <dd className="path">{f.path}</dd>
        {f.size !== undefined && (<><dt>Size</dt><dd>{humanSize(f.size)}</dd></>)}
        {f.mtime !== undefined && (<><dt>Last changed</dt><dd>{dateText(f.mtime)}</dd></>)}
        {extra}
      </dl>
      <Button icon="open" onClick={() => reveal(f.path)}>Show in Explorer</Button>
    </>,
  );
}

/** Case-insensitive name filter over a model, never the DOM. */
export function matches(q: string, ...texts: string[]): boolean {
  if (!q) return true;
  const n = q.toLowerCase();
  return texts.some((t) => t.toLowerCase().includes(n));
}
