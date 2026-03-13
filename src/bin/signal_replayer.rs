use trz_bot::config::Config;
use trz_bot::binance::BinanceClient;
use trz_bot::http_api::BotEvent;
use trz_bot::parser::parse_signal;
use tokio::sync::mpsc;
use std::time::Instant;
use log::{info, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("🎯 INICIANDO SIGNAL REPLAYER (File-based testing)");

    let config = Config::load();
    let (order_tx, mut order_rx) = mpsc::unbounded_channel();
    let (events_tx, _) = tokio::sync::broadcast::channel::<BotEvent>(256);
    let mut client = BinanceClient::new(&config, order_tx, events_tx);

    info!("🔄 Inicializando caches de Binance...");
    client.init_precision_cache().await?;
    client.fetch_initial_balance().await?;
    client.spawn_price_sync();

    // Spawn order logger
    tokio::spawn(async move {
        while let Some((id, payload, _)) = order_rx.recv().await {
            info!("🛒 Replayer detected order sent: [{}] {}", id, payload);
        }
    });

    // Leer señales desde signals.txt
    let content = match std::fs::read_to_string("signals.txt") {
        Ok(c) => c,
        Err(_) => {
            info!("📝 No se encontró signals.txt. Creando uno de ejemplo...");
            let example = "#SOLUSDT Bullish\n#BTCUSDT Bullish";
            std::fs::write("signals.txt", example)?;
            example.to_string()
        }
    };

    for line in content.lines() {
        if line.trim().is_empty() { continue; }
        info!("📖 Procesando línea: {}", line);
        
        if let Some(signal) = parse_signal(line, 12345, Instant::now()) {
            info!("✅ Señal detectada para {}. Ejecutando...", signal.symbol);
            if let Err(e) = client.execute_market_order(&signal).await {
                error!("❌ Error ejecutando señal: {}", e);
            }
        } else {
            error!("❌ No se pudo parsear la señal: {}", line);
        }
    }

    info!("⌛ Replayer terminó de procesar el archivo. Manteniéndome vivo 10s para ver logs...");
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    
    Ok(())
}
