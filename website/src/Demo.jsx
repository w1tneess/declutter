import { useEffect, useMemo, useRef, useState } from 'react';
import {
  SAMPLE_DUPLICATES, SAMPLE_EMPTY, SAMPLE_FILES, SAMPLE_REDOWNLOADS, formatSize, plan,
} from './data';

const TABS = [
  ['sort', 'Sort'],
  ['dupes', 'Duplicates'],
  ['large', 'Large & old'],
  ['empty', 'Empty folders'],
];

const INSTALLERS = ['.exe', '.msi', '.iso'];
const leftoverReason = f => {
  const ext = f.name.slice(f.name.lastIndexOf('.')).toLowerCase();
  if (f.unpacked) return `Likely leftover: unpacked into ${f.unpacked}`;
  if (INSTALLERS.includes(ext) && f.ageDays > 7) return 'Likely leftover: installer over 7 days old';
  return null;
};

const fromBrowserFile = f => {
  const ms = Date.now() - (f.lastModified || Date.now());
  return {
    name: f.name,
    mb: f.size / (1024 * 1024),
    ageDays: Math.max(0, Math.floor(ms / 86400000)),
    hoursOld: Math.max(0, ms / 3600000),
  };
};

export default function Demo() {
  const [tab, setTab] = useState('sort');
  const [files, setFiles] = useState(SAMPLE_FILES);
  const [custom, setCustom] = useState(false);
  const [binned, setBinned] = useState([]); // { name, mb }
  const [sorted, setSorted] = useState(false);
  const [skipSort, setSkipSort] = useState(() => new Set());
  const [keep, setKeep] = useState({ q3: 0, drone: 0 });
  const [dupOff, setDupOff] = useState(() => new Set()); // copies the user unticked, "groupId:index"
  const [redl, setRedl] = useState(() => new Set()); // re-downloads the user ticked
  const [minMb, setMinMb] = useState(100);
  const [olderDays, setOlderDays] = useState(30);
  const [largeTicked, setLargeTicked] = useState(() => new Set());
  const [emptyOff, setEmptyOff] = useState(() => new Set());
  const [gone, setGone] = useState({ dupes: new Set(), empty: new Set() });
  const [dialog, setDialog] = useState(null);
  const [status, setStatus] = useState('');
  const dialogRef = useRef(null);

  useEffect(() => {
    const d = dialogRef.current;
    if (!d) return;
    if (dialog && !d.open) d.showModal();
    if (!dialog && d.open) d.close();
  }, [dialog]);

  const binnedNames = useMemo(() => new Set(binned.map(b => b.name)), [binned]);
  const visible = files.filter(f => !binnedNames.has(f.name));
  const binMb = binned.reduce((sum, b) => sum + b.mb, 0);

  const ask = (title, body, confirm, run) => setDialog({ title, body, confirm, run });
  const finish = message => { setDialog(null); setStatus(message); };

  // ---- Sort
  const rows = visible.map(f => ({ f, ...plan(f) }));
  const movable = rows.filter(r => r.dest);
  const toMove = movable.filter(r => !skipSort.has(r.f.name));
  const toggleSkip = name => setSkipSort(prev => {
    const next = new Set(prev);
    if (next.has(name)) next.delete(name); else next.add(name);
    return next;
  });

  // ---- Duplicates
  const groups = SAMPLE_DUPLICATES
    .map(g => ({ ...g, copies: g.copies.map((c, i) => ({ ...c, i })).filter(c => !gone.dupes.has(`${g.id}:${c.i}`)) }))
    .filter(g => g.copies.length > 1);
  const keeperOf = g => (g.copies.some(c => c.i === keep[g.id]) ? keep[g.id] : g.copies[0].i);
  const dupTicked = groups.flatMap(g => g.copies
    .filter(c => c.i !== keeperOf(g) && !dupOff.has(`${g.id}:${c.i}`))
    .map(c => ({ g, c })));
  const redlShown = SAMPLE_REDOWNLOADS.filter(r => !binnedNames.has(r.name));
  const redlTicked = redlShown.filter(r => redl.has(r.id));
  const dupCount = dupTicked.length + redlTicked.length;

  // ---- Large and old: both filters must match; 0 turns a filter off
  const large = visible
    .filter(f => (!minMb || f.mb >= minMb) && (!olderDays || f.ageDays >= olderDays))
    .sort((a, b) => b.mb - a.mb);
  const largeCount = large.filter(f => largeTicked.has(f.name)).length;

  // ---- Empty folders
  const emptyLeft = SAMPLE_EMPTY.filter(p => !gone.empty.has(p));
  const emptyTicked = emptyLeft.filter(p => !emptyOff.has(p));

  const toggleIn = (setter, value) => setter(prev => {
    const next = new Set(prev);
    if (next.has(value)) next.delete(value); else next.add(value);
    return next;
  });

  const doSort = () => ask(
    `Move ${toMove.length} ${toMove.length === 1 ? 'file' : 'files'}?`,
    'They go into folders inside this one. Nothing is overwritten, and Undo last sort puts them back.',
    `Move ${toMove.length} files`,
    () => { setSorted(true); finish(`Moved ${toMove.length} files. Undo last sort puts them back.`); },
  );

  const doDupes = () => ask(
    `Send ${dupCount} ${dupCount === 1 ? 'file' : 'files'} to the Recycle Bin?`,
    'You can restore them from the Windows Recycle Bin. At least one copy of each file stays.',
    'Send to Recycle Bin',
    () => {
      setGone(prev => ({ ...prev, dupes: new Set([...prev.dupes, ...dupTicked.map(({ g, c }) => `${g.id}:${c.i}`)]) }));
      setBinned(prev => [
        ...prev,
        ...dupTicked.map(({ g }) => ({ name: `${g.name} (copy)`, mb: g.mb })),
        ...redlTicked.map(r => ({ name: r.name, mb: r.mb })),
      ]);
      finish(`Sent ${dupCount} files to the Recycle Bin.`);
    },
  );

  const doLarge = () => {
    const chosen = large.filter(f => largeTicked.has(f.name));
    ask(
      `Send ${chosen.length} ${chosen.length === 1 ? 'file' : 'files'} to the Recycle Bin?`,
      'You can restore them from the Windows Recycle Bin.',
      'Send to Recycle Bin',
      () => {
        setBinned(prev => [...prev, ...chosen.map(f => ({ name: f.name, mb: f.mb }))]);
        setLargeTicked(new Set());
        finish(`Sent ${chosen.length} files to the Recycle Bin.`);
      },
    );
  };

  const doEmpty = () => ask(
    `Send ${emptyTicked.length} ${emptyTicked.length === 1 ? 'folder' : 'folders'} to the Recycle Bin?`,
    'Deepest folders go first. You can restore them from the Windows Recycle Bin.',
    'Send to Recycle Bin',
    () => {
      setGone(prev => ({ ...prev, empty: new Set([...prev.empty, ...emptyTicked]) }));
      finish(`Sent ${emptyTicked.length} empty folders to the Recycle Bin.`);
    },
  );

  const reset = () => {
    setFiles(SAMPLE_FILES); setCustom(false); setBinned([]); setSorted(false); setSkipSort(new Set());
    setKeep({ q3: 0, drone: 0 }); setDupOff(new Set()); setRedl(new Set()); setMinMb(100); setOlderDays(30);
    setLargeTicked(new Set()); setEmptyOff(new Set()); setGone({ dupes: new Set(), empty: new Set() });
    setStatus('Demo reset.');
  };

  const onPick = e => {
    const picked = Array.from(e.target.files || []).slice(0, 300);
    if (!picked.length) return;
    setFiles(picked.map(fromBrowserFile));
    setCustom(true); setSorted(false); setSkipSort(new Set()); setLargeTicked(new Set()); setBinned([]);
    setStatus(`Loaded ${picked.length} file names. Duplicates and Empty folders keep the sample data.`);
    e.target.value = '';
  };

  return (
    <div className="demo">
      <div className="demo-tabs" role="tablist" aria-label="Declutter tools">
        {TABS.map(([id, label]) => (
          <button
            key={id}
            id={`tab-${id}`}
            role="tab"
            type="button"
            aria-selected={tab === id}
            aria-controls="demo-panel"
            className="demo-tab"
            onClick={() => setTab(id)}
          >
            {label}
          </button>
        ))}
      </div>

      <div className="demo-panel" id="demo-panel" role="tabpanel" aria-labelledby={`tab-${tab}`}>
        {tab === 'sort' && (
          <>
            <p className="demo-lead">
              Loose files at the top of the folder go into one folder per type. Untick any file to keep it where it is.
            </p>
            <ul className="demo-list">
              {rows.map(({ f, dest, stays }) => (
                <li key={f.name} className={`demo-row ${stays ? 'is-muted' : ''}`}>
                  <input
                    type="checkbox"
                    id={`s-${f.name}`}
                    disabled={!dest || sorted}
                    checked={Boolean(dest) && !skipSort.has(f.name)}
                    onChange={() => toggleSkip(f.name)}
                  />
                  <label htmlFor={`s-${f.name}`} className="mono demo-name">{f.name}</label>
                  <span className="demo-size">{formatSize(f.mb)}</span>
                  <span className="demo-dest">
                    {stays ?? (sorted && !skipSort.has(f.name) ? `Moved to ${dest}/` : `${dest}/`)}
                  </span>
                </li>
              ))}
            </ul>
            <div className="demo-actions">
              <span className="demo-selected">{toMove.length} of {movable.length} files will move</span>
              {sorted ? (
                <button type="button" className="btn btn-quiet" onClick={() => { setSorted(false); setStatus('Undid the last sort.'); }}>
                  Undo last sort
                </button>
              ) : (
                <button type="button" className="btn btn-primary" disabled={!toMove.length} onClick={doSort}>
                  Move {toMove.length} files
                </button>
              )}
            </div>
          </>
        )}

        {tab === 'dupes' && (
          <>
            <p className="demo-lead">
              Files with identical content. The oldest copy is kept; choose a different one if you prefer. At least one copy always stays.
            </p>
            {groups.length === 0 && redlShown.length === 0 && <p className="demo-empty">No duplicates left. Reset the demo to see them again.</p>}
            {groups.map(g => (
              <fieldset key={g.id} className="dupe-group">
                <legend><span className="mono">{g.name}</span> · {g.copies.length} copies · {formatSize(g.mb)} each</legend>
                {g.copies.map(c => {
                  const isKeeper = c.i === keeperOf(g);
                  const key = `${g.id}:${c.i}`;
                  return (
                    <div key={key} className="dupe-copy">
                      <input
                        type="radio"
                        name={`keep-${g.id}`}
                        id={`k-${key}`}
                        checked={isKeeper}
                        onChange={() => setKeep(prev => ({ ...prev, [g.id]: c.i }))}
                      />
                      <label htmlFor={`k-${key}`}>Keep</label>
                      <input
                        type="checkbox"
                        id={`d-${key}`}
                        disabled={isKeeper}
                        checked={!isKeeper && !dupOff.has(key)}
                        onChange={() => toggleIn(setDupOff, key)}
                        aria-label={`Send ${c.path} to the Recycle Bin`}
                      />
                      <span className="mono demo-name">{c.path}</span>
                      <span className="demo-size">{c.modified}</span>
                    </div>
                  );
                })}
              </fieldset>
            ))}
            {redlShown.length > 0 && (
              <fieldset className="dupe-group">
                <legend>Possible re-downloads · never ticked for you</legend>
                {redlShown.map(r => (
                  <div key={r.id} className="dupe-copy">
                    <input
                      type="checkbox"
                      id={`r-${r.id}`}
                      checked={redl.has(r.id)}
                      onChange={() => toggleIn(setRedl, r.id)}
                    />
                    <label htmlFor={`r-${r.id}`} className="mono demo-name">{r.name}</label>
                    <span className="demo-size">{r.note}; {r.original} stays</span>
                  </div>
                ))}
              </fieldset>
            )}
            <div className="demo-actions">
              <span className="demo-selected">{dupCount} files selected</span>
              <button type="button" className="btn btn-primary" disabled={!dupCount} onClick={doDupes}>
                Send to Recycle Bin
              </button>
            </div>
          </>
        )}

        {tab === 'large' && (
          <>
            <p className="demo-lead">
              Big or long-untouched files. Nothing is selected for you. Both filters must match; 0 turns a filter off.
            </p>
            <div className="demo-filters">
              <label>
                Bigger than (MB)
                <input type="number" min="0" value={minMb} onChange={e => setMinMb(Math.max(0, Number(e.target.value) || 0))} />
              </label>
              <label>
                Older than (days)
                <input type="number" min="0" value={olderDays} onChange={e => setOlderDays(Math.max(0, Number(e.target.value) || 0))} />
              </label>
            </div>
            {large.length === 0 ? (
              <p className="demo-empty">Nothing matches both filters.</p>
            ) : (
              <ul className="demo-list">
                {large.map(f => (
                  <li key={f.name} className="demo-row">
                    <input
                      type="checkbox"
                      id={`l-${f.name}`}
                      checked={largeTicked.has(f.name)}
                      onChange={() => toggleIn(setLargeTicked, f.name)}
                    />
                    <label htmlFor={`l-${f.name}`} className="mono demo-name">{f.name}</label>
                    <span className="demo-size">{formatSize(f.mb)} · {f.ageDays} days old</span>
                    <span className="demo-dest">{leftoverReason(f)}</span>
                  </li>
                ))}
              </ul>
            )}
            <div className="demo-actions">
              <span className="demo-selected">{largeCount} files selected</span>
              <button type="button" className="btn btn-primary" disabled={!largeCount} onClick={doLarge}>
                Send to Recycle Bin
              </button>
            </div>
          </>
        )}

        {tab === 'empty' && (
          <>
            <p className="demo-lead">
              Folders with no files at any depth. desktop.ini and Thumbs.db do not count. The deepest folders go first.
            </p>
            {emptyLeft.length === 0 ? (
              <p className="demo-empty">Nothing to clean. Nice.</p>
            ) : (
              <ul className="demo-list">
                {emptyLeft.map(p => (
                  <li key={p} className="demo-row">
                    <input type="checkbox" id={`e-${p}`} checked={!emptyOff.has(p)} onChange={() => toggleIn(setEmptyOff, p)} />
                    <label htmlFor={`e-${p}`} className="mono demo-name">{p}</label>
                  </li>
                ))}
              </ul>
            )}
            <div className="demo-actions">
              <span className="demo-selected">{emptyTicked.length} folders selected</span>
              <button type="button" className="btn btn-primary" disabled={!emptyTicked.length} onClick={doEmpty}>
                Send to Recycle Bin
              </button>
            </div>
          </>
        )}
      </div>

      <div className="demo-foot">
        <p className="demo-status" role="status">
          {status || (binned.length || gone.dupes.size || gone.empty.size
            ? ''
            : 'This is a simulation in your browser. It moves no real files.')}
          {binned.length > 0 && ` In the demo's Recycle Bin: ${binned.length} ${binned.length === 1 ? 'item' : 'items'}, ${formatSize(binMb)}. In the real app you restore items from the Windows Recycle Bin.`}
        </p>
        <div className="demo-tools">
          <label className="link-button demo-pick">
            Use names from your own files
            <input type="file" multiple onChange={onPick} />
          </label>
          <button type="button" className="link-button" onClick={reset}>Reset demo</button>
        </div>
        {custom && <p className="demo-fineprint">Your browser reads names, sizes and dates only. Nothing leaves this page.</p>}
      </div>

      <dialog ref={dialogRef} className="confirm" onClose={() => setDialog(null)} aria-labelledby="confirm-title">
        {dialog && (
          <form method="dialog">
            <h3 id="confirm-title">{dialog.title}</h3>
            <p>{dialog.body}</p>
            <div className="confirm-actions">
              <button type="button" className="btn btn-quiet" onClick={() => setDialog(null)}>Cancel</button>
              <button type="button" className="btn btn-primary" onClick={dialog.run}>{dialog.confirm}</button>
            </div>
          </form>
        )}
      </dialog>
    </div>
  );
}
