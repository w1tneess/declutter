import { Icon } from './components';
import type { IconName } from './icons';

export interface NavItem<T extends string> {
  id: T;
  label: string;
  icon: IconName;
  iconOn: IconName;
  count?: number;
}

/** Windows 11 style navigation: filled icon and an accent pill on the current page; labels hide below 720 px. */
export function Sidebar<T extends string>({ items, current, onSelect, footer }: { items: NavItem<T>[]; current: T; onSelect: (id: T) => void; footer?: NavItem<T> }) {
  const item = (it: NavItem<T>) => {
    const on = it.id === current;
    return (
      <li key={it.id}>
        <button
          type="button"
          className={`nav-item${on ? ' is-on' : ''}`}
          aria-current={on ? 'page' : undefined}
          aria-label={it.count ? `${it.label}, ${it.count.toLocaleString('en-US')} found` : undefined}
          title={it.label}
          onClick={() => onSelect(it.id)}
        >
          <Icon name={on ? it.iconOn : it.icon} />
          <span className="nav-label">{it.label}</span>
          {!!it.count && <span className="nav-count" aria-hidden="true">{it.count > 9999 ? '9999+' : it.count.toLocaleString('en-US')}</span>}
        </button>
      </li>
    );
  };
  return (
    <nav className="sidebar" aria-label="Main">
      <div className="brand">
        <span className="brand-mark"><Icon name="broom" /></span>
        <span className="brand-name">Declutter</span>
      </div>
      <ul className="nav-list">{items.map(item)}</ul>
      {footer && <ul className="nav-list nav-foot">{item(footer)}</ul>}
    </nav>
  );
}
