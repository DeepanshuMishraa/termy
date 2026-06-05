import { createFileRoute, Link, notFound } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { HomeLayout } from 'fumadocs-ui/layouts/home';
import { baseOptions } from '@/lib/layout.shared';
import { fetchReleaseBySlug } from '@/lib/notra';
import { Markdown } from '@/components/markdown';
import { PoweredByNotra } from '@/components/powered-by-notra';

const loadRelease = createServerFn({ method: 'GET' })
  .inputValidator((slug: string) => slug)
  .handler(async ({ data: slug }) => {
    const post = await fetchReleaseBySlug(slug);
    if (!post) return { post: null, notFound: true as const };
    return { post, notFound: false as const };
  });

export const Route = createFileRoute('/releases/$slug')({
  component: ReleaseDetail,
  loader: async ({ params }) => {
    const result = await loadRelease({ data: params.slug });
    if (result.notFound) throw notFound();
    return result;
  },
});

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

const reveal =
  'motion-safe:animate-[termy-fade-up_0.7s_cubic-bezier(0.22,1,0.36,1)_both]';

function ReleaseDetail() {
  const { post } = Route.useLoaderData();
  if (!post) return null;

  return (
    <HomeLayout {...baseOptions()}>
      <main className="flex flex-1 flex-col">
        <article className="mx-auto w-full max-w-3xl px-6 pt-20 pb-12">
          <Link
            to="/releases"
            className={`font-mono text-xs text-fd-muted-foreground transition-colors hover:text-fd-foreground ${reveal}`}
          >
            ← all releases
          </Link>

          <header
            className={`mt-10 border-b border-fd-border pb-10 ${reveal}`}
            style={{ animationDelay: '80ms' }}
          >
            <time
              dateTime={post.createdAt}
              className="font-mono text-xs text-fd-muted-foreground"
            >
              {formatDate(post.createdAt)}
            </time>
            <h1 className="mt-4 text-balance font-medium text-4xl tracking-tight md:text-5xl">
              {post.title}
            </h1>
          </header>

          <div
            className={`prose prose-sm mt-10 max-w-none text-fd-foreground ${reveal}`}
            style={{ animationDelay: '160ms' }}
          >
            <Markdown text={post.markdown || post.content} />
          </div>
        </article>

        <PoweredByNotra />
      </main>
    </HomeLayout>
  );
}
