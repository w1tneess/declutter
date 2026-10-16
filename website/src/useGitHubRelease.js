import { useState, useEffect } from 'react';

const REPO = 'w1tneess/declutter';
const FALLBACK_VERSION = 'v1.0.0';
const REPO_URL = `https://github.com/${REPO}`;

export function useGitHubRelease() {
  const [data, setData] = useState({
    version: FALLBACK_VERSION,
    releaseDate: null,
    setupUrl: `${REPO_URL}/releases/latest`,
    portableUrl: `${REPO_URL}/releases/latest`,
    setupSize: null,
    portableSize: null,
    downloadCount: 0,
    notes: 'Initial production release for Windows 10 & 11.',
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
        if (mounted) {
          setData(prev => ({
            ...prev,
            setupUrl: `${REPO_URL}/releases/download/${FALLBACK_VERSION}/Declutter-Setup.exe`,
            portableUrl: `${REPO_URL}/releases/download/${FALLBACK_VERSION}/Declutter-portable.zip`,
            loading: false,
          }));
        }
      }
    }

    fetchRelease();
    return () => { mounted = false; };
  }, []);

  return data;
}
