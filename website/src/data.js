// Facts mirrored from the app's core.py (v0.2.2). If the app changes, change these.

export const CATEGORIES = [
  ['Images', ['.jpg', '.jpeg', '.png', '.gif', '.bmp', '.webp', '.svg', '.heic', '.tiff', '.ico']],
  ['Documents', ['.pdf', '.doc', '.docx', '.txt', '.rtf', '.odt', '.md', '.ppt', '.pptx', '.xls', '.xlsx', '.csv']],
  ['Videos', ['.mp4', '.mkv', '.avi', '.mov', '.wmv', '.flv', '.webm']],
  ['Audio', ['.mp3', '.wav', '.flac', '.aac', '.ogg', '.m4a']],
  ['Archives', ['.zip', '.rar', '.7z', '.tar', '.gz', '.bz2']],
  ['Installers', ['.exe', '.msi', '.msix', '.apk', '.dmg', '.iso']],
  ['Code', ['.py', '.js', '.ts', '.html', '.css', '.json', '.java', '.c', '.cpp', '.cs', '.ipynb', '.sql']],
];

const NEVER_MOVED = ['.lnk', '.url', '.ini'];
const IN_PROGRESS = ['.crdownload', '.part'];
const RECENT_HOURS = 24;

function extOf(name) {
  const i = name.lastIndexOf('.');
  return i > 0 ? name.slice(i).toLowerCase() : '';
}

/** Where the app would put a loose file, or why it would leave it alone. */
export function plan(file) {
  const ext = extOf(file.name);
  if (NEVER_MOVED.includes(ext)) return { stays: ext === '.ini' ? 'Settings file, never moved' : 'Shortcut, never moved' };
  if (IN_PROGRESS.includes(ext)) return { stays: 'Still downloading' };
  if (file.hoursOld < RECENT_HOURS) return { stays: `Touched in the last ${RECENT_HOURS} hours` };
  const hit = CATEGORIES.find(([, exts]) => exts.includes(ext));
  const category = hit ? hit[0] : 'Other';
  if (category === 'Images') {
    const stem = ext ? file.name.slice(0, -ext.length) : file.name;
    if (/^(screenshot|snip|capture)/i.test(stem)) return { dest: 'Images/Screenshots' };
    if (/^(IMG-\d{8}-WA\d+|WhatsApp Image)/i.test(stem)) return { dest: 'Images/Chat' };
  }
  return { dest: category };
}

export function formatSize(mb) {
  if (mb < 1) return `${Math.max(1, Math.round(mb * 1024))} KB`;
  if (mb < 1024) return `${mb >= 100 ? Math.round(mb) : mb.toFixed(1)} MB`;
  return `${(mb / 1024).toFixed(1)} GB`;
}

// One messy Downloads folder, used by the hero and by the try-it section.
export const SAMPLE_FILES = [
  { name: 'IMG_4471.jpg', mb: 3.4, ageDays: 5, hoursOld: 120 },
  { name: 'Screenshot 2026-10-12 091544.png', mb: 0.8, ageDays: 9, hoursOld: 216 },
  { name: 'invoice_october.pdf', mb: 0.4, ageDays: 3, hoursOld: 72 },
  { name: 'docker_setup.exe', mb: 48, ageDays: 62, hoursOld: 1488 },
  { name: 'interview_ep12.mp3', mb: 64, ageDays: 30, hoursOld: 720 },
  { name: 'Q3_report.pdf', mb: 14.2, ageDays: 17, hoursOld: 408 },
  { name: 'backup_assets.zip', mb: 185, ageDays: 46, hoursOld: 1104, unpacked: 'backup_assets' },
  { name: 'drone_footage.mp4', mb: 420, ageDays: 87, hoursOld: 2088 },
  { name: 'ubuntu_24_04.iso', mb: 320, ageDays: 119, hoursOld: 2856 },
  { name: 'notes.txt', mb: 0.002, ageDays: 11, hoursOld: 264 },
  { name: 'app.py', mb: 0.01, ageDays: 21, hoursOld: 504 },
  { name: 'Zoom.lnk', mb: 0.002, ageDays: 40, hoursOld: 960 },
  { name: 'film.mp4.crdownload', mb: 210, ageDays: 0, hoursOld: 2 },
  { name: 'draft_today.docx', mb: 0.3, ageDays: 0, hoursOld: 3 },
];

export const SAMPLE_DUPLICATES = [
  {
    id: 'q3',
    name: 'Q3_report.pdf',
    mb: 14.2,
    copies: [
      { path: 'Downloads\\Q3_report.pdf', modified: '28 Sep 2026' },
      { path: 'Downloads\\old\\Q3_report.pdf', modified: '12 Oct 2026' },
    ],
  },
  {
    id: 'drone',
    name: 'drone_footage.mp4',
    mb: 420,
    copies: [
      { path: 'Downloads\\drone_footage.mp4', modified: '2 Jul 2026' },
      { path: 'Downloads\\backup\\drone_copy.mp4', modified: '14 Oct 2026' },
      { path: 'Desktop\\drone_footage.mp4', modified: '15 Oct 2026' },
    ],
  },
];

export const SAMPLE_REDOWNLOADS = [
  { id: 'inv', name: 'invoice_october (1).pdf', original: 'invoice_october.pdf', mb: 0.41, note: '412 KB vs 414 KB' },
];

export const SAMPLE_EMPTY = [
  'Downloads\\cache\\temporary_build',
  'Downloads\\extracted_assets\\empty_media',
  'Downloads\\logs\\2025_sessions',
];
