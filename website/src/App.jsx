import { useGitHubRelease } from './useGitHubRelease';
import HeroFolder from './HeroFolder';
import Demo from './Demo';
import { CATEGORIES } from './data';

const REPO = 'https://github.com/w1tneess/declutter';

const TOOLS = [
  {
    name: 'Sort by type',
    does: 'Moves loose files from the top of a folder into Images, Documents, Videos, Audio, Archives, Installers, Code and Other. Screenshots and chat pictures go into Images/Screenshots and Images/Chat.',
    holds: 'Shortcuts and .ini files stay put. So do downloads in progress and files touched in the last 24 hours (a setting, 0 to 72). A name clash becomes name (1); nothing is overwritten.',
  },
  {
    name: 'Duplicates',
    does: 'Finds files with the same size, the same first and last 64 KB, and then the same SHA-256 hash.',
    holds: 'The oldest copy is kept unless you pick another, and at least one copy always stays. Possible re-downloads such as report (1).pdf are listed apart and never ticked for you.',
  },
  {
    name: 'Large & old',
    does: 'Lists files by size and age. Both filters must match, and 0 turns a filter off.',
    holds: 'Nothing is ticked for you. Likely leftovers (unpacked zips, installers over 7 days old in Downloads, unfinished downloads over 3 days old) are marked, not selected.',
  },
  {
    name: 'Empty folders',
    does: 'Finds folders with no files at any depth. desktop.ini and Thumbs.db do not count.',
    holds: 'The deepest folders go first, and each goes to the Recycle Bin.',
  },
];

const STEPS = [
  ['Choose a folder', 'Pick Downloads, Desktop or Documents, or browse to any other folder.'],
  ['Scan', 'All four checks run in the background. You can cancel at any time.'],
  ['Review', 'Each tool lists what it found. Tick or untick anything.'],
  ['Confirm', 'A dialog states the exact number of files and where they go before anything changes.'],
];

const LEAVES_ALONE = [
  'Drive roots, your home folder, Windows, Program Files, ProgramData and AppData',
  'Code projects (.git, node_modules, .venv, package.json, .sln and similar) and portable-app folders',
  'Symlinks, junctions, hidden files and hidden folders',
  'Downloads in progress',
  'OneDrive online-only files. They are never read, so nothing gets downloaded',
  'Anything you pin under "Never touch"',
];

const FAQ = [
  ['Can Declutter permanently destroy my files?', 'Removals go to the Windows Recycle Bin on local fixed drives, where you can restore them. On USB sticks and network drives removals are refused, because those drives may have no Recycle Bin.'],
  ['How does Undo work?', 'Declutter logs the moves of your most recent sort. Undo last sort puts those files back and removes the empty folders it made. A file that changed since the sort is skipped and reported. Undo covers the last sort only. To bring back something sent to the Recycle Bin, restore it from the Recycle Bin.'],
  ['What if a file with the same name already exists?', 'Declutter never overwrites. The file being moved gets a counter instead, for example photo (1).jpg.'],
  ['Does it download my OneDrive online-only files?', 'No. Online-only placeholders are never read, moved or removed, so nothing is downloaded. Other cloud drives are not supported.'],
  ['How are duplicates found?', 'By content, not by name. Files must have the same size, the same first and last 64 KB, and then the same SHA-256 hash. The oldest copy is kept by default and at least one copy always remains.'],
  ['What are likely leftovers and possible re-downloads?', 'Large & old marks likely leftovers: zips already unpacked next to them, installers over 7 days old in Downloads, and unfinished downloads over 3 days old. Duplicates lists possible re-downloads such as report (1).pdf. Neither is ever ticked for you.'],
  ['Where does Declutter keep its data?', 'Settings and the undo log are in %LOCALAPPDATA%\\Declutter. The installer adds a Start menu entry and an uninstall entry for your account. A right-click "Tidy with Declutter" folder entry is added only if you tick that option.'],
  ['Does the app use the internet?', 'No. The app makes no network connections and has no telemetry. This website makes one request, to GitHub, to show the latest release.'],
  ['Does it need administrator rights?', 'No. It installs for your account only, with no UAC prompt.'],
  ['Does it work on USB drives and network folders?', 'You can scan and sort there. Removing files is refused on those drives.'],
];

const DownloadIcon = () => (
  <svg viewBox="0 0 20 20" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <path d="M10 3v9m0 0 3.5-3.5M10 12 6.5 8.5M4 16h12" />
  </svg>
);

export default function App() {
  const release = useGitHubRelease();

  return (
    <>
      <a className="skip" href="#main">Skip to content</a>

      <header className="site-nav">
        <div className="wrap nav-row">
          <a href="#top" className="brand" aria-label="Declutter, back to top">
            <svg viewBox="0 0 32 32" width="26" height="26" aria-hidden="true">
              <rect width="32" height="32" rx="8" fill="#0071E3" />
              <path d="M9 10a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v3a2 2 0 0 1-2 2H11a2 2 0 0 1-2-2v-3Z" fill="#fff" />
              <rect x="9" y="17" width="6" height="7" rx="2" fill="#fff" fillOpacity=".7" />
              <rect x="17" y="17" width="6" height="7" rx="2" fill="#fff" fillOpacity=".7" />
            </svg>
            <span>Declutter</span>
          </a>
          <nav aria-label="Page sections">
            <ul className="nav-links">
              <li><a href="#try">Try it</a></li>
              <li><a href="#tools">Tools</a></li>
              <li><a href="#safety">Safety</a></li>
              <li><a href="#faq">FAQ</a></li>
              <li><a href="#download" className="nav-download">Download</a></li>
            </ul>
          </nav>
        </div>
      </header>

      <main id="main">
        <section className="hero" id="top">
          <div className="wrap hero-grid">
            <div className="hero-copy">
              <h1>Tidy a messy folder. Check every change first.</h1>
              <p className="lead">
                Declutter sorts loose files, finds duplicates and large old files, and spots empty folders. It shows what it will do, asks before it does it, and sends removals to the Recycle Bin.
              </p>
              <div className="hero-actions">
                <a href={release.setupUrl} className="btn btn-primary btn-large">
                  <DownloadIcon />
                  Download for Windows{release.setupSize ? ` (${release.setupSize})` : ''}
                </a>
                <a href={REPO} className="btn btn-quiet btn-large">View source</a>
              </div>
              <p className="hero-fine">
                {release.version ? `Version ${release.version}. ` : ''}Free and open source (MIT) for Windows 10 and 11. The app is not code-signed, so Windows SmartScreen shows &ldquo;Unknown publisher&rdquo;: choose <strong>More info</strong>, then <strong>Run anyway</strong>.
              </p>
            </div>
            <HeroFolder />
          </div>
        </section>

        <section className="band" id="try">
          <div className="wrap">
            <h2>Try the four tools</h2>
            <p className="section-lead">
              Work through a sample Downloads folder. Every tool shows a preview, and every removal asks first, as it does in the app. This is a simulation in your browser: it moves no real files.
            </p>
            <Demo />
          </div>
        </section>

        <section className="band" id="tools">
          <div className="wrap">
            <h2>What each tool does, and what it holds back</h2>
            <div className="tool-table" role="table" aria-label="The four tools">
              <div className="tool-head" role="row">
                <span role="columnheader">Tool</span>
                <span role="columnheader">What it does</span>
                <span role="columnheader">What it holds back</span>
              </div>
              {TOOLS.map(t => (
                <div className="tool-row" role="row" key={t.name}>
                  <h3 role="rowheader">{t.name}</h3>
                  <p role="cell">{t.does}</p>
                  <p role="cell">{t.holds}</p>
                </div>
              ))}
            </div>
          </div>
        </section>

        <section className="band" id="safety">
          <div className="wrap safety-grid">
            <div>
              <h2>Nothing changes until you confirm</h2>
              <ol className="steps">
                {STEPS.map(([title, text]) => (
                  <li key={title}>
                    <strong>{title}.</strong> {text}
                  </li>
                ))}
              </ol>
              <h3>Undo and the Recycle Bin</h3>
              <p>
                Removals go to the Windows Recycle Bin, on local fixed drives only. On USB sticks and network drives they are refused. <strong>Undo last sort</strong> puts back the files of your most recent sort; it does not bring back items from the Recycle Bin, so restore those from the Bin itself.
              </p>
              <h3>Where your data stays</h3>
              <p>
                The app makes no network connections and has no telemetry. Settings and the undo log stay in <code className="mono">%LOCALAPPDATA%\Declutter</code>.
              </p>
            </div>
            <div>
              <h3 className="list-title">Declutter never enters or changes</h3>
              <ul className="plain-list">
                {LEAVES_ALONE.map(item => <li key={item}>{item}</li>)}
              </ul>
            </div>
          </div>
        </section>

        <section className="band" id="app">
          <div className="wrap">
            <h2>The app</h2>
            <p className="section-lead">
              It follows your Windows light or dark setting. The screenshots show generated test files.
            </p>
            <div className="shots">
              <figure>
                <img src="./app-duplicates-light.png" width="1100" height="720" loading="lazy" alt="The Duplicates screen in the light theme. Each group lists its copies with a Keep option on the oldest, and a Send to Recycle Bin button at the bottom." />
                <figcaption>Duplicates, light theme</figcaption>
              </figure>
              <figure>
                <img src="./app-large-old-dark.png" width="1375" height="900" loading="lazy" alt="The Large and old files screen in the dark theme, with size and age filters, nothing ticked, and a Send to Recycle Bin button." />
                <figcaption>Large &amp; old, dark theme</figcaption>
              </figure>
            </div>
          </div>
        </section>

        <section className="band" id="folders">
          <div className="wrap">
            <h2>Where sorted files go</h2>
            <dl className="folders">
              {CATEGORIES.map(([name, exts]) => (
                <div key={name} className="folders-row">
                  <dt>{name}</dt>
                  <dd className="mono">{exts.join(' ')}</dd>
                </div>
              ))}
              <div className="folders-row">
                <dt>Other</dt>
                <dd>Any other extension, so nothing is left without a home. Shortcuts and .ini files are never moved.</dd>
              </div>
            </dl>
          </div>
        </section>

        <section className="band" id="download">
          <div className="wrap">
            <h2>Download Declutter</h2>
            <p className="section-lead">
              Free and open source under the MIT license, for Windows 10 and 11.{release.releaseDate ? ` Latest release: ${release.version}, ${release.releaseDate}.` : ''}
            </p>
            <div className="downloads">
              <div className="download-card">
                <h3>Installer</h3>
                <p>Installs for your account only, with no administrator rights. Adds a Start menu entry and an uninstaller.</p>
                <a href={release.setupUrl} className="btn btn-primary">
                  <DownloadIcon />
                  Declutter-Setup.exe{release.setupSize ? ` (${release.setupSize})` : ''}
                </a>
              </div>
              <div className="download-card">
                <h3>Portable zip</h3>
                <p>Extract it anywhere, including a USB stick, and run Declutter.exe. Nothing is installed.</p>
                <a href={release.portableUrl} className="btn btn-quiet">
                  <DownloadIcon />
                  Declutter-portable.zip{release.portableSize ? ` (${release.portableSize})` : ''}
                </a>
              </div>
            </div>
            <p className="smartscreen">
              <strong>Windows may warn you the first time.</strong> Declutter is not code-signed, so SmartScreen shows &ldquo;Windows protected your PC&rdquo; and an unknown publisher. Choose <strong>More info</strong>, then <strong>Run anyway</strong>. The files are built in public on GitHub Actions and attached to the release, and the source and build scripts are in the <a href={REPO}>repository</a>.
            </p>
          </div>
        </section>

        <section className="band" id="faq">
          <div className="wrap faq-wrap">
            <h2>Questions</h2>
            <div className="faq">
              {FAQ.map(([q, a]) => (
                <details key={q}>
                  <summary>{q}</summary>
                  <p>{a}</p>
                </details>
              ))}
            </div>
          </div>
        </section>
      </main>

      <footer className="site-footer">
        <div className="wrap footer-row">
          <p>Declutter {release.version ?? ''} &middot; MIT License &middot; &copy; 2026 w1tneess</p>
          <ul>
            <li><a href={REPO}>GitHub</a></li>
            <li><a href={`${REPO}/releases`}>Releases</a></li>
            <li><a href={`${REPO}/issues`}>Report a problem</a></li>
            <li><a href="https://support.advaitachandra.in">Support the developer</a></li>
            <li><a href={`${REPO}/blob/main/LICENSE`}>License</a></li>
          </ul>
        </div>
      </footer>
    </>
  );
}
