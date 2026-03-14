use crate::binance::BinanceClient;
use crate::models::{Side, Signal};
use crate::runtime_config::SharedRuntimeConfig;
use crate::http_api::{BotEvent, EventSender};
use log::{info, error, debug};
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};
use std::collections::HashMap;

/// Atomic counter for generating unique signal IDs
static SIGNAL_COUNTER: AtomicI32 = AtomicI32::new(1_000_000);

/// Kline (OHLCV) data for one candle
#[derive(Debug)]
struct Kline {
    open: f64,
    close: f64,
    volume: f64,
}

/// Candidate signal with ranking info
#[derive(Debug)]
struct Candidate {
    symbol: String,
    rvol: f64,
    jump_pct: f64,
}

/// Start the scanner loop with hot-reloadable config.
pub fn spawn_scanner(binance: Arc<BinanceClient>, runtime_config: SharedRuntimeConfig, events_tx: EventSender) {
    tokio::spawn(async move {
        info!("🔍 Gainer Predator Scanner started.");

        // Wait for price sync and balance to initialize
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Per-symbol cooldown: track recent trade timestamps (max 3 per symbol per hour)
        let mut symbol_trade_times: HashMap<String, Vec<Instant>> = HashMap::new();
        let max_trades_per_symbol_per_hour = 3usize;

        loop {
            // Read config fresh each cycle
            let cfg = runtime_config.read().await;
            let rvol_threshold = cfg.rvol_threshold;
            let jump_min = cfg.jump_min_pct;
            let jump_max = cfg.jump_max_pct;
            let max_positions = cfg.max_positions;
            let good_hours = cfg.good_hours.clone();
            let universe_size = cfg.universe_size;
            let scan_interval = cfg.scan_interval_secs;
            let current_experiment_id = cfg.experiment_id;
            drop(cfg); // Release lock before sleeping

            tokio::time::sleep(Duration::from_secs(scan_interval)).await;

            // Check current UTC hour
            let now_utc = current_utc_hour();
            if !good_hours.contains(&now_utc) {
                debug!("⏰ Hour {} UTC not in good hours. Skipping.", now_utc);
                continue;
            }

            let active_count = binance.active_positions.len();
            if active_count >= max_positions {
                debug!("📊 Max positions reached ({}/{}).", active_count, max_positions);
                continue;
            }

            let slots_available = max_positions - active_count;

            match scan_universe(&binance, universe_size, rvol_threshold, jump_min, jump_max).await {
                Ok(candidates) => {
                    if candidates.is_empty() {
                        debug!("🔍 No signals this cycle (hour {} UTC).", now_utc);

                        // Emit scan result event
                        let _ = events_tx.send(BotEvent {
                            event_type: "scan_result".to_string(),
                            data: serde_json::json!({
                                "hour": now_utc, "candidates": 0, "slots": slots_available
                            }),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        });
                        continue;
                    }

                    info!("🎯 Found {} candidates at hour {} UTC", candidates.len(), now_utc);

                    let _ = events_tx.send(BotEvent {
                        event_type: "scan_result".to_string(),
                        data: serde_json::json!({
                            "hour": now_utc,
                            "candidates": candidates.len(),
                            "symbols": candidates.iter().map(|c| &c.symbol).collect::<Vec<_>>(),
                        }),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    });

                    for candidate in candidates.into_iter().take(slots_available) {
                        if binance.active_positions.contains_key(&candidate.symbol) {
                            continue;
                        }

                        // Per-symbol cooldown: max 3 trades per symbol per hour
                        let now = Instant::now();
                        let times = symbol_trade_times.entry(candidate.symbol.clone()).or_default();
                        times.retain(|t| now.duration_since(*t) < Duration::from_secs(3600));
                        if times.len() >= max_trades_per_symbol_per_hour {
                            debug!("🧊 Cooldown: {} already has {} trades this hour. Skipping.",
                                candidate.symbol, times.len());
                            continue;
                        }

                        let signal_id = SIGNAL_COUNTER.fetch_add(1, Ordering::SeqCst);
                        let signal = Signal {
                            msg_id: signal_id,
                            symbol: candidate.symbol.clone(),
                            side: Side::Long,
                            leverage: None,
                            entry: None,
                            sl: None,
                            tp: None,
                            received_at: Instant::now(),
                        };

                        info!("🚀 SCANNER SIGNAL: {} | RVol: {:.1}x | Jump: +{:.1}%",
                            candidate.symbol, candidate.rvol, candidate.jump_pct);

                        binance.send_notification(format!(
                            "🔍 *SCANNER SIGNAL*\n*Symbol:* `{}`\n*RVol:* `{:.1}x`\n*Jump:* `+{:.1}%`\n*Hour:* `{} UTC`",
                            candidate.symbol, candidate.rvol, candidate.jump_pct, now_utc
                        ));

                        let _ = events_tx.send(BotEvent {
                            event_type: "signal_detected".to_string(),
                            data: serde_json::json!({
                                "symbol": candidate.symbol,
                                "rvol": candidate.rvol,
                                "jump_pct": candidate.jump_pct,
                                "hour": now_utc,
                            }),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        });

                        // Record trade time for cooldown
                        symbol_trade_times.entry(candidate.symbol.clone()).or_default().push(Instant::now());

                        // Store rvol/jump/experiment_id in the signal metadata for trade logging
                        let bc = binance.clone();
                        let rvol = candidate.rvol;
                        let jump = candidate.jump_pct;
                        let exp_id = current_experiment_id;
                        tokio::spawn(async move {
                            if let Err(e) = bc.execute_market_order_with_metadata(&signal, rvol, jump, exp_id).await {
                                error!("❌ Scanner trade failed for {}: {}", signal.symbol, e);
                            }
                        });
                    }
                }
                Err(e) => {
                    error!("❌ Scanner scan failed: {}", e);
                }
            }
        }
    });
}

async fn scan_universe(
    binance: &BinanceClient,
    universe_size: usize,
    rvol_threshold: f64,
    jump_min: f64,
    jump_max: f64,
) -> Result<Vec<Candidate>, Box<dyn std::error::Error + Send + Sync>> {
    let top_symbols = get_top_symbols(binance, universe_size).await?;
    let mut candidates = Vec::new();

    for symbol in &top_symbols {
        if binance.active_positions.contains_key(symbol) {
            continue;
        }

        match fetch_klines_1h(binance, symbol, 25).await {
            Ok(klines) => {
                if klines.len() < 25 { continue; }

                let latest = &klines[klines.len() - 1];
                let prev_candles = &klines[..klines.len() - 1];

                let avg_volume: f64 = prev_candles.iter().map(|k| k.volume).sum::<f64>() / prev_candles.len() as f64;
                if avg_volume <= 0.0 { continue; }
                let rvol = latest.volume / avg_volume;

                if latest.open <= 0.0 { continue; }
                let jump_pct = ((latest.close - latest.open) / latest.open) * 100.0;

                if rvol >= rvol_threshold && jump_pct >= jump_min && jump_pct <= jump_max {
                    candidates.push(Candidate { symbol: symbol.clone(), rvol, jump_pct });
                }
            }
            Err(e) => { debug!("Failed klines for {}: {}", symbol, e); }
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    candidates.sort_by(|a, b| b.rvol.partial_cmp(&a.rvol).unwrap_or(std::cmp::Ordering::Equal));
    Ok(candidates)
}

async fn get_top_symbols(binance: &BinanceClient, n: usize) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("{}/fapi/v1/ticker/24hr", binance.base_url);
    let res = binance.client.get(&url).send().await?;
    let tickers: serde_json::Value = res.json().await?;

    let mut symbols_vol: Vec<(String, f64)> = tickers.as_array()
        .ok_or("Invalid ticker response")?
        .iter()
        .filter(|t| t["symbol"].as_str().map(|s| s.ends_with("USDT") && !s.contains("_")).unwrap_or(false))
        .filter_map(|t| {
            let symbol = t["symbol"].as_str()?.to_string();
            let vol = t["quoteVolume"].as_str()?.parse::<f64>().ok()?;
            Some((symbol, vol))
        })
        .collect();

    symbols_vol.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    Ok(symbols_vol.into_iter().take(n).map(|(s, _)| s).collect())
}

async fn fetch_klines_1h(binance: &BinanceClient, symbol: &str, limit: usize) -> Result<Vec<Kline>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("{}/fapi/v1/klines?symbol={}&interval=1h&limit={}", binance.base_url, symbol, limit);
    let res = binance.client.get(&url).send().await?;
    let data: serde_json::Value = res.json().await?;

    let klines = data.as_array()
        .ok_or("Invalid klines response")?
        .iter()
        .filter_map(|k| {
            let arr = k.as_array()?;
            Some(Kline {
                open: arr.get(1)?.as_str()?.parse().ok()?,
                close: arr.get(4)?.as_str()?.parse().ok()?,
                volume: arr.get(5)?.as_str()?.parse().ok()?,
            })
        })
        .collect();

    Ok(klines)
}

fn current_utc_hour() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    ((secs % 86400) / 3600) as u32
}
