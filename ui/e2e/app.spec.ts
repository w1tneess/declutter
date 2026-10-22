// End to end against the production build with the mock Api (no Tauri). Keyboard only where it matters.
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';

async function axe(page: Page) {
  const r = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa']).analyze();
  expect(r.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(' | ')}`)).toEqual([]);
}

async function scan(page: Page, query = '?delay=20') {
  await page.goto(`/${query}`);
  await page.getByRole('button', { name: 'Downloads' }).click();
  await page.getByRole('button', { name: 'Scan', exact: true }).click();
  await expect(page.getByRole('status').first()).toContainText(/Scan finished/, { timeout: 15_000 });
}

for (const scheme of ['light', 'dark'] as const) {
  test(`first run and results pass axe (${scheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto('/?delay=20');
    await expect(page.getByRole('heading', { name: /drop a folder here/i })).toBeVisible();
    await axe(page);
    await page.getByRole('button', { name: 'Downloads' }).click();
    await page.getByRole('button', { name: 'Scan', exact: true }).click();
    await expect(page.getByRole('status').first()).toContainText(/Scan finished/, { timeout: 15_000 });
    await axe(page);
    for (const name of ['Duplicates', 'Large & old', 'Empty folders', 'Settings']) {
      await page.getByRole('navigation').getByRole('button', { name: new RegExp(`^${name}`) }).click();
      await axe(page);
    }
  });
}

test('progress becomes determinate with a time estimate, and Cancel stops the scan', async ({ page }) => {
  await page.goto('/?delay=400');
  await page.getByRole('button', { name: 'Downloads' }).click();
  await page.getByRole('button', { name: 'Scan', exact: true }).click();
  const bar = page.getByRole('progressbar', { name: 'Scan progress' });
  await expect(bar).toHaveAttribute('aria-valuenow', /\d+/, { timeout: 10_000 });
  await expect(page.getByText(/left$/)).toBeVisible({ timeout: 10_000 });
  await page.getByRole('button', { name: 'Cancel' }).click();
  await expect(page.getByRole('status').first()).toContainText('Cancelled');
  await expect(bar).toBeHidden();
});

test('duplicates: keyboard selection, preview dialog, Recycle Bin wording, result toast', async ({ page }) => {
  await scan(page);
  await page.keyboard.press('Control+3');
  const list = page.getByRole('listbox', { name: 'Duplicate files' });
  await list.focus();
  await page.keyboard.press('ArrowDown'); // first row is the keeper, second the copy
  await page.keyboard.press('Space');
  await expect(page.getByText(/^Selected: 1 item, /)).toBeVisible();
  await page.getByRole('button', { name: 'Send to Recycle Bin' }).click();
  const dialog = page.getByRole('dialog');
  await expect(dialog).toContainText('Send 1 item to the Recycle Bin? They are not deleted.');
  await expect(dialog.getByRole('button', { name: 'Send 1 item' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.getByText(/Sent 1 item to the Recycle Bin/)).toBeVisible();
});

test('sort then undo', async ({ page }) => {
  await scan(page);
  await page.getByRole('navigation').getByRole('button', { name: /^Sort by type/ }).click();
  await page.getByRole('button', { name: /^Sort \d+ files$/ }).click();
  await page.getByRole('dialog').getByRole('button', { name: /^Sort \d+ files$/ }).click();
  await expect(page.getByText(/^Moved \d+ files\.$/)).toBeVisible();
  await expect(page.getByText(/Nothing to sort/)).toBeVisible({ timeout: 15_000 });
  await page.getByRole('button', { name: 'Undo last sort' }).click();
  await page.getByRole('dialog').getByRole('button', { name: /^Undo \d+ files$/ }).click();
  await expect(page.getByText(/^Restored \d+ files\.$/)).toBeVisible();
});

test('a refused folder explains why and nothing is scanned', async ({ page }) => {
  await page.goto('/?pick=C:%5CWindows');
  await page.getByRole('button', { name: /choose folder/i }).click();
  await expect(page.getByRole('alertdialog')).toContainText('Declutter does not touch Windows');
  await page.keyboard.press('Escape');
  await expect(page.getByRole('button', { name: 'Scan', exact: true })).toHaveCount(0);
});

test('100,000 rows stay virtualized', async ({ page }) => {
  await scan(page, '?delay=0&rows=100000');
  await page.getByRole('navigation').getByRole('button', { name: /^Sort by type/ }).click();
  const list = page.getByRole('listbox', { name: 'Files to sort' });
  await expect(list).toBeVisible();
  expect(await list.getByRole('option').count()).toBeLessThan(80);
  await list.focus();
  await page.keyboard.press('End');
  await expect(list.locator('.is-active')).toBeVisible();
});
