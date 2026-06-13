import { createFileRoute, Link, notFound } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { HomeLayout } from 'fumadocs-ui/layouts/home';
import { baseOptions } from '@/lib/layout.shared';
import { fetchReleaseBySlug, formatReleaseDate } from '@/lib/notra';
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

function ReleaseDetail() {
  const { post } = Route.useLoaderData();
  if (!post) return null;

  return (
    <HomeLayout {...baseOptions()}>
      <main className="mx-auto flex w-full max-w-3xl flex-1 flex-col px-6 pb-16 pt-20">
        <Link
          to="/releases"
          className="font-mono text-xs text-fd-muted-foreground hover:text-fd-foreground"
        >
          ← all releases
        </Link>

        <article className="mt-10">
          <time
            dateTime={post.createdAt}
            className="font-mono text-xs text-fd-muted-foreground"
          >
            {formatReleaseDate(post.createdAt)}
          </time>
          <h1 className="mt-3 text-balance font-medium text-4xl tracking-tight md:text-5xl">
            {post.title}
          </h1>
          <div className="prose prose-sm mt-10 max-w-none border-t border-fd-border pt-10 text-fd-foreground">
            <Markdown text={post.markdown || post.content} />
          </div>
        </article>

        <PoweredByNotra />
      </main>
    </HomeLayout>
  );
}
