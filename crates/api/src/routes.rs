use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use better_auth::adapters::SqlxAdapter;
use better_auth::handlers::CurrentSession;
use better_auth::AuthUser as _;

use crate::db::QueryPool;

pub(crate) async fn health(State(db): State<Arc<QueryPool>>) -> impl IntoResponse {
    let db_ok = neon_serverless_sqlx::sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(db.pg())
        .await
        .is_ok();
    let status = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(serde_json::json!({
            "status": if db_ok { "ok" } else { "degraded" },
            "database": db_ok,
        })),
    )
}

pub(crate) async fn me(session: CurrentSession<SqlxAdapter>) -> impl IntoResponse {
    Json(serde_json::json!({
        "id": session.user.id(),
        "email": session.user.email(),
        "name": session.user.name(),
    }))
}
