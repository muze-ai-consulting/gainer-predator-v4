use crate::models::{Side, Signal};
use crate::config::Config;
use crate::http_api::{BotEvent, EventSender};
use crate::trade_logger;
use reqwest::{Client, header};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH, Instant, Duration};
use log::{info, error, debug};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use dashmap::DashMap;
use futures_util::{StreamExt, SinkExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
struct SymbolInfo {
    pub step_size: f64,
    pub quantity_precision: usize,
    pub tick_size: f64,
    pub price_precision: usize,
}

#[derive(Clone)]
pub struct BinanceClient {
    pub client: Client,
    pub api_key: String,
    pub api_secret: String,
    pub base_url: String,
    pub ws_url: String,
    pub order_ws_url: String,
    symbol_cache: Arc<DashMap<String, SymbolInfo>>,
    leverage_cache: Arc<DashMap<String, u32>>,
    margin_cache: Arc<DashMap<String, String>>,
    pub prices: Arc<DashMap<String, (f64, f64)>>, // (bid, ask)
    pub balance: Arc<RwLock<f64>>,
    pub risk_percent: f64,
    pub use_apex_exit: bool,
    pub apex_retracement: f64,
    pub apex_activation_pct: f64,
    pub apex_tight_activation_pct: f64,
    pub apex_tight_retracement: f64,
    pub slippage_pct: f64,
    pub max_dynamic_slippage_pct: f64,
    pub stop_loss_pct: f64,
    pub abort_slippage_pct: f64,
    pub exit_grace_period_secs: u64,
    pub default_leverage: u32,
    pub processed_msg_ids: Arc<DashMap<i32, Instant>>,
    pub active_positions: Arc<DashMap<String, serde_json::Value>>,
    order_tx: mpsc::UnboundedSender<(String, String, std::time::Instant)>,
    pub telegram_bot_token: Option<String>,
    pub notif_chat_id: Option<String>,
    pub preheat_top_n: usize,
    pub preheat_refresh_hours: u64,
    pub margin_type: String,
    pub max_hold_secs: u64,
    pub events_tx: EventSender,
    pub paper_mode: bool,
}

impl BinanceClient {
    pub fn new(config: &Config, order_tx: mpsc::UnboundedSender<(String, String, std::time::Instant)>, events_tx: EventSender) -> Self {
        let mut headers = header::HeaderMap::new();
        headers.insert("X-MBX-APIKEY", header::HeaderValue::from_str(&config.binance_api_key).unwrap());
        
        let client = Client::builder()
            .default_headers(headers)
            .pool_idle_timeout(None) 
            .tcp_nodelay(true) // 🏎️ CTO Optimization: Disable Nagle's algorithm
            .build()
            .unwrap();

        let (base_url, ws_url, order_ws_url) = if config.use_testnet {
            ("https://testnet.binancefuture.com".to_string(), 
             "wss://fstream.binancefuture.com/ws".to_string(),
             "wss://testnet.binancefuture.com/ws-fapi/v1".to_string())
        } else {
            ("https://fapi.binance.com".to_string(), 
             "wss://fstream.binance.com/ws".to_string(),
             "wss://ws-fapi.binance.com/ws-fapi/v1".to_string())
        };

        Self {
            client,
            api_key: config.binance_api_key.clone(),
            api_secret: config.binance_api_secret.clone(),
            base_url,
            ws_url,
            order_ws_url,
            symbol_cache: Arc::new(DashMap::new()),
            leverage_cache: Arc::new(DashMap::new()),
            margin_cache: Arc::new(DashMap::new()),
            prices: Arc::new(DashMap::new()),
            balance: Arc::new(RwLock::new(0.0)),
            risk_percent: config.risk_percent,
            use_apex_exit: config.use_apex_exit,
            apex_retracement: config.apex_retracement,
            apex_activation_pct: config.apex_activation_pct,
            apex_tight_activation_pct: config.apex_tight_activation_pct,
            apex_tight_retracement: config.apex_tight_retracement,
            slippage_pct: config.slippage_pct,
            max_dynamic_slippage_pct: config.max_dynamic_slippage_pct,
            stop_loss_pct: config.stop_loss_pct,
            abort_slippage_pct: config.abort_slippage_pct,
            exit_grace_period_secs: config.exit_grace_period_secs,
            default_leverage: config.default_leverage,
            processed_msg_ids: Arc::new(DashMap::new()),
            active_positions: Arc::new(DashMap::new()),
            order_tx,
            telegram_bot_token: config.telegram_bot_token.clone(),
            notif_chat_id: config.notif_chat_id.clone(),
            preheat_top_n: config.preheat_top_n,
            preheat_refresh_hours: config.preheat_refresh_hours,
            margin_type: config.margin_type.clone(),
            max_hold_secs: config.max_hold_secs,
            events_tx,
            paper_mode: config.trading_mode == crate::config::TradingMode::Paper,
        }
    }

    pub fn spawn_order_ws(&self, mut rx: mpsc::UnboundedReceiver<(String, String, std::time::Instant)>, _api_key: String) {
        let order_ws_url = self.order_ws_url.clone();
        let bc = self.clone();
        
        tokio::spawn(async move {
            info!("🔥 Background Task: Order WebSocket starting...");
            loop {
                info!("📡 Connecting to Binance Order WebSocket API at {}...", order_ws_url);
                match connect_async(&order_ws_url).await {
                    Ok((ws_stream, _)) => {
                        info!("✅ Order WebSocket Connected successfully.");
                        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
                        let pending_orders = DashMap::new();
                        
                        loop {
                            tokio::select! {
                                Some((id, payload, start)) = rx.recv() => {
                                    pending_orders.insert(id, start);
                                    
                                    if let Err(e) = ws_sender.send(Message::Text(payload.into())).await {
                                        error!("❌ WS Send Error: {}", e);
                                        break;
                                    }
                                }
                                Some(msg) = ws_receiver.next() => {
                                    match msg {
                                        Ok(Message::Text(text)) => {
                                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                                if let Some(id) = json["id"].as_str() {
                                                    if let Some((_, start)) = pending_orders.remove(id) {
                                                        let total_time = start.elapsed();
                                                        info!("🚀 WS Confirmation [{}]: Total E2E Latency: {:?}. Response: {}", id, total_time, text);
                                                        
                                                        // Handle Rejections: Close state if Binance says NO
                                                        if let Some(status) = json["status"].as_i64() {
                                                                if status != 200 {
                                                                    let error_code = json["error"]["code"].as_i64().unwrap_or(0);
                                                                    if error_code == -2022 {
                                                                        info!("ℹ️ Position for symbol in ID {} already closed (ReduceOnly -2022). Syncing state.", id);
                                                                        if let Some(symbol) = id.split('_').nth(1).or_else(|| id.split('_').nth(2)) {
                                                                            bc.active_positions.remove(symbol);
                                                                        }
                                                                    } else if error_code == -2027 {
                                                                        // 🚀 Leverage Auto-Scaling Retry logic
                                                                        let parts: Vec<&str> = id.split('_').collect();
                                                                        if parts.len() >= 4 {
                                                                            let symbol = parts[1].to_string();
                                                                            let current_lev = parts[2].parse::<u32>().unwrap_or(20);
                                                                            
                                                                            if current_lev > 10 {
                                                                                let next_lev = if current_lev > 15 { 15 } else { 10 };
                                                                                info!("⚠️ Leverage Limit Exceeded for {}. Current: {}x. Retrying with {}x...", symbol, current_lev, next_lev);
                                                                                
                                                                                let bc_retry = bc.clone();
                                                                                tokio::spawn(async move {
                                                                                    // Use a dummy signal for retry - only needs symbol, side, leverage
                                                                                    // Optimization: extract previous side from memory or id if we encoded it (we didn't yet)
                                                                                    if let Some(pos) = bc_retry.active_positions.get(&symbol) {
                                                                                        if let Ok(signal) = serde_json::from_value::<crate::models::Signal>(pos.value()["signal"].clone()) {
                                                                                            let mut signal_retry = signal;
                                                                                            signal_retry.leverage = Some(next_lev);
                                                                                            let _ = bc_retry.execute_market_order(&signal_retry).await;
                                                                                        }
                                                                                    }
                                                                                });
                                                                            } else {
                                                                                error!("❌ Max Position Limit reached even at 10x for {}. Giving up.", symbol);
                                                                                bc.send_notification(format!("❌ *LIMIT EXCEEDED*\n\n*Symbol:* `{}`\nExceeded max position limits even at 10x leverage.", symbol));
                                                                                bc.active_positions.remove(&symbol);
                                                                            }
                                                                        }
                                                                    } else {
                                                                        error!("❌ Order REJECTED by Binance: {}. Cleaning up state...", text);
                                                                        if let Some(symbol) = id.split('_').nth(1).or_else(|| id.split('_').nth(2)) {
                                                                            bc.send_notification(format!("❌ *ORDER REJECTED*\n\n*Symbol:* `{}`\n*Details:* `{}`", symbol, text));
                                                                            bc.active_positions.remove(symbol);
                                                                        }
                                                                    }
                                                                }
                                                        }
                                                    } else {
                                                        info!("🚀 WS Response [{}]: {}", id, text);
                                                    }
                                                    // On any successful order response, save state for security
                                                    bc.save_state();
                                                } else {
                                                    info!("🚀 WS Response: {}", text);
                                                }
                                            }
                                        }
                                        Ok(Message::Ping(p)) => {
                                            let _ = ws_sender.send(Message::Pong(p)).await;
                                        }
                                        Ok(Message::Close(_)) => {
                                            error!("❌ WS Closed by server");
                                            break;
                                        }
                                        Err(e) => {
                                            error!("❌ WS Receiver Error: {}", e);
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    },
                    Err(e) => error!("❌ Order WebSocket connection failed: {}. Retrying in 5s...", e),
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        });
    }

    pub async fn init_precision_cache(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("🔄 Initializing symbol precision cache from Binance...");
        self.spawn_cache_cleanup();
        let url = format!("{}/fapi/v1/exchangeInfo", self.base_url);
        let res = self.client.get(&url).send().await?;
        let json: serde_json::Value = res.json().await?;
        
        if let Some(symbols) = json["symbols"].as_array() {
            for s in symbols {
                let symbol_name = s["symbol"].as_str().unwrap_or("").to_string();
                let quantity_precision = s["quantityPrecision"].as_u64().unwrap_or(3) as usize;
                let price_precision = s["pricePrecision"].as_u64().unwrap_or(2) as usize;
                
                let mut step_size = 0.001;
                let mut tick_size = 0.01;

                if let Some(filters) = s["filters"].as_array() {
                    for f in filters {
                        match f["filterType"].as_str() {
                            Some("LOT_SIZE") => {
                                step_size = f["stepSize"].as_str().unwrap_or("0.001").parse().unwrap_or(0.001);
                            }
                            Some("PRICE_FILTER") => {
                                tick_size = f["tickSize"].as_str().unwrap_or("0.01").parse().unwrap_or(0.01);
                            }
                            _ => {}
                        }
                    }
                }
                
                self.symbol_cache.insert(symbol_name, SymbolInfo {
                    step_size,
                    quantity_precision,
                    tick_size,
                    price_precision,
                });
            }
        }
        info!("✅ Cached precision for {} symbols.", self.symbol_cache.len());
        Ok(())
    }

    /// Método manual para testing sin API real
    pub async fn init_manual_precision(&mut self, symbol: &str, step_size: f64, quantity_precision: usize, tick_size: f64, price_precision: usize) {
        self.symbol_cache.insert(symbol.to_string(), SymbolInfo {
            step_size,
            quantity_precision,
            tick_size,
            price_precision,
        });
    }

    /// 🔥 Pre-Heating: Precarga la configuración de los Top N símbolos por volumen
    pub async fn preheat_active_symbols(&self) {
        if self.preheat_top_n == 0 { return; }
        
        info!("🔥 Starting Trends Pre-Heating (Top {} symbols by volume)...", self.preheat_top_n);
        let url = format!("{}/fapi/v1/ticker/24hr", self.base_url);
        
        let tickers = match self.client.get(&url).send().await {
            Ok(res) => match res.json::<serde_json::Value>().await {
                Ok(json) => json,
                Err(_) => return,
            },
            Err(_) => return,
        };

        if let Some(ticker_list) = tickers.as_array() {
            let mut symbols_with_volume: Vec<(String, f64)> = ticker_list.iter()
                .filter(|t| t["symbol"].as_str().map(|s| s.ends_with("USDT")).unwrap_or(false))
                .filter_map(|t| {
                    let symbol = t["symbol"].as_str()?.to_string();
                    let volume = t["quoteVolume"].as_str()?.parse().unwrap_or(0.0);
                    Some((symbol, volume))
                })
                .collect();

            // Sort by volume descending
            symbols_with_volume.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            let top_symbols: Vec<String> = symbols_with_volume.into_iter()
                .take(self.preheat_top_n)
                .map(|(s, _)| s)
                .collect();

            info!("📊 Top Trending Symbols to pre-heat: {:?}", top_symbols);

            let bc = self.clone();
            tokio::spawn(async move {
                let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
                for symbol in top_symbols {
                    // Solo pre-calentamos si NO está en caché (para no spammar Binance en reinicios rápidos)
                    if !bc.leverage_cache.contains_key(&symbol) || !bc.margin_cache.contains_key(&symbol) {
                        debug!("🔥 Pre-heating {}...", symbol);
                        
                        // Configurar Leverage
                        let lev_query = format!("symbol={}&leverage={}&timestamp={}", symbol, bc.default_leverage, timestamp);
                        let lev_sig = bc.generate_signature(&lev_query);
                        let lev_url = format!("{}/fapi/v1/leverage?{}&signature={}", bc.base_url, lev_query, lev_sig);
                        let _ = bc.client.post(&lev_url).send().await;
                        bc.leverage_cache.insert(symbol.clone(), bc.default_leverage);

                        // Configurar Margin Type
                        let margin_query = format!("symbol={}&marginType={}&timestamp={}", symbol, bc.margin_type, timestamp);
                        let margin_sig = bc.generate_signature(&margin_query);
                        let margin_url = format!("{}/fapi/v1/marginType?{}&signature={}", bc.base_url, margin_query, margin_sig);
                        let _ = bc.client.post(&margin_url).send().await;
                        bc.margin_cache.insert(symbol.clone(), bc.margin_type.clone());

                        // 🛡️ Rate Limit protection
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
                info!("✅ Trends Pre-Heating completed.");
            });
        }
    }

    /// Spawns a background loop to refresh trending symbols periodically
    pub fn spawn_preheat_loop(&self) {
        if self.preheat_refresh_hours == 0 || self.preheat_top_n == 0 { return; }
        
        let bc = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(bc.preheat_refresh_hours * 3600));
            // First tick finishes immediately, but we already called preheat manually in main.rs
            // so we skip the first tick or just let it run (it checks cache anyway).
            loop {
                interval.tick().await;
                debug!("🔄 Scheduled pre-heat refresh triggered...");
                bc.preheat_active_symbols().await;
            }
        });
    }

    pub fn spawn_user_data_stream(&self) {
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let base_url = self.base_url.clone();
        // Use standard futures websocket for user data
        let ws_url = self.ws_url.replace("wss://fstream.binance.com/det-ws", "wss://fstream.binance.com/ws");
        let balance_arc = self.balance.clone();
        let bc = self.clone();

        tokio::spawn(async move {
            loop {
                // 1. Get Listen Key via REST
                let mut listen_key = String::new();
                if let Ok(res) = client.post(format!("{}/fapi/v1/listenKey", base_url))
                    .header("X-MBX-APIKEY", &api_key)
                    .send().await {
                    if let Ok(json) = res.json::<serde_json::Value>().await {
                        listen_key = json["listenKey"].as_str().unwrap_or("").to_string();
                    }
                }

                if listen_key.is_empty() {
                    error!("❌ Failed to get ListenKey. Retrying in 5s...");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }

                info!("🔑 User Data Stream: ListenKey acquired. Connecting...");

                // 2. Connect to User Stream WS
                let stream_url = format!("{}/{}", ws_url, listen_key);
                match connect_async(&stream_url).await {
                    Ok((ws_stream, _)) => {
                        let (_, mut ws_receiver) = ws_stream.split();
                        info!("✅ User Data Stream Connected.");

                        // 3. Keep-alive loop triggered by timer
                        let ka_client = client.clone();
                        let ka_api_key = api_key.clone();
                        let ka_base_url = base_url.clone();
                        
                        tokio::spawn(async move {
                            let mut interval = tokio::time::interval(Duration::from_secs(1800)); // 30 mins
                            loop {
                                interval.tick().await;
                                let _ = ka_client.put(format!("{}/fapi/v1/listenKey", ka_base_url))
                                    .header("X-MBX-APIKEY", &ka_api_key)
                                    .send().await;
                                debug!("🔄 User Data Stream: ListenKey refreshed.");
                            }
                        });

                        while let Some(msg) = ws_receiver.next().await {
                            match msg {
                                Ok(Message::Text(text)) => {
                                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                        if json["e"] == "ACCOUNT_UPDATE" {
                                            if let Some(balances) = json["a"]["B"].as_array() {
                                                for b in balances {
                                                    if b["a"] == "USDT" {
                                                        if let Some(wb) = b["wb"].as_str().and_then(|s| s.parse::<f64>().ok()) {
                                                            let mut bal = balance_arc.write().await;
                                                            *bal = wb;
                                                            info!("💰 Balance Updated (WS): ${}", wb);
                                                        }
                                                    }
                                                }
                                            }
                                        } else if json["e"] == "ORDER_TRADE_UPDATE" {
                                            if let Some(o) = json.get("o") {
                                                if o["X"] == "FILLED" || o["X"] == "PARTIALLY_FILLED" {
                                                    if let (Some(s), Some(ap), Some(side)) = (o["s"].as_str(), o["ap"].as_str(), o["S"].as_str()) {
                                                        if let Ok(avg_price) = ap.parse::<f64>() {
                                                            let bc_clone = bc.clone();
                                                            let symbol_name = s.to_string();
                                                            let side_str = side.to_string();
                                                            tokio::spawn(async move {
                                                                bc_clone.check_abort_condition(symbol_name, avg_price, side_str).await;
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => break,
                            }
                        }
                    },
                    Err(e) => error!("❌ User Data Stream Connection Error: {}. Retrying...", e),
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });
    }

    pub async fn fetch_initial_balance(&self) -> Result<(), Box<dyn std::error::Error>> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
        let query = format!("timestamp={}", timestamp);
        let sig = self.generate_signature(&query);
        let url = format!("{}/fapi/v2/account?{}&signature={}", self.base_url, query, sig);
        
        let res = self.client.get(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send().await?;
        
        let json: serde_json::Value = res.json().await?;
        if let Some(assets) = json["assets"].as_array() {
            for asset in assets {
                if asset["asset"] == "USDT" {
                    if let Some(bal_str) = asset["availableBalance"].as_str() {
                        let bal_f = bal_str.parse::<f64>()?;
                        let mut bal = self.balance.write().await;
                        *bal = bal_f;
                        info!("💰 Initial Balance: ${}", bal_f);
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn check_abort_condition(&self, symbol: String, average_price: f64, order_side: String) {
        if let Some(pos) = self.active_positions.get(&symbol) {
            let entry_price_goal = pos.value()["entry_price"].as_f64().unwrap_or(0.0);
            if entry_price_goal == 0.0 { return; }
            
            let signal_side = pos.value()["signal"]["side"].as_str().unwrap_or("");
            let is_entry = (signal_side == "Long" && order_side == "BUY") || (signal_side == "Short" && order_side == "SELL");
            
            if is_entry {
                let distance = if signal_side == "Long" {
                    (average_price - entry_price_goal) / entry_price_goal
                } else {
                    (entry_price_goal - average_price) / entry_price_goal
                };

                if distance > self.abort_slippage_pct {
                    info!("🚨 ABORT PROTOCOL ACTIVATED for {}: Late Entry Detected. Goal: {}, Actual: {}, Slippage: {:.2}%", symbol, entry_price_goal, average_price, distance * 100.0);
                    
                    let qty = pos.value()["quantity"].as_str().unwrap_or("0.0").to_string();
                    let close_side = if signal_side == "Long" { crate::models::Side::Short } else { crate::models::Side::Long };
                    
                    self.active_positions.remove(&symbol); // Bypass Apex trailing
                    let lev = self.leverage_cache.get(&symbol).map(|v| *v.value()).unwrap_or(self.default_leverage);
                    
                    let close_signal = crate::models::Signal {
                        msg_id: -1, // Use -1 or dummy for internally generated signals
                        symbol: symbol.clone(),
                        side: close_side,
                        leverage: Some(lev),
                        entry: None,
                        sl: None,
                        tp: None,
                        received_at: std::time::Instant::now(),
                    };
                    let _ = self.execute_market_order_with_retries(&close_signal, qty).await;
                    
                    let msg = format!("🚨 *ABORT PROTOCOL ACTIVATED*\n\n*Symbol:* `{}`\n*Entry Goal:* `${}`\n*Actual Fill:* `${}`\n*Slippage:* `-{:.2}%`\n\n_Position force-closed to prevent Mecha trap._", symbol, entry_price_goal, average_price, distance * 100.0);
                    self.send_notification(msg);
                }
            }
        }
    }

    pub fn spawn_price_sync(&self) {
        let ws_url = format!("{}/!bookTicker", self.ws_url);
        let prices = self.prices.clone();

        tokio::spawn(async move {
            loop {
                info!("📡 Connecting to Binance BookTicker (Real-time Bid/Ask) WebSocket...");
                match connect_async(&ws_url).await {
                    Ok((mut ws_stream, _)) => {
                        info!("✅ Connected to Real-time BookTicker Feed.");
                        while let Some(msg) = ws_stream.next().await {
                            if let Ok(Message::Text(text)) = msg {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                    if let (Some(s), Some(b), Some(a)) = (json["s"].as_str(), json["b"].as_str(), json["a"].as_str()) {
                                        if let (Ok(bid), Ok(ask)) = (b.parse::<f64>(), a.parse::<f64>()) {
                                            prices.insert(s.to_string(), (bid, ask));
                                        }
                                    }
                                }
                            }
                        }
                    },
                    Err(e) => error!("BookTicker WebSocket connection error: {}. Retrying in 5s...", e),
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        });
    }

    fn generate_signature(&self, query: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(self.api_secret.as_bytes()).unwrap();
        mac.update(query.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    pub fn send_notification(&self, message: String) {
        let token = match &self.telegram_bot_token {
            Some(t) => t.clone(),
            None => return,
        };
        let chat_id = match &self.notif_chat_id {
            Some(c) => c.clone(),
            None => return,
        };
        let client = self.client.clone();

        tokio::spawn(async move {
            let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
            let payload = serde_json::json!({
                "chat_id": chat_id,
                "text": message,
                "parse_mode": "Markdown"
            });
            let _ = client.post(&url).json(&payload).send().await;
        });
    }

    pub async fn execute_market_order(&self, signal: &Signal) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let t0 = signal.received_at;
        info!("🚀 Sniper Execution Started: T0 -> T1 (Parse): {:?}", t0.elapsed());
        // 1. Anti-Reentry Check (Msg ID)
        if self.processed_msg_ids.contains_key(&signal.msg_id) {
            debug!("Skipping already processed message ID: {}", signal.msg_id);
            return Ok(());
        }

        // 2. Active Position Check (Symbol)
        if self.active_positions.contains_key(&signal.symbol) {
            info!("⚠️ Active position already exists for {}. Skipping re-entry.", signal.symbol);
            return Ok(());
        }

        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
        
        // 3. Get stats from memory (0 RTT)
        let available_balance = *self.balance.read().await;
        if available_balance <= 0.0 {
            return Err("Available USDT balance is 0. Sync might be in progress.".into());
        }

        let (bid, ask) = *self.prices.get(&signal.symbol).ok_or("Price not in cache")?.value();
        if bid == 0.0 || ask == 0.0 {
            return Err("Cached price is 0".into());
        }

        let current_price = match signal.side {
            Side::Long => ask,
            Side::Short => bid,
        };

        let info = self.symbol_cache.get(&signal.symbol).ok_or("Symbol not found in cache")?;

        // 4. Mark as processed BEFORE sending order to ensure safety
        self.processed_msg_ids.insert(signal.msg_id, Instant::now());

        // 5. Leverage Optimization (Cache check)
        let target_leverage = signal.leverage.unwrap_or(self.default_leverage);

        if !self.paper_mode {
            let cached_lev = self.leverage_cache.get(&signal.symbol).map(|v| *v.value());

            if cached_lev != Some(target_leverage) {
                info!("⚙️ Adjusting leverage for {} to {}x (Sync for safety)", signal.symbol, target_leverage);
                let lev_query = format!("symbol={}&leverage={}&timestamp={}", signal.symbol, target_leverage, timestamp);
                let lev_sig = self.generate_signature(&lev_query);
                let lev_url = format!("{}/fapi/v1/leverage?{}&signature={}", self.base_url, lev_query, lev_sig);

                match self.client.post(&lev_url).send().await {
                    Ok(res) if res.status().is_success() => {
                        self.leverage_cache.insert(signal.symbol.clone(), target_leverage);
                        info!("✅ Leverage for {} adjusted to {}x", signal.symbol, target_leverage);
                    }
                    Ok(res) => error!("❌ Failed to adjust leverage for {}: {}", signal.symbol, res.status()),
                    Err(e) => error!("❌ Network error adjusting leverage for {}: {}", signal.symbol, e),
                }
            }

            // 5.1 Margin Type Sync (Sync for safety)
            let target_margin = self.margin_type.as_str();
            let cached_margin = self.margin_cache.get(&signal.symbol).map(|v| v.value().clone());

            if cached_margin != Some(target_margin.to_string()) {
                info!("⚙️ Changing margin type for {} to {} (Sync for safety)", signal.symbol, target_margin);
                let margin_query = format!("symbol={}&marginType={}&timestamp={}", signal.symbol, target_margin, timestamp);
                let margin_sig = self.generate_signature(&margin_query);
                let margin_url = format!("{}/fapi/v1/marginType?{}&signature={}", self.base_url, margin_query, margin_sig);

                match self.client.post(&margin_url).send().await {
                    Ok(res) if res.status().is_success() => {
                        self.margin_cache.insert(signal.symbol.clone(), target_margin.to_string());
                        info!("✅ Margin type for {} set to {}", signal.symbol, target_margin);
                    }
                    Ok(res) => {
                        debug!("⚠️ Margin type info for {}: {}", signal.symbol, res.status());
                        self.margin_cache.insert(signal.symbol.clone(), target_margin.to_string());
                    }
                    Err(e) => error!("❌ Network error setting margin type for {}: {}", signal.symbol, e),
                }
            }

            // 5.2 Propagation Delay (Binance matching engine sync)
            if cached_lev != Some(target_leverage) || cached_margin != Some(target_margin.to_string()) {
                debug!("⏳ Waiting 20ms for account changes to propagate on {}...", signal.symbol);
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        } else {
            // Paper mode: just cache the leverage locally
            self.leverage_cache.insert(signal.symbol.clone(), target_leverage);
            self.margin_cache.insert(signal.symbol.clone(), self.margin_type.clone());
        }

        let usd_to_risk = available_balance * self.risk_percent;
        let raw_quantity = (usd_to_risk * target_leverage as f64) / current_price;
        let step = info.step_size;
        let quantity = (raw_quantity / step).floor() * step;
        
        if quantity <= 0.0 {
            let msg = format!("❌ Skipping order for {}: Calculated quantity is 0.0 (Risk: ${:.2}, Leverage: {}x, Price: ${})", signal.symbol, usd_to_risk, target_leverage, current_price);
            error!("{}", msg);
            self.send_notification(msg);
            return Err("Calculated order quantity is less than or equal to zero".into());
        }
        
        let quantity_str = format!("{:.*}", info.quantity_precision, quantity);

        let pos_state = serde_json::json!({
            "signal": signal,
            "entry_price": current_price,
            "quantity": quantity_str
        });
        self.active_positions.insert(signal.symbol.clone(), pos_state);
        self.save_state_async();

        let side_str = match signal.side {
            Side::Long => "BUY",
            Side::Short => "SELL",
        };

        // Slippage Guard: Calculate LIMIT price with dynamic offset based on spread
        let spread_pct = if bid > 0.0 { (ask - bid) / bid } else { 0.0 };
        let dynamic_slippage_pct = (self.slippage_pct + spread_pct).min(self.max_dynamic_slippage_pct);

        let limit_price = match signal.side {
            Side::Long => current_price * (1.0 + dynamic_slippage_pct),
            Side::Short => current_price * (1.0 - dynamic_slippage_pct),
        };
        // Rounded to tick size
        let tick = info.tick_size;
        let rounded_limit = (limit_price / tick).round() * tick;
        let limit_price_str = format!("{:.*}", info.price_precision, rounded_limit);

        if self.paper_mode {
            // PAPER MODE: Simulate instant fill at market price
            info!("📝 [PAPER] Simulated fill for {} {} @ ${} qty={}", signal.symbol, side_str, current_price, quantity_str);
        } else {
            // LIVE MODE: Send real order via WebSocket
            let api_key = &self.api_key;
            let ts_str = timestamp.to_string();

            let params_raw = [
                ("apiKey", api_key.as_str()),
                ("price", limit_price_str.as_str()),
                ("quantity", quantity_str.as_str()),
                ("side", side_str),
                ("symbol", signal.symbol.as_str()),
                ("timeInForce", "IOC"),
                ("timestamp", ts_str.as_str()),
                ("type", "LIMIT"),
            ];

            let query_string = params_raw.iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");

            let signature = self.generate_signature(&query_string);

            let order_id = format!("order_{}_{}_{}", signal.symbol, target_leverage, timestamp);
            let raw_payload = format!(
                r#"{{"id":"{}","method":"order.place","params":{{"apiKey":"{}","symbol":"{}","side":"{}","type":"LIMIT","timeInForce":"IOC","quantity":"{}","price":"{}","timestamp":{},"signature":"{}"}}}}"#,
                order_id, api_key, signal.symbol, side_str, quantity_str, limit_price_str, timestamp, signature
            );

            // 7. Fire Order via WS
            if let Err(e) = self.order_tx.send((order_id, raw_payload, t0)) {
                self.active_positions.remove(&signal.symbol);
                return Err(format!("Failed to send order to WS task: {}", e).into());
            }

            info!("🛡️ Slippage Guard: Sent LIMIT IOC for {} @ ${} (Entry: ${})", signal.symbol, limit_price_str, current_price);

            // Zero-Latency Notification: Dispatched to background task
            self.send_notification(format!(
                "🎯 *SIGNAL IDENTIFIED & ORDER SENT*\n\n*Symbol:* `{}`\n*Side:* `{}`\n*Qty:* `{}`\n*Lev:* `{}x`\n*Latency:* `{:?}` (T0-T1)",
                signal.symbol, side_str, quantity_str, target_leverage, t0.elapsed()
            ));
        }

        self.save_state_async();

        // 8. Spawn Apex Detection (Handles all exits)
        let bc = self.clone();
        let signal_clone = signal.clone();
        let qty_clone = quantity_str.clone();

        tokio::spawn(async move {
            bc.spawn_apex_exit(signal_clone, qty_clone, current_price).await;
        });

        Ok(())
    }

    /// Execute a market order with scanner metadata (rvol, jump) stored in position state
    pub async fn execute_market_order_with_metadata(&self, signal: &Signal, rvol: f64, jump_pct: f64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Store metadata that will be used by trade_logger when the position closes
        // We do this by inserting it after execute_market_order creates the position
        let result = self.execute_market_order(signal).await;
        if result.is_ok() {
            // Attach scanner metadata to the position state
            if let Some(mut pos) = self.active_positions.get_mut(&signal.symbol) {
                pos.value_mut()["rvol"] = serde_json::json!(rvol);
                pos.value_mut()["jump_pct"] = serde_json::json!(jump_pct);
                pos.value_mut()["entry_time"] = serde_json::json!(chrono::Utc::now().to_rfc3339());
            }

            let _ = self.events_tx.send(BotEvent {
                event_type: "trade_opened".to_string(),
                data: serde_json::json!({
                    "symbol": signal.symbol,
                    "side": "Long",
                    "rvol": rvol,
                    "jump_pct": jump_pct,
                }),
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }
        result
    }

    /// Algoritmo Apex Detection: Monitorea el agotamiento del momentum en tiempo real
    pub async fn spawn_apex_exit(&self, signal: Signal, quantity_str: String, entry_price: f64) {
        info!("🧠 Apex Detection active for {}. Monitoring for peak...", signal.symbol);

        let mut apex_activated = false;
        let mut tight_mode = false;
        let mut hwm = entry_price; // High Water Mark
        let poll_interval = Duration::from_millis(50); // Polling cada 50ms para HFT
        let entry_time = Instant::now();

        // 🛡️ Volatility Protection: Wait for market to stabilize
        if self.exit_grace_period_secs > 0 {
            info!("⏳ Grace Period: Waiting {}s before activating SL/Apex for {}...", self.exit_grace_period_secs, signal.symbol);
            tokio::time::sleep(Duration::from_secs(self.exit_grace_period_secs)).await;
        }

        let mut exit_reason = "Manual/Unknown";
        let mut exit_price = entry_price;

        loop {
            // Max Hold Timer: Force exit after max_hold_secs
            if self.max_hold_secs > 0 && entry_time.elapsed() >= Duration::from_secs(self.max_hold_secs) {
                let (bid, ask) = self.prices.get(&signal.symbol).map(|p| *p.value()).unwrap_or((entry_price, entry_price));
                exit_price = match signal.side {
                    Side::Long => bid,
                    Side::Short => ask,
                };
                info!("⏰ Max Hold Timer: Forcing exit for {} after {}h", signal.symbol, self.max_hold_secs / 3600);
                exit_reason = "Max Hold Timer";
                break;
            }
            // Break early if the position was removed (e.g., due to rejection or manual intervention)
            if !self.active_positions.contains_key(&signal.symbol) {
                debug!("🛑 Apex Detection: Position for {} no longer active. Exiting monitor.", signal.symbol);
                return;
            }

            tokio::time::sleep(poll_interval).await;

            let (bid, ask) = match self.prices.get(&signal.symbol) {
                Some(p) => *p.value(),
                None => continue,
            };

            // Exit evaluation: if Long, we exit by selling (Bid). If Short, we exit by buying (Ask).
            let current_price = match signal.side {
                Side::Long => bid,
                Side::Short => ask,
            };

            // Hard Stop Loss (Kill Switch) evalúa siempre en primer lugar
            let loss = match signal.side {
                Side::Long => (entry_price - current_price) / entry_price,
                Side::Short => (current_price - entry_price) / entry_price,
            };

            if loss >= self.stop_loss_pct {
                info!("🛑 Kill Switch: Stop Loss hit for {} at ${} (Loss: {:.2}%)", signal.symbol, current_price, loss * 100.0);
                exit_reason = "Stop Loss";
                exit_price = current_price;
                break;
            }

            // Actualizar High Water Mark (HWM) o activar Apex
            match signal.side {
                Side::Long => {
                    let profit = (current_price - entry_price) / entry_price;
                    if !apex_activated {
                        if profit >= self.apex_activation_pct {
                            apex_activated = true;
                            hwm = current_price;
                            info!("🚀 Apex Activated for {} at ${} (+{:.2}%)", signal.symbol, current_price, profit * 100.0);
                        }
                    } else {
                        if current_price > hwm { 
                            hwm = current_price; 
                            let profit_at_hwm = (hwm - entry_price) / entry_price;
                            if !tight_mode && profit_at_hwm >= self.apex_tight_activation_pct {
                                tight_mode = true;
                                info!("🔒 Apex Tight Mode Activated for {}! Secured profit >= {:.2}%", signal.symbol, self.apex_tight_activation_pct * 100.0);
                            }
                        }
                        
                        let current_retracement_limit = if tight_mode { self.apex_tight_retracement } else { self.apex_retracement };
                        let retracement = (hwm - current_price) / hwm;
                        
                        // Si el precio cae un % desde el máximo, cerramos
                        if retracement >= current_retracement_limit {
                            info!("🎯 Apex Detection: Peak detected for {} at ${}. Retracement: {:.2}% (Limit: {:.2}%)", signal.symbol, hwm, retracement * 100.0, current_retracement_limit * 100.0);
                            exit_reason = "Apex Trailing";
                            exit_price = current_price;
                            break;
                        }
                    }
                }
                Side::Short => {
                    let profit = (entry_price - current_price) / entry_price;
                    if !apex_activated {
                        if profit >= self.apex_activation_pct {
                            apex_activated = true;
                            hwm = current_price;
                            info!("🚀 Apex Activated for {} at ${} (+{:.2}%)", signal.symbol, current_price, profit * 100.0);
                        }
                    } else {
                        if current_price < hwm { 
                            hwm = current_price; 
                            let profit_at_hwm = (entry_price - hwm) / entry_price;
                            if !tight_mode && profit_at_hwm >= self.apex_tight_activation_pct {
                                tight_mode = true;
                                info!("🔒 Apex Tight Mode Activated for {}! Secured profit >= {:.2}%", signal.symbol, self.apex_tight_activation_pct * 100.0);
                            }
                        }
                        
                        let current_retracement_limit = if tight_mode { self.apex_tight_retracement } else { self.apex_retracement };
                        let retracement = (current_price - hwm) / hwm;
                        
                        if retracement >= current_retracement_limit {
                            info!("🎯 Apex Detection: Peak detected for {} at ${}. Retracement: {:.2}% (Limit: {:.2}%)", signal.symbol, hwm, retracement * 100.0, current_retracement_limit * 100.0);
                            exit_reason = "Apex Trailing";
                            exit_price = current_price;
                            break;
                        }
                    }
                }
            }
        }

        // Ejecutar Cierre
        let symbol_name = signal.symbol.clone();
        let close_side = match signal.side {
            Side::Long => Side::Short,
            Side::Short => Side::Long,
        };
        
        let close_signal = Signal {
            msg_id: signal.msg_id,
            symbol: signal.symbol.clone(),
            side: close_side.clone(),
            leverage: None,
            entry: None,
            sl: None,
            tp: None,
            received_at: std::time::Instant::now(),
        };

        let close_result = if self.paper_mode {
            info!("📝 [PAPER] Simulated close for {} @ ${:.5} (reason: {})", symbol_name, exit_price, exit_reason);
            Ok(())
        } else {
            self.execute_market_order_with_retries(&close_signal, quantity_str.clone()).await
        };

        if let Err(e) = close_result {
            error!("❌ Apex Detection Final Exit Failure for {}: {}", symbol_name, e);
            self.send_notification(format!("❌ *EXIT FAILURE*\n\n*Symbol:* `{}`\n*Error:* `{}`", symbol_name, e));
        } else {
            let lev_val = signal.leverage.unwrap_or(self.default_leverage) as f64;
            let pnl_base = match signal.side {
                Side::Long => (exit_price - entry_price) / entry_price,
                Side::Short => (entry_price - exit_price) / entry_price,
            };
            let pnl_lev = pnl_base * lev_val * 100.0;
            
            let header = if exit_reason == "Apex Trailing" {
                "✅ *PROFIT SECURED (Apex Trailing Hit)*"
            } else if exit_reason == "Stop Loss" {
                "❌ *STOP LOSS HIT*"
            } else {
                "🔄 *POSITION CLOSED*"
            };
            
            self.send_notification(format!(
                "{}\n\n*Symbol:* `{}`\n*Entry:* `${:.5}`\n*Exit:* `${:.5}`\n*Lev:* `{}x`\n*PnL:* `{:.2}%`\n*Qty:* `{}`\n*Side:* `{:?}`",
                header, symbol_name, entry_price, exit_price, lev_val, pnl_lev, quantity_str, close_side
            ));

            // Log completed trade
            let pos_data = self.active_positions.get(&signal.symbol);
            let (rvol, jump) = pos_data.as_ref().map(|p| {
                (p.value()["rvol"].as_f64().unwrap_or(0.0), p.value()["jump_pct"].as_f64().unwrap_or(0.0))
            }).unwrap_or((0.0, 0.0));

            let hold_secs = entry_time.elapsed().as_secs();
            let fees = 0.08; // 0.08% round-trip

            let trade = trade_logger::CompletedTrade {
                timestamp: chrono::Utc::now().to_rfc3339(),
                symbol: symbol_name.clone(),
                side: format!("{:?}", signal.side),
                entry_price,
                exit_price,
                pnl_raw: pnl_base * 100.0,
                pnl_lev,
                rvol,
                jump,
                exit_reason: exit_reason.to_string(),
                hold_time_secs: hold_secs,
                leverage: lev_val as u32,
                fees,
                experiment_id: None,
            };
            trade_logger::append_trade(&trade);

            let _ = self.events_tx.send(BotEvent {
                event_type: "trade_closed".to_string(),
                data: serde_json::to_value(&trade).unwrap_or_default(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }
        self.active_positions.remove(&signal.symbol);
        self.save_state(); // 🛡️ CTO Hardening: State must reflect closure
    }

    /// Cierre de posición con reintentos para asegurar el "Kill" de la posición
    async fn execute_market_order_with_retries(&self, signal: &Signal, quantity_str: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut attempts = 0;
        let max_attempts = 3;

        while attempts < max_attempts {
            let now = std::time::Instant::now();
            match self.execute_market_order_internal(signal, quantity_str.clone(), now).await {
                Ok(_) => {
                    info!("✅ Position closed for {} on attempt {}", signal.symbol, attempts + 1);
                    self.save_state();
                    return Ok(());
                }
                Err(e) => {
                    attempts += 1;
                    error!("⚠️ Failed to close position for {} (Attempt {}/{}): {}", signal.symbol, attempts, max_attempts, e);
                    if attempts < max_attempts {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }
        }
        Err("Max retries reached for closing order".into())
    }

    /// Internal helper to execute an order with a pre-calculated quantity
    async fn execute_market_order_internal(&self, signal: &Signal, quantity_str: String, start_instant: std::time::Instant) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
        let api_key = &self.api_key;
        let ts_str = timestamp.to_string();

        let side_str = match signal.side {
            Side::Long => "BUY",
            Side::Short => "SELL",
        };

        let params_raw = [
            ("apiKey", api_key.as_str()),
            ("quantity", quantity_str.as_str()),
            ("reduceOnly", "true"),
            ("side", side_str),
            ("symbol", signal.symbol.as_str()),
            ("timestamp", ts_str.as_str()),
            ("type", "MARKET"),
        ];
        
        let query_string = params_raw.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        let signature = self.generate_signature(&query_string);
        
        let order_id = format!("close_{}_{}", signal.symbol, timestamp);
        let raw_payload = format!(
            r#"{{"id":"{}","method":"order.place","params":{{"apiKey":"{}","symbol":"{}","side":"{}","type":"MARKET","quantity":"{}","reduceOnly":true,"timestamp":{},"signature":"{}"}}}}"#,
            order_id, api_key, signal.symbol, side_str, quantity_str, timestamp, signature
        );

        if let Err(e) = self.order_tx.send((order_id, raw_payload, start_instant)) {
            return Err(format!("Failed to send close order to WS task: {}", e).into());
        }
        
        Ok(())
    }

    /// Persistencia asíncrona: No bloquea el flujo principal post-fire
    pub fn save_state_async(&self) {
        let bc = self.clone();
        tokio::spawn(async move {
            bc.save_state();
        });
    }

    /// Limpieza automática de caché: Evita el bloat de memoria en ejecuciones largas
    pub fn spawn_cache_cleanup(&self) {
        let bc = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(3600)).await; // Una vez por hora
                let now = Instant::now();
                let ttl = Duration::from_secs(24 * 3600); // 24 horas
                
                let mut to_remove = Vec::new();
                for entry in bc.processed_msg_ids.iter() {
                    if now.duration_since(*entry.value()) > ttl {
                        to_remove.push(*entry.key());
                    }
                }
                
                for id in to_remove {
                    bc.processed_msg_ids.remove(&id);
                }
                info!("🧹 Memory Hardening: Cleaned up message ID cache.");
            }
        });
    }

    /// Persistencia de estado: Guarda las posiciones activas e IDs procesados
    pub fn save_state(&self) {
        let mut active_pos_data = Vec::new();
        for entry in self.active_positions.iter() {
            active_pos_data.push(entry.value().clone());
        }

        let state = serde_json::json!({
            "processed_msg_ids": self.processed_msg_ids.iter().map(|e| *e.key()).collect::<Vec<i32>>(),
            "active_positions": active_pos_data
        });

        if let Ok(json_str) = serde_json::to_string_pretty(&state) {
            if let Err(e) = std::fs::write("bot_state.json", json_str) {
                error!("❌ Error saving state: {}", e);
            }
        }
    }

    pub fn load_state(&self) {
        if let Ok(content) = std::fs::read_to_string("bot_state.json") {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(ids) = json["processed_msg_ids"].as_array() {
                    for id in ids {
                        if let Some(i) = id.as_i64() {
                            self.processed_msg_ids.insert(i as i32, Instant::now());
                        }
                    }
                }
                if let Some(positions) = json["active_positions"].as_array() {
                    for pos_val in positions {
                        if let (Some(signal_json), Some(entry_price), Some(qty)) = (
                            pos_val.get("signal"),
                            pos_val.get("entry_price").and_then(|v| v.as_f64()),
                            pos_val.get("quantity").and_then(|v| v.as_str())
                        ) {
                            if let Ok(signal) = serde_json::from_value::<Signal>(signal_json.clone()) {
                                info!("♻️ Recovering active position for {} @ ${}", signal.symbol, entry_price);
                                self.active_positions.insert(signal.symbol.clone(), pos_val.clone());
                                
                                // Re-spawn monitoring
                                let bc = self.clone();
                                let qty_clone = qty.to_string();
                                tokio::spawn(async move {
                                    bc.spawn_apex_exit(signal, qty_clone, entry_price).await;
                                });
                            }
                        }
                    }
                }
                info!("📂 State loaded successfully ({} IDs, {} positions recovered).", self.processed_msg_ids.len(), self.active_positions.len());
            }
        }
    }
}
