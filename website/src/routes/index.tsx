import { createFileRoute, Link } from '@tanstack/react-router';
import { HomeLayout } from 'fumadocs-ui/layouts/home';
import { baseOptions } from '@/lib/layout.shared';
import { sponsors } from '@/lib/sponsors';

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
    description: 'One TOML file. Full control over everything.',
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
    title: 'Lightweight',
    description: '18MB memory footprint. No Electron bloat.',
  },
  {
    title: 'Powerful',
    description: 'Splits, tabs, and multiplexing built-in.',
  },
];

const reveal =
  'motion-safe:animate-[termy-fade-up_0.7s_cubic-bezier(0.22,1,0.36,1)_both]';

function Caret({ className = 'bg-fd-primary' }: { className?: string }) {
  return (
    <span
      aria-hidden
      className={`ml-1 inline-block h-[1em] w-[0.55ch] translate-y-[0.12em] ${className} motion-safe:animate-[termy-caret-blink_1.1s_steps(1)_infinite]`}
    />
  );
}

function TerminalWindow() {
  return (
    <div className="overflow-hidden rounded-lg border border-fd-border bg-termy-bg shadow-[0_24px_48px_-24px_rgba(11,16,32,0.4)]">
      <div className="flex items-center gap-1.5 border-b border-termy-bright-black/40 px-4 py-3">
        <span className="size-2.5 rounded-full bg-termy-bright-black" />
        <span className="size-2.5 rounded-full bg-termy-bright-black" />
        <span className="size-2.5 rounded-full bg-termy-bright-black" />
        <span className="ml-3 font-mono text-xs text-termy-magenta">
          termy
        </span>
      </div>
      <div className="px-4 py-5 font-mono text-sm leading-7">
        <p>
          <span className="select-none text-termy-green">$ </span>
          <span className="text-termy-fg">which terminal</span>
        </p>
        <p className="text-termy-cyan">/usr/local/bin/termy</p>
        <p>
          <span className="select-none text-termy-green">$ </span>
          <Caret className="bg-termy-green" />
        </p>
      </div>
    </div>
  );
}

function Home() {
  return (
    <HomeLayout {...baseOptions()}>
      <main className="flex flex-1 flex-col">
        <section className="mx-auto w-full max-w-3xl px-6 pt-28 md:pt-40">
          <p className={`font-mono text-xs text-fd-muted-foreground ${reveal}`}>
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
            className={`mt-6 max-w-xl text-balance text-fd-muted-foreground md:text-lg ${reveal}`}
            style={{ animationDelay: '160ms' }}
          >
            Termy is a modern terminal emulator that gets out of your way.
            GPU-accelerated, configurable, and lightweight.
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
        </section>

        <section className="mx-auto w-full max-w-3xl px-6 pt-16 pb-20">
          <div className={reveal} style={{ animationDelay: '320ms' }}>
            <TerminalWindow />
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
