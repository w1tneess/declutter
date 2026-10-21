import { useEffect } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { api } from './api/client';
import { Button, ProgressBar } from './design/components';
import { DialogHost, Toasts } from './design/Dialog';
import { Sidebar, type NavItem } from './design/Sidebar';
import { eta } from './format';
import { Duplicates } from './screens/Duplicates';
import { EmptyFolders } from './screens/EmptyFolders';
import { Home } from './screens/Home';
import { LargeOld } from './screens/LargeOld';
import { Settings } from './screens/Settings';
import { Similar } from './screens/Similar';
import { Sort } from './screens/Sort';
import { cancelScan, chooseFolder, counts, go, init, useApp, type Page } from './state/store';

const SCREENS: Record<Page, () => React.JSX.Element> = {
  home: Home, sort: Sort, duplicates: Duplicates, similar: Similar, largeOld: LargeOld, empty: EmptyFolders, settings: Settings,
};
const ORDER: Page[] = ['home', 'sort', 'duplicates', 'similar', 'largeOld', 'empty', 'settings'];

export function App() {
  const page = useApp((s) => s.page);
  const results = useApp((s) => s.results);
  const modal = useApp((s) => s.dialog !== null);

  useEffect(() => {
    void init();
    const off = api.onFolderDrop((p) => void chooseFolder(p));
    const onKey = (e: KeyboardEvent) => {
      if (!e.ctrlKey || e.altKey || useApp.getState().dialog) return;
      const n = Number(e.key);
      if (n >= 1 && n <= ORDER.length) {
        e.preventDefault();
        go(ORDER[n - 1] as Page);
      } else if (e.key === 'f' || e.key === 'F') {
        const f = document.querySelector<HTMLInputElement>('[data-filter]');
        if (f) {
          e.preventDefault();
          f.focus();
        }
      }
    };
    window.addEventListener('keydown', onKey);
    return () => {
      off();
      window.removeEventListener('keydown', onKey);
    };
  }, []);

  useEffect(() => {
    document.getElementById('main')?.focus({ preventScroll: true });
  }, [page]);

  const c = counts(results);
  const items: NavItem<Page>[] = [
    { id: 'home', label: 'Home', icon: 'home', iconOn: 'homeFilled' },
    { id: 'sort', label: 'Sort by type', icon: 'arrowSort', iconOn: 'arrowSortFilled', count: c.sort },
    { id: 'duplicates', label: 'Duplicates', icon: 'copy', iconOn: 'copyFilled', count: c.duplicates },
    { id: 'similar', label: 'Similar images', icon: 'imageMultiple', iconOn: 'imageMultipleFilled', count: c.similar },
    { id: 'largeOld', label: 'Large & old', icon: 'storage', iconOn: 'storageFilled', count: c.largeOld },
    { id: 'empty', label: 'Empty folders', icon: 'folder', iconOn: 'folderFilled', count: c.empty },
  ];
  const Screen = SCREENS[page];
  return (
    <>
      <div className="app" inert={modal}>
        <Sidebar items={items} current={page} onSelect={go} footer={{ id: 'settings', label: 'Settings', icon: 'settings', iconOn: 'settingsFilled' }} />
        <main id="main" className="page" tabIndex={-1} aria-label={items.find((i) => i.id === page)?.label ?? 'Settings'}>
          <Screen />
        </main>
        <StatusBar />
      </div>
      <Toasts />
      <DialogHost />
    </>
  );
}

function StatusBar() {
  const { status, progress, message, busy } = useApp(useShallow((s) => ({ status: s.status, progress: s.progress, message: s.message, busy: s.busy })));
  const running = status === 'scanning' || status === 'cancelling';
  const value = progress?.total ? progress.done / progress.total : null;
  return (
    <footer className="statusbar">
      <p className="status-text" role="status" aria-live="polite">{busy ? 'Working…' : message}</p>
      {running && progress?.etaSecs != null && <span className="status-eta">{eta(progress.etaSecs)}</span>}
      {running && (
        <>
          <ProgressBar value={value} label="Scan progress" />
          <Button onClick={() => void cancelScan()} disabled={status === 'cancelling'}>Cancel</Button>
        </>
      )}
    </footer>
  );
}
