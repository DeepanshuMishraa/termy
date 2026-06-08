const NOTRA_API_URL = 'https://api.usenotra.com/v1/posts';

export interface NotraPost {
  id: string;
  title: string;
  slug: string | null;
  content: string;
  markdown: string;
  contentType: string;
  status: 'draft' | 'published';
  createdAt: string;
  updatedAt: string;
}

interface NotraListResponse {
  posts: NotraPost[];
  pagination: {
    limit: number;
    currentPage: number;
    nextPage: number | null;
    previousPage: number | null;
    totalPages: number;
    totalItems: number;
  };
}

function apiKey(): string {
  const key = process.env.NOTRA_API_KEY;
  if (!key) throw new Error('NOTRA_API_KEY is not set');
  return key;
}

export function releaseSlug(post: NotraPost): string {
  return post.slug ?? post.id;
}

export interface ReleaseYearGroup {
  year: string;
  posts: NotraPost[];
}

export function formatReleaseDay(iso: string): string {
  return new Date(iso).toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
  });
}

export function formatReleaseDate(iso: string): string {
  return new Date(iso).toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

export function groupReleasesByYear(posts: NotraPost[]): ReleaseYearGroup[] {
  const groups: ReleaseYearGroup[] = [];

  for (const post of posts) {
    const year = String(new Date(post.createdAt).getFullYear());
    const last = groups[groups.length - 1];
    if (last?.year === year) last.posts.push(post);
    else groups.push({ year, posts: [post] });
  }

  return groups;
}

export async function fetchReleases(): Promise<NotraPost[]> {
  const url = new URL(NOTRA_API_URL);
  url.searchParams.set('contentType', 'changelog');
  url.searchParams.set('status', 'published');
  url.searchParams.set('sort', 'desc');
  url.searchParams.set('limit', '50');

  const res = await fetch(url, {
    headers: {
      Authorization: `Bearer ${apiKey()}`,
      Accept: 'application/json',
    },
  });

  if (!res.ok) {
    throw new Error(`Notra API ${res.status}: ${await res.text()}`);
  }

  const data = (await res.json()) as NotraListResponse;
  return data.posts;
}

export async function fetchReleaseBySlug(
  slug: string,
): Promise<NotraPost | null> {
  const posts = await fetchReleases();
  return posts.find((p) => releaseSlug(p) === slug) ?? null;
}
