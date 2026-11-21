# Declutter website

The site at the project's Vercel deployment: React 19 + Vite 8, one page. The root `vercel.json` builds it with
`--prefix website`; `website/vercel.json` mirrors the security headers.

## Commands
Run from `website/`:
- `npm ci`: install from the lockfile
- `npm run dev`: dev server
- `npm run lint`: oxlint (must pass)
- `npm run build`: production build into `dist/` (must pass)

## Layout
- `src/App.jsx`: the page (hero, try-it, tools table, safety, screenshots, sort folders, download, FAQ)
- `src/HeroFolder.jsx`: the simulated Downloads window in the hero
- `src/Demo.jsx`: the try-it simulation of the four tools; it moves no real files
- `src/data.js`: categories, sort rules and sample data, mirroring `src/declutter/core.py`
- `src/useGitHubRelease.js`: the only network request, to the public GitHub Releases API
- `src/index.css`: all styles; colour and type tokens at the top, light and dark from the Windows setting
- `public/`: the app screenshots, icon and the social-share image

## Rules
See [`AGENTS.md`](AGENTS.md): what the site may claim, the banned words, no third-party hosts, one change per PR.
