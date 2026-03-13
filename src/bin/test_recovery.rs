use trz_bot::config::Config;
use trz_bot::binance::BinanceClient;
use trz_bot::models::{Signal, Side};
use trz_bot::http_api::BotEvent;
use tokio::sync::mpsc;
use std::time::Duration;
use log::{info, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("🧪 INICIANDO TEST DE RECOUPERACIÓN (Persistence Crash Test)");

    let config = Config::load();
    let (order_tx, _order_rx) = mpsc::unbounded_channel();
    let (events_tx, _) = tokio::sync::broadcast::channel::<BotEvent>(256);
    let client = BinanceClient::new(&config, order_tx, events_tx);

    // 1. Simular un estado previo en el archivo
    let fake_signal = Signal {
        msg_id: 888888,
        symbol: "SOLUSDT".to_string(),
        side: Side::Long,
        leverage: Some(10),
        entry: None,
        sl: None,
        tp: None,
        received_at: std::time::Instant::now(),
    };

    let fake_state = serde_json::json!({
        "processed_msg_ids": [888888],
        "active_positions": [
            {
                "signal": fake_signal,
                "entry_price": 84.50,
                "quantity": "0.1"
            }
        ]
    });

    std::fs::write("bot_state.json", serde_json::to_string_pretty(&fake_state)?)?;
    info!("📝 Archivo bot_state.json creado con una posición huérfana para SOLUSDT.");

    // 2. Inicializar y cargar estado
    info!("🔄 Inicializando caches y cargando estado...");
    // No necesitamos init_precision_cache completo para este test de lógica
    
    client.load_state();

    info!("⏳ Esperando 5 segundos para ver si el monitor Apex se re-activa solo...");
    tokio::time::sleep(Duration::from_secs(5)).await;

    info!("🏆 TEST FINALIZADO: Verifica los logs anteriores para ver el mensaje '♻️ Recovering active position'.");
    
    Ok(())
}
