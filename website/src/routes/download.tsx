import { createFileRoute } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { HomeLayout } from 'fumadocs-ui/layouts/home';
import { baseOptions } from '@/lib/layout.shared';
import {
  fetchLatestGitHubRelease,
  type GitHubRelease,
  type GitHubReleaseAsset,
} from '@/lib/github-release';

const loadLatestRelease = createServerFn({ method: 'GET' }).handler(
  async () => {
    try {
      return {
        release: await fetchLatestGitHubRelease(),
        error: null as string | null,
      };
    } catch (err) {
      return {
        release: null as GitHubRelease | null,
        error:
          err instanceof Error ? err.message : 'Failed to load latest release',
      };
    }
  },
);

export const Route = createFileRoute('/download')({
  component: DownloadPage,
  loader: () => loadLatestRelease(),
});

type Platform = 'macos' | 'linux' | 'windows' | 'other';

interface PlatformGroup {
  id: Exclude<Platform, 'other'>;
  title: string;
  description: string;
  assets: GitHubReleaseAsset[];
}

function assetPlatform(asset: GitHubReleaseAsset): Platform {
  const name = asset.name.toLowerCase();

  if (name.includes('mac') || name.includes('darwin') || name.endsWith('.dmg')) {
    return 'macos';
  }

  if (
    name.includes('linux') ||
    name.includes('appimage') ||
    name.endsWith('.deb') ||
    name.endsWith('.rpm')
  ) {
    return 'linux';
  }

  if (
    name.includes('windows') ||
    name.includes('win') ||
    name.endsWith('.msi') ||
    name.endsWith('.exe')
  ) {
    return 'windows';
  }

  return 'other';
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB'];
  let size = bytes / 1024;
  let unit = units[0];

  for (const nextUnit of units.slice(1)) {
    if (size < 1024) break;
    size /= 1024;
    unit = nextUnit;
  }

  return `${size.toFixed(size >= 10 ? 0 : 1)} ${unit}`;
}

function platformGroups(assets: GitHubReleaseAsset[]): PlatformGroup[] {
  const groups: PlatformGroup[] = [
    {
      id: 'macos',
      title: 'macOS',
      description: 'Disk images and macOS builds.',
      assets: [],
    },
    {
      id: 'linux',
      title: 'Linux',
      description: 'AppImage, tarball, and package builds.',
      assets: [],
    },
    {
      id: 'windows',
      title: 'Windows',
      description: 'Installer and Windows builds.',
      assets: [],
    },
  ];

  for (const asset of assets) {
    const platform = assetPlatform(asset);
    if (platform === 'other') continue;
    groups.find((group) => group.id === platform)?.assets.push(asset);
  }

  return groups.filter((group) => group.assets.length > 0);
}

function DownloadPage() {
  const { release, error } = Route.useLoaderData();
  const groups = release ? platformGroups(release.assets) : [];

  return (
    <HomeLayout {...baseOptions()}>
      <main className="flex flex-1 flex-col">
        <section className="mx-auto w-full max-w-5xl px-6 pt-24 pb-12 md:pt-32">
          <div className="max-w-3xl">
            <span className="inline-flex items-center rounded-full border border-fd-border bg-fd-card px-3 py-1 text-xs uppercase tracking-wider text-fd-muted-foreground">
              Download
            </span>
            <h1 className="mt-6 text-balance font-medium text-5xl tracking-tight md:text-6xl">
              Get the latest Termy release.
            </h1>
            <p className="mt-5 max-w-2xl text-balance text-fd-muted-foreground md:text-lg">
              The latest release is fetched from GitHub and matched to the
              available installers, packages, and archives.
            </p>
          </div>
        </section>

        <section className="mx-auto w-full max-w-5xl px-6 pb-20">
          <div className="flex flex-col gap-4">
            {error && (
              <div className="rounded-lg border border-fd-border bg-fd-card p-5 text-sm text-fd-muted-foreground">
                Unable to load the latest GitHub release right now. Use the
                GitHub releases page below while the API is unavailable.
              </div>
            )}

            {release && groups.length === 0 && (
              <div className="rounded-lg border border-fd-border bg-fd-card p-5 text-sm text-fd-muted-foreground">
                This release does not have downloadable assets attached yet.
                Source archives are still available from GitHub.
              </div>
            )}

            {groups.map((group) => (
              <section
                key={group.id}
                className="rounded-lg border border-fd-border bg-fd-card p-5"
              >
                <div>
                  <h2 className="font-medium text-xl tracking-tight">
                    {group.title}
                  </h2>
                  <p className="mt-1 text-sm text-fd-muted-foreground">
                    {group.description}
                  </p>
                </div>

                <div className="mt-5 divide-y divide-fd-border overflow-hidden rounded-md border border-fd-border bg-fd-background">
                  {group.assets.map((asset) => (
                    <a
                      key={asset.id}
                      href={asset.downloadUrl}
                      className="flex flex-col gap-3 px-4 py-4 transition-colors hover:bg-fd-accent sm:flex-row sm:items-center sm:justify-between"
                    >
                      <span className="min-w-0">
                        <span className="block break-all font-medium text-sm text-fd-foreground">
                          {asset.name}
                        </span>
                        <span className="mt-1 block text-xs text-fd-muted-foreground">
                          {formatSize(asset.size)}
                        </span>
                      </span>
                      <span className="inline-flex shrink-0 items-center gap-1.5 text-sm font-medium text-fd-foreground">
                        Download
                      </span>
                    </a>
                  ))}
                </div>
              </section>
            ))}
          </div>
        </section>
      </main>
    </HomeLayout>
  );
}
