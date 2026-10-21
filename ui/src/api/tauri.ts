// Real client: Tauri 2 commands (docs/API.md). Argument names are camelCase; Tauri maps them to the Rust snake_case.
import { Channel, invoke } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { save } from '@tauri-apps/plugin-dialog';
import type { Api } from './client';
import type { ScanEvent } from './generated/ScanEvent';

export function createTauriApi(): Api {
  return {
    appInfo: () => invoke('app_info'),
    quickFolders: () => invoke('quick_folders'),
    pickFolder: () => invoke('pick_folder'),
    pickSavePath: (defaultName) => save({ defaultPath: defaultName, filters: [{ name: 'CSV file', extensions: ['csv'] }] }),
    checkFolder: (path) => invoke('check_folder', { path }),
    startScan: (path, options, onEvent) => {
      const channel = new Channel<ScanEvent>();
      channel.onmessage = onEvent;
      return invoke('start_scan', { path, options, onEvent: channel });
    },
    cancelScan: (id) => invoke('cancel_scan', { id }),
    applySort: (root, moves, recentHours) => invoke('apply_sort', { root, moves, recentHours }),
    trashPaths: (root, paths) => invoke('trash_paths', { root, paths }),
    trashDuplicates: (root, removals) => invoke('trash_duplicates', { root, removals }),
    trashRedownloads: (root, pairs, removing) => invoke('trash_redownloads', { root, pairs, removing }),
    trashSimilar: (root, removals) => invoke('trash_similar', { root, removals }),
    trashEmptyDirs: (root, paths) => invoke('trash_empty_dirs', { root, paths }),
    peekUndo: () => invoke('peek_undo'),
    undoLastSort: () => invoke('undo_last_sort'),
    getSettings: () => invoke('get_settings'),
    saveSettings: (settings) => invoke('save_settings', { settings }),
    addPin: (path) => invoke('add_pin', { path }),
    removePin: (path) => invoke('remove_pin', { path }),
    listPresets: () => invoke('list_presets'),
    savePreset: (preset) => invoke('save_preset', { preset }),
    deletePreset: (id) => invoke('delete_preset', { id }),
    exportCsv: (path, payload) => invoke('export_csv', { path, payload }),
    spaceFreed: () => invoke('space_freed'),
    resetSpaceFreed: () => invoke('reset_space_freed'),
    binInfo: () => invoke('bin_info'),
    openRecycleBin: () => invoke('open_recycle_bin'),
    revealInExplorer: (path) => invoke('reveal_in_explorer', { path }),
    reminderStatus: () => invoke('reminder_status'),
    reminderEnable: (schedule) => invoke('reminder_enable', { schedule }),
    reminderDisable: () => invoke('reminder_disable'),
    onFolderDrop: (cb) => {
      const off = getCurrentWebview().onDragDropEvent((e) => {
        const first = e.payload.type === 'drop' ? e.payload.paths[0] : undefined;
        if (first) cb(first);
      });
      return () => void off.then((f) => f());
    },
  };
}
