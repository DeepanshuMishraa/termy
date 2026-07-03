import { createFileRoute, Link } from '@tanstack/react-router';
import { HomeLayout } from 'fumadocs-ui/layouts/home';
import { baseOptions } from '@/lib/layout.shared';
import { sponsors } from '@/lib/sponsors';

const landingScreenshot = '/termy-landing.png';

export const Route = createFileRoute('/')({
  component: Home,
  head: () => ({
    links: [
      { rel: 'preconnect', href: 'https://fonts.googleapis.com' },
      {
        rel: 'preconnect',
        href: 'https://fonts.gstatic.com',
        crossOrigin: 'anonymous',
      },
      {
        rel: 'stylesheet',
        href: 'https://fonts.googleapis.com/css2?family=Instrument+Serif:ital@0;1&display=swap',
      },
    ],
  }),
});

const features = [
  {
    id: '01',
    command: 'render --gpu',
    title: 'Fast',
    description:
      'GPU-accelerated rendering with instant startup. Built on the same UI framework as Zed, so every frame is painted by your graphics card — not your patience.',
    span: true,
  },
  {
    id: '02',
    command: 'vim ~/.config/termy',
    title: 'Configurable',
    description: 'One plain-text config file. Full control, no dialogs.',
  },
  {
    id: '03',
    command: 'uname -a',
    title: 'Native',
    description: 'Runs natively on macOS, Windows, and Linux.',
  },
  {
    id: '04',
    command: 'termy theme list',
    title: 'Themable',
    description: 'Built-in themes or create your own.',
  },
  {
    id: '05',
    command: 'split --right',
    title: 'Powerful',
    description: 'Splits, tabs, and multiplexing built-in.',
  },
];

const sessionLines = [
  { prompt: true, text: 'termy --version', delay: '0.2s' },
  { prompt: false, text: 'termy — fast, native, GPU-accelerated', delay: '0.9s' },
  { prompt: true, text: 'time termy', delay: '1.5s' },
  { prompt: false, text: '0.00s user  0.01s system — ready.', delay: '2.2s' },
];

const reveal =
  'motion-safe:animate-[termy-fade-up_0.8s_cubic-bezier(0.22,1,0.36,1)_both]';

function Home() {
  return (
    <HomeLayout {...baseOptions()}>
      <main className="termy-home relative flex flex-1 flex-col overflow-x-clip">
        {/* Atmosphere: glow, grid, noise */}
        <div aria-hidden className="termy-atmosphere">
          <div className="termy-glow" />
          <div className="termy-grid" />
          <div className="termy-noise" />
        </div>

        {/* ── Hero ─────────────────────────────────────────── */}
        <section className="relative mx-auto w-full max-w-5xl px-6 pt-24 text-center md:pt-32">
          <p
            className={`inline-flex items-center gap-2 rounded-full border border-fd-border bg-fd-card/60 px-3.5 py-1.5 font-mono text-[11px] tracking-wide text-fd-muted-foreground backdrop-blur ${reveal}`}
          >
            <span className="relative flex size-1.5">
              <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-fd-success opacity-60" />
              <span className="relative inline-flex size-1.5 rounded-full bg-fd-success" />
            </span>
            free &amp; open source · macOS / Windows / Linux
          </p>

          <h1
            className={`termy-display mx-auto mt-8 max-w-4xl text-balance text-6xl leading-[1.02] md:text-7xl lg:text-8xl ${reveal}`}
            style={{ animationDelay: '90ms' }}
          >
            The terminal,
            <br />
            <em className="termy-display-accent">beautifully native.</em>
          </h1>

          <p
            className={`mx-auto mt-7 max-w-xl text-balance text-fd-muted-foreground md:text-lg ${reveal}`}
            style={{ animationDelay: '180ms' }}
          >
            GPU-accelerated rendering, one plain-text config, and multiplexing
            built in. Made for people who live in the shell.
          </p>

          <div
            className={`mt-10 flex flex-wrap items-center justify-center gap-4 ${reveal}`}
            style={{ animationDelay: '260ms' }}
          >
            <Link to="/download" className="termy-btn-primary">
              <span className="font-mono opacity-70">↓</span> Download Termy
            </Link>
            <Link
              to="/docs/$"
              params={{ _splat: '' }}
              className="termy-btn-ghost"
            >
              read the docs →
            </Link>
          </div>

          {/* Self-typing session */}
          <div
            className={`mx-auto mt-14 w-full max-w-md ${reveal}`}
            style={{ animationDelay: '340ms' }}
          >
            <div className="termy-session text-left font-mono text-[13px] leading-7">
              {sessionLines.map((line) => (
                <p
                  key={line.text}
                  className="termy-session-line"
                  style={{ animationDelay: line.delay }}
                >
                  {line.prompt ? (
                    <>
                      <span className="text-fd-success select-none">❯ </span>
                      <span className="text-fd-foreground">{line.text}</span>
                    </>
                  ) : (
                    <span className="text-fd-muted-foreground">
                      {line.text}
                    </span>
                  )}
                </p>
              ))}
              <p
                className="termy-session-line"
                style={{ animationDelay: '2.9s' }}
              >
                <span className="text-fd-success select-none">❯ </span>
                <span className="termy-caret" aria-hidden />
              </p>
            </div>
          </div>
        </section>

        {/* ── Screenshot ───────────────────────────────────── */}
        <section className="relative mx-auto w-full max-w-6xl px-6 pt-16 md:pt-20">
          <div
            className={`termy-hero-border ${reveal}`}
            style={{ animationDelay: '420ms' }}
          >
            <figure className="termy-hero-border-inner">
              <div className="flex items-center gap-1.5 border-b border-fd-border bg-fd-card px-4 py-2.5">
                <span className="size-3 rounded-full bg-fd-error/80" />
                <span className="size-3 rounded-full bg-fd-warning/80" />
                <span className="size-3 rounded-full bg-fd-success/80" />
                <span className="ml-3 font-mono text-[11px] text-fd-muted-foreground">
                  termy — ~/dev
                </span>
              </div>
              <img
                src={landingScreenshot}
                alt="Termy on macOS with Tokyo Night theme and appearance settings"
                width={3007}
                height={1894}
                loading="eager"
                decoding="async"
                className="block h-auto w-full"
              />
            </figure>
          </div>
        </section>

        {/* ── Features bento ───────────────────────────────── */}
        <section className="relative mx-auto w-full max-w-6xl px-6 pt-24 md:pt-32">
          <div className={reveal} style={{ animationDelay: '80ms' }}>
            <p className="font-mono text-xs tracking-widest text-fd-primary uppercase">
              $ termy --features
            </p>
            <h2 className="termy-display mt-4 max-w-2xl text-4xl md:text-5xl">
              Everything a shell deserves,{' '}
              <em className="termy-display-accent">nothing it doesn&apos;t.</em>
            </h2>
          </div>

          <div className="mt-12 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {features.map((feature, index) => (
              <article
                key={feature.id}
                className={`termy-card ${feature.span ? 'sm:col-span-2' : ''} ${reveal}`}
                style={{ animationDelay: `${140 + index * 70}ms` }}
              >
                <div className="flex items-baseline justify-between gap-4">
                  <span className="font-mono text-xs text-fd-muted-foreground/60">
                    {feature.id}
                  </span>
                  <span className="truncate font-mono text-[11px] text-fd-primary/80">
                    $ {feature.command}
                  </span>
                </div>
                <h3 className="termy-display mt-6 text-2xl text-fd-foreground md:text-3xl">
                  {feature.title}
                </h3>
                <p className="mt-3 text-sm leading-relaxed text-fd-muted-foreground">
                  {feature.description}
                </p>
              </article>
            ))}
          </div>
        </section>

        {/* ── Sponsors ─────────────────────────────────────── */}
        <section className="relative mx-auto w-full max-w-6xl px-6 pb-28 pt-24 md:pt-32">
          <div className={reveal}>
            <p className="font-mono text-xs tracking-widest text-fd-primary uppercase">
              $ cat SPONSORS
            </p>
            <h2 className="termy-display mt-4 text-4xl md:text-5xl">
              Backed by{' '}
              <em className="termy-display-accent">
                {sponsors.length}{' '}
                {sponsors.length === 1 ? 'supporter' : 'supporters'}.
              </em>
            </h2>
          </div>

          <div className="mt-10 grid gap-4 md:grid-cols-2">
            {sponsors.map((sponsor, index) => (
              <a
                key={sponsor.name}
                href={sponsor.url}
                target="_blank"
                rel="noreferrer"
                className={`termy-card group flex items-center gap-5 ${reveal}`}
                style={{ animationDelay: `${120 + index * 80}ms` }}
              >
                <span className="flex h-9 w-24 shrink-0 items-center">
                  <img
                    src={sponsor.logo.light}
                    alt={`${sponsor.name} logo`}
                    className={`h-8 w-auto dark:hidden ${sponsor.avatar ? 'rounded-full' : ''}`}
                  />
                  <img
                    src={sponsor.logo.dark}
                    alt={`${sponsor.name} logo`}
                    className={`hidden h-8 w-auto dark:block ${sponsor.avatar ? 'rounded-full' : ''}`}
                  />
                </span>
                <span className="min-w-0 text-sm leading-relaxed text-fd-muted-foreground transition-colors group-hover:text-fd-foreground">
                  {sponsor.description ?? sponsor.name}
                </span>
                <span
                  aria-hidden
                  className="ml-auto shrink-0 text-fd-muted-foreground transition-all group-hover:-translate-y-0.5 group-hover:translate-x-0.5 group-hover:text-fd-primary"
                >
                  ↗
                </span>
              </a>
            ))}
          </div>
        </section>

        {/* ── Final CTA ────────────────────────────────────── */}
        <section className="relative border-t border-fd-border">
          <div className="mx-auto w-full max-w-6xl px-6 py-20 text-center md:py-28">
            <p className="font-mono text-xs text-fd-muted-foreground">
              <span className="text-fd-success">❯</span> exec termy
            </p>
            <h2 className="termy-display mx-auto mt-5 max-w-2xl text-balance text-5xl md:text-6xl">
              Your shell is waiting.
            </h2>
            <div className="mt-9 flex flex-wrap items-center justify-center gap-4">
              <Link to="/download" className="termy-btn-primary">
                <span className="font-mono opacity-70">↓</span> Download Termy
              </Link>
              <a
                href="https://github.com/lassejlv/termy"
                target="_blank"
                rel="noreferrer"
                className="termy-btn-ghost"
              >
                star on GitHub ↗
              </a>
            </div>
          </div>
        </section>
      </main>
    </HomeLayout>
  );
}
