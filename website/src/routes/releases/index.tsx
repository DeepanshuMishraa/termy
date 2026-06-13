import { createFileRoute, Link } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { HomeLayout } from 'fumadocs-ui/layouts/home';
import { baseOptions } from '@/lib/layout.shared';
import {
  fetchReleases,
  formatReleaseDay,
  groupReleasesByYear,
  releaseSlug,
  type NotraPost,
} from '@/lib/notra';
import { PoweredByNotra } from '@/components/powered-by-notra';

const loadReleases = createServerFn({ method: 'GET' }).handler(async () => {
  try {
    return {
      posts: await fetchReleases(),
      error: null as string | null,
    };
  } catch (err) {
    return {
      posts: [] as NotraPost[],
      error: err instanceof Error ? err.message : 'Failed to load releases',
    };
  }
});

export const Route = createFileRoute('/releases/')({
  component: ReleasesPage,
  loader: () => loadReleases(),
});

function ReleasesPage() {
  const { posts, error } = Route.useLoaderData();
  const groups = groupReleasesByYear(posts);

  return (
    <HomeLayout {...baseOptions()}>
      <main className="mx-auto flex w-full max-w-3xl flex-1 flex-col px-6 pb-16 pt-28 md:pt-36">
        <h1 className="font-medium text-5xl tracking-tight md:text-6xl">
          Releases
        </h1>

        <div className="mt-12 divide-y divide-fd-border border-t border-fd-border">
          {error && (
            <p className="py-8 font-mono text-sm text-fd-muted-foreground">
              <span className="text-fd-error">error:</span> could not load
              releases.
            </p>
          )}

          {!error && posts.length === 0 && (
            <p className="py-8 font-mono text-sm text-fd-muted-foreground">
              No releases yet.
            </p>
          )}

          {groups.map((group) => (
            <section key={group.year} className="py-8">
              <h2 className="font-mono text-xs text-fd-muted-foreground">
                {group.year}
              </h2>
              <ul className="mt-3 divide-y divide-fd-border">
                {group.posts.map((post) => (
                  <li key={post.id}>
                    <Link
                      to="/releases/$slug"
                      params={{ slug: releaseSlug(post) }}
                      className="group -mx-2 flex items-baseline gap-4 rounded-md px-2 py-3 transition-colors hover:bg-fd-accent"
                    >
                      <time
                        dateTime={post.createdAt}
                        className="w-14 shrink-0 font-mono text-xs text-fd-muted-foreground"
                      >
                        {formatReleaseDay(post.createdAt)}
                      </time>
                      <span className="min-w-0 flex-1 text-sm font-medium">
                        {post.title}
                      </span>
                    </Link>
                  </li>
                ))}
              </ul>
            </section>
          ))}
        </div>

        <PoweredByNotra />
      </main>
    </HomeLayout>
  );
}
