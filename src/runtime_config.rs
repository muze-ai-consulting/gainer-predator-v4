use serde::{Serialize, Deserialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Hot-reloadable parameters for the scanner and exit logic.
/// Updated via POST /api/experiment without restarting the bot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub rvol_threshold: f64,
    pub jump_min_pct: f64,
    pub jump_max_pct: f64,
    pub max_positions: usize,
    pub position_size_pct: f64,
    pub good_hours: Vec<u32>,
    pub scan_interval_secs: u64,
    pub universe_size: usize,
    pub apex_retracement: f64,   // ratio, e.g. 0.005 = 0.5%
    pub stop_loss_pct: f64,      // ratio, e.g. 0.015 = 1.5%
    pub max_hold_secs: u64,
    pub default_leverage: u32,
    pub apex_activation_pct: f64,
    pub apex_tight_activation_pct: f64,
    pub apex_tight_retracement: f64,
    #[serde(default)]
    pub experiment_id: Option<u64>,
}

pub type SharedRuntimeConfig = Arc<RwLock<RuntimeConfig>>;

impl RuntimeConfig {
    pub fn from_env() -> Self {
        Self {
            rvol_threshold: parse_env("RVOL_THRESHOLD", 2.0),
            jump_min_pct: parse_env("JUMP_MIN_PCT", 0.5),
            jump_max_pct: parse_env("JUMP_MAX_PCT", 15.0),
            max_positions: parse_env("MAX_POSITIONS", 3) as usize,
            position_size_pct: parse_env("POSITION_SIZE_PCT", 20.0) / 100.0,
            good_hours: parse_hours(&std::env::var("GOOD_HOURS_UTC")
                .unwrap_or_else(|_| "1,2,5,6,9,10,11,13,14,21".to_string())),
            scan_interval_secs: parse_env("SCAN_INTERVAL_SECS", 60) as u64,
            universe_size: parse_env("SCAN_UNIVERSE_SIZE", 50) as usize,
            apex_retracement: parse_env("APEX_RETRACEMENT", 0.5) / 100.0,
            stop_loss_pct: parse_env("STOP_LOSS_PCT", 1.5) / 100.0,
            max_hold_secs: parse_env("MAX_HOLD_HOURS", 6) as u64 * 3600,
            default_leverage: parse_env("DEFAULT_LEVERAGE", 10) as u32,
            apex_activation_pct: parse_env("APEX_ACTIVATION_PCT", 0.0) / 100.0,
            apex_tight_activation_pct: parse_env("APEX_TIGHT_ACTIVATION_PCT", 3.0) / 100.0,
            apex_tight_retracement: parse_env("APEX_TIGHT_RETRACEMENT", 0.5) / 100.0,
            experiment_id: None,
        }
    }

    /// Update fields from a JSON object. Only present keys are updated.
    pub fn update_from_json(&mut self, params: &serde_json::Value) {
        if let Some(v) = params["rvol_threshold"].as_f64() { self.rvol_threshold = v; }
        if let Some(v) = params["jump_min_pct"].as_f64() { self.jump_min_pct = v; }
        if let Some(v) = params["jump_max_pct"].as_f64() { self.jump_max_pct = v; }
        if let Some(v) = params["max_positions"].as_u64() { self.max_positions = v as usize; }
        if let Some(v) = params["apex_retracement"].as_f64() { self.apex_retracement = v / 100.0; }
        if let Some(v) = params["stop_loss_pct"].as_f64() { self.stop_loss_pct = v / 100.0; }
        if let Some(v) = params["max_hold_hours"].as_u64() { self.max_hold_secs = v * 3600; }
        if let Some(v) = params["default_leverage"].as_u64() { self.default_leverage = v as u32; }
        if let Some(v) = params["apex_activation_pct"].as_f64() { self.apex_activation_pct = v / 100.0; }
        if let Some(v) = params["apex_tight_activation_pct"].as_f64() { self.apex_tight_activation_pct = v / 100.0; }
        if let Some(v) = params["apex_tight_retracement"].as_f64() { self.apex_tight_retracement = v / 100.0; }
        if let Some(arr) = params["good_hours"].as_array() {
            self.good_hours = arr.iter().filter_map(|h| h.as_u64().map(|v| v as u32)).collect();
        }
        if let Some(v) = params["experiment_id"].as_u64() { self.experiment_id = Some(v); }
    }

    pub fn into_shared(self) -> SharedRuntimeConfig {
        Arc::new(RwLock::new(self))
    }
}

fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}

fn parse_hours(s: &str) -> Vec<u32> {
    s.split(',').filter_map(|h| h.trim().parse().ok()).collect()
}
