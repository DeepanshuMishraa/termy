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
