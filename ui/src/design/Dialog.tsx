import { useEffect, useId, useRef } from 'react';
import { closeDialog, dismissToast, useApp } from '../state/store';
import { Button, Icon } from './components';

const FOCUSABLE = 'button:not([disabled]), [href], input:not([disabled]), select, textarea, [tabindex]:not([tabindex="-1"])';

/** Modal dialog from the store: traps Tab, closes on Esc, and gives focus back to whatever had it before. */
export function DialogHost() {
  const d = useApp((s) => s.dialog);
  const ref = useRef<HTMLDivElement>(null);
  const id = useId();

  useEffect(() => {
    const el = ref.current;
    if (!d || !el) return;
    const before = document.activeElement as HTMLElement | null;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        closeDialog(false);
      } else if (e.key === 'Tab') {
        const items = [...el.querySelectorAll<HTMLElement>(FOCUSABLE)];
        const first = items[0];
        const last = items[items.length - 1];
        if (!first || !last) return;
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };
    // The confirm button gets focus, except for alerts with a single OK there is nothing else to pick anyway.
    el.querySelector<HTMLElement>('[data-autofocus]')?.focus();
    el.addEventListener('keydown', onKeyDown);
    return () => {
      el.removeEventListener('keydown', onKeyDown);
      before?.focus?.();
    };
  }, [d]);

  if (!d) return null;
  return (
    <div className="scrim">
      <div ref={ref} className="dialog" role={d.cancelLabel === null ? 'alertdialog' : 'dialog'} aria-modal="true" aria-labelledby={`${id}-t`} aria-describedby={`${id}-b`}>
        <h2 id={`${id}-t`} className="dialog-title">{d.title}</h2>
        <div id={`${id}-b`} className="dialog-body">{d.body}</div>
        <div className="dialog-actions">
          <Button variant={d.variant ?? 'primary'} data-autofocus onClick={() => closeDialog(true)}>{d.confirmLabel ?? 'OK'}</Button>
          {d.cancelLabel !== null && <Button onClick={() => closeDialog(false)}>{d.cancelLabel ?? 'Cancel'}</Button>}
        </div>
      </div>
    </div>
  );
}

export function Toasts() {
  const toasts = useApp((s) => s.toasts);
  return (
    <div className="toasts" role="status" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className="toast">
          <Icon name="info" />
          <span>{t.text}</span>
          <button type="button" className="toast-close" aria-label="Dismiss" onClick={() => dismissToast(t.id)}>
            <Icon name="dismiss" size={16} />
          </button>
        </div>
      ))}
    </div>
  );
}
