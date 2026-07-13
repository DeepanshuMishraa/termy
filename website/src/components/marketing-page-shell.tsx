import { Link } from '@tanstack/react-router';
import { Moon, Sun } from 'lucide-react';
import { type ReactNode, useEffect, useState } from 'react';

export const marketingFontLinks = [
  { rel: 'preconnect', href: 'https://fonts.googleapis.com' },
  {
    rel: 'preconnect',
    href: 'https://fonts.gstatic.com',
    crossOrigin: 'anonymous' as const,
  },
  {
    rel: 'stylesheet',
    href: 'https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500;700&display=swap',
  },
];

export const marketingMono =
  "'JetBrains Mono', ui-monospace, SFMono-Regular, Menlo, monospace";

function setTheme(theme: 'light' | 'dark') {
  document.documentElement.classList.toggle('dark', theme === 'dark');
  try {
    localStorage.setItem('theme', theme);
  } catch {
    // Theme still applies when storage is unavailable.
  }
}

export function ThemeToggle() {
  const [theme, setCurrentTheme] = useState<'light' | 'dark'>('dark');

  useEffect(() => {
    const root = document.documentElement;
    const syncTheme = () =>
      setCurrentTheme(root.classList.contains('dark') ? 'dark' : 'light');
    const observer = new MutationObserver(syncTheme);

    syncTheme();
    observer.observe(root, { attributes: true, attributeFilter: ['class'] });
    return () => observer.disconnect();
  }, []);

  const selectTheme = (nextTheme: 'light' | 'dark') => {
    setCurrentTheme(nextTheme);
    setTheme(nextTheme);
  };

  return (
    <div
      className="relative flex items-center rounded-full border border-white/[0.08] bg-[#14141c]/70 p-1.5 backdrop-blur-md"
      role="group"
      aria-label="Color theme"
    >
      <span
        aria-hidden
        className={`pointer-events-none absolute left-1.5 size-8 rounded-full bg-[#24283b] shadow-[inset_0_0_0_1px_rgba(255,255,255,0.08),0_3px_10px_rgba(0,0,0,0.24)] transition-transform duration-300 motion-reduce:transition-none ${
          theme === 'dark' ? 'translate-x-8' : 'translate-x-0'
        }`}
        style={{ transitionTimingFunction: 'cubic-bezier(0.23, 1, 0.32, 1)' }}
      />
      <button
        type="button"
        aria-label="Switch to light theme"
        aria-pressed={theme === 'light'}
        onClick={() => selectTheme('light')}
        className={`relative z-10 flex size-8 items-center justify-center rounded-full transition-colors duration-200 active:scale-[0.96] ${
          theme === 'light' ? 'text-[#f4c76b]' : 'text-[#565f89] hover:text-[#c0caf5]'
        }`}
      >
        <Sun
          className={`size-4 transition-[transform,opacity] duration-300 motion-reduce:transition-none ${
            theme === 'light' ? 'rotate-0 opacity-100' : '-rotate-45 opacity-60'
          }`}
        />
      </button>
      <button
        type="button"
        aria-label="Switch to dark theme"
        aria-pressed={theme === 'dark'}
        onClick={() => selectTheme('dark')}
        className={`relative z-10 flex size-8 items-center justify-center rounded-full transition-colors duration-200 active:scale-[0.96] ${
          theme === 'dark' ? 'text-[#c0caf5]' : 'text-[#565f89] hover:text-[#c0caf5]'
        }`}
      >
        <Moon
          className={`size-4 transition-[transform,opacity] duration-300 motion-reduce:transition-none ${
            theme === 'dark' ? 'rotate-0 opacity-100' : 'rotate-45 opacity-60'
          }`}
        />
      </button>
    </div>
  );
}

function MarketingNav() {
  const pill =
    'flex items-center rounded-full border border-white/[0.08] bg-[#14141c]/70 backdrop-blur-md';

  return (
    <header className="relative z-20 flex w-full flex-wrap items-center justify-center gap-3 px-6 pt-7 sm:gap-4">
      <Link
        to="/"
        className="flex items-center gap-2.5 rounded-full border border-[#7aa2f7]/40 bg-[#16161e]/80 py-2.5 pr-5 pl-4 shadow-[inset_0_0_18px_rgba(122,162,247,0.18),0_0_24px_rgba(122,162,247,0.12)] backdrop-blur-md"
      >
        <img src="/termy-icon.svg" alt="" className="h-5 w-5" />
        <span className="text-[15px] font-medium tracking-tight">termy</span>
      </Link>

      <nav className={`${pill} px-2 py-1 text-sm text-[#c0caf5]`}>
        <Link to="/download" className="rounded-full px-4 py-2 hover:text-white">
          Download
        </Link>
        <Link
          to="/docs/$"
          params={{ _splat: '' }}
          className="rounded-full px-4 py-2 hover:text-white"
        >
          Docs
        </Link>
        <Link to="/releases" className="rounded-full px-4 py-2 hover:text-white">
          Releases
        </Link>
        <a
          href="https://github.com/lassejlv/termy"
          target="_blank"
          rel="noreferrer"
          className="rounded-full px-4 py-2 hover:text-white"
        >
          GitHub
        </a>
      </nav>

      <ThemeToggle />
    </header>
  );
}

function PageStars() {
  return (
    <div aria-hidden className="pointer-events-none absolute inset-0 overflow-hidden">
      {Array.from({ length: 72 }, (_, index) => {
        const x = (index * 47 + 13) % 100;
        const y = (index * 71 + 7) % 100;
        const bright = index % 11 === 0;
        return (
          <span
            key={index}
            className="absolute rounded-full bg-[#9fc0ff] motion-safe:animate-[marketing-star_7s_ease-in-out_infinite]"
            style={{
              left: `${x}%`,
              top: `${y}%`,
              width: bright ? 2 : 1,
              height: bright ? 2 : 1,
              opacity: bright ? 0.58 : 0.22,
              animationDelay: `${-(index % 14) / 2}s`,
            }}
          />
        );
      })}
      <style>{`
        @keyframes marketing-star {
          0%, 100% { transform: scale(1); opacity: 0.22; }
          50% { transform: scale(0.65); opacity: 0.5; }
        }
      `}</style>
    </div>
  );
}

export function MarketingPageShell({ children }: { children: ReactNode }) {
  return (
    <div
      className="marketing-theme relative min-h-screen overflow-hidden bg-[#0d0f17] text-[#c0caf5]"
      style={{
        background:
          'radial-gradient(900px 440px at 68% 3%, rgba(56,79,148,0.25), transparent 64%), radial-gradient(700px 480px at 12% 54%, rgba(40,56,110,0.15), transparent 65%), #0d0f17',
      }}
    >
      <PageStars />
      <MarketingNav />
      <div className="relative z-10">{children}</div>
    </div>
  );
}

export const marketingLinkClass =
  'text-[#c0caf5] underline decoration-white/20 underline-offset-4 transition-colors hover:text-white hover:decoration-[#7aa2f7]';

export const marketingPanelClass =
  'overflow-hidden rounded-2xl border border-white/[0.08] bg-[#16161e]/82 shadow-[0_24px_80px_rgba(0,0,0,0.35)] backdrop-blur-sm';
