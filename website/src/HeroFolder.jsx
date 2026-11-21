import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { CATEGORIES, SAMPLE_FILES, formatSize, plan } from './data';

const ORDER = [
  'Images', 'Images/Screenshots', 'Images/Chat',
  ...CATEGORIES.map(([name]) => name).filter(n => n !== 'Images'),
  'Other',
];

const planned = SAMPLE_FILES.map(f => ({ ...f, ...plan(f) }));

// Before: the folder as it is. After: files grouped by destination, the ones left alone last.
function buildRows(tidy) {
  if (!tidy) return planned;
  const rank = f => (f.dest ? ORDER.indexOf(f.dest) : ORDER.length);
  return [...planned].sort((a, b) => rank(a) - rank(b));
}

const FolderGlyph = () => (
  <svg className="glyph glyph-folder" viewBox="0 0 20 20" aria-hidden="true">
    <path d="M2 5.5A1.5 1.5 0 0 1 3.5 4h3.6c.4 0 .8.16 1.06.44L9 5.5h7.5A1.5 1.5 0 0 1 18 7v7.5a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 2 14.5v-9Z" />
  </svg>
);

const FileGlyph = () => (
  <svg className="glyph glyph-file" viewBox="0 0 20 20" aria-hidden="true">
    <path d="M5 2.5h6l4 4v10a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1v-13a1 1 0 0 1 1-1Z" />
    <path d="M11 2.5v4h4" />
  </svg>
);

export default function HeroFolder() {
  const reduced = typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  const [tidy, setTidy] = useState(reduced);
  const rowEls = useRef(new Map());
  const before = useRef(null);

  const rows = buildRows(tidy);
  const toSort = planned.filter(f => f.dest).length;

  const flip = next => {
    const tops = new Map();
    rowEls.current.forEach((el, key) => tops.set(key, el.getBoundingClientRect().top));
    before.current = tops;
    setTidy(next);
  };

  // Once, shortly after load: the files sort themselves. Reduced motion starts already sorted.
  useEffect(() => {
    if (reduced) return undefined;
    const t = setTimeout(() => flip(true), 1200);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useLayoutEffect(() => {
    const tops = before.current;
    before.current = null;
    if (!tops || reduced) return;
    let i = 0;
    rowEls.current.forEach((el, key) => {
      const was = tops.get(key);
      if (was === undefined) return;
      const dy = was - el.getBoundingClientRect().top;
      if (!dy) return;
      el.animate(
        [{ transform: `translateY(${dy}px)` }, { transform: 'none' }],
        { duration: 620, delay: (i++ % 14) * 28, easing: 'cubic-bezier(.2,.8,.2,1)', fill: 'backwards' },
      );
    });
  }, [tidy, reduced]);

  return (
    <figure className="folder-window" aria-label="A simulated Downloads folder being sorted">
      <div className="folder-titlebar">
        <FolderGlyph />
        <span className="folder-title">Downloads</span>
        <span className="folder-count">{SAMPLE_FILES.length} items</span>
      </div>

      <ul className="folder-list" aria-live="polite">
        {rows.map(f => (
          <li
            key={f.name}
            ref={el => (el ? rowEls.current.set(f.name, el) : rowEls.current.delete(f.name))}
            className="folder-row"
          >
            {tidy && f.dest ? <FolderGlyph /> : <FileGlyph />}
            <span className="folder-name mono">{f.name}</span>
            <span className={`folder-meta ${tidy && f.dest ? 'is-dest' : ''}`}>
              {tidy ? (f.dest ? `${f.dest}/` : f.stays) : formatSize(f.mb)}
            </span>
          </li>
        ))}
      </ul>

      <figcaption className="folder-foot">
        <span>
          {tidy ? `${toSort} files sorted, ${SAMPLE_FILES.length - toSort} left alone` : `${toSort} files to sort`}
        </span>
        <button type="button" className="link-button" onClick={() => flip(!tidy)}>
          {tidy ? 'Undo last sort' : 'Sort this folder'}
        </button>
      </figcaption>
      <p className="folder-note">A simulation. Nothing on your computer is touched.</p>
    </figure>
  );
}
