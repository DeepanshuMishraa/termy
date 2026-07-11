import { createFileRoute, Link } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import {
  MarketingPageShell,
  marketingFontLinks,
  marketingLinkClass,
  marketingMono,
  marketingPanelClass,
  marketingSerif,
} from '@/components/marketing-page-shell';
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
  head: () => ({ links: marketingFontLinks }),
  component: DownloadPage,
  loader: () => loadLatestRelease(),
});

function DownloadPage() {
  const { release, error } = Route.useLoaderData();
  const groups = release ? groupReleaseAssets(release.assets) : [];
  const githubUrl =
    release?.htmlUrl ?? 'https://github.com/lassejlv/termy/releases';

  return (
    <MarketingPageShell>
      <main className="mx-auto flex w-full max-w-4xl flex-col px-6 pt-20 pb-24 md:pt-28">
        <p className="text-sm text-[#7aa2f7]" style={{ fontFamily: marketingMono }}>
          $ termy install
        </p>
        <h1 className="mt-3 text-6xl leading-none tracking-tight text-[#e8eeff] md:text-7xl" style={{ fontFamily: marketingSerif }}>
          Download
        </h1>
        {release && (
          <p className="mt-5 text-sm text-[#787c99]" style={{ fontFamily: marketingMono }}>
            <span className="text-[#c0caf5]">{release.tagName}</span>
            {' · '}
            {formatReleaseDate(release.publishedAt)}
            {' · '}
            <Link to="/releases" className={marketingLinkClass}>
              release notes
            </Link>
          </p>
        )}

        <div className={`${marketingPanelClass} mt-12 divide-y divide-white/[0.07] px-5 sm:px-7`}>
          {error && (
            <p className="py-8 font-mono text-sm text-fd-muted-foreground">
              <span className="text-fd-error">error:</span> could not reach
              GitHub.{' '}
              <a
                href="https://github.com/lassejlv/termy/releases/latest"
                target="_blank"
                rel="noreferrer"
                className={marketingLinkClass}
              >
                Download from GitHub →
              </a>
            </p>
          )}

          {release && groups.length === 0 && (
            <p className="py-8 font-mono text-sm text-fd-muted-foreground">
              No binaries for this release yet.{' '}
              <a href={githubUrl} target="_blank" rel="noreferrer" className={marketingLinkClass}>
                View on GitHub →
              </a>
            </p>
          )}

          {groups.map((group) => (
            <section key={group.id} className="py-7">
              <h2 className="text-lg text-[#e8eeff]" style={{ fontFamily: marketingSerif }}>{group.title}</h2>
              <ul className="mt-3 divide-y divide-white/[0.06]">
                {group.assets.map((asset) => {
                  const arch = assetArch(asset.name);
                  return (
                    <li key={asset.id}>
                      <a
                        href={asset.downloadUrl}
                        className="group -mx-2 flex items-center gap-3 rounded-lg px-2 py-3.5 transition-colors hover:bg-white/[0.04]"
                      >
                        <span className="min-w-0 flex-1 break-all text-sm text-[#c0caf5]" style={{ fontFamily: marketingMono }}>
                          {asset.name}
                        </span>
                        {arch && (
                          <span className="hidden shrink-0 text-[10px] text-[#565f89] sm:inline" style={{ fontFamily: marketingMono }}>
                            {arch}
                          </span>
                        )}
                        <span className="shrink-0 text-xs text-[#787c99] tabular-nums" style={{ fontFamily: marketingMono }}>
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

        <footer className="mt-8 flex flex-wrap gap-x-6 gap-y-2 text-xs text-[#787c99]" style={{ fontFamily: marketingMono }}>
          <Link to="/releases" className="hover:text-white">
            all releases →
          </Link>
          <a
            href={githubUrl}
            target="_blank"
            rel="noreferrer"
            className="hover:text-white"
          >
            GitHub ↗
          </a>
          {release && (
            <a href={release.tarballUrl} className="hover:text-white">
              source tarball ↓
            </a>
          )}
        </footer>
      </main>
    </MarketingPageShell>
  );
}
