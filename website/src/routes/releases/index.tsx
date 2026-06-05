import { createFileRoute, Link } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { HomeLayout } from 'fumadocs-ui/layouts/home';
import { baseOptions } from '@/lib/layout.shared';
import { fetchReleases, releaseSlug, type NotraPost } from '@/lib/notra';
import { PoweredByNotra } from '@/components/powered-by-notra';

const loadReleases = createServerFn({ method: 'GET' }).handler(async () => {
  try {
    const posts = await fetchReleases();
    return { posts, error: null as string | null };
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

function formatDay(iso: string): string {
  return new Date(iso).toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
  });
}

function postYear(post: NotraPost): string {
  return String(new Date(post.createdAt).getFullYear());
}

interface YearGroup {
  year: string;
  posts: NotraPost[];
}

function groupByYear(posts: NotraPost[]): YearGroup[] {
  const groups: YearGroup[] = [];

  for (const post of posts) {
    const year = postYear(post);
    const last = groups[groups.length - 1];
    if (last && last.year === year) {
      last.posts.push(post);
    } else {
      groups.push({ year, posts: [post] });
    }
  }

  return groups;
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

function ReleasesPage() {
  const { posts, error } = Route.useLoaderData();
  const groups = groupByYear(posts);

  return (
    <HomeLayout {...baseOptions()}>
      <main className="flex flex-1 flex-col">
        <section className="mx-auto w-full max-w-3xl px-6 pt-28 pb-16 md:pt-40">
          <p className={`font-mono text-xs text-fd-muted-foreground ${reveal}`}>
            <span className="select-none text-fd-primary">$ </span>
            termy changelog
            <Caret />
          </p>
          <h1
            className={`mt-6 font-medium text-5xl tracking-tight md:text-6xl ${reveal}`}
            style={{ animationDelay: '80ms' }}
          >
            Releases.
          </h1>
          <p
            className={`mt-6 max-w-xl text-balance text-fd-muted-foreground md:text-lg ${reveal}`}
            style={{ animationDelay: '160ms' }}
          >
            New features, fixes, and improvements shipping in Termy.
          </p>
        </section>

        <section className="mx-auto w-full max-w-3xl px-6 pb-20">
          {error && (
            <div
              className={`border-t border-fd-border py-10 font-mono text-sm text-fd-muted-foreground ${reveal}`}
              style={{ animationDelay: '200ms' }}
            >
              <span className="text-fd-error">error:</span> could not load
              releases right now.
            </div>
          )}

          {!error && posts.length === 0 && (
            <div
              className={`border-t border-fd-border py-10 font-mono text-sm text-fd-muted-foreground ${reveal}`}
              style={{ animationDelay: '200ms' }}
            >
              No releases yet.
            </div>
          )}

          {groups.map((group, groupIndex) => (
            <section
              key={group.year}
              className={`border-t border-fd-border py-10 ${reveal}`}
              style={{ animationDelay: `${240 + groupIndex * 90}ms` }}
            >
              <div className="grid gap-6 md:grid-cols-[10rem_1fr]">
                <div>
                  <h2 className="font-medium text-fd-foreground tracking-tight">
                    {group.year}
                  </h2>
                  <p className="mt-1 font-mono text-xs text-fd-muted-foreground">
                    {group.posts.length}{' '}
                    {group.posts.length === 1 ? 'release' : 'releases'}
                  </p>
                </div>

                <div className="divide-y divide-fd-border">
                  {group.posts.map((post) => (
                    <Link
                      key={post.id}
                      to="/releases/$slug"
                      params={{ slug: releaseSlug(post) }}
                      className="group -mx-3 flex items-baseline gap-4 rounded-md px-3 py-3.5 transition-colors hover:bg-fd-accent"
                    >
                      <time
                        dateTime={post.createdAt}
                        className="w-14 shrink-0 font-mono text-xs text-fd-muted-foreground"
                      >
                        {formatDay(post.createdAt)}
                      </time>
                      <span className="min-w-0 flex-1 font-medium text-sm text-fd-foreground">
                        {post.title}
                      </span>
                      <span
                        aria-hidden
                        className="shrink-0 self-center text-fd-muted-foreground transition-all group-hover:translate-x-0.5 group-hover:text-fd-primary"
                      >
                        →
                      </span>
                    </Link>
                  ))}
                </div>
              </div>
            </section>
          ))}
        </section>

        <PoweredByNotra />
      </main>
    </HomeLayout>
  );
}
