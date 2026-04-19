//! `server` binary — **horizontally scalable** HTTP front for `DecisionMachine`.
//!
//! - **Stateless requests:** every input for an evaluation is in the `POST` JSON body. Shared
//!   [`Arc`] state is **read-only** after startup (no cross-request mutable server state).
//! - **Scale out:** run **N** identical processes behind a load balancer; capacity grows with
//!   **N × (per-machine CPU/RAM throughput)**. No sticky sessions. Use the same optional env
//!   (`EVALUATE_API_KEY`, `VOL_BASELINE_LOOKBACK_BARS`, …) on every replica when those matter.
//! - **Concurrency:** omit `EVALUATE_MAX_INFLIGHT` for **no** software inflight cap on evaluate
//!   (only hardware / the runtime limit you). Set a positive value to cap concurrent evaluates
//!   **per process** (overload protection).

#![allow(non_snake_case)] // Same package name as library crate (`binance_BTC`).
#![allow(clippy::multiple_crate_versions)] // Transitive duplicates; see `lib.rs`.

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::DefaultBodyLimit,
    extract::Request,
    extract::State,
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::IntoResponse,
    routing::{get, post},
};
use binance_BTC::{
    DecisionMachine, MachineCapabilities, MachineRequest, MachineResponse, StrategyConfig,
};
use tower::limit::ConcurrencyLimitLayer;
use tower::util::option_layer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

/// Max JSON body size for `POST /v1/evaluate` (and any future JSON routes on this router).
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

    let evaluate_api_key = std::env::var("EVALUATE_API_KEY")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(Arc::<str>::from);

    if evaluate_api_key.is_some() {
        tracing::info!("EVALUATE_API_KEY is set; POST /v1/evaluate requires authentication");
    }

    let evaluate_max_inflight = std::env::var("EVALUATE_MAX_INFLIGHT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&n| n > 0);
    let evaluate_concurrency_limit = evaluate_max_inflight.map(ConcurrencyLimitLayer::new);
    if let Some(n) = evaluate_max_inflight {
        tracing::info!(
            evaluate_max_inflight = n,
            "EVALUATE_MAX_INFLIGHT: concurrent POST /v1/evaluate capped per process"
        );
    } else {
        tracing::info!(
            "EVALUATE_MAX_INFLIGHT unset: no per-process evaluate concurrency cap (horizontal scale + hardware bound)"
        );
    }

    let machine = Arc::new(machine_from_env());

    let evaluate_auth = {
        let evaluate_api_key = evaluate_api_key.clone();
        middleware::from_fn(move |request: Request, next: Next| {
            let evaluate_api_key = evaluate_api_key.clone();
            async move {
                if let Some(expected) = evaluate_api_key.as_ref()
                    && !header_matches_api_key(request.headers(), expected)
                {
                    return unauthorized_response();
                }
                next.run(request).await
            }
        })
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/capabilities", get(capabilities))
        .route(
            "/v1/evaluate",
            post(evaluate)
                .layer(option_layer(evaluate_concurrency_limit))
                .layer(evaluate_auth),
        )
        .layer(DefaultBodyLimit::max(MAX_JSON_BODY))
        .layer(TraceLayer::new_for_http())
        .with_state(machine);

    let listener = tokio::net::TcpListener::bind((host, port)).await?;
    tracing::info!(%host, %port, "listening");
    axum::serve(listener, app).await?;
    Ok(())
}

fn header_matches_api_key(headers: &HeaderMap, expected: &str) -> bool {
    if let Some(value) = headers.get("x-api-key").and_then(|h| h.to_str().ok())
        && constant_time_str_eq(value.trim(), expected)
    {
        return true;
    }
    if let Some(auth) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        let auth = auth.trim();
        let mut parts = auth.splitn(2, char::is_whitespace);
        let scheme = parts.next().unwrap_or("");
        if scheme.eq_ignore_ascii_case("bearer")
            && let Some(token) = parts.next()
            && constant_time_str_eq(token.trim(), expected)
        {
            return true;
        }
    }
    false
}

/// Best-effort constant-time compare for equal-length secrets (mitigates byte-at-a-time guessing).
fn constant_time_str_eq(a: &str, b: &str) -> bool {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    if ab.len() != bb.len() {
        return false;
    }
    ab.iter()
        .zip(bb.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

fn unauthorized_response() -> axum::response::Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "error": "unauthorized" })),
    )
        .into_response()
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

async fn evaluate(
    State(machine): State<Arc<DecisionMachine>>,
    Json(request): Json<MachineRequest>,
) -> Result<Json<MachineResponse>, ApiError> {
    machine.evaluate(request).map(Json).map_err(ApiError)
}

struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        tracing::warn!(error = %self.0, "evaluate failed");
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "invalid_request" })),
        )
            .into_response()
    }
}
