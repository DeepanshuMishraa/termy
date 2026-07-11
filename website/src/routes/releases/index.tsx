import { createFileRoute, Link } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import {
  MarketingPageShell,
  marketingFontLinks,
  marketingMono,
  marketingPanelClass,
  marketingSerif,
} from '@/components/marketing-page-shell';
import {
  fetchGitHubReleases,
  formatReleaseDay,
  groupGitHubReleasesByYear,
  type GitHubRelease,
} from '@/lib/github-release';

const loadReleases = createServerFn({ method: 'GET' }).handler(async () => {
  try {
    return {
      releases: await fetchGitHubReleases(),
      error: null as string | null,
    };
  } catch (err) {
    return {
      releases: [] as GitHubRelease[],
      error: err instanceof Error ? err.message : 'Failed to load releases',
    };
  }
});

export const Route = createFileRoute('/releases/')({
  head: () => ({ links: marketingFontLinks }),
  component: ReleasesPage,
  loader: () => loadReleases(),
});

function ReleasesPage() {
  const { releases, error } = Route.useLoaderData();
  const groups = groupGitHubReleasesByYear(releases);

  return (
    <MarketingPageShell>
      <main className="mx-auto flex w-full max-w-4xl flex-col px-6 pt-20 pb-20 md:pt-28">
        <p className="text-sm text-[#7aa2f7]" style={{ fontFamily: marketingMono }}>$ termy releases</p>
        <h1 className="mt-3 text-6xl leading-none tracking-tight text-[#e8eeff] md:text-7xl" style={{ fontFamily: marketingSerif }}>
          Releases
        </h1>

        <div className={`${marketingPanelClass} mt-12 divide-y divide-white/[0.07] px-5 sm:px-7`}>
          {error && (
            <p className="py-8 font-mono text-sm text-fd-muted-foreground">
              <span className="text-fd-error">error:</span> could not load
              releases.
            </p>
          )}

          {!error && releases.length === 0 && (
            <p className="py-8 font-mono text-sm text-fd-muted-foreground">
              No releases yet.
            </p>
          )}

          {groups.map((group) => (
            <section key={group.year} className="py-8">
              <h2 className="text-xs text-[#565f89]" style={{ fontFamily: marketingMono }}>
                {group.year}
              </h2>
              <ul className="mt-3 divide-y divide-white/[0.06]">
                {group.releases.map((release) => (
                  <li key={release.id}>
                    <Link
                      to="/releases/$slug"
                      params={{ slug: release.tagName }}
                      className="group -mx-2 flex items-baseline gap-4 rounded-lg px-2 py-3.5 transition-colors hover:bg-white/[0.04]"
                    >
                      <time
                        dateTime={release.publishedAt}
                        className="w-14 shrink-0 text-xs text-[#565f89]" style={{ fontFamily: marketingMono }}
                      >
                        {formatReleaseDay(release.publishedAt)}
                      </time>
                      <span className="min-w-0 flex-1 text-sm text-[#c0caf5] transition-colors group-hover:text-white">
                        {release.name}
                        {release.prerelease && (
                          <span className="ml-2 text-[10px] text-[#7aa2f7]">
                            prerelease
                          </span>
                        )}
                      </span>
                    </Link>
                  </li>
                ))}
              </ul>
            </section>
          ))}
        </div>

        <a
          href="https://github.com/lassejlv/termy/releases"
          target="_blank"
          rel="noreferrer"
          className="mt-8 text-xs text-[#787c99] hover:text-white"
          style={{ fontFamily: marketingMono }}
        >
          View releases on GitHub ↗
        </a>
      </main>
    </MarketingPageShell>
  );
}
