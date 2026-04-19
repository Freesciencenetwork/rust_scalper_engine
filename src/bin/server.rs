//! `server` binary — **horizontally scalable** HTTP front for `DecisionMachine`.
//!
//! - **Stateless requests:** every input for a compute call is in the `POST` JSON body. Shared
//!   [`Arc`] state is **read-only** after startup (no cross-request mutable server state).
//! - **Scale out:** run **N** identical processes behind a load balancer; capacity grows with
//!   **N × (per-machine CPU/RAM throughput)**. No sticky sessions. Use the same optional env
//!   (`VOL_BASELINE_LOOKBACK_BARS`, …) on every replica when those matter. **No HTTP authentication**
//!   — run behind your own reverse proxy / VPN if you need access control.
//! - **Concurrency:** omit `EVALUATE_MAX_INFLIGHT` for **no** software inflight cap on indicator
//!   compute (only hardware / the runtime limit you). Set a positive value to cap concurrent
//!   requests **per process** (overload protection).
//!
//! ## Route map
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET | `/health` | Liveness check. |
//! | GET | `/v1/capabilities` | Machine metadata. |
//! | GET | `/v1/catalog` | Full combined discovery (strategies + indicators). |
//! | GET | `/v1/indicators` | List all indicator entries. |
//! | GET | `/v1/indicators/{name}` | Metadata for one indicator (404 if unknown). |
//! | GET | `/v1/strategies` | List all strategy entries. |
//! | GET | `/v1/strategies/{id}` | Metadata for one strategy (404 if unknown). |
//! | POST | `/v1/indicators/{name}` | Compute last-bar value for one indicator. |
//! | POST | `/v1/indicators/{name}/replay` | Replay one indicator over a bar window. |
//! | POST | `/v1/indicators/replay` | Replay multiple indicators (list in body) over a bar window. |
//! | POST | `/v1/strategies/replay` | Linear strategy decisions over a bar index range (JSON). |

#![allow(non_snake_case)] // Same package name as library crate (`binance_BTC`).
#![allow(clippy::multiple_crate_versions)] // Transitive duplicates; see `lib.rs`.

use std::sync::Arc;

use anyhow::anyhow;
use axum::{
    Json, Router,
    extract::DefaultBodyLimit,
    extract::Path,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use binance_BTC::{
    CatalogIndicatorEntry, CatalogResponse, CatalogStrategyEntry, DecisionMachine,
    EvaluateIndicatorError, EvaluateStrategyError, IndicatorEvaluateResponse,
    IndicatorReplayRequest, IndicatorReplayResponse, MachineCapabilities, MachineRequest,
    StrategyConfig, StrategyReplayRequest, StrategyReplayResponse,
};
use tower::limit::ConcurrencyLimitLayer;
use tower::util::option_layer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

/// Max JSON body size for JSON POST routes (`/v1/indicators/*`, `/v1/strategies/replay`).
const MAX_JSON_BODY: usize = 10 * 1024 * 1024;

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

    let host = std::env::var("HOST")
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));

    let evaluate_max_inflight = std::env::var("EVALUATE_MAX_INFLIGHT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&n| n > 0);
    let evaluate_concurrency_limit = evaluate_max_inflight.map(ConcurrencyLimitLayer::new);
    if let Some(n) = evaluate_max_inflight {
        tracing::info!(
            evaluate_max_inflight = n,
            "EVALUATE_MAX_INFLIGHT: concurrent POST /v1/indicators/* and /v1/strategies/replay capped per process"
        );
    } else {
        tracing::info!(
            "EVALUATE_MAX_INFLIGHT unset: no per-process concurrency cap (horizontal scale + hardware bound)"
        );
    }

    let machine = Arc::new(machine_from_env());

    let v1_post = Router::new()
        .route("/indicators/{indicator_name}", post(evaluate_indicator))
        .route(
            "/indicators/{indicator_name}/replay",
            post(evaluate_indicator_replay),
        )
        .route("/indicators/replay", post(evaluate_indicators_replay))
        .route("/strategies/replay", post(evaluate_strategy_replay))
        .layer(option_layer(evaluate_concurrency_limit));

    let v1 = Router::new()
        .route("/catalog", get(catalog))
        .route("/indicators", get(list_indicators))
        .route("/indicators/{indicator_name}", get(get_indicator))
        .route("/strategies", get(list_strategies))
        .route("/strategies/{strategy_id}", get(get_strategy))
        .merge(v1_post);

    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/capabilities", get(capabilities))
        .nest("/v1", v1)
        .layer(DefaultBodyLimit::max(MAX_JSON_BODY))
        .layer(TraceLayer::new_for_http())
        .with_state(machine);

    let listener = tokio::net::TcpListener::bind((host, port)).await?;
    tracing::info!(%host, %port, "listening");
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
    Json(machine.capabilities().clone())
}

async fn evaluate_strategy_replay(
    State(machine): State<Arc<DecisionMachine>>,
    Json(request): Json<StrategyReplayRequest>,
) -> Result<Json<StrategyReplayResponse>, ApiError> {
    machine
        .evaluate_strategy_replay(request)
        .map(Json)
        .map_err(|e| {
            let EvaluateStrategyError::Dataset(err) = e;
            ApiError(err)
        })
}

async fn evaluate_indicator(
    State(machine): State<Arc<DecisionMachine>>,
    Path(indicator_name): Path<String>,
    Json(request): Json<MachineRequest>,
) -> Result<Json<IndicatorEvaluateResponse>, IndicatorApiError> {
    machine
        .evaluate_indicator(indicator_name.trim(), request)
        .map(Json)
        .map_err(IndicatorApiError)
}

async fn evaluate_indicator_replay(
    State(machine): State<Arc<DecisionMachine>>,
    Path(indicator_name): Path<String>,
    Json(request): Json<IndicatorReplayRequest>,
) -> Result<Json<IndicatorReplayResponse>, IndicatorApiError> {
    let path = indicator_name.trim().to_string();
    machine
        .evaluate_indicator_replay(&[path.as_str()], request)
        .map(Json)
        .map_err(IndicatorApiError)
}

async fn evaluate_indicators_replay(
    State(machine): State<Arc<DecisionMachine>>,
    Json(request): Json<IndicatorReplayRequest>,
) -> Result<Json<IndicatorReplayResponse>, IndicatorApiError> {
    if request.indicators.is_empty() {
        return Err(IndicatorApiError(EvaluateIndicatorError::Dataset(anyhow!(
            "indicators list must be non-empty for POST /v1/indicators/replay"
        ))));
    }
    let path_strings: Vec<String> = request.indicators.clone();
    let paths: Vec<&str> = path_strings.iter().map(String::as_str).collect();
    machine
        .evaluate_indicator_replay(&paths, request)
        .map(Json)
        .map_err(IndicatorApiError)
}

async fn catalog() -> Json<CatalogResponse> {
    Json(DecisionMachine::catalog())
}

async fn list_indicators() -> Json<Vec<CatalogIndicatorEntry>> {
    Json(DecisionMachine::catalog().indicators)
}

async fn get_indicator(
    Path(name): Path<String>,
) -> Result<Json<CatalogIndicatorEntry>, (StatusCode, Json<serde_json::Value>)> {
    DecisionMachine::catalog()
        .indicators
        .into_iter()
        .find(|e| e.path == name.trim())
        .map(Json)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "unknown_indicator", "path": name })),
            )
        })
}

async fn list_strategies() -> Json<Vec<CatalogStrategyEntry>> {
    Json(DecisionMachine::catalog().strategies)
}

async fn get_strategy(
    Path(id): Path<String>,
) -> Result<Json<CatalogStrategyEntry>, (StatusCode, Json<serde_json::Value>)> {
    DecisionMachine::catalog()
        .strategies
        .into_iter()
        .find(|e| e.id == id.trim())
        .map(Json)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "unknown_strategy", "id": id })),
            )
        })
}

struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        tracing::warn!(error = %self.0, "machine JSON route failed");
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "invalid_request" })),
        )
            .into_response()
    }
}

struct IndicatorApiError(EvaluateIndicatorError);

impl IntoResponse for IndicatorApiError {
    fn into_response(self) -> axum::response::Response {
        match self.0 {
            EvaluateIndicatorError::Unknown { path } => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "unknown_indicator",
                    "path": path,
                })),
            )
                .into_response(),
            EvaluateIndicatorError::Dataset(e) => ApiError(e).into_response(),
        }
    }
}
