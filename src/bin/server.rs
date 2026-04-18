use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use binance_BTC::{
    DecisionMachine, MachineCapabilities, MachineRequest, MachineResponse,
};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

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

    let machine = Arc::new(DecisionMachine::default());

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
    machine
        .evaluate(request)
        .map(Json)
        .map_err(|err| ApiError(err))
}

struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let body = self.0.to_string();
        tracing::warn!(error = %body, "evaluate failed");
        (StatusCode::BAD_REQUEST, body).into_response()
    }
}
