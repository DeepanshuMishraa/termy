import { createFileRoute, Link } from '@tanstack/react-router';
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

function assetArch(asset: GitHubReleaseAsset): string | null {
  const name = asset.name.toLowerCase();
  if (name.includes('aarch64') || name.includes('arm64')) return 'arm64';
  if (name.includes('x86_64') || name.includes('amd64') || name.includes('x64'))
    return 'x64';
  return null;
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

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

function platformGroups(assets: GitHubReleaseAsset[]): PlatformGroup[] {
  const groups: PlatformGroup[] = [
    { id: 'macos', title: 'macOS', assets: [] },
    { id: 'linux', title: 'Linux', assets: [] },
    { id: 'windows', title: 'Windows', assets: [] },
  ];

  for (const asset of assets) {
    const platform = assetPlatform(asset);
    if (platform === 'other') continue;
    groups.find((group) => group.id === platform)?.assets.push(asset);
  }

  return groups.filter((group) => group.assets.length > 0);
}

const reveal =
  'motion-safe:animate-[termy-fade-up_0.7s_cubic-bezier(0.22,1,0.36,1)_both]';

function Caret() {
  return (
    <span
      aria-hidden
      className="ml-1 inline-block h-[1em] w-[0.55ch] translate-y-[0.12em] bg-fd-primary motion-safe:animate-[termy-caret-blink_1.1s_steps(1)_infinite]"
    />
  );
}

function DownloadPage() {
  const { release, error } = Route.useLoaderData();
  const groups = release ? platformGroups(release.assets) : [];

  return (
    <HomeLayout {...baseOptions()}>
      <main className="flex flex-1 flex-col">
        <section className="mx-auto w-full max-w-3xl px-6 pt-28 pb-16 md:pt-40">
          <p
            className={`font-mono text-xs text-fd-muted-foreground ${reveal}`}
          >
            <span className="select-none text-fd-primary">$ </span>
            termy update
            <Caret />
          </p>
          <h1
            className={`mt-6 text-balance font-medium text-5xl tracking-tight md:text-6xl ${reveal}`}
            style={{ animationDelay: '80ms' }}
          >
            Download Termy.
          </h1>
          {release && (
            <p
              className={`mt-6 font-mono text-sm text-fd-muted-foreground ${reveal}`}
              style={{ animationDelay: '160ms' }}
            >
              <span className="text-fd-foreground">{release.tagName}</span>
              <span className="mx-3 select-none opacity-50">·</span>
              {formatDate(release.publishedAt)}
              <span className="mx-3 select-none opacity-50">·</span>
              <Link
                to="/releases"
                className="underline decoration-fd-border underline-offset-4 transition-colors hover:text-fd-foreground hover:decoration-fd-primary"
              >
                release notes
              </Link>
            </p>
          )}
        </section>

        <section className="mx-auto w-full max-w-3xl px-6 pb-12">
          {error && (
            <div
              className={`border-t border-fd-border py-10 font-mono text-sm text-fd-muted-foreground ${reveal}`}
              style={{ animationDelay: '200ms' }}
            >
              <span className="text-fd-error">error:</span> could not reach the
              GitHub API.{' '}
              <a
                href="https://github.com/lassejlv/termy/releases/latest"
                target="_blank"
                rel="noreferrer"
                className="text-fd-foreground underline decoration-fd-border underline-offset-4 hover:decoration-fd-primary"
              >
                Download from GitHub instead →
              </a>
            </div>
          )}

          {release && groups.length === 0 && (
            <div
              className={`border-t border-fd-border py-10 font-mono text-sm text-fd-muted-foreground ${reveal}`}
              style={{ animationDelay: '200ms' }}
            >
              No binaries attached to this release yet.{' '}
              <a
                href={release.htmlUrl}
                target="_blank"
                rel="noreferrer"
                className="text-fd-foreground underline decoration-fd-border underline-offset-4 hover:decoration-fd-primary"
              >
                Get the source from GitHub →
              </a>
            </div>
          )}

          {groups.map((group, groupIndex) => (
            <section
              key={group.id}
              className={`border-t border-fd-border py-10 ${reveal}`}
              style={{ animationDelay: `${240 + groupIndex * 90}ms` }}
            >
              <div className="grid gap-6 md:grid-cols-[10rem_1fr]">
                <div>
                  <h2 className="font-medium text-fd-foreground tracking-tight">
                    {group.title}
                  </h2>
                  <p className="mt-1 font-mono text-xs text-fd-muted-foreground">
                    {group.assets.length}{' '}
                    {group.assets.length === 1 ? 'build' : 'builds'}
                  </p>
                </div>

                <div className="divide-y divide-fd-border">
                  {group.assets.map((asset) => {
                    const arch = assetArch(asset);
                    return (
                      <a
                        key={asset.id}
                        href={asset.downloadUrl}
                        className="group -mx-3 flex items-center gap-4 rounded-md px-3 py-3.5 transition-colors hover:bg-fd-accent"
                      >
                        <span className="min-w-0 flex-1 break-all font-mono text-sm text-fd-foreground">
                          {asset.name}
                        </span>
                        {arch && (
                          <span className="hidden shrink-0 rounded-sm border border-fd-border px-1.5 py-0.5 font-mono text-[10px] text-fd-muted-foreground sm:inline-block">
                            {arch}
                          </span>
                        )}
                        <span className="shrink-0 font-mono text-xs text-fd-muted-foreground tabular-nums">
                          {formatSize(asset.size)}
                        </span>
                        <span
                          aria-hidden
                          className="shrink-0 text-fd-muted-foreground transition-all group-hover:translate-y-0.5 group-hover:text-fd-primary"
                        >
                          ↓
                        </span>
                      </a>
                    );
                  })}
                </div>
              </div>
            </section>
          ))}
        </section>

        <section className="mx-auto w-full max-w-3xl px-6 pb-24">
          <div
            className={`flex flex-wrap items-center gap-x-6 gap-y-2 border-t border-fd-border pt-8 font-mono text-xs text-fd-muted-foreground ${reveal}`}
            style={{ animationDelay: `${240 + groups.length * 90}ms` }}
          >
            <Link
              to="/releases"
              className="transition-colors hover:text-fd-foreground"
            >
              all releases →
            </Link>
            <a
              href={
                release?.htmlUrl ?? 'https://github.com/lassejlv/termy/releases'
              }
              target="_blank"
              rel="noreferrer"
              className="transition-colors hover:text-fd-foreground"
            >
              view on GitHub ↗
            </a>
            {release && (
              <a
                href={release.tarballUrl}
                className="transition-colors hover:text-fd-foreground"
              >
                source tarball ↓
              </a>
            )}
          </div>
        </section>
      </main>
    </HomeLayout>
  );
}
