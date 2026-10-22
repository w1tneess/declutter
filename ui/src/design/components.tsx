import { cloneElement, useId, useState, type ButtonHTMLAttributes, type KeyboardEvent, type ReactElement, type ReactNode } from 'react';
import { icons, type IconName } from './icons';

export function Icon({ name, size = 20 }: { name: IconName; size?: number }) {
  return (
    <svg className="icon" width={size} height={size} viewBox="0 0 20 20" fill="currentColor" aria-hidden="true" focusable="false">
      <path d={icons[name]} />
    </svg>
  );
}

const cx = (...c: (string | false | null | undefined)[]) => c.filter(Boolean).join(' ');

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: 'primary' | 'secondary' | 'subtle' | 'danger';
  icon?: IconName;
  large?: boolean;
};

export function Button({ variant = 'secondary', icon, large, className, children, ...rest }: ButtonProps) {
  return (
    <button type="button" className={cx('btn', `btn-${variant}`, large && 'btn-lg', className)} {...rest}>
      {icon && <Icon name={icon} />}
      {children}
    </button>
  );
}

export function Switch({ label, description, checked, onChange }: { label: string; description?: ReactNode; checked: boolean; onChange: (on: boolean) => void }) {
  const id = useId();
  return (
    <div className="setting-row">
      <div className="setting-text">
        <span id={`${id}-l`} className="setting-label">{label}</span>
        {description && <span id={`${id}-d`} className="setting-desc">{description}</span>}
      </div>
      <button
        type="button"
        role="switch"
        className="switch"
        aria-checked={checked}
        aria-labelledby={`${id}-l`}
        aria-describedby={description ? `${id}-d` : undefined}
        onClick={() => onChange(!checked)}
      >
        <span className="switch-track"><span className="switch-thumb" /></span>
        <span className="switch-state" aria-hidden="true">{checked ? 'On' : 'Off'}</span>
      </button>
    </div>
  );
}

export function Segmented<T extends string>({ label, value, options, onChange }: { label: string; value: T; options: { value: T; label: string }[]; onChange: (v: T) => void }) {
  const name = useId();
  return (
    <div className="segmented" role="radiogroup" aria-label={label}>
      {options.map((o) => (
        <label key={o.value} className={cx('segment', o.value === value && 'is-on')}>
          <input type="radio" name={name} value={o.value} checked={o.value === value} onChange={() => onChange(o.value)} />
          {o.label}
        </label>
      ))}
    </div>
  );
}

type FieldProps = { label: string; hint?: ReactNode; error?: string | null; className?: string };

export function TextField({ label, hint, error, className, value, onChange, ...rest }: FieldProps & { value: string; onChange: (v: string) => void } & Omit<React.InputHTMLAttributes<HTMLInputElement>, 'value' | 'onChange'>) {
  const id = useId();
  const described = [hint && `${id}-h`, error && `${id}-e`].filter(Boolean).join(' ') || undefined;
  return (
    <div className={cx('field', className)}>
      <label htmlFor={id} className="field-label">{label}</label>
      <input id={id} className="input" value={value} onChange={(e) => onChange(e.target.value)} aria-invalid={error ? true : undefined} aria-describedby={described} {...rest} />
      {hint && <span id={`${id}-h`} className="field-hint">{hint}</span>}
      {error && <span id={`${id}-e`} className="field-error" role="alert"><Icon name="errorCircle" size={16} />{error}</span>}
    </div>
  );
}

/** Commits a whole number clamped to [min, max] on blur or Enter; shows a message while the text is not a number. */
export function NumberField({ label, hint, value, min, max, onCommit, className, suffix }: FieldProps & { value: number; min: number; max?: number; onCommit: (n: number) => void; suffix?: string }) {
  const [text, setText] = useState(String(value));
  const [seen, setSeen] = useState(value);
  if (seen !== value) {
    setSeen(value);
    setText(String(value));
  }
  const n = Number(text);
  const bad = text.trim() === '' || !Number.isInteger(n) || n < min || (max !== undefined && n > max);
  const commit = () => {
    if (!bad && n !== value) onCommit(n);
  };
  const range = max === undefined ? `${min} or more` : `${min} to ${max}`;
  return (
    <TextField
      className={cx('field-number', className)}
      label={label}
      hint={hint}
      error={bad ? `Enter a whole number from ${range}.` : null}
      value={text}
      inputMode="numeric"
      onChange={setText}
      onBlur={commit}
      onKeyDown={(e) => e.key === 'Enter' && commit()}
      aria-label={suffix ? `${label} (${suffix})` : undefined}
    />
  );
}

export function Card({ children, className }: { children: ReactNode; className?: string }) {
  return <section className={cx('card', className)}>{children}</section>;
}

export function Badge({ children, tone = 'neutral' }: { children: ReactNode; tone?: 'neutral' | 'accent' | 'warning' }) {
  return <span className={cx('badge', `badge-${tone}`)}>{children}</span>;
}

export function ProgressBar({ value, label }: { value: number | null; label: string }) {
  return value === null ? (
    <div className="progress is-indeterminate" role="progressbar" aria-label={label}>
      <span />
    </div>
  ) : (
    <div className="progress" role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(value * 100)}>
      <span style={{ transform: `scaleX(${Math.min(1, Math.max(0, value))})` }} />
    </div>
  );
}

export function EmptyState({ icon = 'broom', title, children, action }: { icon?: IconName; title: string; children?: ReactNode; action?: ReactNode }) {
  return (
    <div className="empty">
      <span className="empty-icon"><Icon name={icon} size={20} /></span>
      <p className="empty-title">{title}</p>
      {children && <p className="empty-body">{children}</p>}
      {action}
    </div>
  );
}

/** Shows `text` while the trigger is hovered or focused (CSS); Esc hides it (WCAG 1.4.13). */
export function Tooltip({ text, children }: { text: string; children: ReactElement<{ 'aria-describedby'?: string; onKeyDown?: (e: KeyboardEvent) => void; onBlur?: () => void; onMouseEnter?: () => void }> }) {
  const id = useId();
  const [hidden, setHidden] = useState(false);
  return (
    <span className={cx('tip-wrap', hidden && 'is-dismissed')}>
      {cloneElement(children, {
        'aria-describedby': id,
        onKeyDown: (e: KeyboardEvent) => e.key === 'Escape' && setHidden(true),
        onBlur: () => setHidden(false),
        onMouseEnter: () => setHidden(false),
      })}
      <span role="tooltip" id={id} className="tip">{text}</span>
    </span>
  );
}
