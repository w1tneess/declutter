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

## Claims that must stay fixed
The 2026-11 redesign removed these. Do not bring them back, and check any new copy against them:
- No "100%", "guarantee", "impossible" or "instant". Recycle Bin removal is local fixed drives only; USB and network drives are refused.
- Undo is the **last sort only** and skips files changed since. It does not restore Recycle Bin items; the Bin does.
- Duplicates are "compared by SHA-256" (after size and the first and last 64 KB). The keeper is a default the user can change.
- No competitor comparison. Describe Declutter only.
- The try-it demo is labelled a simulation and moves no real files.
- The v0.2.0 features are covered: likely leftovers, possible re-downloads, screenshot sub-folders, "Never touch", the right-click "Tidy with Declutter" entry (FAQ).
- The page makes one request, to the GitHub Releases API (`useGitHubRelease.js`). No third-party fonts or scripts; `vercel.json` (root and `website/`) allows only the site itself and `api.github.com`.

## Keeping the site true to the app
`src/data.js` mirrors `CATEGORIES`, the sort rules and the left-alone rules in `src/declutter/core.py`. When those change, change `data.js` and the folder table in `src/App.jsx` in the same PR. Vercel previews sit behind a login, so check header changes by serving `dist` with the headers from `vercel.json` and looking for CSP violations in the console.

## Design
Match the app's calm look (see `DESIGN.md`, `src/declutter/theme.py`): accent `#0071E3`, Segoe UI / system font,
light and dark. Readable contrast (4.5:1 for body text), visible keyboard focus, `prefers-reduced-motion` respected,
works at phone width. Real screenshots of the real app beat illustrations; do not mock features the app lacks.

## Before you open the PR
Say what you checked: the build, the lint, each claim above against SPEC.md, light and dark, a narrow window.
List what you could not check (for example the live GitHub Release lookup, other browsers).
