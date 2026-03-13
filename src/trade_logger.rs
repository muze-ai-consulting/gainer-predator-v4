use serde::{Serialize, Deserialize};
use std::fs::OpenOptions;
use std::io::Write;
use log::error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedTrade {
    pub timestamp: String,
    pub symbol: String,
    pub side: String,
    pub entry_price: f64,
    pub exit_price: f64,
    pub pnl_raw: f64,
    pub pnl_lev: f64,
    pub rvol: f64,
    pub jump: f64,
    pub exit_reason: String,
    pub hold_time_secs: u64,
    pub leverage: u32,
    pub fees: f64,
    pub experiment_id: Option<u64>,
}

const TRADES_FILE: &str = "trades.jsonl";

/// Append a completed trade to trades.jsonl
pub fn append_trade(trade: &CompletedTrade) {
    let line = match serde_json::to_string(trade) {
        Ok(json) => json,
        Err(e) => {
            error!("Failed to serialize trade: {}", e);
            return;
        }
    };

    let mut file = match OpenOptions::new().create(true).append(true).open(TRADES_FILE) {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to open {}: {}", TRADES_FILE, e);
            return;
        }
    };

    if let Err(e) = writeln!(file, "{}", line) {
        error!("Failed to write trade to {}: {}", TRADES_FILE, e);
    }
}

/// Read all trades from trades.jsonl
pub fn read_trades() -> Vec<CompletedTrade> {
    let content = match std::fs::read_to_string(TRADES_FILE) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    content.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// Compute metrics from a list of trades
pub fn compute_metrics(trades: &[CompletedTrade]) -> serde_json::Value {
    if trades.is_empty() {
        return serde_json::json!({
            "total_trades": 0, "win_rate": 0.0, "total_pnl": 0.0,
            "avg_pnl": 0.0, "profit_factor": 0.0, "max_drawdown": 0.0,
            "best_trade": 0.0, "worst_trade": 0.0, "score": 0.0
        });
    }

    let total = trades.len() as f64;
    let winners: Vec<f64> = trades.iter().filter(|t| t.pnl_lev > 0.0).map(|t| t.pnl_lev).collect();
    let losers: Vec<f64> = trades.iter().filter(|t| t.pnl_lev < 0.0).map(|t| t.pnl_lev.abs()).collect();

    let win_rate = winners.len() as f64 / total;
    let total_pnl: f64 = trades.iter().map(|t| t.pnl_lev).sum();
    let avg_pnl = total_pnl / total;

    let gross_profit: f64 = winners.iter().sum();
    let gross_loss: f64 = losers.iter().sum();
    let profit_factor = if gross_loss > 0.0 { gross_profit / gross_loss } else { 999.0 };

    let best = trades.iter().map(|t| t.pnl_lev).fold(f64::NEG_INFINITY, f64::max);
    let worst = trades.iter().map(|t| t.pnl_lev).fold(f64::INFINITY, f64::min);

    // Max drawdown from cumulative PnL
    let mut peak = 0.0_f64;
    let mut max_dd = 0.0_f64;
    let mut cum = 0.0_f64;
    for t in trades {
        cum += t.pnl_lev;
        peak = peak.max(cum);
        max_dd = max_dd.max(peak - cum);
    }

    // Composite score (same as autoresearch)
    let score = 0.30 * win_rate
        + 0.25 * (avg_pnl / 2.0).min(1.0).max(0.0)
        + 0.25 * (profit_factor / 3.0).min(1.0).max(0.0)
        + 0.20 * (1.0 - max_dd / 10.0).max(0.0);

    serde_json::json!({
        "total_trades": trades.len(),
        "win_rate": (win_rate * 10000.0).round() / 10000.0,
        "total_pnl": (total_pnl * 100.0).round() / 100.0,
        "avg_pnl": (avg_pnl * 100.0).round() / 100.0,
        "profit_factor": (profit_factor * 100.0).round() / 100.0,
        "max_drawdown": (max_dd * 100.0).round() / 100.0,
        "best_trade": (best * 100.0).round() / 100.0,
        "worst_trade": (worst * 100.0).round() / 100.0,
        "score": (score * 100000.0).round() / 100000.0
    })
}
