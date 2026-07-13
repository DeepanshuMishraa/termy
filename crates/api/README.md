# termy_api

Cloud API backend for Termy cloud agents (`termy-api` binary). Serves account
auth via [better-auth](https://github.com/better-auth-rs/better-auth-rs)
(email/password + sessions) mounted under `/auth`, with application routes
under `/api`. Backed by Postgres via sqlx; migrations live in `migrations/`
and run automatically on startup.

Application queries use a separate query pool (`src/db.rs`) built on
[neon-serverless-sqlx](https://github.com/lassejlv/neon-serverless-sqlx)
(sqlx 0.9): URLs on `*.neon.tech` are tunneled over Neon's WebSocket proxy,
anything else gets a plain direct pool (no TLS — local/dev only). Auth traffic
and migrations connect directly via sqlx 0.8, independent of the query pool.

## Owner

Owns the hosted Termy cloud API surface: HTTP routing, auth configuration,
and the auth database schema. Must not contain terminal runtime or desktop
UI behavior — embed `termy_core` only if a future endpoint genuinely needs
terminal semantics.

## Environment

- `DATABASE_URL` (required): Postgres connection string.
- `TERMY_API_SECRET` (required): auth signing secret, 32+ characters.
- `TERMY_API_BASE_URL` (optional, default `http://localhost:8080`).
- `PORT` (optional, default `8080`).

Run locally:

```sh
DATABASE_URL=postgres://localhost/termy TERMY_API_SECRET=<32+ chars> cargo run -p termy_api
```

## Validation

```sh
cargo check -p termy_api
```

```sh
cargo test -p termy_api
```

## Forbidden Dependencies

- `gpui` — this is a headless server crate; no UI framework.
- `termy_terminal_ui`, `termy` (desktop app) — no desktop presentation code.
