import { createFileRoute, Link } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { HomeLayout } from 'fumadocs-ui/layouts/home';
import { baseOptions } from '@/lib/layout.shared';
import {
  assetArch,
  fetchLatestGitHubRelease,
  formatBytes,
  formatReleaseDate,
  groupReleaseAssets,
  type GitHubRelease,
} from '@/lib/github-release';

const loadLatestRelease = createServerFn({ method: 'GET' }).handler(async () => {
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
});

export const Route = createFileRoute('/download')({
  component: DownloadPage,
  loader: () => loadLatestRelease(),
});

const linkClass =
  'text-fd-foreground underline decoration-fd-border underline-offset-4 hover:decoration-fd-primary';

function DownloadPage() {
  const { release, error } = Route.useLoaderData();
  const groups = release ? groupReleaseAssets(release.assets) : [];
  const githubUrl =
    release?.htmlUrl ?? 'https://github.com/lassejlv/termy/releases';

  return (
    <HomeLayout {...baseOptions()}>
      <main className="mx-auto flex w-full max-w-3xl flex-1 flex-col px-6 pb-24 pt-28 md:pt-36">
        <h1 className="font-medium text-5xl tracking-tight md:text-6xl">
          Download
        </h1>
        {release && (
          <p className="mt-4 font-mono text-sm text-fd-muted-foreground">
            <span className="text-fd-foreground">{release.tagName}</span>
            {' · '}
            {formatReleaseDate(release.publishedAt)}
            {' · '}
            <Link to="/releases" className={linkClass}>
              release notes
            </Link>
          </p>
        )}

        <div className="mt-12 divide-y divide-fd-border border-t border-fd-border">
          {error && (
            <p className="py-8 font-mono text-sm text-fd-muted-foreground">
              <span className="text-fd-error">error:</span> could not reach
              GitHub.{' '}
              <a
                href="https://github.com/lassejlv/termy/releases/latest"
                target="_blank"
                rel="noreferrer"
                className={linkClass}
              >
                Download from GitHub →
              </a>
            </p>
          )}

          {release && groups.length === 0 && (
            <p className="py-8 font-mono text-sm text-fd-muted-foreground">
              No binaries for this release yet.{' '}
              <a href={githubUrl} target="_blank" rel="noreferrer" className={linkClass}>
                View on GitHub →
              </a>
            </p>
          )}

          {groups.map((group) => (
            <section key={group.id} className="py-8">
              <h2 className="font-medium text-fd-foreground">{group.title}</h2>
              <ul className="mt-3 divide-y divide-fd-border">
                {group.assets.map((asset) => {
                  const arch = assetArch(asset.name);
                  return (
                    <li key={asset.id}>
                      <a
                        href={asset.downloadUrl}
                        className="group -mx-2 flex items-center gap-3 rounded-md px-2 py-3 transition-colors hover:bg-fd-accent"
                      >
                        <span className="min-w-0 flex-1 break-all font-mono text-sm">
                          {asset.name}
                        </span>
                        {arch && (
                          <span className="hidden shrink-0 font-mono text-[10px] text-fd-muted-foreground sm:inline">
                            {arch}
                          </span>
                        )}
                        <span className="shrink-0 font-mono text-xs text-fd-muted-foreground tabular-nums">
                          {formatBytes(asset.size)}
                        </span>
                      </a>
                    </li>
                  );
                })}
              </ul>
            </section>
          ))}
        </div>

        <footer className="mt-4 flex flex-wrap gap-x-6 gap-y-2 border-t border-fd-border pt-8 font-mono text-xs text-fd-muted-foreground">
          <Link to="/releases" className="hover:text-fd-foreground">
            all releases →
          </Link>
          <a
            href={githubUrl}
            target="_blank"
            rel="noreferrer"
            className="hover:text-fd-foreground"
          >
            GitHub ↗
          </a>
          {release && (
            <a href={release.tarballUrl} className="hover:text-fd-foreground">
              source tarball ↓
            </a>
          )}
        </footer>
      </main>
    </HomeLayout>
  );
}
