# Brief for agents working on the Declutter website

You may redesign and rewrite anything inside `website/`. This file says what the site must stay true to.
The site tells people what a tool that touches their files will and will not do, so accuracy comes before style.

## Scope
- Edit only `website/` (and `vercel.json` if deployment needs it). Never touch `src/`, `tests/`, `installer/`,
  `.github/`, `SPEC.md`, `CLAUDE.md`, `ROADMAP.md` or the README.
- No new dependency without the owner's OK: name it, give its size and say why.
- One change = one branch = one PR. `npm run build` and `npm run lint` must pass. Never push to `main`.
- No tracking, analytics, cookies, fonts or scripts from third-party hosts. The only allowed request is the
  public GitHub Releases lookup in `useGitHubRelease.js`; do not describe the *site* as "offline".

## Sources of truth
1. `SPEC.md` (safety rules and feature table). 2. `README.md`. If the site disagrees with them, the site is wrong.
If a claim is not in those two files, do not write it. Ask the owner.

## Facts you can use
- Free, open source (MIT), Windows 10/11 only. Python app; no macOS or Linux build.
- Downloads: `Declutter-Setup.exe` (per-user install, no admin rights) and `Declutter-portable.zip`,
  from the latest GitHub Release. The app is **not code-signed**: Windows SmartScreen shows "Unknown publisher"
  (More info, then Run anyway). Say so next to the download button.
- The app makes no network connections and has no telemetry. Settings and the undo log stay in `%LOCALAPPDATA%\Declutter`.
- Four tools: Sort by type, Duplicates, Large & old, Empty folders. Each shows a preview and asks to confirm the exact count.
- Sort: top-level loose files only, into Images, Documents, Videos, Audio, Archives, Installers, Code, Other.
  Screenshots and chat pictures go to `Images/Screenshots` and `Images/Chat`. `.lnk`, `.url`, `.ini` are never moved.
  Files touched in the last 24 hours (setting, 0 to 72) are left alone. Name clashes become `name (1)`.
- Undo: "Undo last sort" puts the files of the **most recent sort** back. It does not undo Recycle Bin removals;
  restore those from the Recycle Bin. Files changed since the sort are skipped and reported.
- Duplicates: same size, then first and last 64 KB, then SHA-256. The oldest copy is kept by default; at least one
  copy always stays. "Possible re-downloads" (`name (1).pdf`) are never ticked for you.
- Large & old: filters by size and age. "Likely leftovers" (unpacked zips, installers over 7 days old in Downloads,
  unfinished downloads over 3 days old) are never ticked for you.
- Removals go to the Recycle Bin, **only on local fixed drives**. On USB sticks and network drives removal is refused
  (they may have no Recycle Bin). Scanning and sorting there is allowed.
- Never entered or touched: drive roots, the home folder, Windows, Program Files, ProgramData, AppData, code
  projects (`.git`, `node_modules`, `.venv`, `package.json`, `.sln`), symlinks and junctions, hidden files,
  downloads in progress, OneDrive online-only files. Anything the user pins under "Never touch".

## Words
- Banned: "100%", "guarantee", "impossible", "zero risk", "military-grade", "safe to delete", "safely deleted",
  "bit-for-bit", "instant", "perfect". Use plain, checkable statements ("goes to the Recycle Bin", "asks first").
- Say "likely leftover", not "safe to delete". Say "removed to the Recycle Bin", never "deleted".
- Do not attack other cleaners. Remove the "risky black boxes" comparison column; describe Declutter only.
- Do not promise features that are not built: folder watching, scheduling, cloud drives, drag and drop into the app,
  auto-update, Dropbox or Google Drive handling, macOS or Linux, AI.
- Plain words, short sentences, no jargon, no emoji in headings or toasts (the app has none).

## Claims on the current site to fix first (`src/App.jsx`, line numbers as of this brief)
| Line | Now says | Problem |
|---|---|---|
| ~551-552 | "100% Recycle Bin safety" | Not on USB/network drives; Undo is not Recycle Bin |
| ~1322 | "makes catastrophic file loss impossible" | Overpromise |
| ~1373, ~1381 | "100% restorable", "100% Windows Recycle Bin Guarantee" | Same; restoring is done from the Bin |
| ~560, ~1388 | "Instant 1-Click Undo ... recovery point" | Last sort only; skips changed files |
| ~1437 | "external USB storage" | Add that removals are refused there |
| ~1149, ~1159 | "guarantee true duplicates", "permanent keeper" | Say "compared by SHA-256"; keeper is a default the user can change |
| ~1330-1360 | Competitor column ("black boxes", "ad upsells") | Unverifiable; remove |
| ~266, ~575 | "100% private in your browser" demo | Keep, but label it a simulation: the demo is not the app and moves no real files |
| missing | v0.2.0 features | Add: likely leftovers, possible re-downloads, screenshot sub-folders, "Never touch" list, right-click "Tidy with Declutter" |

## Design
Match the app's calm look (see `DESIGN.md`, `src/declutter/theme.py`): accent `#0071E3`, Segoe UI / system font,
light and dark. Readable contrast (4.5:1 for body text), visible keyboard focus, `prefers-reduced-motion` respected,
works at phone width. Real screenshots of the real app beat illustrations; do not mock features the app lacks.

## Before you open the PR
Say what you checked: the build, the lint, each claim above against SPEC.md, light and dark, a narrow window.
List what you could not check (for example the live GitHub Release lookup, other browsers).
