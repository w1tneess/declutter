# Declutter design (1.0 UI)

The React UI in `ui/` follows Windows 11 (Fluent) conventions. `ui/src/design/tokens.css` is the only place colours,
type, spacing, radii, elevation and motion are defined; component styles (`app.css`) and code use the tokens only,
never a literal colour. The v0.2.2 CustomTkinter palette (`src/declutter/theme.py`) is superseded for 1.0.

## Themes
Light and dark. `System` (default) follows Windows through `prefers-color-scheme`; Settings can force Light or Dark
(`data-theme` on `<html>`), saved in settings. The dark background is `#202020`, never black.

## Colour tokens
| Token | Light | Dark | Used for |
|---|---|---|---|
| `bg` | `#F3F3F3` | `#202020` | Window background |
| `surface` | `#FFFFFF` | `#2B2B2B` | Cards, lists, inputs, dialogs |
| `surface-alt` | `#F9F9F9` | `#272727` | Sidebar, status bar, group headers |
| `surface-hover` / `-pressed` | `#F0F0F0` / `#E8E8E8` | `#323232` / `#292929` | Hover and pressed fills |
| `selected` | `#E6F0FA` | `#19324A` | Ticked rows |
| `border` | `#E3E3E3` | `#3A3A3A` | Card and divider hairlines (decorative) |
| `border-control` | `#858585` | `#8F8F8F` | Button, input, checkbox and switch borders (>= 3:1) |
| `text` / `text-muted` | `#1B1B1B` / `#5C5C5C` | `#FFFFFF` / `#C8C8C8` | Primary and secondary text |
| `accent` / `on-accent` | `#005FB8` / `#FFFFFF` | `#60CDFF` / `#000000` | Primary button, checked controls, progress |
| `accent-text` | `#005A9E` | `#8FDCFF` | Links, subtle buttons, summary hints |
| `danger` / `danger-text` | `#C42B1C` / `#B3261A` | `#FF99A4` / `#FFADB6` | Errors; danger buttons (non-file actions only) |
| `warning-text`, `success-text` | `#8A5300`, `#0E700E` | `#FCE100`, `#6CCB5F` | "Likely leftover" badge, success |
| `focus` | `#000000` | `#FFFFFF` | 2 px focus ring |

Contrast is measured, not estimated: `ui/src/design/contrast.test.ts` parses `tokens.css` and fails CI if any text pair
drops below 4.5:1 or a control border, focus ring or accent graphic below 3:1, in either theme. Playwright also runs
axe (WCAG 2.2 AA, including colour contrast) on Home, Duplicates, Large & old, Empty folders and Settings in both themes.

## Type, spacing and shape
- Font: Segoe UI Variable (Text for body, Display for titles), Segoe UI fallback.
- Type scale 12 / 13 / 14 / 20 / 32 (caption, small, body, subtitle, title). Nothing else.
- Spacing on a 4 / 8 grid (`--sp-1` 4 px to `--sp-10` 40 px). Page padding 32 / 40.
- Radius 4 for controls, 8 for cards and overlays. Controls 36 px high; rows, nav items and main actions 44 px.
- Icons: Fluent UI System Icons (MIT), 20 px, path data in `icons.ts`; filled variant for the current page. No emoji.
- Sidebar 248 px; below 720 px it collapses to icons (labels stay as accessible names).

## Behaviour the design assumes
- Every result list is virtualized (`@tanstack/react-virtual`, fixed row heights); 100,000 rows keep a constant DOM.
- Scans stream results per phase; each tab says "Looking for ..." until its part arrives. The status bar shows the
  phase label, a progress bar that turns determinate once the total is known, an ETA ("About 2 minutes left") once
  hashing has a rate, and Cancel.
- First run: Home shows a drop zone with Downloads, Desktop, Documents and "Choose folder". Each tab without results
  points back to Home; an empty result says "Nothing to clean. Nice."
- Safety (SPEC rules 1, 2, 3, 6): nothing changes without a confirm dialog that states the exact count; removals say
  they go to the Recycle Bin and can be restored there; Sort shows "Undo last sort". Only moves (undoable) and empty
  folders start ticked; duplicates, re-downloads, similar pictures, large and leftover files never do. A duplicate
  group's keeper cannot be ticked, and a selection that would remove every copy is refused.
- Button hierarchy: one filled accent button per view (Scan, the footer action, the dialog's confirm). Secondary
  actions are bordered; Select all / none are subtle text buttons.
- Keyboard: everything is reachable with Tab. Lists are listboxes: arrows, Home/End, PageUp/PageDown move, Space
  ticks, Ctrl+A selects all, Enter shows details, K keeps a duplicate. Ctrl+1-7 switch pages, Ctrl+F focuses the
  filter. Dialogs trap focus, close on Esc and return focus. Tooltips close on Esc.
- Motion: Fluent durations (83 / 167 / 250 ms); all zero under `prefers-reduced-motion`.
- Results: success is a toast (about 4.5 s); results with problems open a dialog listing them.
