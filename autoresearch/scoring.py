"""Composite score calculation for auto-research experiments."""

def compute_score(trades: list[dict]) -> dict:
    """Compute composite score from a list of trades.

    Score = 0.30 * win_rate
          + 0.25 * normalized(avg_pnl)
          + 0.25 * normalized(profit_factor)
          + 0.20 * (1 - normalized(max_drawdown))
    """
    if not trades:
        return {
            "score": 0.0,
            "win_rate": 0.0,
            "avg_pnl": 0.0,
            "profit_factor": 0.0,
            "max_drawdown": 0.0,
            "total_trades": 0,
        }

    wins = [t for t in trades if t.get("pnl_lev", 0) > 0]
    losses = [t for t in trades if t.get("pnl_lev", 0) <= 0]

    win_rate = len(wins) / len(trades)
    avg_pnl = sum(t.get("pnl_lev", 0) for t in trades) / len(trades)

    gross_profit = sum(t.get("pnl_lev", 0) for t in wins) if wins else 0
    gross_loss = abs(sum(t.get("pnl_lev", 0) for t in losses)) if losses else 0
    profit_factor = gross_profit / gross_loss if gross_loss > 0 else 10.0

    # Max drawdown from cumulative PnL
    cumulative = 0.0
    peak = 0.0
    max_dd = 0.0
    for t in trades:
        cumulative += t.get("pnl_lev", 0)
        if cumulative > peak:
            peak = cumulative
        dd = peak - cumulative
        if dd > max_dd:
            max_dd = dd

    # Normalize components to [0, 1] range
    norm_avg_pnl = _sigmoid(avg_pnl, scale=2.0)
    norm_pf = min(profit_factor / 5.0, 1.0)
    norm_dd = min(max_dd / 20.0, 1.0)  # 20% max DD = 1.0

    score = (
        0.30 * win_rate
        + 0.25 * norm_avg_pnl
        + 0.25 * norm_pf
        + 0.20 * (1.0 - norm_dd)
    )

    return {
        "score": round(score, 4),
        "win_rate": round(win_rate, 4),
        "avg_pnl": round(avg_pnl, 4),
        "profit_factor": round(profit_factor, 4),
        "max_drawdown": round(max_dd, 4),
        "total_trades": len(trades),
    }


def _sigmoid(x: float, scale: float = 1.0) -> float:
    """Map x to [0, 1] via sigmoid centered at 0."""
    import math
    return 1.0 / (1.0 + math.exp(-x / scale))
