// Deterministic in-memory backend for the browser preview, unit tests and e2e. Streams ScanEvents in the documented
// order (docs/API.md) with realistic delays, supports cancel, scan errors and refused folders, and can produce
// 100,000 rows for performance checks. URL options (preview/e2e): ?rows=N&delay=ms&fail=scan&pick=<path>.
import type { Api } from './client';
import type { ActionResult } from './generated/ActionResult';
import type { DupGroup } from './generated/DupGroup';
import type { FileRef } from './generated/FileRef';
import type { Leftover } from './generated/Leftover';
import type { Move } from './generated/Move';
import type { Preset } from './generated/Preset';
import type { Redownload } from './generated/Redownload';
import type { ReminderStatus } from './generated/ReminderStatus';
import type { ScanEvent } from './generated/ScanEvent';
import type { ScanOptions } from './generated/ScanOptions';
import type { Settings } from './generated/Settings';
import type { SimilarGroup } from './generated/SimilarGroup';
import type { SpaceFreed } from './generated/SpaceFreed';
import type { UndoInfo } from './generated/UndoInfo';
import { plural } from '../format';

export interface MockOptions {
  /** Rows per list; large values exercise virtualization. */
  rows?: number;
  /** Base delay between scan phases in ms (0 for fast tests). */
  delay?: number;
  /** End every scan with an `error` event after the walk. */
  failScan?: boolean;
  /** What the folder picker returns. */
  pick?: string | null;
}

const HOME = 'C:\\Users\\Alex';
const MB = 1024 * 1024;
const DAY = 86400;

function rng(seed: number) {
  return () => {
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const CATS: [string, string[], string[]][] = [
  ['Images', ['.jpg', '.png', '.heic'], ['IMG_', 'photo-', 'holiday-', 'scan-']],
  ['Documents', ['.pdf', '.docx', '.xlsx', '.txt'], ['invoice-', 'report-', 'budget-', 'notes-', 'letter-']],
  ['Videos', ['.mp4', '.mov'], ['VID_', 'clip-', 'recording-']],
  ['Audio', ['.mp3', '.m4a'], ['voice-memo-', 'track-']],
  ['Archives', ['.zip', '.7z'], ['backup-', 'export-', 'photos-']],
  ['Installers', ['.exe', '.msi'], ['setup-', 'installer-']],
];

function pick<T>(r: () => number, xs: readonly T[]): T {
  return xs[Math.floor(r() * xs.length)] as T;
}

interface Data {
  moves: Move[];
  recentLeft: number;
  largeOld: FileRef[];
  leftovers: Leftover[];
  emptyDirs: string[];
  redownloads: Redownload[];
  duplicates: DupGroup[];
  similar: SimilarGroup[];
  skipped: string[];
}

function generate(root: string, rows: number, now: number): Data {
  const r = rng(42);
  const name = (i: number) => {
    const [cat, exts, pre] = pick(r, CATS);
    return { cat, file: `${pick(r, pre)}${1000 + i}${pick(r, exts)}` };
  };
  const moves: Move[] = [];
  for (let i = 0; i < rows; i++) {
    const { cat, file } = name(i);
    const category = cat === 'Images' && i % 5 === 0 ? 'Images/Screenshots' : cat;
    const shown = category === 'Images/Screenshots' ? `Screenshot ${2024 + (i % 2)}-0${1 + (i % 9)}-1${i % 9} ${i}.png` : file;
    moves.push({ src: `${root}\\${shown}`, dst: `${root}\\${category.replace('/', '\\')}\\${shown}`, size: Math.floor(r() * 40 * MB) + 2048, category });
  }
  moves.sort((a, b) => (a.src < b.src ? -1 : 1));
  const largeOld: FileRef[] = [];
  const sub = ['Videos', 'Old projects', 'Backups', 'Games', 'ISO'];
  for (let i = 0; i < rows; i++) {
    const { file } = name(i);
    largeOld.push({ path: `${root}\\${pick(r, sub)}\\${file}`, size: Math.floor((100 + r() * 3900) * MB), mtime: now - Math.floor((60 + r() * 1400) * DAY) });
  }
  if (rows <= 1000) largeOld.push({ path: `${root}\\Videos\\locked-recording.mp4`, size: 812 * MB, mtime: now - 400 * DAY });
  largeOld.sort((a, b) => b.size - a.size);
  const leftovers: Leftover[] = [
    { path: `${root}\\setup-zoom-5.17.exe`, size: 96 * MB, mtime: now - 40 * DAY, reason: 'Old installer' },
    { path: `${root}\\holiday-photos.zip`, size: 412 * MB, mtime: now - 120 * DAY, reason: 'Unpacked .zip' },
    { path: `${root}\\movie-trailer.mp4.part`, size: 38 * MB, mtime: now - 9 * DAY, reason: 'Unfinished download' },
  ].sort((a, b) => b.size - a.size);
  const emptyDirs = Array.from({ length: Math.min(rows, 2000) / 50 + 4 }, (_, i) => `${root}\\Old projects\\draft-${i}${i % 3 === 0 ? '\\assets\\icons' : ''}`)
    .sort((a, b) => b.length - a.length);
  const redownloads: Redownload[] = ['report-2024.pdf', 'budget.xlsx', 'cv.docx'].map((f, i) => {
    const size = (1 + i) * 1.3 * MB;
    const copy = f.replace(/(\.\w+)$/, i === 2 ? ' - Copy$1' : ` (${i + 1})$1`);
    return { original: { path: `${root}\\${f}`, size: Math.floor(size), mtime: now - 30 * DAY }, copy: { path: `${root}\\${copy}`, size: Math.floor(size * 1.01), mtime: now - 3 * DAY } };
  });
  const duplicates: DupGroup[] = [];
  for (let i = 0; i < Math.max(4, Math.floor(rows / 10)); i++) {
    const { file } = name(i + 5000);
    const size = Math.floor((0.2 + r() * 300) * MB);
    const where = ['', '\\Old projects', '\\Backups', '\\Phone'];
    const n = 2 + Math.floor(r() * 3);
    const files = Array.from({ length: n }, (_, k) => ({ path: `${root}${where[k]}\\${file}`, size, mtime: now - (900 - k * 100 - i) * DAY }));
    duplicates.push({ size, files });
  }
  duplicates.sort((a, b) => b.size * (b.files.length - 1) - a.size * (a.files.length - 1));
  const similar: SimilarGroup[] = Array.from({ length: Math.max(3, Math.floor(rows / 40)) }, (_, i) => ({
    files: Array.from({ length: 2 + (i % 2) }, (_, k) => ({
      path: `${root}\\Phone\\IMG_${2000 + i * 3 + k}.jpg`,
      size: Math.floor((2 + r() * 4) * MB),
      mtime: now - (200 - i) * DAY,
      width: k === 1 ? 3024 : 4032,
      height: k === 1 ? 2268 : 3024,
      distance: k === 0 ? 0 : 2 + k * 3,
    })),
  }));
  const skipped = [`${root}\\Tools\\portable-editor`, `${root}\\Code\\website`];
  return { moves, recentLeft: 3, largeOld, leftovers, emptyDirs, redownloads, duplicates, similar, skipped };
}

function fromUrl(): MockOptions {
  if (typeof location === 'undefined') return {};
  const q = new URLSearchParams(location.search);
  return {
    rows: q.has('rows') ? Number(q.get('rows')) : undefined,
    delay: q.has('delay') ? Number(q.get('delay')) : undefined,
    failScan: q.get('fail') === 'scan',
    pick: q.get('pick') ?? undefined,
  };
}

export function createMockApi(options: MockOptions = fromUrl()): Api & { lastExport: unknown } {
  const rows = options.rows ?? 60;
  const delay = options.delay ?? 300;
  const sleep = (k: number) => new Promise<void>((res) => setTimeout(res, delay * k));
  const removed = new Set<string>();
  const sorted = new Set<string>();
  let nextId = 1;
  const cancelled = new Set<number>();
  let undo: (UndoInfo & { srcs: string[]; at: number }) | null = null;
  let freed: SpaceFreed = { bytes: 7.4 * 1024 * MB, files: 312, since: '2026-03-02' };
  let bin = { bytes: 2.3 * 1024 * MB, items: 148 };
  let reminder: ReminderStatus = { enabled: false, nextRun: null };
  let settings: Settings = {
    theme: 'system', recentHours: 24, minMb: 100, olderDays: 0, lastFolder: null, rememberLastFolder: false,
    pins: [], activePreset: null,
  };
  const builtins: Preset[] = [
    { id: 'downloads', name: 'Downloads', builtin: true, rules: [{ extensions: ['.exe', '.msi'], folder: 'Installers' }, { extensions: ['.pdf', '.docx'], folder: 'Documents' }, { extensions: ['.zip', '.7z'], folder: 'Archives' }] },
    { id: 'photos', name: 'Photos', builtin: true, rules: [{ extensions: ['.jpg', '.png', '.heic'], folder: 'Photos' }, { extensions: ['.mp4', '.mov'], folder: 'Photos\\Videos' }] },
    { id: 'work', name: 'Work', builtin: true, rules: [{ extensions: ['.docx', '.xlsx', '.pptx'], folder: 'Office' }, { extensions: ['.pdf'], folder: 'PDF' }] },
  ];
  let custom: Preset[] = [];
  const presets = () => [...builtins, ...custom];
  const keep = <T,>(xs: T[], path: (x: T) => string) => xs.filter((x) => !removed.has(path(x)));
  const ok = (count: number, errors: string[] = []): ActionResult => ({ count, errors });

  const sizes = new Map<string, number>();
  function trash(paths: string[]): ActionResult {
    const errors: string[] = [];
    let count = 0;
    let bytes = 0;
    for (const p of paths) {
      if (p.includes('locked')) {
        errors.push(`Couldn't send ${p.slice(p.lastIndexOf('\\') + 1)} to the Recycle Bin: it is open in another program.`);
        continue;
      }
      removed.add(p);
      count++;
      bytes += sizes.get(p) ?? 0;
    }
    freed = { ...freed, bytes: freed.bytes + bytes, files: freed.files + count };
    bin = { bytes: bin.bytes + bytes, items: bin.items + count };
    return ok(count, errors);
  }

  function checkFolder(path: string) {
    const p = path.replace(/[\\/]+$/, '');
    if (/^[a-z]:$/i.test(p)) return { ok: false, reason: 'This is a whole drive. Choose a folder inside it.' };
    if (p.toLowerCase() === HOME.toLowerCase()) return { ok: false, reason: 'This is your whole user folder. Choose a folder inside it, like Downloads.' };
    if (/\\(windows|program files( \(x86\))?|programdata|appdata|node_modules|\.git)(\\|$)/i.test(p))
      return { ok: false, reason: 'Declutter does not touch Windows, program or system folders.' };
    return { ok: true, reason: '' };
  }

  const api: Api & { lastExport: unknown } = {
    lastExport: null,
    appInfo: async () => ({ version: '1.0.0', dataDir: `${HOME}\\AppData\\Local\\Declutter` }),
    quickFolders: async () => ({ downloads: `${HOME}\\Downloads`, desktop: `${HOME}\\Desktop`, documents: `${HOME}\\Documents` }),
    pickFolder: async () => (options.pick === undefined ? `${HOME}\\Downloads` : options.pick),
    pickSavePath: async (name) => `${HOME}\\Documents\\${name}`,
    checkFolder: async (path) => checkFolder(path),
    async startScan(path: string, opts: ScanOptions, emit: (e: ScanEvent) => void) {
      const id = nextId++;
      const now = Math.floor(Date.now() / 1000);
      const started = Date.now();
      const check = checkFolder(path);
      void (async () => {
        const stop = () => {
          if (!cancelled.has(id)) return false;
          emit({ type: 'done', cancelled: true, tookMs: Date.now() - started });
          return true;
        };
        if (!check.ok) return emit({ type: 'error', message: check.reason });
        const d = generate(path, rows, now);
        for (const f of [...d.largeOld, ...d.leftovers, ...d.duplicates.flatMap((g) => g.files), ...d.redownloads.map((p) => p.copy), ...d.similar.flatMap((g) => g.files)])
          sizes.set(f.path, f.size);
        const pinned = (p: string) => opts.ignore.some((x) => p === x || p.startsWith(x + '\\'));
        const gone = (p: string) => removed.has(p) || pinned(p);
        const total = rows * 3 + 40;
        for (let i = 1; i <= 4; i++) {
          await sleep(0.5);
          if (stop()) return;
          emit({ type: 'progress', progress: { phase: 'walking', done: Math.floor((total * i) / 4), total: null, bytes: i * 900 * MB, etaSecs: null, label: 'Reading the folder…' } });
        }
        if (options.failScan) {
          await sleep(0.5);
          return emit({ type: 'error', message: "Can't read this folder anymore. It may have been moved, renamed or disconnected." });
        }
        emit({ type: 'skipped', folders: d.skipped });
        const moves = d.moves.filter((m) => !gone(m.src) && !sorted.has(m.src));
        emit({ type: 'sort', plan: { moves, recentLeft: opts.recentHours > 0 ? d.recentLeft : 0 } });
        await sleep(0.6);
        if (stop()) return;
        const large = d.largeOld.filter((f) => !gone(f.path) && f.size >= opts.minMb * MB && (opts.olderDays === 0 || now - f.mtime >= opts.olderDays * DAY));
        emit({ type: 'largeOld', files: large });
        emit({ type: 'leftovers', items: keep(d.leftovers, (x) => x.path) });
        emit({
          type: 'breakdown',
          breakdown: {
            folders: [['Videos', 41], ['Old projects', 23], ['Backups', 17], ['Phone', 9], ['Games', 6], ['ISO', 4]].map(([n, gb]) => ({ name: `${path}\\${n}`, bytes: (gb as number) * 1024 * MB, files: (gb as number) * 37 })),
            types: [['Videos', 52], ['Archives', 21], ['Images', 14], ['Installers', 6], ['Documents', 3], ['Other', 2]].map(([n, gb]) => ({ name: n as string, bytes: (gb as number) * 1024 * MB, files: (gb as number) * 91 })),
          },
        });
        emit({ type: 'emptyDirs', dirs: d.emptyDirs.filter((p) => !gone(p)) });
        for (let i = 1; i <= 6; i++) {
          await sleep(0.5);
          if (stop()) return;
          emit({ type: 'progress', progress: { phase: 'hashing', done: Math.floor((total * i) / 6), total, bytes: i * 1.6 * 1024 * MB, etaSecs: i < 2 ? null : Math.round(((6 - i) * delay * 0.5) / 1000) + 1, label: `Checking ${plural(d.duplicates.length * 3, 'possible duplicate')} in full` } });
        }
        const groups = d.duplicates
          .map((g) => ({ size: g.size, files: g.files.filter((f) => !gone(f.path)) }))
          .filter((g) => g.files.length > 1);
        emit({ type: 'redownloads', pairs: d.redownloads.filter((p) => !gone(p.copy.path) && !gone(p.original.path)) });
        emit({ type: 'duplicates', groups });
        if (opts.similarImages) {
          await sleep(1);
          if (stop()) return;
          emit({ type: 'progress', progress: { phase: 'similar', done: total, total, bytes: 0, etaSecs: null, label: 'Comparing pictures…' } });
          await sleep(0.5);
          emit({ type: 'similar', groups: d.similar.map((g) => ({ files: g.files.filter((f) => !gone(f.path)) })).filter((g) => g.files.length > 1) });
        }
        if (stop()) return;
        emit({ type: 'done', cancelled: false, tookMs: Date.now() - started });
      })();
      return id;
    },
    cancelScan: async (id) => void cancelled.add(id),
    async applySort(root, moves) {
      moves.forEach((m) => sorted.add(m.src));
      undo = { folder: root, count: moves.length, ageSeconds: 0, srcs: moves.map((m) => m.src), at: Date.now() };
      return ok(moves.length);
    },
    trashPaths: async (_root, paths) => trash(paths),
    trashDuplicates: async (_root, removals) => trash(removals.flatMap((r) => r.remove)),
    trashRedownloads: async (_root, pairs, removing) => trash(pairs.filter((p) => !removing.includes(p.original.path)).map((p) => p.copy.path)),
    trashSimilar: async (_root, removals) => trash(removals.flatMap((r) => r.remove)),
    trashEmptyDirs: async (_root, paths) => trash(paths),
    peekUndo: async () => (undo ? { folder: undo.folder, count: undo.count, ageSeconds: Math.floor((Date.now() - undo.at) / 1000) } : null),
    async undoLastSort() {
      if (!undo) return ok(0, ['Nothing to undo.']);
      undo.srcs.forEach((s) => sorted.delete(s));
      const n = undo.count;
      undo = null;
      return ok(n);
    },
    getSettings: async () => structuredClone(settings),
    saveSettings: async (s) => (settings = structuredClone(s)),
    addPin: async (p) => (settings = { ...settings, pins: settings.pins.includes(p) ? settings.pins : [...settings.pins, p] }),
    removePin: async (p) => (settings = { ...settings, pins: settings.pins.filter((x) => x !== p) }),
    listPresets: async () => presets(),
    async savePreset(p) {
      if (p.builtin || builtins.some((b) => b.id === p.id)) throw "Built-in presets can't be changed. Make a copy instead.";
      if (!p.name.trim()) throw 'Give the preset a name.';
      if (!p.rules.length) throw 'Add at least one rule.';
      const seen = new Set<string>();
      for (const rule of p.rules) {
        const f = rule.folder.trim();
        if (!f || /^[a-z]:|^[\\/]|\.\./i.test(f) || f.split(/[\\/]/).length > 2)
          throw 'Folder names must stay inside the chosen folder, at most two levels deep (like "Work\\Invoices").';
        if (!rule.extensions.length) throw `The rule for "${f}" has no file types.`;
        for (const e of rule.extensions) {
          if (!/^\.[a-z0-9]+$/.test(e)) throw `"${e}" is not a file type. Use a dot and lower case, like .pdf.`;
          if (seen.has(e)) throw `${e} is in more than one rule. Each file type can go to one folder only.`;
          seen.add(e);
        }
      }
      custom = [...custom.filter((c) => c.id !== p.id), { ...p, id: p.id || `custom-${Date.now()}` }];
      return presets();
    },
    async deletePreset(id) {
      if (builtins.some((b) => b.id === id)) throw "Built-in presets can't be deleted.";
      custom = custom.filter((c) => c.id !== id);
      return presets();
    },
    async exportCsv(_path, payload) {
      api.lastExport = payload;
    },
    spaceFreed: async () => freed,
    resetSpaceFreed: async () => (freed = { bytes: 0, files: 0, since: new Date().toISOString().slice(0, 10) }),
    binInfo: async () => bin,
    openRecycleBin: async () => undefined,
    revealInExplorer: async () => undefined,
    reminderStatus: async () => reminder,
    reminderEnable: async () => (reminder = { enabled: true, nextRun: 'Monday 10:00' }),
    reminderDisable: async () => (reminder = { ...reminder, enabled: false, nextRun: null }),
    onFolderDrop: () => () => undefined,
  };
  return api;
}
