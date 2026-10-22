import type { ReactNode } from 'react';
import { create } from 'zustand';
import { api, errorText } from '../api/client';
import type { ActionResult } from '../api/generated/ActionResult';
import type { AppInfo } from '../api/generated/AppInfo';
import type { Breakdown } from '../api/generated/Breakdown';
import type { DupGroup } from '../api/generated/DupGroup';
import type { ExportPayload } from '../api/generated/ExportPayload';
import type { FileRef } from '../api/generated/FileRef';
import type { Leftover } from '../api/generated/Leftover';
import type { Preset } from '../api/generated/Preset';
import type { Progress } from '../api/generated/Progress';
import type { QuickFolders } from '../api/generated/QuickFolders';
import type { Redownload } from '../api/generated/Redownload';
import type { ScanEvent } from '../api/generated/ScanEvent';
import type { Settings } from '../api/generated/Settings';
import type { SimilarGroup } from '../api/generated/SimilarGroup';
import type { SortPlan } from '../api/generated/SortPlan';
import type { UndoInfo } from '../api/generated/UndoInfo';
import { baseName, plural } from '../format';
import { Selection } from './selection';

export type Page = 'home' | 'sort' | 'duplicates' | 'similar' | 'largeOld' | 'empty' | 'settings';
export type ListTab = Exclude<Page, 'home' | 'settings'>;

/** Each field is undefined until its ScanEvent arrives, so a tab can say "still looking" instead of "nothing". */
export interface Results {
  skipped?: string[];
  sort?: SortPlan;
  largeOld?: FileRef[];
  leftovers?: Leftover[];
  breakdown?: Breakdown;
  emptyDirs?: string[];
  redownloads?: Redownload[];
  duplicates?: DupGroup[];
  similar?: SimilarGroup[];
}

export type ScanStatus = 'idle' | 'scanning' | 'cancelling' | 'done' | 'cancelled' | 'error';

export interface DialogSpec {
  title: string;
  body: ReactNode;
  confirmLabel?: string;
  /** null hides the cancel button (an alert). */
  cancelLabel?: string | null;
  variant?: 'primary' | 'danger';
  resolve: (ok: boolean) => void;
}

export interface Toast {
  id: number;
  text: string;
}

export interface AppState {
  page: Page;
  folder: string | null;
  quick: QuickFolders | null;
  info: AppInfo | null;
  settings: Settings | null;
  presets: Preset[];
  undo: UndoInfo | null;
  status: ScanStatus;
  progress: Progress | null;
  message: string;
  similarImages: boolean;
  scannedOnce: boolean;
  busy: boolean;
  results: Results;
  sel: Record<ListTab, Selection>;
  /** The kept copy of each duplicate group, by path. Never part of `sel.duplicates`. */
  keep: Selection;
  dialog: DialogSpec | null;
  toasts: Toast[];
}

const freshSelections = (): Record<ListTab, Selection> => ({
  sort: new Selection(), duplicates: new Selection(), similar: new Selection(), largeOld: new Selection(), empty: new Selection(),
});

function readScanned(): boolean {
  try {
    return localStorage.getItem('declutter.scanned') === '1';
  } catch {
    return false;
  }
}

export const useApp = create<AppState>(() => ({
  page: 'home', folder: null, quick: null, info: null, settings: null, presets: [], undo: null,
  status: 'idle', progress: null, message: 'Choose a folder on Home to begin.', similarImages: false,
  scannedOnce: readScanned(), busy: false, results: {}, sel: freshSelections(), keep: new Selection(),
  dialog: null, toasts: [],
}));

const set = useApp.setState;
const get = useApp.getState;

export const go = (page: Page) => set({ page });

// ---------- dialogs and toasts ----------

export function confirm(spec: Omit<DialogSpec, 'resolve'>): Promise<boolean> {
  return new Promise((resolve) => set({ dialog: { ...spec, resolve } }));
}

export function alert(title: string, body: ReactNode): Promise<boolean> {
  return confirm({ title, body, confirmLabel: 'OK', cancelLabel: null });
}

export function closeDialog(ok: boolean) {
  const d = get().dialog;
  set({ dialog: null });
  d?.resolve(ok);
}

let toastId = 0;
export function toast(text: string, ms = 4500) {
  const id = ++toastId;
  set((s) => ({ toasts: [...s.toasts.slice(-2), { id, text }] }));
  setTimeout(() => dismissToast(id), ms);
}

export const dismissToast = (id: number) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));

// ---------- start-up and settings ----------

export function applyTheme(theme: string) {
  const root = document.documentElement;
  if (theme === 'light' || theme === 'dark') root.dataset.theme = theme;
  else delete root.dataset.theme;
}

export async function init() {
  try {
    const [info, quick, settings, presets, undo] = await Promise.all([
      api.appInfo(), api.quickFolders(), api.getSettings(), api.listPresets(), api.peekUndo(),
    ]);
    applyTheme(settings.theme);
    set({ info, quick, settings, presets, undo });
    if (settings.rememberLastFolder && settings.lastFolder) await chooseFolder(settings.lastFolder, true);
  } catch (e) {
    void alert('Something went wrong', errorText(e));
  }
}

export async function updateSettings(change: Partial<Settings>) {
  const cur = get().settings;
  if (!cur) return;
  const next = { ...cur, ...change };
  set({ settings: next });
  if (change.theme) applyTheme(change.theme);
  try {
    set({ settings: await api.saveSettings(next) });
  } catch (e) {
    toast(errorText(e));
  }
}

export async function refreshUndo() {
  try {
    set({ undo: await api.peekUndo() });
  } catch {
    set({ undo: null });
  }
}

// ---------- folder and scan ----------

export async function chooseFolder(path?: string, quiet = false) {
  const { status, busy } = get();
  if (status === 'scanning' || status === 'cancelling' || busy) {
    toast('Please wait for the current job to finish (or press Cancel).');
    return;
  }
  try {
    const p = path ?? (await api.pickFolder());
    if (!p) return;
    const check = await api.checkFolder(p);
    if (!check.ok) {
      if (!quiet) await alert("Can't use this folder", check.reason);
      return;
    }
    token++; // events from any earlier scan must not land on the new folder
    set({ folder: p, results: {}, sel: freshSelections(), keep: new Selection(), status: 'idle', progress: null, message: 'Folder chosen. Press Scan.' });
    const s = get().settings;
    if (s?.rememberLastFolder && s.lastFolder !== p) void updateSettings({ lastFolder: p });
  } catch (e) {
    await alert("Can't use this folder", errorText(e));
  }
}

let token = 0;
let scanId: number | null = null;
let queue: ScanEvent[] = [];
let scheduled = false;
const nextFrame = (fn: () => void) =>
  typeof requestAnimationFrame === 'function' ? requestAnimationFrame(fn) : setTimeout(fn, 16);

export async function startScan() {
  const { folder, settings, similarImages, status, busy } = get();
  if (!folder || !settings || busy || status === 'scanning' || status === 'cancelling') return;
  const mine = ++token;
  scanId = null; // Cancel before the new id arrives must not target the previous scan
  queue = [];
  set({
    status: 'scanning', progress: null, message: 'Reading the folder…', results: {}, sel: freshSelections(), keep: new Selection(),
  });
  try {
    scanId = await api.startScan(
      folder,
      {
        minMb: settings.minMb, olderDays: settings.olderDays, recentHours: settings.recentHours,
        similarImages, ignore: settings.pins, presetId: settings.activePreset,
      },
      (e) => {
        if (mine !== token) return;
        queue.push(e);
        if (!scheduled) {
          scheduled = true;
          nextFrame(flush);
        }
      },
    );
  } catch (e) {
    if (mine === token) set({ status: 'error', message: errorText(e) });
  }
}

export async function cancelScan() {
  if (get().status !== 'scanning' || scanId === null) return;
  set({ status: 'cancelling', message: 'Cancelling…' });
  try {
    await api.cancelScan(scanId);
  } catch (e) {
    toast(errorText(e));
  }
}

const sizes = (sel: Selection, files: { path: string; size: number }[]) => files.forEach((f) => sel.sizes.set(f.path, f.size));

/** Applies every event queued since the last frame in one store update, so a burst of events costs one render. */
export function flush() {
  scheduled = false;
  const batch = queue;
  queue = [];
  if (!batch.length) return;
  const s = get();
  const { sel, keep } = s;
  let { results, progress, status, message } = s;
  let scannedOnce = s.scannedOnce;
  for (const e of batch) {
    switch (e.type) {
      case 'progress':
        progress = e.progress;
        if (status === 'scanning') message = e.progress.label;
        break;
      case 'skipped':
        results = { ...results, skipped: e.folders };
        break;
      case 'sort':
        e.plan.moves.forEach((m) => sel.sort.sizes.set(m.src, m.size));
        sel.sort.set(e.plan.moves.map((m) => m.src), true); // moves are undoable, so they start ticked as in v0.2.2
        results = { ...results, sort: e.plan };
        break;
      case 'largeOld':
        sizes(sel.largeOld, e.files);
        results = { ...results, largeOld: e.files };
        break;
      case 'leftovers':
        sizes(sel.largeOld, e.items);
        results = { ...results, leftovers: e.items };
        break;
      case 'breakdown':
        results = { ...results, breakdown: e.breakdown };
        break;
      case 'emptyDirs':
        sel.empty.set(e.dirs, true); // whole empty folders, re-checked by the shell; ticked as in v0.2.2
        results = { ...results, emptyDirs: e.dirs };
        break;
      case 'redownloads':
        sizes(sel.duplicates, e.pairs.map((p) => p.copy));
        results = { ...results, redownloads: e.pairs };
        break;
      case 'duplicates':
        e.groups.forEach((g) => g.files.forEach((f) => sel.duplicates.sizes.set(f.path, g.size)));
        keep.set(e.groups.flatMap((g) => (g.files[0] ? [g.files[0].path] : [])), true);
        results = { ...results, duplicates: e.groups };
        break;
      case 'similar':
        sizes(sel.similar, e.groups.flatMap((g) => g.files));
        results = { ...results, similar: e.groups };
        break;
      case 'done':
        status = e.cancelled ? 'cancelled' : 'done';
        progress = null;
        message = e.cancelled ? 'Cancelled. Nothing was changed by the scan.' : finishedMessage(results);
        if (!e.cancelled && !scannedOnce) {
          scannedOnce = true;
          try {
            localStorage.setItem('declutter.scanned', '1');
          } catch {
            // remembered for this session only
          }
        }
        break;
      case 'error':
        status = 'error';
        progress = null;
        message = e.message;
        break;
    }
  }
  set({ results, progress, status, message, scannedOnce });
}

export interface Counts {
  sort?: number;
  duplicates?: number;
  reclaim?: number;
  redownloads?: number;
  similar?: number;
  largeOld?: number;
  empty?: number;
}

export function counts(r: Results): Counts {
  const large = r.largeOld && r.leftovers ? new Set([...r.largeOld.map((f) => f.path), ...r.leftovers.map((f) => f.path)]).size : undefined;
  return {
    sort: r.sort?.moves.length,
    duplicates: r.duplicates && r.redownloads ? r.duplicates.reduce((n, g) => n + g.files.length - 1, 0) + r.redownloads.length : undefined,
    reclaim: r.duplicates?.reduce((n, g) => n + g.size * (g.files.length - 1), 0),
    redownloads: r.redownloads?.length,
    similar: r.similar?.reduce((n, g) => n + g.files.length, 0),
    largeOld: large,
    empty: r.emptyDirs?.length,
  };
}

function finishedMessage(r: Results): string {
  const c = counts(r);
  const total = (c.sort ?? 0) + (c.duplicates ?? 0) + (c.similar ?? 0) + (c.largeOld ?? 0) + (c.empty ?? 0);
  return total ? 'Scan finished. Review the cards on Home.' : 'Scan finished. This folder is already tidy.';
}

// ---------- actions ----------

/** Shared by every tab: confirm with the exact count, run, report, then scan again so the lists match the disk. */
export async function act(o: {
  title: string;
  body: ReactNode;
  confirmLabel: string;
  run: (root: string) => Promise<ActionResult>;
  done: (n: number) => string;
}) {
  const root = get().folder;
  if (!root || get().busy) return;
  if (!(await confirm({ title: o.title, body: o.body, confirmLabel: o.confirmLabel }))) return;
  set({ busy: true, message: 'Working…' });
  try {
    const r = await o.run(root);
    report(o.done(r.count), r.errors);
  } catch (e) {
    await alert('Something went wrong', errorText(e));
  } finally {
    set({ busy: false });
  }
  await refreshUndo();
  await startScan();
}

export function report(msg: string, errors: string[]) {
  if (!errors.length) return toast(msg);
  const shown = errors.slice(0, 8);
  void alert('Done, with some problems', `${msg}\n\n${plural(errors.length, 'item')} had problems:\n${shown.join('\n')}${errors.length > 8 ? `\n…and ${errors.length - 8} more.` : ''}`);
}

/** Shown instead of a confirm when unpinning left nothing to act on. */
export const ALL_PINNED = 'Nothing to do: everything selected is on your Never touch list.';

/** The Recycle Bin confirm every removal tab uses. `n` counts what is left after pinned paths were dropped. */
export function recycleSelected(n: number, run: (root: string) => Promise<ActionResult>) {
  if (!n) return void toast(ALL_PINNED);
  return act({
    title: 'Send to Recycle Bin?',
    body: `Send ${plural(n, 'item')} to the Recycle Bin? They are not deleted. You can restore them from the Recycle Bin.`,
    confirmLabel: `Send ${plural(n, 'item')}`,
    run,
    done: (k) => `Sent ${plural(k, 'item')} to the Recycle Bin. Restore them from the Bin if needed.`,
  });
}

/** Drops pinned paths right before acting: a pin added after the scan still protects its files. */
export function unpinned(paths: string[]): string[] {
  // Windows paths are case-insensitive, and a pin may end with a slash.
  const pins = (get().settings?.pins ?? []).map((x) => x.replace(/[\\/]+$/, '').toLowerCase());
  return paths.filter((p) => {
    const l = p.toLowerCase();
    return !pins.some((x) => l === x || l.startsWith(x + '\\'));
  });
}

export async function exportCsv(name: string, payload: ExportPayload) {
  try {
    const path = await api.pickSavePath(`declutter-${name}.csv`);
    if (!path) return;
    await api.exportCsv(path, payload);
    toast(`Saved the list as ${baseName(path)}.`);
  } catch (e) {
    await alert("Couldn't save the list", errorText(e));
  }
}

export function reveal(path: string) {
  api.revealInExplorer(path).catch((e: unknown) => toast(errorText(e)));
}
