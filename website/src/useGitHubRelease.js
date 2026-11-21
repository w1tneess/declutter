import { useState, useEffect } from 'react';

const REPO = 'w1tneess/declutter';
const REPO_URL = `https://github.com/${REPO}`;
// Until the lookup answers (or if it fails) these links still reach the latest release.
const LATEST_SETUP = `${REPO_URL}/releases/latest/download/Declutter-Setup.exe`;
const LATEST_PORTABLE = `${REPO_URL}/releases/latest/download/Declutter-portable.zip`;

export function useGitHubRelease() {
  const [data, setData] = useState({
    version: null,
    releaseDate: null,
    setupUrl: LATEST_SETUP,
    portableUrl: LATEST_PORTABLE,
    setupSize: null,
    portableSize: null,
    downloadCount: 0,
    notes: '',
    isLive: false,
    loading: true,
  });

  useEffect(() => {
    let mounted = true;

    async function fetchRelease() {
      try {
        const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`);
        if (!res.ok) throw new Error('No release found or rate-limited');
        const json = await res.json();

        if (mounted && json && json.tag_name) {
          let setupAsset = null;
          let portableAsset = null;
          let totalDownloads = 0;

          if (Array.isArray(json.assets)) {
            json.assets.forEach(asset => {
              totalDownloads += asset.download_count || 0;
              const name = (asset.name || '').toLowerCase();
              if (name.includes('setup') || name.endsWith('.exe')) {
                setupAsset = asset;
              } else if (name.includes('portable') || name.endsWith('.zip')) {
                portableAsset = asset;
              }
            });
          }

          const formatBytes = (bytes) => {
            if (!bytes) return null;
            const mb = bytes / (1024 * 1024);
            return `${mb.toFixed(1)} MB`;
          };

          const releaseDateFormatted = json.published_at
            ? new Date(json.published_at).toLocaleDateString(undefined, {
                year: 'numeric',
                month: 'short',
                day: 'numeric',
              })
            : null;

          setData({
            version: json.tag_name,
            releaseDate: releaseDateFormatted,
            setupUrl: setupAsset?.browser_download_url || `${REPO_URL}/releases/download/${json.tag_name}/Declutter-Setup.exe`,
            portableUrl: portableAsset?.browser_download_url || `${REPO_URL}/releases/download/${json.tag_name}/Declutter-portable.zip`,
            setupSize: formatBytes(setupAsset?.size),
            portableSize: formatBytes(portableAsset?.size),
            downloadCount: totalDownloads,
            notes: json.body || 'Latest Windows release with automatic folder classification.',
            isLive: true,
            loading: false,
          });
        }
      } catch {
        if (mounted) setData(prev => ({ ...prev, loading: false }));
      }
    }

    fetchRelease();
    return () => { mounted = false; };
  }, []);

  return data;
}
