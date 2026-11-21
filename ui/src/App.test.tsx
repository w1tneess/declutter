// The whole app against the mock Api: scan flow, safety rules in the store, keyboard paths and axe on every page.
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import axe from 'axe-core';
import { beforeAll, describe, expect, it } from 'vitest';

// The mock reads its options from the URL when the Api module loads, so set them first and import afterwards.
window.history.replaceState(null, '', '/?delay=0&rows=30');
const { App } = await import('./App');
const store = await import('./state/store');
const { useApp } = store;

async function scanned() {
  render(<App />);
  await waitFor(() => expect(useApp.getState().settings).not.toBeNull());
  await act(() => store.chooseFolder(String.raw`C:\Users\Alex\Downloads`));
  await act(() => store.startScan());
  await waitFor(() => expect(useApp.getState().status).toBe('done'));
}

async function noViolations() {
  // jsdom cannot compute colours; contrast is covered by contrast.test.ts and the Playwright axe run.
  const r = await axe.run(document.body, { rules: { 'color-contrast': { enabled: false } } });
  expect(r.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.html).join(' | ')}`)).toEqual([]);
}

beforeAll(() => {
  // jsdom has no layout; the virtualizer only needs a size to render rows.
  Object.defineProperty(HTMLElement.prototype, 'clientHeight', { configurable: true, get: () => 600 });
  Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get: () => 800 });
});

describe('first run', () => {
  it('shows the empty state with the folder choices and passes axe', async () => {
    render(<App />);
    expect(await screen.findByRole('heading', { name: /drop a folder here/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /choose folder/i })).toBeTruthy();
    await noViolations();
  });
});

describe('after a scan', () => {
  it('fills every tab, preselects only undoable work and passes axe on every page', async () => {
    await scanned();
    const s = useApp.getState();
    expect(s.sel.sort.count).toBe(s.results.sort?.moves.length);
    expect(s.sel.largeOld.count).toBe(0); // never preselected
    expect(s.sel.duplicates.count).toBe(0);
    // The oldest copy of every duplicate group is the keeper.
    for (const g of s.results.duplicates ?? []) expect(s.keep.has(g.files[0]?.path ?? '')).toBe(true);
    for (const page of ['home', 'sort', 'duplicates', 'similar', 'largeOld', 'empty', 'settings'] as const) {
      act(() => store.go(page));
      await noViolations();
    }
  });

  it('confirms with the exact count and the Recycle Bin wording before removing, and Esc cancels', async () => {
    await scanned();
    act(() => store.go('empty'));
    const list = await screen.findByRole('listbox', { name: 'Empty folders' });
    list.focus();
    fireEvent.keyDown(list, { key: 'a', ctrlKey: true });
    fireEvent.keyDown(list, { key: ' ' }); // untick the first: n - 1 left
    const n = useApp.getState().sel.empty.count;
    fireEvent.click(screen.getByRole('button', { name: 'Send to Recycle Bin' }));
    const dialog = await screen.findByRole('dialog');
    expect(dialog.textContent).toContain(`Send ${n} items to the Recycle Bin`);
    expect(dialog.textContent).toContain('restore them');
    fireEvent.keyDown(dialog, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(useApp.getState().results.emptyDirs?.length).toBeGreaterThan(n - 1); // nothing was removed
  });

  it('never lets every copy of a duplicate group be ticked', async () => {
    await scanned();
    act(() => store.go('duplicates'));
    const list = await screen.findByRole('listbox', { name: 'Duplicate files' });
    const keeper = useApp.getState().results.duplicates?.[0]?.files[0]?.path ?? '';
    const option = within(list).getAllByRole('option')[0] as HTMLElement;
    expect(option.getAttribute('aria-disabled')).toBe('true'); // the keeper row
    fireEvent.click(option);
    expect(useApp.getState().sel.duplicates.has(keeper)).toBe(false);
  });

  it('moves between pages with Ctrl+number', async () => {
    await scanned();
    fireEvent.keyDown(window, { key: '3', ctrlKey: true });
    expect(useApp.getState().page).toBe('duplicates');
  });
});
