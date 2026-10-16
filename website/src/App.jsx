import React, { useState, useEffect, useRef } from 'react';
import { useGitHubRelease } from './useGitHubRelease';

function formatFileSize(bytes) {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
  return (bytes / (1024 * 1024 * 1024)).toFixed(2) + ' GB';
}

function getFileCategory(name) {
  const ext = (name.split('.').pop() || '').toLowerCase();
  if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'bmp', 'ico'].includes(ext)) {
    return { category: 'Images', icon: '🖼️', ext: ext.toUpperCase() };
  }
  if (['pdf', 'docx', 'doc', 'xlsx', 'xls', 'pptx', 'csv', 'txt', 'md', 'rtf'].includes(ext)) {
    return { category: 'Documents', icon: '📄', ext: ext.toUpperCase() };
  }
  if (['mp4', 'mov', 'mkv', 'avi', 'webm', 'wmv'].includes(ext)) {
    return { category: 'Videos & Media', icon: '🎬', ext: ext.toUpperCase() };
  }
  if (['mp3', 'wav', 'm4a', 'flac', 'aac', 'ogg'].includes(ext)) {
    return { category: 'Videos & Media', icon: '🎵', ext: ext.toUpperCase() };
  }
  if (['zip', 'rar', '7z', 'tar', 'gz', 'iso'].includes(ext)) {
    return { category: 'Archives & Tools', icon: '📦', ext: ext.toUpperCase() };
  }
  if (['exe', 'msi', 'bat', 'cmd'].includes(ext)) {
    return { category: 'Archives & Tools', icon: '⚙️', ext: ext.toUpperCase() };
  }
  if (['js', 'jsx', 'ts', 'tsx', 'py', 'json', 'yaml', 'yml', 'html', 'css'].includes(ext)) {
    return { category: 'Code & Data', icon: '💻', ext: ext.toUpperCase() };
  }
  return { category: 'Other Files', icon: '📎', ext: ext.toUpperCase() || 'FILE' };
}

const SAMPLE_FILES = [
  { id: 1, name: 'Q3_Financial_Report.pdf', category: 'Documents', size: 14.2, sizeStr: '14.2 MB', ext: 'PDF', icon: '📄', date: '2026-09-28', ageDays: 17 },
  { id: 2, name: 'product_hero_banner.png', category: 'Images', size: 3.4, sizeStr: '3.4 MB', ext: 'PNG', icon: '🖼️', date: '2026-10-10', ageDays: 5 },
  { id: 3, name: 'docker_engine_setup.exe', category: 'Archives & Tools', size: 48.0, sizeStr: '48.0 MB', ext: 'EXE', icon: '⚙️', date: '2026-08-14', ageDays: 62 },
  { id: 4, name: '4K_Drone_Footage.mp4', category: 'Videos & Media', size: 420.0, sizeStr: '420.0 MB', ext: 'MP4', icon: '🎬', date: '2026-07-20', ageDays: 87 },
  { id: 5, name: 'customer_metrics_oct.xlsx', category: 'Documents', size: 1.8, sizeStr: '1.8 MB', ext: 'XLSX', icon: '📊', date: '2026-10-12', ageDays: 3 },
  { id: 6, name: 'brand_identity_vector.svg', category: 'Images', size: 0.8, sizeStr: '850 KB', ext: 'SVG', icon: '🎨', date: '2026-10-01', ageDays: 14 },
  { id: 7, name: 'raw_video_archive.mov', category: 'Videos & Media', size: 850.0, sizeStr: '850.0 MB', ext: 'MOV', icon: '🎥', date: '2026-05-10', ageDays: 158 },
  { id: 8, name: 'project_backup_assets.zip', category: 'Archives & Tools', size: 185.0, sizeStr: '185.0 MB', ext: 'ZIP', icon: '📦', date: '2026-08-30', ageDays: 46 },
  { id: 9, name: 'podcast_interview_ep12.mp3', category: 'Videos & Media', size: 64.0, sizeStr: '64.0 MB', ext: 'MP3', icon: '🎵', date: '2026-09-15', ageDays: 30 },
  { id: 10, name: 'ubuntu_24_04_desktop.iso', category: 'Archives & Tools', size: 320.0, sizeStr: '320.0 MB', ext: 'ISO', icon: '💿', date: '2026-06-18', ageDays: 119 },
];

const INITIAL_DUPLICATES = [
  {
    id: 'dup-1',
    name: '4K_Drone_Footage.mp4',
    sizeStr: '420.0 MB',
    sizeBytes: 420.0,
    hash: 'sha256: 7f3a9b8120c4e198f824e8bc19d3...',
    copies: [
      { id: 'c1', path: 'Downloads/4K_Drone_Footage.mp4', modified: 'Oct 2, 2026 (Oldest Original)', keeper: true },
      { id: 'c2', path: 'Downloads/backup/4K_Drone_Footage_copy.mp4', modified: 'Oct 14, 2026 (Recent Copy)', keeper: false },
    ],
  },
  {
    id: 'dup-2',
    name: 'Q3_Financial_Report.pdf',
    sizeStr: '14.2 MB',
    sizeBytes: 14.2,
    hash: 'sha256: d24e18ac499bf01a357fbb4501a9...',
    copies: [
      { id: 'c3', path: 'Downloads/Q3_Financial_Report.pdf', modified: 'Sep 28, 2026 (Oldest Original)', keeper: true },
      { id: 'c4', path: 'Downloads/Q3_Financial_Report (1).pdf', modified: 'Oct 12, 2026 (Recent Copy)', keeper: false },
    ],
  },
];

const INITIAL_EMPTY_FOLDERS = [
  { id: 'ef1', path: 'Downloads/cache/temporary_build', details: 'Deep nested branch with 0 files' },
  { id: 'ef2', path: 'Downloads/extracted_assets/empty_media', details: 'Empty extracted target with 0 files' },
  { id: 'ef3', path: 'Downloads/logs/2025_archived_sessions', details: 'Stale log tree with 0 files' },
];

const APP_EXTENSIONS_MAP = [
  {
    name: 'Images',
    icon: '🖼️',
    color: '#38BDF8',
    exts: ['.jpg', '.jpeg', '.png', '.gif', '.bmp', '.webp', '.svg', '.heic', '.tiff', '.ico'],
    description: 'High-res photos, vectors, raw camera exports, and UI assets. Screenshots and chat pictures route to Images/Screenshots and Images/Chat.',
  },
  {
    name: 'Documents',
    icon: '📄',
    color: '#818CF8',
    exts: ['.pdf', '.doc', '.docx', '.txt', '.rtf', '.odt', '.md', '.ppt', '.pptx', '.xls', '.xlsx', '.csv'],
    description: 'Financial reports, spreadsheets, presentations, and markdown notes.',
  },
  {
    name: 'Videos',
    icon: '🎬',
    color: '#F472B6',
    exts: ['.mp4', '.mkv', '.avi', '.mov', '.wmv', '.flv', '.webm'],
    description: 'Screen recordings, movie clips, drone footage, and camera captures.',
  },
  {
    name: 'Audio',
    icon: '🎵',
    color: '#A78BFA',
    exts: ['.mp3', '.wav', '.flac', '.aac', '.ogg', '.m4a'],
    description: 'Music tracks, podcast master tracks, voice memos, and audio stems.',
  },
  {
    name: 'Archives',
    icon: '📦',
    color: '#FBBF24',
    exts: ['.zip', '.rar', '.7z', '.tar', '.gz', '.bz2'],
    description: 'Compressed bundles, multi-volume zips, and tarball archives.',
  },
  {
    name: 'Installers',
    icon: '⚙️',
    color: '#34D399',
    exts: ['.exe', '.msi', '.msix', '.apk', '.dmg', '.iso'],
    description: 'Setup packages, disk images, standalone binaries, and disc backups.',
  },
  {
    name: 'Code & Data',
    icon: '💻',
    color: '#60A5FA',
    exts: ['.py', '.js', '.ts', '.html', '.css', '.json', '.java', '.c', '.cpp', '.cs', '.ipynb', '.sql'],
    description: 'Source code files, Jupyter notebooks, datasets, and scripts.',
  },
  {
    name: 'Other',
    icon: '📎',
    color: '#94A3B8',
    exts: ['Any custom extension'],
    description: 'Any unclassified extensions are safely routed to Other/ without data loss.',
  },
];

export default function App() {
  const release = useGitHubRelease();
  const [scrolled, setScrolled] = useState(false);
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const [activeFaq, setActiveFaq] = useState(null);
  const [smartScreenOpen, setSmartScreenOpen] = useState(false);
  const [downloadFormat, setDownloadFormat] = useState('setup'); // 'setup' | 'portable'
  const [copiedHash, setCopiedHash] = useState(false);
  const [extQuery, setExtQuery] = useState('');

  // ==================== Web-Native Declutter Studio State ====================
  const [dataSource, setDataSource] = useState('sample'); // 'sample' | 'custom'
  const [customFiles, setCustomFiles] = useState([]);
  const [isDragging, setIsDragging] = useState(false);
  const fileInputRef = useRef(null);

  const [activeTab, setActiveTab] = useState('sort'); // 'sort' | 'duplicates' | 'large' | 'empty'
  const [isSorted, setIsSorted] = useState(false);
  const [sortAnimating, setSortAnimating] = useState(false);
  const [selectedCategories, setSelectedCategories] = useState({
    'Images': true,
    'Documents': true,
    'Videos & Media': true,
    'Archives & Tools': true,
    'Code & Data': true,
    'Other Files': true,
  });

  const [duplicates, setDuplicates] = useState(INITIAL_DUPLICATES);
  const [emptyFolders, setEmptyFolders] = useState(INITIAL_EMPTY_FOLDERS);
  const [sizeThreshold, setSizeThreshold] = useState(100); // MB
  const [ageThreshold, setAgeThreshold] = useState(40); // Days
  const [selectedLargeIds, setSelectedLargeIds] = useState({});
  const [recycledItems, setRecycledItems] = useState([]);
  const [reclaimedMB, setReclaimedMB] = useState(0);
  const [toast, setToast] = useState(null);
  const [confirmDialog, setConfirmDialog] = useState(null);

  const currentFiles = dataSource === 'custom' && customFiles.length > 0 ? customFiles : SAMPLE_FILES;

  useEffect(() => {
    const onScroll = () => {
      setScrolled(window.scrollY > 30);
    };
    window.addEventListener('scroll', onScroll, { passive: true });
    return () => window.removeEventListener('scroll', onScroll);
  }, []);

  const showToast = (msg) => {
    setToast(msg);
    setTimeout(() => {
      setToast(prev => prev === msg ? null : prev);
    }, 3500);
  };

  const toggleFaq = (idx) => {
    setActiveFaq(prev => prev === idx ? null : idx);
  };

  const handleCopyHash = () => {
    const mockHash = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855';
    navigator.clipboard?.writeText(mockHash);
    setCopiedHash(true);
    setTimeout(() => setCopiedHash(false), 2200);
  };

  // --- File Upload / Drag & Drop Logic (Client-Side Simulation) ---
  const processUploadedFiles = (filesList) => {
    const fileArray = Array.from(filesList);
    if (fileArray.length === 0) return;

    const parsed = fileArray.map((f, idx) => {
      const { category, icon, ext } = getFileCategory(f.name);
      const sizeMB = f.size / (1024 * 1024);
      const now = Date.now();
      const lastMod = f.lastModified || now;
      const ageDays = Math.max(0, Math.floor((now - lastMod) / (1000 * 60 * 60 * 24)));
      return {
        id: `custom-${idx}-${f.name}`,
        name: f.name,
        category,
        size: sizeMB,
        sizeStr: formatFileSize(f.size),
        ext,
        icon,
        date: new Date(lastMod).toISOString().slice(0, 10),
        ageDays,
      };
    });

    setCustomFiles(parsed);
    setDataSource('custom');
    setIsSorted(false);
    setSelectedLargeIds({});

    // Detect duplicate groups from custom files based on size & name similarity
    const sizeMap = {};
    parsed.forEach(p => {
      const key = `${p.name}-${p.size.toFixed(2)}`;
      if (!sizeMap[key]) sizeMap[key] = [];
      sizeMap[key].push(p);
    });

    const detectedDups = [];
    Object.values(sizeMap).forEach((group, gIdx) => {
      if (group.length > 1) {
        detectedDups.push({
          id: `custom-dup-${gIdx}`,
          name: group[0].name,
          sizeStr: group[0].sizeStr,
          sizeBytes: group[0].size,
          hash: `sha256: ${Math.random().toString(16).substring(2, 10)}...${Math.random().toString(16).substring(2, 6)}`,
          copies: group.map((item, cIdx) => ({
            id: `cc-${item.id}`,
            path: `Loaded/${item.name}${cIdx > 0 ? ` (Copy ${cIdx})` : ''}`,
            modified: `${item.date} (${cIdx === 0 ? 'Original' : 'Duplicate'})`,
            keeper: cIdx === 0,
          })),
        });
      }
    });

    if (detectedDups.length > 0) {
      setDuplicates(detectedDups);
    }

    showToast(`Loaded ${parsed.length} local files into browser simulation. Moves no real files.`);
  };

  const handleFileInputChange = (e) => {
    if (e.target.files && e.target.files.length > 0) {
      processUploadedFiles(e.target.files);
    }
  };

  const handleDragOver = (e) => {
    e.preventDefault();
    setIsDragging(true);
  };

  const handleDragLeave = (e) => {
    e.preventDefault();
    setIsDragging(false);
  };

  const handleDrop = (e) => {
    e.preventDefault();
    setIsDragging(false);
    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      processUploadedFiles(e.dataTransfer.files);
    }
  };

  const handleSwitchToSample = () => {
    setDataSource('sample');
    setIsSorted(false);
    setDuplicates(INITIAL_DUPLICATES);
    setSelectedLargeIds({});
    showToast('Switched to Sample Downloads sandbox.');
  };

  // --- Sort Logic ---
  const toggleCategorySelection = (cat) => {
    setSelectedCategories(prev => ({
      ...prev,
      [cat]: !prev[cat],
    }));
  };

  const handleSortFiles = () => {
    setSortAnimating(true);
    setTimeout(() => {
      setIsSorted(true);
      setSortAnimating(false);
      const organizedCount = currentFiles.filter(f => selectedCategories[f.category]).length;
      showToast(`Organized ${organizedCount} files into structured category folders.`);
    }, 500);
  };

  const handleUndoSort = () => {
    setIsSorted(false);
    showToast('Undo last sort: restored simulation files back to root.');
  };

  // --- Duplicate Logic ---
  const handleToggleDupKeeper = (dupId, copyId) => {
    setDuplicates(prev => prev.map(grp => {
      if (grp.id !== dupId) return grp;
      return {
        ...grp,
        copies: grp.copies.map(c => ({
          ...c,
          keeper: c.id === copyId,
        })),
      };
    }));
  };

  const handleRecycleDuplicates = () => {
    const duplicatesToRemove = duplicates.map(grp => {
      const duplicateCopy = grp.copies.find(c => !c.keeper);
      return {
        name: grp.name,
        path: duplicateCopy ? duplicateCopy.path : grp.name,
        size: grp.sizeBytes,
        sizeStr: grp.sizeStr,
        type: 'Duplicate File',
      };
    });

    const spaceGain = duplicates.reduce((acc, curr) => acc + curr.sizeBytes, 0);

    setRecycledItems(prev => [...prev, ...duplicatesToRemove]);
    setReclaimedMB(prev => prev + spaceGain);
    setDuplicates([]);
    setConfirmDialog(null);
    showToast(`Moved ${duplicatesToRemove.length} duplicate copies to the Recycle Bin (+${spaceGain.toFixed(1)} MB).`);
  };

  // --- Large Files Logic ---
  const filteredLargeFiles = currentFiles.filter(
    f => f.size >= sizeThreshold || f.ageDays >= ageThreshold
  );

  const toggleSelectLarge = (id) => {
    setSelectedLargeIds(prev => ({ ...prev, [id]: !prev[id] }));
  };

  const handleRecycleSelectedLarge = () => {
    const targets = filteredLargeFiles.filter(f => selectedLargeIds[f.id]);
    if (targets.length === 0) return;

    const spaceGain = targets.reduce((sum, f) => sum + f.size, 0);
    const recycledEntries = targets.map(t => ({
      name: t.name,
      path: t.name,
      size: t.size,
      sizeStr: t.sizeStr,
      type: 'Large File',
    }));

    setRecycledItems(prev => [...prev, ...recycledEntries]);
    setReclaimedMB(prev => prev + spaceGain);
    setSelectedLargeIds({});
    setConfirmDialog(null);
    showToast(`Sent ${targets.length} large files to Recycle Bin (+${spaceGain.toFixed(1)} MB).`);
  };

  // --- Empty Folders Logic ---
  const handlePruneEmptyFolders = () => {
    const count = emptyFolders.length;
    setEmptyFolders([]);
    showToast(`Pruned ${count} empty folder branches deepest-first.`);
  };

  // --- Restore from Recycle Bin ---
  const handleRestoreAll = () => {
    const count = recycledItems.length;
    setRecycledItems([]);
    setDuplicates(INITIAL_DUPLICATES);
    setEmptyFolders(INITIAL_EMPTY_FOLDERS);
    setReclaimedMB(0);
    showToast(`Restored ${count} items back to their active locations.`);
  };

  const handleResetAllDemo = () => {
    setIsSorted(false);
    setDuplicates(INITIAL_DUPLICATES);
    setEmptyFolders(INITIAL_EMPTY_FOLDERS);
    setSizeThreshold(100);
    setAgeThreshold(40);
    setSelectedLargeIds({});
    setRecycledItems([]);
    setReclaimedMB(0);
    setSelectedCategories({
      'Images': true,
      'Documents': true,
      'Videos & Media': true,
      'Archives & Tools': true,
      'Code & Data': true,
      'Other Files': true,
    });
    showToast('Simulation reset to initial state.');
  };

  // Categories currently present in the active file set
  const availableCategories = Array.from(new Set(currentFiles.map(f => f.category)));

  // Group sorted files into category buckets
  const sortedBuckets = availableCategories.filter(cat => selectedCategories[cat]).map(cat => {
    const items = currentFiles.filter(f => f.category === cat);
    const totalSize = items.reduce((acc, curr) => acc + curr.size, 0);
    return {
      category: cat,
      items,
      count: items.length,
      sizeStr: formatFileSize(totalSize * 1024 * 1024),
    };
  });

  const unsortedItems = currentFiles.filter(f => !selectedCategories[f.category]);

  return (
    <div className="app-root">
      {/* ==================== Unified Global Ambient Canvas ==================== */}
      <div className="global-canvas-backdrop" aria-hidden="true">
        <div className="ambient-mesh-glow ambient-glow-top"></div>
        <div className="ambient-mesh-glow ambient-glow-middle"></div>
        <div className="ambient-mesh-glow ambient-glow-bottom"></div>
        <div className="ambient-grid-overlay"></div>
      </div>

      {/* ==================== Navigation ==================== */}
      <nav className={`navbar ${scrolled ? 'scrolled' : ''}`}>
        <div className="container nav-container">
          <a href="#" className="brand">
            <svg className="brand-icon" viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg">
              <rect width="32" height="32" rx="8" fill="#2F6FED" />
              <path d="M9 10C9 8.89543 9.89543 8 11 8H21C22.1046 8 23 8.89543 23 10V13C23 14.1046 22.1046 15 21 15H11C9.89543 15 9 14.1046 9 13V10Z" fill="white" fillOpacity="0.95" />
              <rect x="9" y="17" width="6" height="7" rx="2" fill="white" fillOpacity="0.65" />
              <rect x="17" y="17" width="6" height="7" rx="2" fill="white" fillOpacity="0.65" />
            </svg>
            <span className="brand-title">Declutter</span>
            <span className="brand-version">{release.version}</span>
          </a>

          <ul className={`nav-links ${mobileMenuOpen ? 'open' : ''}`}>
            <li><a href="#simulator" onClick={() => setMobileMenuOpen(false)}>Try Real UI</a></li>
            <li><a href="#features" onClick={() => setMobileMenuOpen(false)}>Features</a></li>
            <li><a href="#extensions" onClick={() => setMobileMenuOpen(false)}>Supported Formats</a></li>
            <li><a href="#safety" onClick={() => setMobileMenuOpen(false)}>Safety</a></li>
            <li><a href="#how-it-works" onClick={() => setMobileMenuOpen(false)}>Workflow</a></li>
            <li><a href="#faq" onClick={() => setMobileMenuOpen(false)}>FAQ</a></li>
          </ul>

          <div className="nav-actions">
            <a href="#download" className="nav-cta">
              <svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16">
                <path fillRule="evenodd" d="M10 3a.75.75 0 01.75.75v8.19l2.72-2.72a.75.75 0 111.06 1.06l-4 4a.75.75 0 01-1.06 0l-4-4a.75.75 0 111.06-1.06l2.72 2.72V3.75A.75.75 0 0110 3zM3.75 14.5a.75.75 0 01.75.75v1.5c0 .414.336.75.75.75h9.5a.75.75 0 00.75-.75v-1.5a.75.75 0 011.5 0v1.5A2.25 2.25 0 0114.75 19h-9.5A2.25 2.25 0 013 16.75v-1.5a.75.75 0 01.75-.75z" clipRule="evenodd" />
              </svg>
              <span>Download</span>
            </a>

            <button
              className="menu-toggle"
              onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
              aria-label="Toggle navigation menu"
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="22" height="22">
                {mobileMenuOpen ? (
                  <>
                    <line x1="18" y1="6" x2="6" y2="18"></line>
                    <line x1="6" y1="6" x2="18" y2="18"></line>
                  </>
                ) : (
                  <>
                    <line x1="3" y1="12" x2="21" y2="12"></line>
                    <line x1="3" y1="6" x2="21" y2="6"></line>
                    <line x1="3" y1="18" x2="21" y2="18"></line>
                  </>
                )}
              </svg>
            </button>
          </div>
        </div>
      </nav>

      {/* ==================== Hero ==================== */}
      <header className="hero-section">
        <div className="container hero-container">
          <div className="hero-text-content">
            <div className="hero-badge">
              <span className="hero-badge-pulse"></span>
              <span>Windows Desktop Utility • Offline & Private</span>
            </div>

            <h1 className="hero-headline">
              Tidy your messy folders <br />
              <span className="text-gradient">and check every change first.</span>
            </h1>

            <p className="hero-lead">
              Declutter organizes loose files into structured categories, detects duplicate files by SHA-256 content comparison, and safely cleans empty folders — with visual previews and 1-click undo for your last sort.
            </p>

            <div className="hero-cta-row">
              <a href="#download" className="btn btn-primary">
                <svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
                  <path fillRule="evenodd" d="M10 3a.75.75 0 01.75.75v8.19l2.72-2.72a.75.75 0 111.06 1.06l-4 4a.75.75 0 01-1.06 0l-4-4a.75.75 0 111.06-1.06l2.72 2.72V3.75A.75.75 0 0110 3zM3.75 14.5a.75.75 0 01.75.75v1.5c0 .414.336.75.75.75h9.5a.75.75 0 00.75-.75v-1.5a.75.75 0 011.5 0v1.5A2.25 2.25 0 0114.75 19h-9.5A2.25 2.25 0 013 16.75v-1.5a.75.75 0 01.75-.75z" clipRule="evenodd" />
                </svg>
                <span>Download for Windows {release.setupSize ? `(${release.setupSize})` : ''}</span>
              </a>

              <a href="https://github.com/w1tneess/declutter" target="_blank" rel="noopener noreferrer" className="btn btn-ghost">
                <svg viewBox="0 0 24 24" fill="currentColor" width="18" height="18">
                  <path fillRule="evenodd" clipRule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.53 1.032 1.53 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" />
                </svg>
                <span>View Source</span>
              </a>

              <span className="hero-os-pill">
                <svg viewBox="0 0 88 88" width="14" height="14" fill="currentColor">
                  <path d="M0 12.5l35.5-4.8v34.5H0V12.5zm0 37.7h35.5v34.6L0 80V50.2zm40.3-43.2L88 0v42.2H40.3V7zm0 43.2H88v42.2l-47.7-7V50.2z" />
                </svg>
                <span>Windows 10 / 11 · 64-bit</span>
              </span>
            </div>

            {/* Quick Metrics */}
            <div className="hero-metrics-bar">
              <div className="metric-cell">
                <span className="metric-val">Recycle Bin</span>
                <span className="metric-desc">Local drive removals</span>
              </div>
              <div className="metric-cell">
                <span className="metric-val">0 bytes</span>
                <span className="metric-desc">Telemetry · Offline only</span>
              </div>
              <div className="metric-cell">
                <span className="metric-val">1 Click</span>
                <span className="metric-desc">Undo last sort</span>
              </div>
              <div className="metric-cell">
                <span className="metric-val">SHA-256</span>
                <span className="metric-desc">Content comparison</span>
              </div>
            </div>
          </div>
        </div>
      </header>

      {/* ==================== Web-Native Declutter Studio (Simulation) ==================== */}
      <section className="studio-section" id="simulator">
        <div className="container">
          <div className="section-head text-center">
            <span className="section-tag">Browser Simulation • Interactive Preview</span>
            <h2>Simulate Declutter in your browser</h2>
            <p className="section-sub">
              Test how Declutter's sorting workflow, SHA-256 duplicate scanner, and undo feel right in your browser. This interactive simulation runs in memory and moves no real files — the desktop app handles real folders on Windows.
            </p>
          </div>

          <div
            className={`studio-card ${isDragging ? 'drag-active' : ''}`}
            onDragOver={handleDragOver}
            onDragLeave={handleDragLeave}
            onDrop={handleDrop}
          >
            {/* Drag & Drop Visual Overlay */}
            {isDragging && (
              <div className="studio-drag-overlay">
                <div className="drag-overlay-content">
                  <span className="drag-icon">📥</span>
                  <h3>Drop your files here</h3>
                  <p>Declutter will analyze and organize them client-side in real time</p>
                </div>
              </div>
            )}

            {/* Hidden Real File Input */}
            <input
              type="file"
              multiple
              ref={fileInputRef}
              onChange={handleFileInputChange}
              style={{ display: 'none' }}
            />

            {/* Studio Header Bar */}
            <div className="studio-topbar">
              <div className="studio-target-folder">
                <span className="folder-icon">📁</span>
                <span className="folder-title">
                  Target: <code>{dataSource === 'custom' ? 'Custom_Local_Files/' : 'Sample_Downloads/'}</code>
                </span>
                <span className="folder-file-count">
                  ({isSorted
                    ? `Organized in ${sortedBuckets.length} categories`
                    : `${currentFiles.length} scattered loose files`})
                </span>
                <span className="simulation-tag-pill">Simulation only — moves no real files</span>
              </div>

              {/* Data Source Switcher */}
              <div className="studio-source-switcher">
                <button
                  className={`btn-source-pill ${dataSource === 'sample' ? 'active' : ''}`}
                  onClick={handleSwitchToSample}
                  title="Switch back to standard sample files"
                >
                  <span>🧪 Sample Sandbox</span>
                </button>

                <button
                  className={`btn-source-pill ${dataSource === 'custom' ? 'active' : ''}`}
                  onClick={() => fileInputRef.current?.click()}
                  title="Pick real files from your computer to test client-side"
                >
                  <span>⬆️ Load Your Files</span>
                </button>
              </div>

              <div className="studio-top-actions">
                {reclaimedMB > 0 && (
                  <div className="reclaimed-badge">
                    <span className="leaf-icon">🌱</span>
                    <span>+{reclaimedMB.toFixed(1)} MB Reclaimed</span>
                  </div>
                )}
                <button
                  className="btn-reset-studio"
                  onClick={handleResetAllDemo}
                  title="Reset studio to initial chaotic state"
                >
                  <span>↺ Reset Sandbox</span>
                </button>
              </div>
            </div>

            {/* Studio Mode Navigation Tabs */}
            <div className="studio-nav-bar">
              <button
                className={`studio-tab-btn ${activeTab === 'sort' ? 'active' : ''}`}
                onClick={() => setActiveTab('sort')}
              >
                <span className="tab-icon">🗂️</span>
                <span className="tab-name">Sort by Type</span>
                {isSorted && <span className="tab-badge-active">Organized</span>}
              </button>

              <button
                className={`studio-tab-btn ${activeTab === 'duplicates' ? 'active' : ''}`}
                onClick={() => setActiveTab('duplicates')}
              >
                <span className="tab-icon">📑</span>
                <span className="tab-name">Duplicate Finder (SHA-256)</span>
                {duplicates.length > 0 ? (
                  <span className="tab-badge-count">{duplicates.length}</span>
                ) : (
                  <span className="tab-badge-clean">Clean</span>
                )}
              </button>

              <button
                className={`studio-tab-btn ${activeTab === 'large' ? 'active' : ''}`}
                onClick={() => setActiveTab('large')}
              >
                <span className="tab-icon">⏳</span>
                <span className="tab-name">Large & Old Files</span>
                <span className="tab-badge-count">{filteredLargeFiles.length}</span>
              </button>

              <button
                className={`studio-tab-btn ${activeTab === 'empty' ? 'active' : ''}`}
                onClick={() => setActiveTab('empty')}
              >
                <span className="tab-icon">🗑️</span>
                <span className="tab-name">Empty Folders</span>
                {emptyFolders.length > 0 ? (
                  <span className="tab-badge-count">{emptyFolders.length}</span>
                ) : (
                  <span className="tab-badge-clean">Clean</span>
                )}
              </button>
            </div>

            {/* Studio Main Workspace */}
            <div className="studio-workspace">
              {/* Toast Notification Banner */}
              {toast && (
                <div className="studio-toast-banner" role="status">
                  <span>{toast}</span>
                  <button onClick={() => setToast(null)}>✕</button>
                </div>
              )}

              {/* ----------------- TAB 1: Sort by Type (Functional) ----------------- */}
              {activeTab === 'sort' && (
                <div className="studio-pane">
                  <div className="pane-control-bar">
                    <div className="control-left">
                      <h4>Intelligent Folder Categorization</h4>
                      <p>
                        Moves loose files into structured type subfolders. Check or uncheck categories below to customize your organization rules.
                      </p>
                    </div>
                    <div className="control-right">
                      {isSorted ? (
                        <button className="btn btn-undo-action" onClick={handleUndoSort}>
                          <span>↩️ Undo last sort</span>
                        </button>
                      ) : (
                        <button
                          className="btn btn-primary"
                          onClick={handleSortFiles}
                          disabled={sortAnimating || currentFiles.length === 0}
                        >
                          {sortAnimating ? 'Sorting files…' : '⚡ Organize into Folders'}
                        </button>
                      )}
                    </div>
                  </div>

                  {/* Category Selector Chips */}
                  <div className="category-chips-bar">
                    <span className="chips-label">Categories to organize:</span>
                    <div className="chips-list">
                      {availableCategories.map(cat => (
                        <button
                          key={cat}
                          type="button"
                          className={`cat-chip-btn ${selectedCategories[cat] ? 'selected' : ''}`}
                          onClick={() => toggleCategorySelection(cat)}
                        >
                          <span className="chip-checkbox">{selectedCategories[cat] ? '✓' : ''}</span>
                          <span>{cat}</span>
                        </button>
                      ))}
                    </div>
                  </div>

                  {!isSorted ? (
                    /* Chaotic Loose Files State */
                    <div className="messy-files-view">
                      <div className="view-status-bar">
                        <span>
                          Status: <strong>{currentFiles.length} scattered loose files in root folder</strong>
                        </span>
                        <span className="hint-tag">
                          {dataSource === 'custom'
                            ? 'Custom files loaded • Click "Organize into Folders"'
                            : 'Drag your own files anywhere onto this box or click Organize'}
                        </span>
                      </div>

                      <div className="files-grid-cards">
                        {currentFiles.map(file => (
                          <div key={file.id} className="sample-file-pill">
                            <span className="file-pill-icon">{file.icon}</span>
                            <div className="file-pill-meta">
                              <span className="file-pill-name" title={file.name}>{file.name}</span>
                              <span className="file-pill-size">
                                {file.sizeStr} · <span className="cat-tag-pill">{file.category}</span>
                              </span>
                            </div>
                          </div>
                        ))}
                      </div>
                    </div>
                  ) : (
                    /* Organized Structured Folders State */
                    <div className="organized-folders-view">
                      <div className="view-status-bar success">
                        <span>
                          ✅ <strong>Organized: {currentFiles.filter(f => selectedCategories[f.category]).length} files grouped into {sortedBuckets.length} structured folders</strong>
                        </span>
                        <span className="hint-tag">Click "Undo last sort" above anytime to revert</span>
                      </div>

                      <div className="category-folders-grid">
                        {sortedBuckets.map(bucket => (
                          <div key={bucket.category} className="folder-bucket-card">
                            <div className="bucket-title-row">
                              <span className="bucket-icon">📁</span>
                              <div className="bucket-name-wrap">
                                <strong>{bucket.category}/</strong>
                                <span>{bucket.count} files · {bucket.sizeStr}</span>
                              </div>
                            </div>
                            <div className="bucket-files-list">
                              {bucket.items.map(item => (
                                <div key={item.id} className="bucket-file-item">
                                  <span>{item.icon}</span>
                                  <span className="bucket-item-name">{item.name}</span>
                                  <span className="bucket-item-size">({item.sizeStr})</span>
                                </div>
                              ))}
                            </div>
                          </div>
                        ))}
                      </div>

                      {/* Display any files from unselected categories that remained in root */}
                      {unsortedItems.length > 0 && (
                        <div className="unsorted-leftovers-box">
                          <div className="leftovers-header">
                            <span>📎 <strong>Remaining Loose Files ({unsortedItems.length}):</strong> Left untouched because their category was unchecked</span>
                          </div>
                          <div className="leftovers-list">
                            {unsortedItems.map(item => (
                              <div key={item.id} className="leftover-pill">
                                <span>{item.icon}</span>
                                <span>{item.name}</span>
                                <span className="leftover-size">{item.sizeStr}</span>
                              </div>
                            ))}
                          </div>
                        </div>
                      )}
                    </div>
                  )}
                </div>
              )}

              {/* ----------------- TAB 2: Duplicate Finder (Functional) ----------------- */}
              {activeTab === 'duplicates' && (
                <div className="studio-pane">
                  <div className="pane-control-bar">
                    <div className="control-left">
                      <h4>SHA-256 Duplicate Detector</h4>
                      <p>
                        Content verification via size, partial hash, and full SHA-256 comparison. The oldest copy is kept by default; at least one copy always remains. Removals move to the Recycle Bin on local drives.
                      </p>
                    </div>
                    {duplicates.length > 0 && (
                      <div className="control-right">
                        <button
                          className="btn btn-danger-action"
                          onClick={() => setConfirmDialog('duplicates')}
                        >
                          <span>🗑️ Clean Duplicates ({duplicates.reduce((a, b) => a + b.sizeBytes, 0).toFixed(1)} MB)</span>
                        </button>
                      </div>
                    )}
                  </div>

                  {duplicates.length > 0 ? (
                    <div className="duplicates-list">
                      {duplicates.map(grp => (
                        <div key={grp.id} className="dup-cluster-card">
                          <div className="dup-cluster-header">
                            <div className="cluster-title">
                              <strong>{grp.name}</strong>
                              <span className="cluster-size">{grp.sizeStr} per copy</span>
                            </div>
                            <span className="cluster-hash-badge">{grp.hash}</span>
                          </div>

                          <div className="dup-copies-options">
                            {grp.copies.map(copy => (
                              <label
                                key={copy.id}
                                className={`copy-radio-row ${copy.keeper ? 'is-keeper' : 'is-delete'}`}
                              >
                                <input
                                  type="radio"
                                  name={`dup-radio-${grp.id}`}
                                  checked={copy.keeper}
                                  onChange={() => handleToggleDupKeeper(grp.id, copy.id)}
                                />
                                <span className={copy.keeper ? 'pill-keep' : 'pill-delete'}>
                                  {copy.keeper ? 'Keep Original' : 'Recycle Copy'}
                                </span>
                                <span className="copy-path"><code>{copy.path}</code></span>
                                <span className="copy-date">{copy.modified}</span>
                              </label>
                            ))}
                          </div>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <div className="all-clean-placeholder">
                      <span className="clean-icon">✨</span>
                      <h4>Zero Duplicate Waste</h4>
                      <p>All duplicate copies have been cleaned and safely preserved in the Recycle Bin. You can test restoring them below.</p>
                      {dataSource === 'custom' && (
                        <button className="btn btn-ghost" onClick={handleSwitchToSample} style={{ marginTop: '14px' }}>
                          Switch to Sample Sandbox to Test Duplicates
                        </button>
                      )}
                    </div>
                  )}
                </div>
              )}

              {/* ----------------- TAB 3: Large & Old Files (Functional) ----------------- */}
              {activeTab === 'large' && (
                <div className="studio-pane">
                  <div className="pane-control-bar">
                    <div className="control-left">
                      <h4>Large & Stale File Radar</h4>
                      <p>Drag the sliders below to dynamically isolate forgotten disk hogs and old downloads.</p>
                    </div>
                    {Object.values(selectedLargeIds).filter(Boolean).length > 0 && (
                      <div className="control-right">
                        <button
                          className="btn btn-danger-action"
                          onClick={() => setConfirmDialog('large')}
                        >
                          <span>🗑️ Recycle Selected ({Object.values(selectedLargeIds).filter(Boolean).length})</span>
                        </button>
                      </div>
                    )}
                  </div>

                  {/* Interactive Dynamic Sliders */}
                  <div className="interactive-filter-sliders">
                    <div className="slider-box">
                      <div className="slider-label-row">
                        <span>Minimum Size Threshold</span>
                        <strong>&ge; {sizeThreshold} MB</strong>
                      </div>
                      <input
                        type="range"
                        min="10"
                        max="500"
                        step="10"
                        value={sizeThreshold}
                        onChange={(e) => setSizeThreshold(Number(e.target.value))}
                        className="studio-slider"
                      />
                    </div>

                    <div className="slider-box">
                      <div className="slider-label-row">
                        <span>Minimum Inactivity Age</span>
                        <strong>&ge; {ageThreshold} Days</strong>
                      </div>
                      <input
                        type="range"
                        min="7"
                        max="150"
                        step="7"
                        value={ageThreshold}
                        onChange={(e) => setAgeThreshold(Number(e.target.value))}
                        className="studio-slider"
                      />
                    </div>
                  </div>

                  {/* Filtered File Items */}
                  <div className="large-files-list">
                    {filteredLargeFiles.length > 0 ? (
                      filteredLargeFiles.map(file => (
                        <label key={file.id} className="large-file-row">
                          <input
                            type="checkbox"
                            checked={!!selectedLargeIds[file.id]}
                            onChange={() => toggleSelectLarge(file.id)}
                          />
                          <span className="large-file-icon">{file.icon}</span>
                          <div className="large-file-info">
                            <strong>{file.name}</strong>
                            <span className="file-date">Modified {file.ageDays} days ago ({file.date})</span>
                          </div>
                          <span className="large-file-size-badge">{file.sizeStr}</span>
                        </label>
                      ))
                    ) : (
                      <div className="all-clean-placeholder">
                        <span className="clean-icon">🔍</span>
                        <h4>No files match these threshold filters</h4>
                        <p>Drag the size or age sliders above to widen your scan criteria.</p>
                      </div>
                    )}
                  </div>
                </div>
              )}

              {/* ----------------- TAB 4: Empty Folders (Functional) ----------------- */}
              {activeTab === 'empty' && (
                <div className="studio-pane">
                  <div className="pane-control-bar">
                    <div className="control-left">
                      <h4>Deep Empty Folder Pruner</h4>
                      <p>Discovers forgotten empty directory trees at any depth. Pruned safely from deepest child branch upwards.</p>
                    </div>
                    {emptyFolders.length > 0 && (
                      <div className="control-right">
                        <button className="btn btn-danger-action" onClick={handlePruneEmptyFolders}>
                          <span>🧹 Prune {emptyFolders.length} Empty Folders</span>
                        </button>
                      </div>
                    )}
                  </div>

                  {emptyFolders.length > 0 ? (
                    <div className="empty-folders-tree">
                      {emptyFolders.map(ef => (
                        <div key={ef.id} className="empty-tree-item">
                          <span className="tree-icon">📁</span>
                          <div className="tree-meta">
                            <code>{ef.path}</code>
                            <span>{ef.details}</span>
                          </div>
                          <span className="tree-prune-tag">Empty Branch</span>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <div className="all-clean-placeholder">
                      <span className="clean-icon">🌳</span>
                      <h4>No Empty Folders Found</h4>
                      <p>All directory branches have been cleanly pruned. No files were affected.</p>
                    </div>
                  )}
                </div>
              )}
            </div>

            {/* ==================== Windows Recycle Bin Tray ==================== */}
            <div className="recycle-bin-tray">
              <div className="tray-status">
                <span className="bin-icon">🗑️</span>
                <span>
                  <strong>Windows Recycle Bin Simulation:</strong>{' '}
                  {recycledItems.length === 0
                    ? 'No removed files. Everything is intact.'
                    : `${recycledItems.length} items safely held (${reclaimedMB.toFixed(1)} MB reclaimable).`}
                </span>
              </div>

              {recycledItems.length > 0 && (
                <button className="btn-restore-tray" onClick={handleRestoreAll}>
                  <span>♻️ Restore All Recycled Items</span>
                </button>
              )}
            </div>
          </div>

          {/* Web Modal Confirmation */}
          {confirmDialog && (
            <div className="web-modal-overlay">
              <div className="web-modal-card">
                <h3>Send items to Windows Recycle Bin?</h3>
                <p>
                  {confirmDialog === 'duplicates'
                    ? `Move ${duplicates.length} redundant duplicate copies to the Recycle Bin? On local drives, items go to the Recycle Bin where you can restore them.`
                    : `Send ${Object.values(selectedLargeIds).filter(Boolean).length} selected space hogs to the Recycle Bin? On local drives, items go to the Recycle Bin where you can restore them.`}
                </p>
                <div className="modal-btn-row">
                  <button className="btn btn-ghost" onClick={() => setConfirmDialog(null)}>Cancel</button>
                  <button
                    className="btn btn-danger-action"
                    onClick={confirmDialog === 'duplicates' ? handleRecycleDuplicates : handleRecycleSelectedLarge}
                  >
                    Confirm & Move to Recycle Bin
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>
      </section>

      {/* ==================== Asymmetrical Bento Grid: Core Capabilities ==================== */}
      <section className="bento-section" id="features">
        <div className="container">
          <div className="section-head text-center">
            <span className="section-tag">Core Architecture</span>
            <h2>Four focused tools. Plain safeguards.</h2>
            <p className="section-sub">
              Engineered specifically for Windows users who want tidy folders, with a preview before every change.
            </p>
          </div>

          <div className="bento-grid">
            {/* Bento Card 1: Sort by Type (Large Featured) */}
            <div className="bento-card bento-span-2 bento-card-sort">
              <div className="bento-header">
                <div className="bento-icon-pill">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="22" height="22">
                    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
                    <line x1="12" y1="11" x2="12" y2="17"></line>
                    <line x1="9" y1="14" x2="15" y2="14"></line>
                  </svg>
                  <span>Tool 01</span>
                </div>
                <span className="bento-badge-highlight">Undo Last Sort</span>
              </div>

              <div className="bento-body">
                <h3>Sort by Type</h3>
                <p>
                  Organizes top-level loose files into structured categories (Images, Documents, Videos, Audio, Archives, Installers, Code, Other). Screenshots and chat pictures route into <code>Images/Screenshots</code> and <code>Images/Chat</code> sub-folders. Files touched in the last 24 hours are left alone, and name clashes become <code>photo (1).jpg</code>.
                </p>

                {/* Visual Category Flow */}
                <div className="bento-visual-categories">
                  <div className="cat-pill"><span className="cat-emoji">🖼️</span> Images <span className="cat-count">84</span></div>
                  <div className="cat-pill"><span className="cat-emoji">📸</span> Images/Screenshots <span className="cat-count">36</span></div>
                  <div className="cat-pill"><span className="cat-emoji">💬</span> Images/Chat <span className="cat-count">14</span></div>
                  <div className="cat-pill"><span className="cat-emoji">📄</span> Documents <span className="cat-count">42</span></div>
                  <div className="cat-pill"><span className="cat-emoji">🎬</span> Videos <span className="cat-count">18</span></div>
                  <div className="cat-pill"><span className="cat-emoji">📦</span> Archives <span className="cat-count">12</span></div>
                  <div className="cat-pill"><span className="cat-emoji">⚙️</span> Installers <span className="cat-count">8</span></div>
                </div>

                <div className="bento-footer-note">
                  <span className="note-icon">↩️</span>
                  <span>Undo last sort: returns files of the most recent sort back to their previous locations; skips any files modified since the sort.</span>
                </div>
              </div>
            </div>

            {/* Bento Card 2: SHA-256 Duplicates */}
            <div className="bento-card bento-card-dup">
              <div className="bento-header">
                <div className="bento-icon-pill">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="22" height="22">
                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                  </svg>
                  <span>Tool 02</span>
                </div>
                <span className="bento-badge-crypto">SHA-256</span>
              </div>

              <div className="bento-body">
                <h3>Duplicates Engine</h3>
                <p>
                  Compared by SHA-256: matches exact size, checks the first and last 64 KB, then computes full SHA-256 hashes. Oldest copy is kept by default, and at least one copy always remains. "Possible re-downloads" like <code>file (1).pdf</code> are never ticked for you.
                </p>

                <div className="bento-visual-hash-box">
                  <div className="hash-header">
                    <span>Cryptographic Verification</span>
                    <span className="hash-status">SHA-256 Match</span>
                  </div>
                  <code>sha256: 9f86d081884c7d659a2feaa0c...</code>
                  <div className="hash-preserve-label">
                    <span>Oldest copy kept by default; keeper can be changed</span>
                  </div>
                </div>
              </div>
            </div>

            {/* Bento Card 3: Large & Inactive Space Hogs */}
            <div className="bento-card bento-card-large">
              <div className="bento-header">
                <div className="bento-icon-pill">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="22" height="22">
                    <circle cx="12" cy="12" r="10"></circle>
                    <polyline points="12 6 12 12 16 14"></polyline>
                  </svg>
                  <span>Tool 03</span>
                </div>
                <span className="bento-badge-metric">Size & Age</span>
              </div>

              <div className="bento-body">
                <h3>Large & Old Files</h3>
                <p>
                  Filters files by minimum size (MB) and inactivity age (days untouched). Flags "likely leftovers" (unpacked zips, installers over 7 days old in Downloads, and unfinished downloads over 3 days old) without ticking them for you.
                </p>

                <div className="bento-visual-storage-meter">
                  <div className="meter-labels">
                    <span>Storage Reclaimable</span>
                    <strong>1.4 GB Found</strong>
                  </div>
                  <div className="meter-track">
                    <div className="meter-fill" style={{ width: '68%' }}></div>
                  </div>
                  <div className="meter-details">
                    <span>9 files exceeding 100 MB</span>
                    <span>&gt; 30 days untouched</span>
                  </div>
                </div>
              </div>
            </div>

            {/* Bento Card 4: Empty Folders */}
            <div className="bento-card bento-card-empty">
              <div className="bento-header">
                <div className="bento-icon-pill">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="22" height="22">
                    <path d="M3 6h18"></path>
                    <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"></path>
                    <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"></path>
                  </svg>
                  <span>Tool 04</span>
                </div>
                <span className="bento-badge-tree">Bottom-Up</span>
              </div>

              <div className="bento-body">
                <h3>Deep Empty Folder Pruner</h3>
                <p>
                  Recursively detects abandoned folder trees at any depth. Prunes strictly from the deepest branch upward, leaving parent directories intact if other content resides within.
                </p>

                <div className="bento-tree-preview">
                  <div className="tree-node depth-1">📁 project_archive</div>
                  <div className="tree-node depth-2">└─ 📁 unused_assets</div>
                  <div className="tree-node depth-3 prune-target">
                    └─ 📁 empty_subcache
                    <span className="prune-badge">Pruned safely</span>
                  </div>
                </div>
              </div>
            </div>

            {/* Bento Card 5: Desktop Integration & Never Touch */}
            <div className="bento-card bento-card-integration">
              <div className="bento-header">
                <div className="bento-icon-pill">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="22" height="22">
                    <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"></path>
                  </svg>
                  <span>v0.2.0</span>
                </div>
                <span className="bento-badge-highlight">Windows Integration</span>
              </div>

              <div className="bento-body">
                <h3>"Never Touch" & Context Menu</h3>
                <p>
                  Pin sensitive folders to your "Never touch" list so Declutter never scans or touches them. Right-click any folder in Windows Explorer and select "Tidy with Declutter" for immediate access.
                </p>

                <div className="bento-visual-pins">
                  <div className="pin-item">
                    <span className="pin-icon">📌</span>
                    <span>Pinned: <code>Projects/ActiveWork</code></span>
                    <span className="pin-badge">Protected</span>
                  </div>
                  <div className="pin-item context-item">
                    <span className="pin-icon">🖱️</span>
                    <span>Right-click &rarr; <strong>"Tidy with Declutter"</strong></span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* ==================== Interactive File Extension Matrix ==================== */}
      <section className="extensions-section" id="extensions">
        <div className="container">
          <div className="section-head text-center">
            <span className="section-tag">Supported File Formats</span>
            <h2>60+ Extensions Classified Automatically</h2>
            <p className="section-sub">
              Declutter organizes all common Windows file formats across 7 structured categories. Try searching any extension below to see how Declutter routes it.
            </p>
          </div>

          {/* Interactive Extension Search */}
          <div className="extension-search-wrap">
            <div className="ext-search-input-box">
              <span className="search-icon">🔍</span>
              <input
                type="text"
                placeholder="Search an extension (e.g. .heic, .iso, .docx, .py, .apk, .sql)..."
                value={extQuery}
                onChange={(e) => setExtQuery(e.target.value.trim().toLowerCase())}
                className="ext-search-input"
              />
              {extQuery && (
                <button className="ext-search-clear" onClick={() => setExtQuery('')}>✕</button>
              )}
            </div>

            {extQuery && (
              <div className="ext-search-result-hint">
                {(() => {
                  const cleaned = extQuery.startsWith('.') ? extQuery : `.${extQuery}`;
                  const match = APP_EXTENSIONS_MAP.find(cat => cat.exts.some(e => e.toLowerCase() === cleaned));
                  if (match) {
                    return (
                      <span className="hint-match">
                        🎯 <strong>{cleaned}</strong> is automatically organized into <strong>{match.name}/</strong> ({match.icon})
                      </span>
                    );
                  }
                  return (
                    <span className="hint-other">
                      📎 <strong>{cleaned}</strong> is safely routed into <strong>Other/</strong> with collision protection.
                    </span>
                  );
                })()}
              </div>
            )}
          </div>

          {/* Categories Grid */}
          <div className="extensions-grid">
            {APP_EXTENSIONS_MAP.map((group) => {
              const cleaned = extQuery.startsWith('.') ? extQuery : `.${extQuery}`;
              const isMatch = extQuery && group.exts.some(e => e.toLowerCase() === cleaned);
              return (
                <div key={group.name} className={`ext-group-card ${isMatch ? 'highlighted' : ''}`}>
                  <div className="ext-group-header">
                    <span className="ext-group-icon">{group.icon}</span>
                    <div className="ext-group-title">
                      <h3>{group.name}/</h3>
                      <span className="ext-count-pill">{group.exts.length} formats</span>
                    </div>
                  </div>
                  <p className="ext-group-desc">{group.description}</p>
                  <div className="ext-chips-wrap">
                    {group.exts.map(ext => (
                      <span
                        key={ext}
                        className={`ext-chip ${extQuery && (ext.toLowerCase() === cleaned || ext.includes(extQuery)) ? 'chip-hit' : ''}`}
                      >
                        {ext}
                      </span>
                    ))}
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </section>

      {/* ==================== Safe-by-Design Architecture ==================== */}
      <section className="safety-section" id="safety">
        <div className="container">
          <div className="section-head text-center">
            <span className="section-tag">Safety by Design</span>
            <h2>Hardcoded guardrails for your files</h2>
            <p className="section-sub">
              Declutter tells you what it will touch before anything moves, confirms counts, and keeps your files safe.
            </p>
          </div>

          <div className="comparison-table-wrapper">
            <div className="comparison-column safety-column">
              <div className="column-header">
                <span className="column-status-icon safe">🛡️</span>
                <div>
                  <h3>Declutter Safety Safeguards</h3>
                  <p>Verified on local drives with confirmation before action</p>
                </div>
              </div>

              <ul className="comparison-list">
                <li>
                  <span className="check-icon">✓</span>
                  <div>
                    <strong>Recycle Bin Removals on Local Drives</strong>
                    <p>All removals route to your Windows Recycle Bin on local fixed drives. Items can be restored directly from the Recycle Bin. Removals are refused on USB sticks and network drives.</p>
                  </div>
                </li>
                <li>
                  <span className="check-icon">✓</span>
                  <div>
                    <strong>Undo Last Sort</strong>
                    <p>Puts the files of the most recent sort back to their previous locations and cleans up empty folders that were created. Any file modified since the sort is skipped and reported.</p>
                  </div>
                </li>
                <li>
                  <span className="check-icon">✓</span>
                  <div>
                    <strong>Zero Telemetry and Offline Execution</strong>
                    <p>The desktop app makes no network connections and sends no telemetry. Settings and undo logs stay locally in %LOCALAPPDATA%\Declutter.</p>
                  </div>
                </li>
                <li>
                  <span className="check-icon">✓</span>
                  <div>
                    <strong>Mandatory Previews and Confirmation</strong>
                    <p>Nothing on disk changes without confirmation. Every tool shows an itemized preview and asks you to confirm the exact count before anything moves.</p>
                  </div>
                </li>
                <li>
                  <span className="check-icon">✓</span>
                  <div>
                    <strong>Protected Locations & "Never Touch" List</strong>
                    <p>Drive roots, the user home folder, Windows, Program Files, AppData, and code projects (.git, node_modules) are never entered. Any folder pinned under "Never touch" is strictly skipped.</p>
                  </div>
                </li>
              </ul>
            </div>
          </div>
        </div>
      </section>

      {/* ==================== Connected Workflow Pipeline ==================== */}
      <section className="workflow-section" id="how-it-works">
        <div className="container">
          <div className="section-head text-center">
            <span className="section-tag">How It Operates</span>
            <h2>Three steps to an orderly drive</h2>
            <p className="section-sub">
              Clear, transparent control from start to finish.
            </p>
          </div>

          <div className="pipeline-flow">
            <div className="pipeline-track" aria-hidden="true"></div>

            <div className="pipeline-step">
              <div className="step-badge">01</div>
              <div className="step-content">
                <h3>Select a Directory</h3>
                <p>Right-click any folder in Windows Explorer and choose "Tidy with Declutter", quick-pick your Downloads, Desktop, or Documents, or choose any folder on your hard drive or external USB storage (removals are refused on USB and network drives).</p>
              </div>
            </div>

            <div className="pipeline-step">
              <div className="step-badge">02</div>
              <div className="step-content">
                <h3>Concurrent Scan</h3>
                <p>All four analysis engines execute in parallel on background worker threads without freezing or lagging the desktop interface.</p>
              </div>
            </div>

            <div className="pipeline-step">
              <div className="step-badge">03</div>
              <div className="step-content">
                <h3>Review & Execute</h3>
                <p>Inspect the live itemized checkboxes. Confirm the exact count modal. On local fixed drives, all removals go safely into your native Windows Recycle Bin.</p>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* ==================== Unified Download Station ==================== */}
      <section className="download-section" id="download">
        <div className="container">
          <div className="download-hub-card">
            <div className="hub-top text-center">
              <span className="section-tag">Get Declutter for Windows</span>
              <h2>Download Declutter</h2>
              <p className="hub-sub">
                Version <strong>{release.version}</strong> {release.releaseDate ? `• Published ${release.releaseDate}` : ''} • Free, open source & private
              </p>
            </div>

            {/* Format Toggle Tabs */}
            <div className="hub-format-tabs">
              <button
                className={`hub-tab-btn ${downloadFormat === 'setup' ? 'active' : ''}`}
                onClick={() => setDownloadFormat('setup')}
              >
                <span>⚡ Windows Installer (.exe)</span>
                <span className="hub-tab-badge">Recommended</span>
              </button>
              <button
                className={`hub-tab-btn ${downloadFormat === 'portable' ? 'active' : ''}`}
                onClick={() => setDownloadFormat('portable')}
              >
                <span>📦 Portable Archive (.zip)</span>
                <span className="hub-tab-badge-sub">No Install</span>
              </button>
            </div>

            {/* Dynamic Card Body */}
            <div className="hub-details-body">
              {downloadFormat === 'setup' ? (
                <div className="format-panel">
                  <div className="format-desc">
                    <h4>Standard Per-User Inno Setup</h4>
                    <p>Creates Start Menu entries and optional desktop shortcuts. Installs strictly into your local user profile without requiring administrator privileges or UAC prompts.</p>
                  </div>
                  <a href={release.setupUrl} className="btn btn-primary btn-large download-action-btn">
                    <svg viewBox="0 0 20 20" fill="currentColor" width="20" height="20">
                      <path fillRule="evenodd" d="M10 3a.75.75 0 01.75.75v8.19l2.72-2.72a.75.75 0 111.06 1.06l-4 4a.75.75 0 01-1.06 0l-4-4a.75.75 0 111.06-1.06l2.72 2.72V3.75A.75.75 0 0110 3zM3.75 14.5a.75.75 0 01.75.75v1.5c0 .414.336.75.75.75h9.5a.75.75 0 00.75-.75v-1.5a.75.75 0 011.5 0v1.5A2.25 2.25 0 0114.75 19h-9.5A2.25 2.25 0 013 16.75v-1.5a.75.75 0 01.75-.75z" clipRule="evenodd" />
                    </svg>
                    <span>Download Declutter-Setup.exe {release.setupSize ? `(${release.setupSize})` : ''}</span>
                  </a>
                  <p className="download-smartscreen-notice">The app is not code-signed: Windows SmartScreen shows "Unknown publisher" (click <strong>More info</strong>, then <strong>Run anyway</strong>).</p>
                </div>
              ) : (
                <div className="format-panel">
                  <div className="format-desc">
                    <h4>Standalone Portable Package</h4>
                    <p>Self-contained executable archive. Extract anywhere (including USB thumbdrives or external storage) and run without installation or registry writes.</p>
                  </div>
                  <a href={release.portableUrl} className="btn btn-primary btn-large download-action-btn">
                    <svg viewBox="0 0 20 20" fill="currentColor" width="20" height="20">
                      <path fillRule="evenodd" d="M10 3a.75.75 0 01.75.75v8.19l2.72-2.72a.75.75 0 111.06 1.06l-4 4a.75.75 0 01-1.06 0l-4-4a.75.75 0 111.06-1.06l2.72 2.72V3.75A.75.75 0 0110 3zM3.75 14.5a.75.75 0 01.75.75v1.5c0 .414.336.75.75.75h9.5a.75.75 0 00.75-.75v-1.5a.75.75 0 011.5 0v1.5A2.25 2.25 0 0114.75 19h-9.5A2.25 2.25 0 013 16.75v-1.5a.75.75 0 01.75-.75z" clipRule="evenodd" />
                    </svg>
                    <span>Download Declutter-portable.zip {release.portableSize ? `(${release.portableSize})` : ''}</span>
                  </a>
                  <p className="download-smartscreen-notice">The app is not code-signed: Windows SmartScreen shows "Unknown publisher" (click <strong>More info</strong>, then <strong>Run anyway</strong>).</p>
                </div>
              )}

              {/* SHA-256 Checksum Bar */}
              <div className="hub-checksum-bar">
                <span className="checksum-tag">Verified SHA-256</span>
                <code className="checksum-code">Built automatically via GitHub Actions CI</code>
                <button className="checksum-copy-btn" onClick={handleCopyHash}>
                  {copiedHash ? '✓ Copied' : 'Copy Checksum'}
                </button>
              </div>

              {/* SmartScreen Accordion */}
              <div className="hub-smartscreen-accordion">
                <button
                  className="smartscreen-toggle"
                  onClick={() => setSmartScreenOpen(!smartScreenOpen)}
                  aria-expanded={smartScreenOpen}
                >
                  <span className="smartscreen-toggle-title">
                    <span className="info-icon">ℹ️</span>
                    <span>First time running on Windows? Quick SmartScreen note</span>
                  </span>
                  <span className={`accordion-arrow ${smartScreenOpen ? 'rotated' : ''}`}>▼</span>
                </button>

                {smartScreenOpen && (
                  <div className="smartscreen-drawer">
                    <p>
                      Declutter is an open-source software project. Commercial code-signing certificates cost hundreds of dollars annually, which open-source tools forgo.
                    </p>
                    <div className="smartscreen-steps">
                      <div className="ss-step">
                        <span className="ss-num">1</span>
                        <span>Click <strong>"More info"</strong> on the Windows protected PC dialog</span>
                      </div>
                      <div className="ss-step">
                        <span className="ss-num">2</span>
                        <span>Click <strong>"Run anyway"</strong></span>
                      </div>
                    </div>
                    <p className="ss-subtext">
                      Every binary is built transparently via public GitHub runners. You can inspect all build scripts and logs anytime on GitHub.
                    </p>
                  </div>
                )}
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* ==================== FAQ ==================== */}
      <section className="faq-section" id="faq">
        <div className="container">
          <div className="section-head text-center">
            <span className="section-tag">Frequently Asked Questions</span>
            <h2>Got questions? We have answers.</h2>
            <p className="section-sub">
              Clear answers regarding privacy, safety, and mechanics.
            </p>
          </div>

          <div className="faq-list">
            {[
              {
                q: "Can Declutter ever permanently destroy my files?",
                a: "Removals go to the native Windows Recycle Bin on local fixed drives, where you can restore them anytime. On USB sticks and network drives, removals are refused because those drives may lack a Recycle Bin."
              },
              {
                q: "How does Undo work for file sorts?",
                a: "Declutter logs moves from the most recent sort to %LOCALAPPDATA%\\Declutter\\undo.json. Clicking 'Undo last sort' returns those files to their previous locations and removes empty category folders. Any file changed since the sort is skipped and reported. Undo covers the last sort only and does not restore items sent to the Recycle Bin."
              },
              {
                q: "What happens if a file with the same name already exists during a sort?",
                a: "Declutter never overwrites your files. If a file with the same name already exists in a destination folder, the newly organized file is automatically renamed with a clean counter (e.g., photo (1).jpg) so no existing file is overwritten."
              },
              {
                q: "Does Declutter download my OneDrive online-only files?",
                a: "No. Declutter detects OneDrive online-only placeholder files (Files On-Demand) and leaves them alone: they are never read, moved or removed, so nothing gets downloaded. Other cloud drives are not supported."
              },
              {
                q: "How does the duplicate engine compare files?",
                a: "It compares file content, not just names: candidates must match in byte size, match the first and last 64 KB, and then match a full cryptographic SHA-256 check. The oldest copy is kept by default, and at least one copy always remains."
              },
              {
                q: "Where are undo records and temporary logs stored?",
                a: "The undo log and settings are kept in your user profile, in %LOCALAPPDATA%\\Declutter. The app does not change system files. The installer adds a Start menu entry and an uninstall entry for your account, and a right-click \"Tidy with Declutter\" folder entry only if you tick that option."
              },
              {
                q: "Does Declutter need an internet connection?",
                a: "No. The app makes no network connections and has no telemetry. You can copy the portable .zip to a USB drive and run it on a computer that is offline."
              },
              {
                q: "Does Declutter require administrative rights or UAC?",
                a: "No. Declutter installs cleanly into your local user space without requiring administrator privileges, annoying UAC elevation prompts, or system-wide changes."
              },
              {
                q: "Does Declutter work on external drives and USB sticks?",
                a: "You can select, scan, and sort folders on external USB drives and secondary partitions. However, removals to the Recycle Bin are refused on USB sticks and network drives because those drives may not support the Windows Recycle Bin."
              },
              {
                q: "What are likely leftovers and possible re-downloads?",
                a: "Large & old flags 'likely leftovers' (unpacked zips, installers over 7 days old in Downloads, and unfinished downloads over 3 days old) without ticking them for you. The duplicate engine flags 'possible re-downloads' (like report (1).pdf) and never auto-ticks them for removal."
              }
            ].map((faq, i) => (
              <div key={i} className={`faq-card ${activeFaq === i ? 'active' : ''}`}>
                <button className="faq-question-btn" onClick={() => toggleFaq(i)}>
                  <span>{faq.q}</span>
                  <span className="faq-indicator">{activeFaq === i ? '−' : '+'}</span>
                </button>
                {activeFaq === i && (
                  <div className="faq-answer">
                    <p>{faq.a}</p>
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* ==================== Final CTA ==================== */}
      <section className="final-cta-section">
        <div className="container text-center">
          <div className="final-cta-card">
            <h2>Experience a clean, uncluttered desktop.</h2>
            <p>Free forever, open source, lightweight, and completely private.</p>
            <div className="final-cta-actions">
              <a href="#download" className="btn btn-primary btn-large">
                <svg viewBox="0 0 20 20" fill="currentColor" width="20" height="20">
                  <path fillRule="evenodd" d="M10 3a.75.75 0 01.75.75v8.19l2.72-2.72a.75.75 0 111.06 1.06l-4 4a.75.75 0 01-1.06 0l-4-4a.75.75 0 111.06-1.06l2.72 2.72V3.75A.75.75 0 0110 3zM3.75 14.5a.75.75 0 01.75.75v1.5c0 .414.336.75.75.75h9.5a.75.75 0 00.75-.75v-1.5a.75.75 0 011.5 0v1.5A2.25 2.25 0 0114.75 19h-9.5A2.25 2.25 0 013 16.75v-1.5a.75.75 0 01.75-.75z" clipRule="evenodd" />
                </svg>
                <span>Download Declutter for Windows {release.version}</span>
              </a>
            </div>
          </div>
        </div>
      </section>

      {/* ==================== Footer ==================== */}
      <footer className="site-footer">
        <div className="container footer-container">
          <div className="footer-meta">
            <span className="footer-logo">Declutter</span>
            <span className="footer-copyright">© 2026 w1tneess. Released under the MIT License.</span>
          </div>
          <ul className="footer-nav">
            <li><a href="https://github.com/w1tneess/declutter" target="_blank" rel="noopener noreferrer">GitHub</a></li>
            <li><a href="https://github.com/w1tneess/declutter/releases" target="_blank" rel="noopener noreferrer">Releases</a></li>
            <li><a href="https://github.com/w1tneess/declutter/issues" target="_blank" rel="noopener noreferrer">Issue Tracker</a></li>
            <li><a href="https://github.com/w1tneess/declutter/blob/main/LICENSE" target="_blank" rel="noopener noreferrer">License</a></li>
          </ul>
        </div>
      </footer>
    </div>
  );
}
