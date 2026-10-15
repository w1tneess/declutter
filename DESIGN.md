# Declutter design tokens

Visual source of truth: the "Declutter UI" canvas (Home dark, Home light, Duplicates, Confirm dialog). Build the app UI to match it. Where this file and the canvas disagree, ask.

## Themes
Two themes, dark and light. Default follows the Windows setting (`System`); the user can override in Settings (System, Light, Dark) and the choice is remembered.

CustomTkinter supports this directly: pass `(light, dark)` tuples for every colour and call `customtkinter.set_appearance_mode("system" | "light" | "dark")`. Define all colours once in `theme.py` as `(light, dark)` tuples. No hex values in widget code.

## Colour tokens

| Token | Light | Dark | Used for |
|---|---|---|---|
| `bg` | `#F3F5F9` | `#0B0F17` | Window background |
| `surface` | `#FFFFFF` | `#121826` | Sidebar, title bar, cards, inputs |
| `surface_alt` | `#EEF1F6` | `#161D2D` | Group headers in lists |
| `border` | `#DFE4EC` | `#232B3B` | Card and panel borders |
| `border_control` | `#CBD2DE` | `#2E384C` | Button and input borders |
| `divider` | `#E8ECF2` | `#1C2434` | Row separators in lists |
| `text` | `#0F1623` | `#E8ECF4` | Primary text |
| `text_muted` | `#566176` | `#9AA4B5` | Secondary text, descriptions |
| `text_nav` | `#36415A` | `#B4BDCC` | Inactive nav and text buttons |
| `accent` | `#2F6FED` | `#2F6FED` | Primary button fill, logo, selected pill |
| `on_accent` | `#FFFFFF` | `#FFFFFF` | Text on accent fill |
| `accent_text` | `#1F4FB8` | `#8DB6FF` | Link-style buttons, active nav text |
| `accent_icon` | `#2F6FED` | `#7DAAFF` | Icons on tinted chips |
| `accent_soft` | `#E6EEFF` | `#1A2744` | Active nav background, icon chips, "Keep" badge |

Notes:
- Light values for `surface_alt` and `divider` are not drawn on the canvas yet (only Home light exists). They are first guesses. Check them visually.
- Contrast was estimated by hand, not measured with a tool. White on `#2F6FED` is about 4.5:1, the minimum for normal text. Verify the muted text colours with a contrast checker in both themes before release.
- Danger or warning colours are not defined yet. Nothing in v1 needs one, because removals go to the Recycle Bin.

## Type and shape
- Font: Segoe UI (Windows system font). Fallback: any default sans.
- Sizes: page title 28 semibold, card number 34 semibold, body 15, secondary 13.5, nav 14.5.
- Radius: cards 14, buttons 10, pills 18, nav items 8, dialog 16.
- Spacing: window padding 32 (vertical) and 40 (horizontal), gap 20 between sections, 16 between cards.
- Sidebar 232 wide. Window design size 1100 x 720; the app must also work when resized.
- Icons: simple outline icons, 1.75 stroke, 20 px in nav. No emoji.

## Behaviour rules the design assumes
- Scan runs in the background with progress and Cancel. The UI never freezes.
- Every destructive action shows the confirm dialog with the exact count first.
- Keep-one rule: in a duplicate group the oldest copy is marked "Keep" and cannot be selected for removal.
- Real checkboxes and buttons with visible keyboard focus in both themes.

## Apple-style revision
Palette, radii and flat cards now follow Apple's system look (grouped surfaces, system blue, hairline dividers). `src/declutter/theme.py` is the source of truth for the colour values; the table above is the earlier palette. Cards: 16, dialog: 14, page title 32. Cards and lists have no border; only sidebar and status bar keep a hairline.
