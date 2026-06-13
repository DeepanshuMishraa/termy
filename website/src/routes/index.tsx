import { createFileRoute, Link } from '@tanstack/react-router';
import { HomeLayout } from 'fumadocs-ui/layouts/home';
import { baseOptions } from '@/lib/layout.shared';
import { sponsors } from '@/lib/sponsors';

const landingScreenshot = '/termy-landing.png';

export const Route = createFileRoute('/')({
  component: Home,
});

const features = [
  {
    title: 'Fast',
    description: 'GPU-accelerated rendering with instant startup.',
  },
  {
    title: 'Configurable',
    description: 'One plain-text config file. Full control.',
  },
  {
    title: 'Native',
    description: 'Runs natively on macOS, Windows, and Linux.',
  },
  {
    title: 'Themable',
    description: 'Built-in themes or create your own.',
  },
  {
    title: 'Powerful',
    description: 'Splits, tabs, and multiplexing built-in.',
  },
];

const reveal =
  'motion-safe:animate-[termy-fade-up_0.7s_cubic-bezier(0.22,1,0.36,1)_both]';

function Home() {
  return (
    <HomeLayout {...baseOptions()}>
      <main className="flex flex-1 flex-col">
        <section className="mx-auto w-full max-w-6xl px-6 pt-28 pb-16 md:pt-32 lg:pb-20">
          <div className="grid items-start gap-10 lg:grid-cols-[minmax(0,20rem)_minmax(0,1fr)] lg:gap-10 xl:grid-cols-[minmax(0,22rem)_minmax(0,1fr)] xl:gap-14">
            <div className="lg:pt-2">
              <p
                className={`font-mono text-xs text-fd-muted-foreground ${reveal}`}
              >
                <span className="select-none text-fd-primary">$ </span>
                termy
              </p>
              <h1
                className={`mt-6 text-balance font-medium text-5xl tracking-tight md:text-6xl ${reveal}`}
                style={{ animationDelay: '80ms' }}
              >
                A fast, native terminal.
              </h1>
              <p
                className={`mt-6 text-balance text-fd-muted-foreground md:text-lg ${reveal}`}
                style={{ animationDelay: '160ms' }}
              >
                GPU-accelerated, configurable, and built for daily terminal work
                on macOS, Windows, and Linux.
              </p>
              <div
                className={`mt-8 flex flex-wrap items-center gap-x-6 gap-y-3 ${reveal}`}
                style={{ animationDelay: '240ms' }}
              >
                <Link
                  to="/download"
                  className="rounded-md bg-fd-primary px-4 py-2 text-sm font-medium text-fd-primary-foreground transition-opacity hover:opacity-90"
                >
                  Download
                </Link>
                <Link
                  to="/docs/$"
                  params={{ _splat: '' }}
                  className="font-mono text-xs text-fd-muted-foreground transition-colors hover:text-fd-foreground"
                >
                  read the docs →
                </Link>
                <a
                  href="https://github.com/lassejlv/termy"
                  target="_blank"
                  rel="noreferrer"
                  className="font-mono text-xs text-fd-muted-foreground transition-colors hover:text-fd-foreground"
                >
                  GitHub ↗
                </a>
              </div>
            </div>

            <div
              className={`termy-hero-border ${reveal}`}
              style={{ animationDelay: '200ms' }}
            >
              <figure className="termy-hero-border-inner">
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
          </div>
        </section>

        <section className="mx-auto w-full max-w-3xl px-6 pb-20">
          <div
            className={`divide-y divide-fd-border border-t border-fd-border ${reveal}`}
            style={{ animationDelay: '400ms' }}
          >
            {features.map((feature, index) => (
              <div
                key={feature.title}
                className="grid gap-x-6 gap-y-1 py-5 sm:grid-cols-[2.5rem_9rem_1fr] sm:items-baseline"
              >
                <span className="font-mono text-xs text-fd-muted-foreground/60">
                  {String(index + 1).padStart(2, '0')}
                </span>
                <h3 className="font-medium text-fd-foreground">
                  {feature.title}
                </h3>
                <p className="text-sm leading-relaxed text-fd-muted-foreground">
                  {feature.description}
                </p>
              </div>
            ))}
          </div>
        </section>

        <section className="mx-auto w-full max-w-3xl px-6 pb-24">
          <div
            className={`border-t border-fd-border pt-10 ${reveal}`}
            style={{ animationDelay: '480ms' }}
          >
            <div className="grid gap-6 md:grid-cols-[10rem_1fr]">
              <div>
                <h2 className="font-medium text-fd-foreground tracking-tight">
                  Sponsors
                </h2>
                <p className="mt-1 font-mono text-xs text-fd-muted-foreground">
                  {sponsors.length}{' '}
                  {sponsors.length === 1 ? 'supporter' : 'supporters'}
                </p>
              </div>

              <div className="divide-y divide-fd-border">
                {sponsors.map((sponsor) => (
                  <a
                    key={sponsor.name}
                    href={sponsor.url}
                    target="_blank"
                    rel="noreferrer"
                    className="group -mx-3 flex items-center gap-4 rounded-md px-3 py-4 transition-colors hover:bg-fd-accent"
                  >
                    <span className="flex h-8 w-24 shrink-0 items-center">
                      <img
                        src={sponsor.logo.light}
                        alt={`${sponsor.name} logo`}
                        className={`h-7 w-auto dark:hidden ${sponsor.avatar ? 'rounded-full' : ''}`}
                      />
                      <img
                        src={sponsor.logo.dark}
                        alt={`${sponsor.name} logo`}
                        className={`hidden h-7 w-auto dark:block ${sponsor.avatar ? 'rounded-full' : ''}`}
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
            </div>
          </div>
        </section>
      </main>
    </HomeLayout>
  );
}
