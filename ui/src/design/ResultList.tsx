import { useVirtualizer } from '@tanstack/react-virtual';
import { memo, useId, useMemo, useRef, useState, type ComponentType, type KeyboardEvent, type MouseEvent } from 'react';
import { useSelectionVersion, type Selection } from '../state/selection';
import { Icon } from './components';

export const ROW_H = 44;

export interface RowProps<R> {
  row: R;
  selected: boolean;
}

interface Props<R> {
  rows: readonly R[];
  rowId: (r: R) => string;
  /** Section headings: shown, skipped by the keyboard and hidden from screen readers (rows carry their own labels). */
  isHeader?: (r: R) => boolean;
  /** False for rows that can never be ticked (the kept copy). */
  selectable?: (r: R) => boolean;
  selection: Selection;
  /** Where tab rules (keep one copy) apply; defaults to ticking exactly what was asked. */
  onToggle?: (ids: string[], on: boolean) => void;
  onSelectAll: () => void;
  Row: ComponentType<RowProps<R>>;
  label: string;
  onOpen?: (r: R) => void;
  onReveal?: (r: R) => void;
  /** Clicks on `[data-action]` inside a row, and single-key shortcuts mapped to the same actions. */
  onAction?: (action: string, r: R) => void;
  keyActions?: Record<string, string>;
  /** Fixed heights (no measuring, so no layout thrash); headers may be taller to fit an explanation. */
  rowHeight?: (r: R) => number;
}

interface ShellProps {
  row: unknown;
  domId: string;
  index: number;
  start: number;
  size: number;
  selected: boolean;
  active: boolean;
  header: boolean;
  disabled: boolean;
  pos: number;
  setSize: number;
  Row: ComponentType<RowProps<unknown>>;
}

/** Memoized: a toggle re-renders only the row whose `selected` changed. */
const RowShell = memo(function RowShell({ row, domId, index, start, size, selected, active, header, disabled, pos, setSize, Row }: ShellProps) {
  const style = { transform: `translateY(${start}px)`, height: size };
  if (header)
    return (
      <div className="rl-head" aria-hidden="true" style={style}>
        <Row row={row} selected={false} />
      </div>
    );
  return (
    <div
      role="option"
      id={domId}
      data-index={index}
      aria-selected={selected}
      aria-disabled={disabled || undefined}
      aria-posinset={pos}
      aria-setsize={setSize}
      className={`rl-row${selected ? ' is-selected' : ''}${active ? ' is-active' : ''}`}
      style={style}
    >
      {disabled ? <span className="box-spacer" /> : (
        <span className={`box box-visual${selected ? ' is-on' : ''}`}>
          <Icon name="checkmark" size={14} />
        </span>
      )}
      <Row row={row} selected={selected} />
    </div>
  );
});

/**
 * Virtualized, keyboard-operable checklist (listbox with aria-activedescendant). DOM size stays constant whatever
 * the row count; ticking works on the Selection model, never on DOM nodes.
 * Keys: arrows/Home/End/PageUp/PageDown move, Space ticks, Ctrl+A selects all, Enter opens details. Shift-click
 * ticks a range; double-click shows the file in Explorer.
 */
export function ResultList<R>(p: Props<R>) {
  const { rows, rowId, isHeader, selectable, selection } = p;
  useSelectionVersion(selection);
  const ref = useRef<HTMLDivElement>(null);
  const baseId = useId();
  const [active, setActive] = useState(-1);
  const anchor = useRef(-1);

  // Position among real rows (headers excluded) for aria-posinset; one pass, only when the rows change.
  const pos = useMemo(() => {
    const out = new Int32Array(rows.length);
    let n = 0;
    rows.forEach((r, i) => (out[i] = isHeader?.(r) ? 0 : ++n));
    return { out, total: n };
  }, [rows, isHeader]);

  const v = useVirtualizer({
    count: rows.length,
    getScrollElement: () => ref.current,
    estimateSize: (i) => (p.rowHeight ? p.rowHeight(rows[i] as R) : ROW_H),
    overscan: 8,
    initialRect: { width: 800, height: 600 },
  });

  const toggle = p.onToggle ?? ((ids: string[], on: boolean) => selection.set(ids, on));
  const canTick = (r: R) => !isHeader?.(r) && (selectable?.(r) ?? true);
  const cur = active >= 0 && active < rows.length && !isHeader?.(rows[active] as R) ? active : firstRow(rows, isHeader, 0, 1);

  const moveTo = (i: number) => {
    setActive(i);
    v.scrollToIndex(i, { align: 'auto' });
  };

  const onKeyDown = (e: KeyboardEvent) => {
    const r = rows[cur];
    const step = (from: number, dir: 1 | -1) => firstRow(rows, isHeader, from, dir);
    let next = -1;
    if (e.key === 'ArrowDown') next = step(cur + 1, 1);
    else if (e.key === 'ArrowUp') next = step(cur - 1, -1);
    else if (e.key === 'Home') next = step(0, 1);
    else if (e.key === 'End') next = step(rows.length - 1, -1);
    else if (e.key === 'PageDown') next = step(Math.min(rows.length - 1, cur + 10), 1);
    else if (e.key === 'PageUp') next = step(Math.max(0, cur - 10), -1);
    else if (e.key === ' ' && r !== undefined) {
      if (canTick(r)) toggle([rowId(r)], !selection.has(rowId(r)));
      anchor.current = cur;
    } else if ((e.key === 'a' || e.key === 'A') && (e.ctrlKey || e.metaKey)) p.onSelectAll();
    else if (e.key === 'Enter' && r !== undefined) p.onOpen?.(r);
    else if (r !== undefined && !e.ctrlKey && !e.altKey && p.keyActions?.[e.key.toLowerCase()]) p.onAction?.(p.keyActions[e.key.toLowerCase()] as string, r);
    else return;
    e.preventDefault();
    if (next >= 0) moveTo(next);
  };

  const rowAt = (e: MouseEvent) => {
    const el = (e.target as HTMLElement).closest<HTMLElement>('[data-index]');
    return el ? Number(el.dataset.index) : -1;
  };

  const onClick = (e: MouseEvent) => {
    const i = rowAt(e);
    const r = rows[i];
    if (r === undefined) return;
    ref.current?.focus({ preventScroll: true });
    setActive(i);
    const action = (e.target as HTMLElement).closest<HTMLElement>('[data-action]')?.dataset.action;
    if (action) return p.onAction?.(action, r);
    if (!canTick(r)) return;
    const on = !selection.has(rowId(r));
    if (e.shiftKey && anchor.current >= 0) {
      const [a, b] = anchor.current < i ? [anchor.current, i] : [i, anchor.current];
      toggle(rows.slice(a, b + 1).filter(canTick).map(rowId), on);
    } else toggle([rowId(r)], on);
    anchor.current = i;
  };

  const onDoubleClick = (e: MouseEvent) => {
    const r = rows[rowAt(e)];
    if (r !== undefined && !(e.target as HTMLElement).closest('[data-action]')) p.onReveal?.(r);
  };

  const Row = p.Row as ComponentType<RowProps<unknown>>;
  return (
    <div
      ref={ref}
      className="rl"
      role="listbox"
      aria-multiselectable="true"
      aria-label={p.label}
      aria-activedescendant={cur >= 0 ? `${baseId}-${cur}` : undefined}
      tabIndex={0}
      onKeyDown={onKeyDown}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
    >
      <div className="rl-inner" style={{ height: v.getTotalSize() }}>
        {v.getVirtualItems().map((vi) => {
          const r = rows[vi.index] as R;
          const id = rowId(r);
          const header = isHeader?.(r) ?? false;
          return (
            <RowShell
              key={vi.key}
              row={r}
              domId={`${baseId}-${vi.index}`}
              index={vi.index}
              start={vi.start}
              size={vi.size}
              selected={!header && selection.has(id)}
              active={vi.index === cur}
              header={header}
              disabled={!header && !(selectable?.(r) ?? true)}
              pos={pos.out[vi.index] ?? 0}
              setSize={pos.total}
              Row={Row}
            />
          );
        })}
      </div>
    </div>
  );
}

function firstRow<R>(rows: readonly R[], isHeader: ((r: R) => boolean) | undefined, from: number, dir: 1 | -1): number {
  for (let i = from; i >= 0 && i < rows.length; i += dir) if (!isHeader?.(rows[i] as R)) return i;
  // Nothing further that way: stay on the nearest real row in the other direction.
  for (let i = from - dir; i >= 0 && i < rows.length; i -= dir) if (!isHeader?.(rows[i] as R)) return i;
  return -1;
}
