//! Application query pool.
//!
//! Queries run on sqlx 0.9 (re-exported by `neon-serverless-sqlx`), separate
//! from the sqlx 0.8 pool better-auth builds internally and the one used for
//! migrations. Neon URLs are tunneled over Neon's WebSocket proxy; other URLs
//! get a plain direct pool (no TLS — local/dev only for now).

use anyhow::Context as _;
use neon_serverless_sqlx::sqlx::postgres::PgPoolOptions;
use neon_serverless_sqlx::sqlx::PgPool;
use neon_serverless_sqlx::NeonPool;

pub(crate) enum QueryPool {
    Neon(NeonPool),
    Direct(PgPool),
}

impl QueryPool {
    pub(crate) async fn connect(database_url: &str) -> anyhow::Result<Self> {
        if is_neon_url(database_url) {
            let pool = NeonPool::connect(database_url)
                .await
                .context("failed to connect Neon query pool")?;
            tracing::info!("query pool: neon websocket transport");
            Ok(Self::Neon(pool))
        } else {
            let pool = PgPoolOptions::new()
                .max_connections(5)
                .connect(database_url)
                .await
                .context("failed to connect direct query pool")?;
            tracing::info!("query pool: direct postgres");
            Ok(Self::Direct(pool))
        }
    }

    pub(crate) fn pg(&self) -> &PgPool {
        match self {
            Self::Neon(pool) => pool.as_pg_pool(),
            Self::Direct(pool) => pool,
        }
    }
}

fn is_neon_url(database_url: &str) -> bool {
    database_url.contains(".neon.tech")
}

#[cfg(test)]
mod tests {
    use super::is_neon_url;

    #[test]
    fn neon_urls_are_detected() {
        assert!(is_neon_url(
            "postgresql://user:pass@ep-cool-tree-123.eu-central-1.aws.neon.tech/app"
        ));
        assert!(!is_neon_url("postgres://termy@127.0.0.1:5432/termy_api"));
    }
}
