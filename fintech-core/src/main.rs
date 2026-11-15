mod payments;
mod services;

use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use serde::Serialize;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{env, net::SocketAddr};
use tracing_subscriber::EnvFilter;

use services::daraja::DarajaClient;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) db: PgPool,
    pub(crate) daraja: Option<DarajaClient>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    database: &'static str,
}

async fn health(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, (StatusCode, String)> {
    sqlx::query!("SELECT COUNT(*) AS health_check FROM chamas")
        .fetch_one(&state.db)
        .await
        .map_err(|error| {
            tracing::error!(%error, "database health check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "database unavailable".to_owned(),
            )
        })?;

    Ok(Json(HealthResponse {
        status: "ok",
        database: "connected",
    }))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let env_filter = match env::var("RUST_LOG") {
        Ok(filter) => EnvFilter::try_new(filter)?,
        Err(env::VarError::NotPresent) => EnvFilter::new("info"),
        Err(error) => return Err(error.into()),
    };

    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let database_url = env::var("DATABASE_URL")?;
    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;
    let daraja = DarajaClient::from_env()?;
    let app = Router::new()
        .route("/health", get(health))
        .route(
            "/api/v1/payments/stk-push",
            axum::routing::post(payments::initiate_stk_push),
        )
        .route(
            "/api/v1/mpesa/callback",
            axum::routing::post(payments::mpesa_callback),
        )
        .with_state(AppState { db, daraja });

    let address = SocketAddr::from(([0, 0, 0, 0], 8000));
    let listener = tokio::net::TcpListener::bind(address).await?;
    tracing::info!(%address, "fintech-core listening");
    axum::serve(listener, app).await?;
    Ok(())
}
