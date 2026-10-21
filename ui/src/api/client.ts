// The only way the UI reaches the shell (docs/API.md). The Tauri client is used inside the app; the mock everywhere
// else (browser preview, unit tests, e2e).
import type { ActionResult } from './generated/ActionResult';
import type { AppInfo } from './generated/AppInfo';
import type { BinInfo } from './generated/BinInfo';
import type { DupRemoval } from './generated/DupRemoval';
import type { ExportPayload } from './generated/ExportPayload';
import type { FolderCheck } from './generated/FolderCheck';
import type { Move } from './generated/Move';
import type { Preset } from './generated/Preset';
import type { QuickFolders } from './generated/QuickFolders';
import type { Redownload } from './generated/Redownload';
import type { ReminderStatus } from './generated/ReminderStatus';
import type { ScanEvent } from './generated/ScanEvent';
import type { ScanOptions } from './generated/ScanOptions';
import type { Settings } from './generated/Settings';
import type { SimilarRemoval } from './generated/SimilarRemoval';
import type { SpaceFreed } from './generated/SpaceFreed';
import type { UndoInfo } from './generated/UndoInfo';
import { createMockApi } from './mock';
import { createTauriApi } from './tauri';

export interface Api {
  appInfo(): Promise<AppInfo>;
  quickFolders(): Promise<QuickFolders>;
  pickFolder(): Promise<string | null>;
  /** Native save dialog (dialog plugin) for Export CSV. */
  pickSavePath(defaultName: string): Promise<string | null>;
  checkFolder(path: string): Promise<FolderCheck>;
  startScan(path: string, options: ScanOptions, onEvent: (e: ScanEvent) => void): Promise<number>;
  cancelScan(id: number): Promise<void>;
  applySort(root: string, moves: Move[], recentHours: number): Promise<ActionResult>;
  trashPaths(root: string, paths: string[]): Promise<ActionResult>;
  trashDuplicates(root: string, removals: DupRemoval[]): Promise<ActionResult>;
  trashRedownloads(root: string, pairs: Redownload[], removing: string[]): Promise<ActionResult>;
  trashSimilar(root: string, removals: SimilarRemoval[]): Promise<ActionResult>;
  trashEmptyDirs(root: string, paths: string[]): Promise<ActionResult>;
  peekUndo(): Promise<UndoInfo | null>;
  undoLastSort(): Promise<ActionResult>;
  getSettings(): Promise<Settings>;
  saveSettings(settings: Settings): Promise<Settings>;
  addPin(path: string): Promise<Settings>;
  removePin(path: string): Promise<Settings>;
  listPresets(): Promise<Preset[]>;
  savePreset(preset: Preset): Promise<Preset[]>;
  deletePreset(id: string): Promise<Preset[]>;
  exportCsv(path: string, payload: ExportPayload): Promise<void>;
  spaceFreed(): Promise<SpaceFreed>;
  resetSpaceFreed(): Promise<SpaceFreed>;
  binInfo(): Promise<BinInfo>;
  openRecycleBin(): Promise<void>;
  revealInExplorer(path: string): Promise<void>;
  reminderStatus(): Promise<ReminderStatus>;
  reminderEnable(): Promise<ReminderStatus>;
  reminderDisable(): Promise<ReminderStatus>;
  /** A folder dropped on the window (Tauri drag and drop). Returns an unsubscribe function. */
  onFolderDrop(cb: (path: string) => void): () => void;
}

/** Errors from the shell are plain-language strings; anything else becomes a generic sentence, never a stack. */
export function errorText(e: unknown): string {
  if (typeof e === 'string' && e.trim()) return e;
  return 'Something went wrong. Please try again.';
}

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export const api: Api =
  typeof window !== 'undefined' && window.__TAURI_INTERNALS__ ? createTauriApi() : createMockApi();
