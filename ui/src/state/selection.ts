import { useSyncExternalStore } from 'react';

/** What the user ticked in one list, keyed by path. A plain model: no DOM, O(1) per toggle, O(n) for select all. */
export class Selection {
  private ids = new Set<string>();
  private listeners = new Set<() => void>();
  readonly sizes = new Map<string, number>();
  bytes = 0;
  version = 0;

  has(id: string): boolean {
    return this.ids.has(id);
  }

  get count(): number {
    return this.ids.size;
  }

  list(): string[] {
    return [...this.ids];
  }

  set(ids: Iterable<string>, on: boolean): void {
    let changed = false;
    for (const id of ids) {
      if (on === this.ids.has(id)) continue;
      if (on) this.ids.add(id);
      else this.ids.delete(id);
      this.bytes += (on ? 1 : -1) * (this.sizes.get(id) ?? 0);
      changed = true;
    }
    if (changed) this.emit();
  }

  clear(): void {
    if (!this.ids.size) return;
    this.ids.clear();
    this.bytes = 0;
    this.emit();
  }

  subscribe = (fn: () => void): (() => void) => {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  };

  private emit() {
    this.version++;
    this.listeners.forEach((fn) => fn());
  }
}

/** Re-render when the selection changes; returns the version so it can be used as a memo dependency. */
export function useSelectionVersion(sel: Selection): number {
  return useSyncExternalStore(sel.subscribe, () => sel.version);
}
