import { createFileRoute, Link, notFound } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import {
  MarketingPageShell,
  marketingFontLinks,
  marketingMono,
  marketingPanelClass,
} from '@/components/marketing-page-shell';
import {
  fetchGitHubReleaseByTag,
  formatReleaseDate,
} from '@/lib/github-release';
import { Markdown } from '@/components/markdown';

const loadRelease = createServerFn({ method: 'GET' })
  .inputValidator((slug: string) => slug)
  .handler(async ({ data: slug }) => {
    const release = await fetchGitHubReleaseByTag(slug);
    if (!release) return { release: null, notFound: true as const };
    return { release, notFound: false as const };
  });

export const Route = createFileRoute('/releases/$slug')({
  head: () => ({ links: marketingFontLinks }),
  component: ReleaseDetail,
  loader: async ({ params }) => {
    const result = await loadRelease({ data: params.slug });
    if (result.notFound) throw notFound();
    return result;
  },
});

function ReleaseDetail() {
  const { release } = Route.useLoaderData();
  if (!release) return null;

  return (
    <MarketingPageShell>
      <main className="mx-auto flex w-full max-w-4xl flex-col px-6 pt-16 pb-20 md:pt-20">
        <Link
          to="/releases"
          className="text-xs text-[#787c99] hover:text-white"
          style={{ fontFamily: marketingMono }}
        >
          ← all releases
        </Link>

        <article className="mt-10">
          <time
            dateTime={release.publishedAt}
            className="text-xs text-[#7aa2f7]"
            style={{ fontFamily: marketingMono }}
          >
            {formatReleaseDate(release.publishedAt)}
          </time>
          <h1 className="mt-3 text-balance text-3xl font-medium leading-tight tracking-tight text-[#e8eeff] md:text-4xl" style={{ fontFamily: marketingMono }}>
            {release.name}
          </h1>
          <div className={`${marketingPanelClass} prose prose-invert prose-sm mt-10 max-w-none px-6 py-8 text-[#c0caf5] sm:px-9 sm:py-10`}>
            <Markdown text={release.body || '_No release notes were provided._'} />
          </div>
        </article>

        <div className="mt-8 flex flex-wrap gap-6 text-xs text-[#787c99]" style={{ fontFamily: marketingMono }}>
          <a href={release.htmlUrl} target="_blank" rel="noreferrer" className="hover:text-white">
            View on GitHub ↗
          </a>
          <a href={release.tarballUrl} className="hover:text-white">
            Source tarball ↓
          </a>
        </div>
      </main>
    </MarketingPageShell>
  );
}
