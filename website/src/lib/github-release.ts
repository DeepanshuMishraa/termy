import { gitConfig } from './shared';

const GITHUB_API = 'https://api.github.com';

export interface GitHubReleaseAsset {
  id: number;
  name: string;
  size: number;
  downloadUrl: string;
  contentType: string;
}

export interface GitHubRelease {
  name: string;
  tagName: string;
  publishedAt: string;
  htmlUrl: string;
  body: string | null;
  tarballUrl: string;
  zipballUrl: string;
  assets: GitHubReleaseAsset[];
}

interface GitHubReleaseResponse {
  name: string | null;
  tag_name: string;
  published_at: string;
  html_url: string;
  body: string | null;
  tarball_url: string;
  zipball_url: string;
  assets: Array<{
    id: number;
    name: string;
    size: number;
    browser_download_url: string;
    content_type: string;
  }>;
}

export interface PlatformAssetGroup {
  id: 'macos' | 'linux' | 'windows';
  title: string;
  assets: GitHubReleaseAsset[];
}

function assetPlatform(
  name: string,
): 'macos' | 'linux' | 'windows' | 'other' {
  const lower = name.toLowerCase();
  if (lower.includes('mac') || lower.includes('darwin') || lower.endsWith('.dmg'))
    return 'macos';
  if (
    lower.includes('linux') ||
    lower.includes('appimage') ||
    lower.endsWith('.deb') ||
    lower.endsWith('.rpm')
  )
    return 'linux';
  if (
    lower.includes('windows') ||
    lower.includes('win') ||
    lower.endsWith('.msi') ||
    lower.endsWith('.exe')
  )
    return 'windows';
  return 'other';
}

export function groupReleaseAssets(
  assets: GitHubReleaseAsset[],
): PlatformAssetGroup[] {
  const groups: PlatformAssetGroup[] = [
    { id: 'macos', title: 'macOS', assets: [] },
    { id: 'linux', title: 'Linux', assets: [] },
    { id: 'windows', title: 'Windows', assets: [] },
  ];

  for (const asset of assets) {
    const platform = assetPlatform(asset.name);
    if (platform === 'other') continue;
    groups.find((group) => group.id === platform)?.assets.push(asset);
  }

  return groups.filter((group) => group.assets.length > 0);
}

export function assetArch(name: string): string | null {
  const lower = name.toLowerCase();
  if (lower.includes('aarch64') || lower.includes('arm64')) return 'arm64';
  if (lower.includes('x86_64') || lower.includes('amd64') || lower.includes('x64'))
    return 'x64';
  return null;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  let size = bytes / 1024;
  let unit = 'KB';
  if (size >= 1024) {
    size /= 1024;
    unit = 'MB';
  }
  if (size >= 1024) {
    size /= 1024;
    unit = 'GB';
  }
  return `${size.toFixed(size >= 10 ? 0 : 1)} ${unit}`;
}

export function formatReleaseDate(iso: string): string {
  return new Date(iso).toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

export async function fetchLatestGitHubRelease(): Promise<GitHubRelease> {
  const res = await fetch(
    `${GITHUB_API}/repos/${gitConfig.user}/${gitConfig.repo}/releases/latest`,
    {
      headers: {
        Accept: 'application/vnd.github+json',
        'User-Agent': `${gitConfig.repo}-website`,
      },
    },
  );

  if (!res.ok) {
    throw new Error(`GitHub API ${res.status}: ${await res.text()}`);
  }

  const data = (await res.json()) as GitHubReleaseResponse;

  return {
    name: data.name || data.tag_name,
    tagName: data.tag_name,
    publishedAt: data.published_at,
    htmlUrl: data.html_url,
    body: data.body,
    tarballUrl: data.tarball_url,
    zipballUrl: data.zipball_url,
    assets: data.assets.map((asset) => ({
      id: asset.id,
      name: asset.name,
      size: asset.size,
      downloadUrl: asset.browser_download_url,
      contentType: asset.content_type,
    })),
  };
}
