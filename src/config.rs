use std::env;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Config {
    // Telegram MTProto (optional - only needed for signal-listener mode)
    pub telegram_api_id: Option<i32>,
    pub telegram_api_hash: Option<String>,
    pub telegram_phone: Option<String>,
    pub target_channel_id: Option<i64>,
    pub target_channel_id_test: Option<i64>,

    // Binance API (required)
    pub binance_api_key: String,
    pub binance_api_secret: String,
    pub use_testnet: bool,

    // Position management
    pub risk_percent: f64,
    pub default_leverage: u32,
    pub preheat_top_n: usize,
    pub margin_type: String,

    // Apex exit parameters
    pub use_apex_exit: bool,
    pub apex_retracement: f64,
    pub apex_activation_pct: f64,
    pub apex_tight_activation_pct: f64,
    pub apex_tight_retracement: f64,

    // Slippage & safety
    pub slippage_pct: f64,
    pub max_dynamic_slippage_pct: f64,
    pub stop_loss_pct: f64,
    pub abort_slippage_pct: f64,
    pub exit_grace_period_secs: u64,

    // Max hold (seconds). 0 = disabled.
    pub max_hold_secs: u64,

    // Notifications (Telegram Bot API)
    pub telegram_bot_token: Option<String>,
    pub notif_chat_id: Option<String>,
    pub preheat_refresh_hours: u64,

    // Mode selection
    pub mode: BotMode,
    pub trading_mode: TradingMode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BotMode {
    Scanner,  // Self-scanning Gainer Predator
    Telegram, // Legacy: listen to Telegram signals
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TradingMode {
    Live,   // Real orders on Binance
    Paper,  // Simulated fills, real market data
}

impl Config {
    pub fn load() -> Self {
        dotenvy::dotenv().ok();

        let mode = match env::var("BOT_MODE").unwrap_or_else(|_| "scanner".to_string()).to_lowercase().as_str() {
            "telegram" => BotMode::Telegram,
            _ => BotMode::Scanner,
        };

        let trading_mode = match env::var("TRADING_MODE").unwrap_or_else(|_| "paper".to_string()).to_lowercase().as_str() {
            "live" => TradingMode::Live,
            _ => TradingMode::Paper, // Default to paper for safety
        };

        Self {
            // Telegram MTProto - optional
            telegram_api_id: env::var("TELEGRAM_API_ID").ok().and_then(|s| s.parse().ok()),
            telegram_api_hash: env::var("TELEGRAM_API_HASH").ok(),
            telegram_phone: env::var("TELEGRAM_PHONE").ok(),
            target_channel_id: env::var("TARGET_CHANNEL_ID").ok().and_then(|s| s.trim().parse().ok()),
            target_channel_id_test: env::var("TARGET_CHANNEL_ID_TEST").ok().and_then(|s| s.trim().parse().ok()),

            // Binance
            binance_api_key: env::var("BINANCE_API_KEY").expect("Missing BINANCE_API_KEY"),
            binance_api_secret: env::var("BINANCE_API_SECRET").expect("Missing BINANCE_API_SECRET"),
            use_testnet: env::var("USE_TESTNET").unwrap_or_else(|_| "true".to_string()).to_lowercase() == "true",

            // Position
            risk_percent: env::var("RISK_PERCENT").unwrap_or_else(|_| "20.0".to_string()).parse().unwrap_or(20.0) / 100.0,
            default_leverage: env::var("DEFAULT_LEVERAGE").unwrap_or_else(|_| "10".to_string()).parse().unwrap_or(10),
            preheat_top_n: env::var("PREHEAT_TOP_N").unwrap_or_else(|_| "50".to_string()).parse().unwrap_or(50),
            margin_type: env::var("MARGIN_TYPE").unwrap_or_else(|_| "CROSSED".to_string()),

            // Apex - for scanner mode: immediate trailing (activation=0), tight retracement
            use_apex_exit: env::var("USE_APEX_EXIT").unwrap_or_else(|_| "true".to_string()).to_lowercase() == "true",
            apex_retracement: env::var("APEX_RETRACEMENT").unwrap_or_else(|_| "0.5".to_string()).parse().unwrap_or(0.5) / 100.0,
            apex_activation_pct: env::var("APEX_ACTIVATION_PCT").unwrap_or_else(|_| "0.0".to_string()).parse().unwrap_or(0.0) / 100.0,
            apex_tight_activation_pct: env::var("APEX_TIGHT_ACTIVATION_PCT").unwrap_or_else(|_| "3.0".to_string()).parse().unwrap_or(3.0) / 100.0,
            apex_tight_retracement: env::var("APEX_TIGHT_RETRACEMENT").unwrap_or_else(|_| "0.5".to_string()).parse().unwrap_or(0.5) / 100.0,

            // Slippage & safety
            slippage_pct: env::var("SLIPPAGE_PCT").unwrap_or_else(|_| "0.5".to_string()).parse().unwrap_or(0.5) / 100.0,
            max_dynamic_slippage_pct: env::var("MAX_DYNAMIC_SLIPPAGE_PCT").unwrap_or_else(|_| "2.0".to_string()).parse().unwrap_or(2.0) / 100.0,
            stop_loss_pct: env::var("STOP_LOSS_PCT").unwrap_or_else(|_| "1.5".to_string()).parse().unwrap_or(1.5) / 100.0,
            abort_slippage_pct: env::var("ABORT_SLIPPAGE_PCT").unwrap_or_else(|_| "2.5".to_string()).parse().unwrap_or(2.5) / 100.0,
            exit_grace_period_secs: env::var("EXIT_GRACE_PERIOD_SECS").unwrap_or_else(|_| "0".to_string()).parse().unwrap_or(0),

            // Max hold
            max_hold_secs: env::var("MAX_HOLD_HOURS").unwrap_or_else(|_| "6".to_string()).parse::<u64>().unwrap_or(6) * 3600,

            // Notifications
            telegram_bot_token: env::var("TELEGRAM_BOT_TOKEN").ok(),
            notif_chat_id: env::var("NOTIF_CHAT_ID").ok(),
            preheat_refresh_hours: env::var("PREHEAT_REFRESH_HOURS").unwrap_or_else(|_| "12".to_string()).parse().unwrap_or(12),

            mode,
            trading_mode,
        }
    }
}
