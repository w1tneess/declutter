// Plain-language formatting, matching the v0.2.2 wording (core.human_size, core.ago, app.plural).

export function humanSize(n: number): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let i = 0;
  while (n >= 1024 && i < units.length - 1) {
    n /= 1024;
    i++;
  }
  return i === 0 ? `${Math.round(n)} B` : `${n.toFixed(1)} ${units[i]}`;
}

export function plural(n: number, word: string): string {
  return `${n.toLocaleString('en-US')} ${n === 1 ? word : word + 's'}`;
}

export function ago(seconds: number): string {
  for (const [size, unit] of [[86400, 'day'], [3600, 'hour'], [60, 'minute']] as const) {
    if (seconds >= size) {
      const n = Math.floor(seconds / size);
      return `${n} ${unit}${n === 1 ? '' : 's'} ago`;
    }
  }
  return 'just now';
}

export function eta(seconds: number): string {
  if (seconds < 60) return `About ${plural(Math.max(1, Math.round(seconds)), 'second')} left`;
  return `About ${plural(Math.round(seconds / 60), 'minute')} left`;
}

/** `mtime` is seconds since the epoch. */
export function dateText(mtime: number): string {
  return new Date(mtime * 1000).toLocaleDateString('en-GB', { day: '2-digit', month: 'short', year: 'numeric' });
}

export function daysOld(mtime: number, now = Date.now()): number {
  return Math.max(0, Math.floor((now / 1000 - mtime) / 86400));
}

export function baseName(path: string): string {
  return path.slice(Math.max(path.lastIndexOf('\\'), path.lastIndexOf('/')) + 1);
}

/** Path relative to the scanned folder, for display only ("." for the folder itself). */
export function relPath(path: string, root: string | null): string {
  if (!root) return path;
  const r = root.replace(/[\\/]+$/, '');
  if (path.toLowerCase() === r.toLowerCase()) return '.';
  return path.toLowerCase().startsWith(r.toLowerCase() + '\\') || path.toLowerCase().startsWith(r.toLowerCase() + '/')
    ? path.slice(r.length + 1)
    : path;
}

export function parentRel(path: string, root: string | null): string {
  const rel = relPath(path, root);
  const cut = Math.max(rel.lastIndexOf('\\'), rel.lastIndexOf('/'));
  return cut < 0 ? '' : rel.slice(0, cut);
}
