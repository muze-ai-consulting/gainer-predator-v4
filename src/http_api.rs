use axum::{
    Router,
    routing::{get, post},
    extract::State,
    response::{Json, sse::{Event, KeepAlive, Sse}},
    http::Method,
};
use tower_http::cors::{CorsLayer, Any};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use std::sync::Arc;
use std::convert::Infallible;
use log::info;

use crate::binance::BinanceClient;
use crate::runtime_config::SharedRuntimeConfig;
use crate::trade_logger;

/// Bot event for SSE streaming
#[derive(Debug, Clone, serde::Serialize)]
pub struct BotEvent {
    pub event_type: String,
    pub data: serde_json::Value,
    pub timestamp: String,
}

pub type EventSender = broadcast::Sender<BotEvent>;

#[derive(Clone)]
struct AppState {
    binance: Arc<BinanceClient>,
    runtime_config: SharedRuntimeConfig,
    events_tx: EventSender,
}

pub async fn serve(
    binance: Arc<BinanceClient>,
    runtime_config: SharedRuntimeConfig,
    events_tx: EventSender,
    port: u16,
) {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    let state = AppState { binance, runtime_config, events_tx };

    let app = Router::new()
        .route("/api/status", get(get_status))
        .route("/api/trades", get(get_trades))
        .route("/api/metrics", get(get_metrics))
        .route("/api/experiments", get(get_experiments))
        .route("/api/experiment", post(post_experiment))
        .route("/api/stream", get(sse_stream))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .expect("Failed to bind HTTP server");

    info!("🌐 HTTP API server listening on http://0.0.0.0:{}", port);

    axum::serve(listener, app).await.unwrap();
}

async fn get_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let balance = *state.binance.balance.read().await;
    let active_positions: Vec<serde_json::Value> = state.binance.active_positions
        .iter()
        .map(|e| {
            let mut pos = e.value().clone();
            // Add live PnL if price available
            if let Some(symbol) = pos["signal"]["symbol"].as_str() {
                if let Some(price) = state.binance.prices.get(symbol) {
                    let (bid, _ask) = *price.value();
                    pos["current_price"] = serde_json::json!(bid);
                    if let Some(entry) = pos["entry_price"].as_f64() {
                        let pnl = (bid - entry) / entry * 100.0;
                        pos["live_pnl_pct"] = serde_json::json!((pnl * 100.0).round() / 100.0);
                    }
                }
            }
            pos
        })
        .collect();

    let cfg = state.runtime_config.read().await;

    Json(serde_json::json!({
        "balance": balance,
        "active_positions": active_positions,
        "active_count": active_positions.len(),
        "config": {
            "rvol_threshold": cfg.rvol_threshold,
            "jump_min_pct": cfg.jump_min_pct,
            "jump_max_pct": cfg.jump_max_pct,
            "max_positions": cfg.max_positions,
            "apex_retracement": cfg.apex_retracement * 100.0,
            "stop_loss_pct": cfg.stop_loss_pct * 100.0,
            "max_hold_hours": cfg.max_hold_secs / 3600,
            "default_leverage": cfg.default_leverage,
            "good_hours": cfg.good_hours.clone(),
        }
    }))
}

async fn get_trades() -> Json<serde_json::Value> {
    let trades = trade_logger::read_trades();
    Json(serde_json::json!(trades))
}

async fn get_metrics(State(state): State<AppState>) -> Json<serde_json::Value> {
    let trades = trade_logger::read_trades();
    let mut metrics = trade_logger::compute_metrics(&trades);

    // Merge balance and active positions count
    let balance = *state.binance.balance.read().await;
    let active_count = state.binance.active_positions.len();
    if let Some(obj) = metrics.as_object_mut() {
        obj.insert("balance".to_string(), serde_json::json!(balance));
        obj.insert("active_positions".to_string(), serde_json::json!(active_count));
    }

    Json(metrics)
}

async fn get_experiments() -> Json<serde_json::Value> {
    let content = std::fs::read_to_string("experiments.jsonl").unwrap_or_default();
    let experiments: Vec<serde_json::Value> = content.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    Json(serde_json::json!(experiments))
}

async fn post_experiment(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let mut cfg = state.runtime_config.write().await;
    cfg.update_from_json(&body);

    // Broadcast config update event
    let _ = state.events_tx.send(BotEvent {
        event_type: "config_updated".to_string(),
        data: serde_json::to_value(&*cfg).unwrap_or_default(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    });

    info!("🔧 RuntimeConfig updated via API: {:?}", body);

    Json(serde_json::json!({
        "status": "ok",
        "config": serde_json::to_value(&*cfg).unwrap_or_default()
    }))
}

async fn sse_stream(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.events_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| {
        match result {
            Ok(event) => {
                let data = serde_json::to_string(&event).unwrap_or_default();
                Some(Ok(Event::default().data(data)))
            }
            Err(_) => None,
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
