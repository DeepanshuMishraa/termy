# Website: deploy to Cloudflare Workers

**Date:** 2026-06-06
**Scope:** `website/` only
**Decision:** Cloudflare Workers replaces all existing deployment targets (Vercel preset, Docker/bun). Deploys happen via Cloudflare Git integration (Workers Builds).

## Context

The website is a Vite + TanStack Start + fumadocs app built with the `nitro/vite` plugin (Nitro 3.0 beta, `3.0.260429-beta`). Today the Nitro preset switches between `vercel` (when `VERCEL` is set) and `bun` (consumed by `website/Dockerfile`). Prerendering is enabled.

Server-side surface that must keep working on Workers:

- Server functions in `src/routes/download.tsx`, `src/routes/releases/index.tsx`, `src/routes/releases/$slug.tsx` (live GitHub release data)
- `src/routes/api/search.ts` (docs search)
- `src/routes/llms[.]txt.ts` and `src/routes/llms-full[.]txt.ts`
- `src/routes/docs/$.tsx` (SSR docs)
- One server env var: `NOTRA_API_KEY` read via `process.env` in `src/lib/notra.ts`

No Node `fs`/`path` usage in app code — the server surface is fetch-based and Workers-compatible.

## Design

### 1. Nitro preset (`website/vite.config.ts`)

Replace `preset: process.env.VERCEL ? 'vercel' : 'bun'` with the `cloudflare_module` preset (verified present in the installed Nitro 3 beta). Keep `traceDeps`, prerender, and all other plugin config unchanged.

### 2. Wrangler config (`website/wrangler.jsonc`, new)

- `name`: `termy-website`
- `main`: `./.output/server/index.mjs`
- `assets`: `{ "directory": "./.output/public", "binding": "ASSETS" }`
- `compatibility_date`: `2026-06-06` (≥ 2025-04-01 so `nodejs_compat` auto-populates `process.env`)
- `compatibility_flags`: `["nodejs_compat"]`

Prerendered pages land in `.output/public` and are served as static assets; everything else hits the worker.

### 3. Secrets

`NOTRA_API_KEY` becomes a Workers secret (set in the Cloudflare dashboard or `wrangler secret put`). Never committed. `.env.local` continues to serve local dev.

### 4. Removals

- `website/Dockerfile` and `website/.dockerignore`
- The `start` script in `website/package.json` (Workers has no long-running server entry)
- The `VERCEL` preset branch (covered by item 1)

### 5. Cloudflare dashboard setup (manual, documented in README)

- Connect the GitHub repo via Workers Builds
- Root directory: `website`
- Build command: `bun run build`
- Deploy command: `npx wrangler deploy`
- Pushes to `main` deploy production; other branches get preview URLs

### 6. Verification

- `bun run build` succeeds locally with the new preset
- `bunx wrangler dev` serves the built output in the real `workerd` runtime; smoke-test `/`, `/download`, `/docs/...`, `/api/search`, `/llms.txt`
- `bun run lint` and `bun run types:check` stay green

### 7. Docs

Update `website/README.md`: replace any Docker/Vercel deployment notes with the Cloudflare Workers flow (build, local preview via `wrangler dev`, dashboard Git-integration steps, secret setup).

## Error handling

- If the Nitro beta's `cloudflare_module` output layout differs from the expected `.output/server/index.mjs` + `.output/public`, adjust `wrangler.jsonc` paths to match the actual build output rather than patching Nitro.
- If `process.env.NOTRA_API_KEY` is absent at runtime, `src/lib/notra.ts` already handles the missing-key path; no new fallback needed.

## Out of scope

- Custom domain wiring (done in the Cloudflare dashboard after first deploy)
- Any change to site content, routing, or search behavior
- CI workflows in GitHub Actions (Git integration replaces this)
