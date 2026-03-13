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

    info!("🚀 INICIANDO TEST LIVE: Apex Detection & Slippage Guard");

    let mut config = Config::load();

    // CONFIGURACIÓN ULTRA-SENSIBLE PARA EL TEST
    // Usamos un retracement minúsculo para que cualquier movimiento natural lo active
    config.use_apex_exit = true;
    config.apex_retracement = 0.0001; // 0.01% - Casi cualquier tick lo activará
    config.slippage_pct = 0.005;     // 0.5% para asegurar entrada rápida

    let (order_tx, mut order_rx) = mpsc::unbounded_channel();
    let (events_tx, _) = tokio::sync::broadcast::channel::<BotEvent>(256);
    let mut client = BinanceClient::new(&config, order_tx, events_tx);
    
    info!("🔄 Inicializando caches de Binance...");
    client.init_precision_cache().await?;
    client.fetch_initial_balance().await?;
    client.spawn_user_data_stream();
    client.spawn_price_sync();
    
    // Esperar sincronización inicial (Aumentado para asegurar que lleguen los precios de todos los símbolos)
    tokio::time::sleep(Duration::from_secs(10)).await;
    
    // Elegimos un símbolo volátil pero activo para el test
    let symbol = "SOLUSDT".to_string(); 
    
    let balance = *client.balance.read().await;
    info!("💰 Balance detectado: ${}", balance);
    
    if balance < 10.0 {
        error!("❌ Balance insuficiente para test en mainnet/testnet. Necesitas al menos $10.");
        return Ok(());
    }

    let signal = Signal {
        msg_id: 999999, // ID único para el test
        symbol: symbol.clone(),
        side: Side::Long,
        leverage: Some(10),
        entry: None,
        sl: None,
        tp: None,
        received_at: std::time::Instant::now(),
    };

    info!("📡 ENVIANDO ORDEN DE PRUEBA PARA: {}", symbol);
    client.execute_market_order(&signal).await.map_err(|e| e as Box<dyn std::error::Error>)?;

    info!("⏳ Esperando ejecución y monitoreo de APEX...");
    
    // Escuchamos el canal de órdenes para ver salir la de entrada y luego la de Apex
    let mut orders_received = 0;
    while orders_received < 2 {
        match tokio::time::timeout(Duration::from_secs(30), order_rx.recv()).await {
            Ok(Some((id, payload, _))) => {
                orders_received += 1;
                let order: serde_json::Value = serde_json::from_str(&payload).unwrap_or(serde_json::json!({}));
                let side = order["params"]["side"].as_str().unwrap_or("?");
                let price = order["params"]["price"].as_str().unwrap_or("?");
                
                if id.starts_with("order_") {
                    info!("✅ [ORDEN 1: ENTRADA] ID: {}, Side: {}, Limit Price: {}", id, side, price);
                } else {
                    info!("✅ [ORDEN 2: APEX EXIT] ID: {}, Side: {}, Limit Price: {}", id, side, price);
                    info!("🏆 TEST COMPLETADO: El algoritmo Apex detectó el movimiento y cerró.");
                    break;
                }
            }
            _ => {
                info!("⏱️ Timeout o Fin de la simulación.");
                break;
            }
        }
    }

    Ok(())
}
