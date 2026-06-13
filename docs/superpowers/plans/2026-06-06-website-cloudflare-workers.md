# Website Cloudflare Workers Deployment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `website/` deploy to Cloudflare Workers (replacing the Vercel/Docker targets), deployed via Cloudflare Git integration.

**Architecture:** Keep the existing Vite + TanStack Start + Nitro 3 (beta) build pipeline and switch the Nitro preset to `cloudflare_module`. Nitro's `cloudflare.deployConfig: true` generates a complete `wrangler.json` in `.output/server/` at build time (correct `main` + static-assets paths) and a `.wrangler/deploy/config.json` redirect so plain `wrangler deploy`/`wrangler dev` find it. A minimal repo-level `website/wrangler.jsonc` supplies the worker name, compatibility date, and `nodejs_compat` flag; Nitro merges it into the generated config.

**Tech Stack:** bun, Vite 8, TanStack Start, Nitro `3.0.260429-beta`, wrangler, Cloudflare Workers (static assets + module worker).

**Spec:** `docs/superpowers/specs/2026-06-06-website-cloudflare-workers-design.md`

**Context for the implementer:**
- All commands run from `/Users/lassevestergaard/Documents/dev/termy/website` unless noted.
- The repo working tree has many unrelated modified files. Every `git add` in this plan lists explicit paths — never use `git add -A` or `git add .`.
- This is deployment configuration; there is no unit-testable logic. Each task's "test" is a build/runtime verification step with expected output. Do not invent unit tests for config files.
- Verified facts (from the installed Nitro beta at `node_modules/nitro/dist/docs/1.deploy/2.providers/5.cloudflare.md` and `node_modules/nitro/dist/_presets.mjs`): preset name is `cloudflare_module`; Nitro reads a user `wrangler.json`/`wrangler.jsonc`/`wrangler.toml` from the project root and merges it (user values win over defaults; Nitro force-overrides only `main` and `assets.directory`); `cloudflare.nodeCompat: true` appends `nodejs_compat` to `compatibility_flags`; the merged config is written to `.output/server/wrangler.json` plus a redirect at `.wrangler/deploy/config.json`.
- `process.env.NOTRA_API_KEY` (read in `src/lib/notra.ts:28`) works on Workers because `nodejs_compat` + `compatibility_date` ≥ 2025-04-01 auto-populates `process.env`. The code already handles a missing key, so local smoke tests pass without it.

---

### Task 1: Switch the Nitro preset to `cloudflare_module`

**Files:**
- Modify: `website/vite.config.ts`

- [ ] **Step 1: Replace the preset config**

Replace the entire contents of `website/vite.config.ts` with:

```ts
import react from '@vitejs/plugin-react';
import { tanstackStart } from '@tanstack/react-start/plugin/vite';
import { defineConfig } from 'vite';
import tailwindcss from '@tailwindcss/vite';
import mdx from 'fumadocs-mdx/vite';
import { nitro } from 'nitro/vite';

export default defineConfig({
  server: {
    port: 3000,
  },
  plugins: [
    mdx(),
    tailwindcss(),
    tanstackStart({
      prerender: {
        enabled: true,
      },
    }),
    react(),
    nitro({
      preset: 'cloudflare_module',
      compatibilityDate: '2026-06-06',
      cloudflare: {
        deployConfig: true,
        nodeCompat: true,
      },
      traceDeps: ['tslib*'],
    }),
  ],
  resolve: {
    tsconfigPaths: true,
    alias: {
      tslib: 'tslib/tslib.es6.js',
    },
  },
});
```

This removes the `process.env.VERCEL ? 'vercel' : 'bun'` branch entirely (spec item 1 + part of item 4).

- [ ] **Step 2: Build and verify the Cloudflare output layout**

Run: `bun run build`
Expected: build succeeds; output mentions the `cloudflare_module` preset (Nitro logs the preset name) and prerendered routes.

If the build fails with an unknown-option error on `compatibilityDate` or `cloudflare` (vite-plugin option passthrough differs in this beta), move those two keys into a new `website/nitro.config.ts`:

```ts
import { defineConfig } from 'nitro';

export default defineConfig({
  compatibilityDate: '2026-06-06',
  cloudflare: {
    deployConfig: true,
    nodeCompat: true,
  },
});
```

and keep `nitro({ preset: 'cloudflare_module', traceDeps: ['tslib*'] })` in `vite.config.ts`, then rerun `bun run build`.

- [ ] **Step 3: Verify the generated artifacts**

Run: `ls .output/server/index.mjs .output/server/wrangler.json .wrangler/deploy/config.json && cat .output/server/wrangler.json`
Expected: all three files exist. The JSON has `"main"` pointing at `index.mjs`, an `"assets"` entry whose `"directory"` resolves to the public output, and `"compatibility_flags"` containing `"nodejs_compat"`. (The `name` will be auto-generated for now — Task 2 pins it.)

- [ ] **Step 4: Commit**

```bash
git add vite.config.ts
git commit -m "website: switch Nitro preset to cloudflare_module"
```

(If Step 2's fallback was needed, also `git add nitro.config.ts`.)

---

### Task 2: Add `wrangler.jsonc`, the wrangler dev-dependency, and scripts

**Files:**
- Create: `website/wrangler.jsonc`
- Modify: `website/package.json`
- Modify: `website/.gitignore`

- [ ] **Step 1: Create `website/wrangler.jsonc`**

```jsonc
{
  "$schema": "node_modules/wrangler/config-schema.json",
  "name": "termy-website",
  "compatibility_date": "2026-06-06",
  "compatibility_flags": ["nodejs_compat"]
}
```

No `main`/`assets` here — Nitro injects those at build time (spec's "adjust to actual output" error-handling case is thereby impossible).

- [ ] **Step 2: Install wrangler and update scripts**

Run: `bun add -d wrangler`
Expected: exit 0; `wrangler` appears in `devDependencies` in `package.json`, `bun.lock` updated.

Then in `website/package.json` `scripts`: delete the `start` entry (`"start": "bun run ./.output/server/index.mjs"`) and add a `deploy` entry, so the scripts block reads:

```json
"scripts": {
  "dev": "vite dev",
  "build": "bun --bun vite build",
  "deploy": "wrangler deploy",
  "preview": "vite preview",
  "types:check": "fumadocs-mdx && tsc --noEmit",
  "postinstall": "fumadocs-mdx",
  "lint": "oxlint"
},
```

- [ ] **Step 3: Ignore wrangler state**

In `website/.gitignore`, add a line after the `.output` entry:

```
.wrangler
```

- [ ] **Step 4: Rebuild and verify the merge picked up the worker name**

Run: `bun run build && cat .output/server/wrangler.json`
Expected: generated config now contains `"name": "termy-website"` and `"compatibility_date": "2026-06-06"`; `compatibility_flags` contains `"nodejs_compat"` exactly once.

- [ ] **Step 5: Commit**

```bash
git add wrangler.jsonc package.json bun.lock .gitignore
git commit -m "website: add wrangler config, dev-dependency, and deploy script"
```

---

### Task 3: Remove the Docker deployment path

**Files:**
- Delete: `website/Dockerfile`
- Delete: `website/.dockerignore`

- [ ] **Step 1: Delete the files**

```bash
git rm Dockerfile .dockerignore
```

(Verified: nothing in `.github/workflows` or `scripts/` references the website Dockerfile.)

- [ ] **Step 2: Commit**

```bash
git commit -m "website: remove Docker deployment path"
```

---

### Task 4: Smoke-test in the real Workers runtime

**Files:** none (verification only)

- [ ] **Step 1: Start wrangler dev against the built output**

Run (background): `bunx wrangler dev`
Expected: "Ready on http://localhost:8787" (wrangler resolves the generated config via `.wrangler/deploy/config.json`). If it instead errors with "config not found", run it explicitly: `bunx wrangler dev -c .output/server/wrangler.json`.

- [ ] **Step 2: Smoke-test the routes**

```bash
for p in / /download /docs /llms.txt "/api/search?query=terminal"; do
  printf '%s -> ' "$p"
  curl -s -o /dev/null -w '%{http_code}\n' "http://localhost:8787$p"
done
```

Expected: every line ends in `200` (a `3xx` for `/docs` redirecting to a docs landing page is also acceptable — follow it with `curl -L` and expect `200`). `/download` proves server functions run on `workerd`; `/api/search` proves the search handler runs; `/llms.txt` proves the custom text routes run.

- [ ] **Step 3: Stop wrangler dev**

Kill the background process.

- [ ] **Step 4: Static checks still green**

Run: `bun run lint && bun run types:check`
Expected: both exit 0.

---

### Task 5: Update README with the Cloudflare deployment flow

**Files:**
- Modify: `website/README.md`

- [ ] **Step 1: Replace README contents**

````markdown
# website

Run development server:

```bash
bun run dev
```

## Deployment (Cloudflare Workers)

The site builds with TanStack Start + Nitro using the `cloudflare_module` preset. `bun run build` emits `.output/` and a generated `.output/server/wrangler.json` — Nitro merges `wrangler.jsonc` (this directory) into it and writes a `.wrangler/deploy/config.json` redirect so plain wrangler commands find it.

Preview the built site in the real Workers runtime:

```bash
bun run build
bunx wrangler dev
```

Manual deploy (requires `wrangler login`):

```bash
bun run build
bun run deploy
```

Production deploys run through Cloudflare Git integration (Workers Builds):

- Root directory: `website`
- Build command: `bun run build`
- Deploy command: `npx wrangler deploy`
- Pushes to `main` deploy production; other branches get preview URLs.

Secrets: set `NOTRA_API_KEY` with `bunx wrangler secret put NOTRA_API_KEY` (or dashboard → Worker → Settings → Variables). Local dev reads `.env.local`.
````

(The nested triple-backtick fences above are part of the README content; keep them.)

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "website: document Cloudflare Workers deployment"
```

---

### Task 6: Cloudflare dashboard setup (manual — hand to the user)

No repo changes. Present these steps to the user; they require dashboard access:

1. Cloudflare dashboard → Compute (Workers) → Create → **Import a repository**, pick the termy repo.
2. Project settings: worker name `termy-website`, root directory `website`, build command `bun run build`, deploy command `npx wrangler deploy`, production branch `main`.
3. After the first deploy: Worker → Settings → Variables and Secrets → add secret `NOTRA_API_KEY`.
4. Optional: attach the custom domain under Worker → Settings → Domains & Routes.
