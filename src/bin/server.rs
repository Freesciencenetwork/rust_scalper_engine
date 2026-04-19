#![allow(non_snake_case)] // Same package name as library crate (`binance_BTC`).
#![allow(clippy::multiple_crate_versions)] // Transitive duplicates; see `lib.rs`.

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use binance_BTC::{
    DecisionMachine, MachineCapabilities, MachineRequest, MachineResponse, StrategyConfig,
};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8080);

    let machine = Arc::new(machine_from_env());

    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/capabilities", get(capabilities))
        .route("/v1/evaluate", post(evaluate))
        .layer(TraceLayer::new_for_http())
        .with_state(machine);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!(%port, "listening");
    axum::serve(listener, app).await?;
    Ok(())
}

fn machine_from_env() -> DecisionMachine {
    let mut config = StrategyConfig::default();
    if let Ok(raw) = std::env::var("VOL_BASELINE_LOOKBACK_BARS")
        && let Ok(n) = raw.parse::<usize>()
    {
        let min = config.vwma_lookback.max(config.runway_lookback).max(2);
        if n >= min {
            config.vol_baseline_lookback_bars = n;
            tracing::info!(
                vol_baseline_lookback_bars = n,
                "VOL_BASELINE_LOOKBACK_BARS override applied"
            );
        } else {
            tracing::warn!(
                value = n,
                min,
                "VOL_BASELINE_LOOKBACK_BARS ignored (must be >= vwma_lookback and runway_lookback)"
            );
        }
    }
    DecisionMachine::new(config)
}

async fn health() -> &'static str {
    "ok"
}

async fn capabilities(State(machine): State<Arc<DecisionMachine>>) -> Json<MachineCapabilities> {
    Json(machine.capabilities())
}

async fn evaluate(
    State(machine): State<Arc<DecisionMachine>>,
    Json(request): Json<MachineRequest>,
) -> Result<Json<MachineResponse>, ApiError> {
    machine.evaluate(request).map(Json).map_err(ApiError)
}

struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let body = self.0.to_string();
        tracing::warn!(error = %body, "evaluate failed");
        (StatusCode::BAD_REQUEST, body).into_response()
    }
}
