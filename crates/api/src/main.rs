//! Termy cloud API server.
//!
//! Serves the account/auth backend for Termy cloud agents (TRM-1/TRM-2).
//! Auth endpoints are provided by `better-auth` mounted under `/auth`;
//! application routes live under `/api`.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context as _;
use axum::routing::get;
use axum::Router;
use better_auth::adapters::SqlxAdapter;
use better_auth::handlers::AxumIntegration as _;
use better_auth::plugins::{
    AccountManagementPlugin, EmailPasswordPlugin, PasswordManagementPlugin, SessionManagementPlugin,
};
use better_auth::{AuthConfig, BetterAuth};
use sqlx::postgres::PgPoolOptions;

mod db;
mod routes;

use db::QueryPool;

pub(crate) type Auth = BetterAuth<SqlxAdapter>;

struct ApiConfig {
    database_url: String,
    secret: String,
    base_url: String,
    listen_addr: SocketAddr,
}

impl ApiConfig {
    fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
        let secret = std::env::var("TERMY_API_SECRET")
            .context("TERMY_API_SECRET must be set (32+ characters)")?;
        anyhow::ensure!(
            secret.len() >= 32,
            "TERMY_API_SECRET must be at least 32 characters"
        );
        let base_url = std::env::var("TERMY_API_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:8080".to_string());
        let port: u16 = std::env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .context("PORT must be a valid u16")?;
        Ok(Self {
            database_url,
            secret,
            base_url,
            listen_addr: SocketAddr::from(([0, 0, 0, 0], port)),
        })
    }
}

async fn build_auth(config: &ApiConfig) -> anyhow::Result<Arc<Auth>> {
    let adapter = SqlxAdapter::new(&config.database_url)
        .await
        .context("failed to connect better-auth to postgres")?;
    let auth = BetterAuth::<SqlxAdapter>::new(
        AuthConfig::new(config.secret.clone()).base_url(config.base_url.clone()),
    )
    .database(adapter)
    .plugin(EmailPasswordPlugin::new().enable_signup(true))
    .plugin(SessionManagementPlugin::new())
    .plugin(PasswordManagementPlugin::new())
    .plugin(AccountManagementPlugin::new())
    .build()
    .await
    .context("failed to build better-auth")?;
    Ok(Arc::new(auth))
}

async fn run_migrations(database_url: &str) -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(database_url)
        .await
        .context("failed to connect to postgres for migrations")?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to run database migrations")?;
    pool.close().await;
    Ok(())
}

fn build_router(auth: Arc<Auth>, db: Arc<QueryPool>) -> Router {
    // better-auth's extractors require the router state to be exactly
    // `Arc<BetterAuth<_>>`, so auth-backed and db-backed routes are built
    // as separate routers and merged.
    let auth_routes = Router::new()
        .route("/api/me", get(routes::me))
        .nest("/auth", auth.clone().axum_router())
        .with_state(auth);
    let db_routes = Router::new()
        .route("/health", get(routes::health))
        .with_state(db);
    auth_routes.merge(db_routes)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = ApiConfig::from_env()?;
    run_migrations(&config.database_url).await?;
    let auth = build_auth(&config).await?;
    let db = Arc::new(QueryPool::connect(&config.database_url).await?);
    let app = build_router(auth, db);

    let listener = tokio::net::TcpListener::bind(config.listen_addr)
        .await
        .with_context(|| format!("failed to bind {}", config.listen_addr))?;
    tracing::info!("termy-api listening on {}", config.listen_addr);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install ctrl-c handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
