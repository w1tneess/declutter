import { Button, Icon, Switch, Tooltip } from '../design/components';
import type { IconName } from '../design/icons';
import { humanSize, plural, relPath } from '../format';
import { alert, chooseFolder, counts, go, startScan, useApp, type ListTab } from '../state/store';
import { ScreenHeader } from './common';

function Quick() {
  const quick = useApp((s) => s.quick);
  const items: [string, IconName, string | undefined][] = [
    ['Downloads', 'arrowDownload', quick?.downloads],
    ['Desktop', 'desktop', quick?.desktop],
    ['Documents', 'document', quick?.documents],
  ];
  return (
    <div className="quick">
      {items.map(([label, icon, path]) => (
        <Tooltip key={label} text={path ?? label}>
          <Button icon={icon} large disabled={!path} onClick={() => path && void chooseFolder(path)}>{label}</Button>
        </Tooltip>
      ))}
      <Button icon="folderOpen" large onClick={() => void chooseFolder()}>Choose folder…</Button>
    </div>
  );
}

function SummaryCard({ tab, icon, n, caption, hint }: { tab: ListTab; icon: IconName; n: number | undefined; caption: string; hint?: string }) {
  const pending = n === undefined;
  return (
    <button type="button" className="summary" onClick={() => go(tab)}>
      <span className="summary-icon"><Icon name={icon} /></span>
      <span className="summary-num">{pending ? '…' : n.toLocaleString('en-US')}</span>
      <span className="summary-cap">{caption}</span>
      <span className="summary-hint">
        {pending ? 'Still looking…' : n ? (hint ?? 'Click to review') : 'Nothing to clean. Nice.'}
        {!pending && !!n && <Icon name="chevronRight" size={16} />}
      </span>
    </button>
  );
}

export function Home() {
  const folder = useApp((s) => s.folder);
  const status = useApp((s) => s.status);
  const busy = useApp((s) => s.busy);
  const message = useApp((s) => s.message);
  const results = useApp((s) => s.results);
  const first = useApp((s) => !s.scannedOnce);
  const similar = useApp((s) => s.similarImages);
  const scanning = status === 'scanning' || status === 'cancelling';
  const started = scanning || status === 'done' || status === 'cancelled' || Object.keys(results).length > 0;
  const c = counts(results);
  const w = (n: number | undefined, word: string) => (n === 1 ? word : word + 's');
  const skipped = results.skipped ?? [];

  return (
    <div className="screen screen-scroll">
      <ScreenHeader title="Tidy up a folder" subtitle="1. Pick a folder   2. Scan   3. Review each tab. Nothing changes until you confirm." />

      {first && !folder ? (
        <section className="hero" aria-labelledby="hero-title">
          <span className="hero-icon"><Icon name="folderOpen" /></span>
          <h2 id="hero-title" className="hero-title">Drop a folder here, or choose one</h2>
          <p className="hero-body">Drag a folder from File Explorer onto this window, or start with one of these.</p>
          <Quick />
        </section>
      ) : (
        <section className="card folder-card" aria-label="Folder">
          <Quick />
          <p className="folder-line">
            {folder ? <>Folder: <span className="path">{folder}</span></> : 'No folder chosen yet'}
          </p>
        </section>
      )}

      {folder && (
        <div className="scan-row">
          <Button variant="primary" large icon="search" disabled={scanning || busy} onClick={() => void startScan()}>
            {scanning ? 'Scanning…' : started ? 'Scan again' : 'Scan'}
          </Button>
          <div className="scan-option">
            <Switch label="Also look for similar pictures" description="Finds bursts, edits and resized copies. Takes a little longer." checked={similar} onChange={(on) => useApp.setState({ similarImages: on })} />
          </div>
        </div>
      )}

      {status === 'error' && (
        <div className="banner banner-error" role="alert">
          <Icon name="errorCircle" />
          <p>{message}</p>
          <Button onClick={() => void chooseFolder()}>Choose another folder</Button>
        </div>
      )}

      {started && (
        <div className="summary-grid">
          <SummaryCard tab="sort" icon="arrowSort" n={c.sort} caption={`${w(c.sort, 'file')} to sort`} />
          <SummaryCard tab="duplicates" icon="copy" n={c.duplicates} caption={`${w(c.duplicates, 'duplicate')}${c.reclaim !== undefined ? ` · ${humanSize(c.reclaim)} reclaimable` : ''}`} hint={c.redownloads ? `Includes ${plural(c.redownloads, 'possible re-download')}` : undefined} />
          {(similar || results.similar) && <SummaryCard tab="similar" icon="imageMultiple" n={c.similar} caption={`similar ${w(c.similar, 'picture')}`} />}
          <SummaryCard tab="largeOld" icon="storage" n={c.largeOld} caption={`large, old or leftover ${w(c.largeOld, 'file')}`} />
          <SummaryCard tab="empty" icon="folder" n={c.empty} caption={`empty ${w(c.empty, 'folder')}`} />
        </div>
      )}

      {skipped.length > 0 && (
        <p className="hint skipped">
          Skipped {plural(skipped.length, 'app/project folder')} to keep them intact.{' '}
          <Button variant="subtle" onClick={() => void alert('Skipped folders', <>
            <p>These look like programs or code projects (a .git or package.json, or an .exe next to a .dll), so Declutter did not look inside them:</p>
            <ul className="plain-list">{skipped.slice(0, 15).map((p) => <li key={p} className="path">{relPath(p, folder)}</li>)}</ul>
            {skipped.length > 15 && <p>…and {skipped.length - 15} more.</p>}
          </>)}>Show them</Button>
        </p>
      )}
    </div>
  );
}
