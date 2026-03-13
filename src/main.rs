use trz_bot::config::{Config, BotMode, TradingMode};
use trz_bot::binance::BinanceClient;
use trz_bot::scanner;
use trz_bot::runtime_config::RuntimeConfig;
use trz_bot::http_api;
use log::{info, error};
use std::sync::Arc;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("🐆 Starting Gainer Predator Bot v0.3.0 (Rust HFT Edition)...");

    let config = Config::load();
    let is_paper = config.trading_mode == TradingMode::Paper;

    if is_paper {
        info!("📝 PAPER TRADING MODE — Real data, simulated execution");
    } else {
        info!("💰 LIVE TRADING MODE — Real orders will be placed!");
    }

    let (order_tx, order_rx) = tokio::sync::mpsc::unbounded_channel::<(String, String, std::time::Instant)>();

    // Event bus for SSE streaming
    let (events_tx, _) = tokio::sync::broadcast::channel::<http_api::BotEvent>(256);

    let mut binance_client_raw = BinanceClient::new(&config, order_tx, events_tx.clone());
    binance_client_raw.init_precision_cache().await?;

    if is_paper {
        // Paper mode: skip preheat (no real leverage/margin API calls needed)
        // Set a simulated starting balance
        let initial_balance: f64 = std::env::var("PAPER_BALANCE")
            .ok().and_then(|s| s.parse().ok()).unwrap_or(10_000.0);
        *binance_client_raw.balance.write().await = initial_balance;
        info!("📝 Paper balance: ${:.2}", initial_balance);
    } else {
        binance_client_raw.preheat_active_symbols().await;
        binance_client_raw.spawn_preheat_loop();
        binance_client_raw.fetch_initial_balance().await?;
        binance_client_raw.spawn_user_data_stream();
    }

    binance_client_raw.spawn_price_sync();

    if !is_paper {
        binance_client_raw.spawn_order_ws(order_rx, config.binance_api_key.clone());
    }

    binance_client_raw.load_state();
    let binance_client = Arc::new(binance_client_raw);

    // Hot-reloadable config
    let runtime_config = RuntimeConfig::from_env().into_shared();

    // HTTP API server
    let http_port: u16 = std::env::var("HTTP_PORT").ok()
        .and_then(|s| s.parse().ok()).unwrap_or(3001);

    let bc_http = binance_client.clone();
    let rc_http = runtime_config.clone();
    let ev_http = events_tx.clone();
    tokio::spawn(async move {
        http_api::serve(bc_http, rc_http, ev_http, http_port).await;
    });

    match config.mode {
        BotMode::Scanner => {
            let mode_label = if is_paper { "SCANNER + PAPER" } else { "SCANNER + LIVE" };
            info!("🔍 Mode: {} (Gainer Predator V4)", mode_label);

            let rc = runtime_config.read().await;
            info!("📊 Config: RVol >= {:.1}x, Jump {:.1}%-{:.1}%, Max {} pos, Leverage {}x",
                rc.rvol_threshold, rc.jump_min_pct, rc.jump_max_pct,
                rc.max_positions, rc.default_leverage);
            info!("⏰ Good Hours UTC: {:?}", rc.good_hours);
            info!("🛡️ Trailing: {:.1}% | SL: {:.1}% | Max Hold: {}h",
                rc.apex_retracement * 100.0, rc.stop_loss_pct * 100.0,
                rc.max_hold_secs / 3600);
            drop(rc);

            binance_client.send_notification(format!(
                "🐆 *GAINER PREDATOR v0.3 ONLINE*\n*Mode:* Scanner + Dashboard\n*API:* http://localhost:{}",
                http_port
            ));

            scanner::spawn_scanner(binance_client.clone(), runtime_config.clone(), events_tx.clone());

            info!("🐆 Gainer Predator is hunting. Dashboard at http://localhost:{}", http_port);
            tokio::signal::ctrl_c().await?;
            info!("🛑 Shutting down...");
        }
        BotMode::Telegram => {
            error!("❌ Telegram mode is disabled in this build. Use BOT_MODE=scanner");
            std::process::exit(1);
        }
    }

    Ok(())
}
