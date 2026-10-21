# Declutter 1.0 performance targets and how to measure them

Owner: release. Targets are fixed here; numbers are recorded in the table at the bottom, never invented. Every
measurement uses the same synthetic tree, so v0.2.2 (Python) and 1.0 (Rust) are compared on identical input.

| # | Target | Where it is measured |
|---|---|---|
| 1 | Cold start to first paint under 400 ms | the 1.0 app, WebView2 first-contentful-paint |
| 2 | First results visible while the scan is still running | the 1.0 app, screen recording |
| 3 | A 100,000-file scan on an SSD never blocks the UI for more than 50 ms | the 1.0 app, Long Tasks API |
| 4 | 60 fps scrolling with 50,000 rows | the 1.0 app, frame timing |
| 5 | Peak RAM under 250 MB at 100,000 files, WebView included | the 1.0 app, process tree sampler |
| 6 | Rescan after an action under 150 ms | core benches, then the app end to end |

## Setup (once)

**The tree.** 100,000 files, about 1.7 GB, deterministic (same command, same tree). Put it on the SSD you mean to
measure, in a folder that does not exist yet (the script refuses drive roots, your home folder and AppData):

```powershell
python tools\gen_tree.py D:\perf\tree100k            # --files N and --seed S to change it
python tools\gen_tree.py D:\perf\tree10k --files 10000
```

It has about 5% exact duplicates, nested folders up to 8 deep, a long-tailed size mix, every file months old, and a few
project, portable-app and hidden folders a scan must skip. Check what a scan found against the script's printed count.

**The machine.** Same machine for every number, plugged in, power mode "Best performance", nothing else running,
Windows Defender left as it is (and noted in the results table) for both builds. A first run after a reboot is the
"cold" one; also record the median of 10 warm runs. The CI-built installer is unsigned: a machine with Smart App
Control in enforcement mode will refuse it (see `docs/DISTRIBUTION.md`), so measure on a machine without it, or use a
signed build.

**Probe prerequisite.** WebView2 can expose the Chrome DevTools Protocol. Start the app from a PowerShell window
that has this set, and Playwright (already a UI dev dependency, run from `ui/`) can attach to it:

```powershell
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9222'
```

Remote debugging itself costs a little, so measurement 1 is taken with it and the result is an upper bound.

## 1. Cold start to first paint (under 400 ms)

Two numbers, both from process creation:

```powershell
# a) main window handle exists (works for 1.0 and v0.2.2, so it is the comparable one)
$sw = [Diagnostics.Stopwatch]::StartNew()
$p = Start-Process .\Declutter.exe -PassThru
while (-not $p.HasExited) { $p.Refresh(); if ($p.MainWindowHandle -ne 0) { break }; Start-Sleep -Milliseconds 5 }
"window: $($sw.ElapsedMilliseconds) ms"

# b) first contentful paint inside the WebView (1.0 only): wall-clock of the paint minus process start
$start = [DateTimeOffset]::new($p.StartTime).ToUnixTimeMilliseconds()
node -e "const {chromium}=require('playwright');(async()=>{const b=await chromium.connectOverCDP('http://127.0.0.1:9222');const pg=b.contexts()[0].pages()[0];const t=await pg.evaluate(()=>performance.timeOrigin+performance.getEntriesByType('paint').find(e=>e.name==='first-contentful-paint').startTime);console.log('first paint:',Math.round(t-$start),'ms');process.exit(0)})()"
```

(b) is the target. Pass: under 400 ms on the first launch after a reboot. For v0.2.2, run (a) against `Declutter.exe` from
a PyInstaller build or `python -m declutter` from `src/`.

## 2. First results visible while scanning

Pass: Sort and Large and old rows are on screen while the progress bar still says the scan (duplicates, similar
images) is running. Measure it with a screen recording (Win+G, 60 fps): scan `tree100k`, then count frames from the click
on Scan to the first result row, and to the frame the progress bar disappears. Report both times. The 1.0 core streams
results phase by phase (`ScanEvent`, `docs/API.md`); v0.2.2 shows nothing until the whole scan has finished, which the
same recording documents.

## 3. UI never blocked for more than 50 ms during a 100k scan

The Long Tasks API reports any main-thread task over 50 ms, so the target is zero long tasks. Save this as
`ui/probe.mjs` (do not commit it), start the app with the probe prerequisite, run `node ui/probe.mjs`, perform the
action in the app, press Enter:

```js
import { chromium } from 'playwright';
import readline from 'node:readline/promises';
const page = (await (await chromium.connectOverCDP('http://127.0.0.1:9222')).contexts()[0]).pages()[0];
await page.evaluate(() => {
  const p = (window.__probe = { long: [], frames: [], last: 0, on: true });
  new PerformanceObserver((l) => l.getEntries().forEach((e) => p.long.push(e.duration))).observe({ type: 'longtask', buffered: false });
  const tick = (t) => { if (p.last) p.frames.push(t - p.last); p.last = t; if (p.on) requestAnimationFrame(tick); };
  requestAnimationFrame(tick);
});
const scroller = process.argv[2]; // optional CSS selector of the list: scrolls it for 10 s (measurement 4)
if (scroller) await page.evaluate((sel) => { const el = document.querySelector(sel); const end = performance.now() + 10000;
  const step = () => { el.scrollTop += 40; if (performance.now() < end) requestAnimationFrame(step); }; step(); }, scroller);
await readline.createInterface({ input: process.stdin, output: process.stdout }).question('Do the action, then press Enter... ');
const r = await page.evaluate(() => { window.__probe.on = false; const f = [...window.__probe.frames].sort((a, b) => a - b);
  return { longTasks: window.__probe.long.length, longestMs: Math.max(0, ...window.__probe.long),
    frames: f.length, p95FrameMs: f[Math.floor(f.length * 0.95)], slowFramePct: 100 * f.filter((x) => x > 25).length / f.length }; });
console.log(r);
process.exit(0);
```

Measurement 3: start the probe, click Scan on `tree100k`, press Enter when the scan has finished. Pass: `longTasks: 0`.

## 4. 60 fps scrolling with 50,000 rows

Put 50,000 rows in a list: scan `tree100k` and open Large and old with both filters at 0 (0 disables a filter, so every
file is listed). Run `node ui/probe.mjs "<selector of the list's scroll container>"`; it scrolls 40 px per frame for 10
s. Pass: `p95FrameMs` at or under 16.7 and `slowFramePct` under 1 (frames over 25 ms, i.e. a clearly dropped frame).

## 5. Peak RAM under 250 MB at 100k files

Sum of the app process and every child (the WebView2 helper processes are its children). Start the sampler right after
starting the app, scan `tree100k`, open the biggest list, then press Ctrl+C:

```powershell
$p = Get-Process Declutter
function Family($id) { $all = Get-CimInstance Win32_Process; $ids = @($id)
  do { $new = $all | Where-Object { $ids -contains $_.ParentProcessId -and $ids -notcontains $_.ProcessId } | ForEach-Object ProcessId; $ids += $new } while ($new)
  $ids }
$peak = 0
try { while ($true) {
  $mb = (Get-Process -Id (Family $p.Id) -ErrorAction SilentlyContinue | Measure-Object WorkingSet64 -Sum).Sum / 1MB
  if ($mb -gt $peak) { $peak = $mb }; Start-Sleep -Milliseconds 250 } }
finally { "peak working set: $([int]$peak) MB" }
```

`WorkingSet64` counts shared pages in every process, so it overstates; a pass here is a pass. If it is over 250 MB, redo
it with the private working set (`\Process(*)\Working Set - Private` in `Get-Counter`) before concluding anything.
For v0.2.2 use `Get-Process Declutter` the same way (one process; there is no WebView).

## 6. Rescan after an action (under 150 ms)

Two levels. The compute floor is the core benches, which need the tree (`DECLUTTER_BENCH_TREE` also accepts
`tree10k`; without it a 3,000-file tree is generated):

```powershell
$env:DECLUTTER_BENCH_TREE = 'D:\perf\tree100k'
cargo bench -p declutter-core --bench walk      # walk_files
cargo bench -p declutter-core --bench dupes     # cold_cache and warm_cache
```

`dupes/warm_cache` is the rescan: the hash cache is full, so no unchanged file may be hashed again. `walk` plus
`dupes/warm_cache` is what a full rescan costs the core. If `walk` alone is above 150 ms at 100k files, the target cannot
be met by rescanning everything and the rescan must be incremental (re-walk only the folders the action touched).
End to end, record the screen at 60 fps and count frames from the action's confirmation to the refreshed list.

The benches need the core modules to be implemented, and `cargo` runs only in CI or on a machine without Smart App
Control (`docs/ARCHITECTURE.md`). Output is plain text, with no HTML reports.

## v0.2.2 baseline (Python)

Same tree, same machine, same day. In a separate checkout of the tag (never in this working tree):

```powershell
git worktree add D:\perf\declutter-0.2.2 v0.2.2
cd D:\perf\declutter-0.2.2
python -m venv .venv; .\.venv\Scripts\pip install -r requirements.txt
$env:PYTHONPATH = 'src'
$py = '.\.venv\Scripts\python.exe'

# scan (walk) and duplicates, cold then immediately again (the second run has warm OS caches, v0.2.2 has no hash cache)
& $py -c "import sys,time; from declutter import core; r=sys.argv[1]; t=time.perf_counter(); e=core.scan_entries(r); print('walk', len(e), 'files', round(time.perf_counter()-t,2), 's'); t=time.perf_counter(); g=core.find_duplicates(r, entries=e); print('dupes', len(g), 'groups', round(time.perf_counter()-t,2), 's')" D:\perf\tree100k
```

Cold start, RAM and the UI measurements for v0.2.2 use the same snippets as above on the PyInstaller build
(`build.bat` produces `dist\Declutter\Declutter.exe`) or `python -m declutter` from `src`.

## Results

Fill in on the measurement day; leave a cell empty rather than guess. Add the machine (CPU, disk, Windows build), the
Defender state and the commit under test.

| # | Metric | Target | v0.2.2 | 1.0 | Date, commit |
|---|---|---|---|---|---|
| 1 | Window visible / first paint | under 400 ms | | | |
| 2 | First rows visible / scan finished | rows before finish | | | |
| 3 | Long tasks during 100k scan | 0 | | | |
| 4 | p95 frame time, 50k rows | 16.7 ms or less | | | |
| 5 | Peak RAM, 100k files | under 250 MB | | | |
| 6 | Rescan after an action | under 150 ms | | | |
| - | Core walk, 100k files | - | | | |
| - | Core duplicates, 100k, cold / warm cache | - | | | |
