# Gainer Predator V4

**Reactive momentum trading system for Binance Futures** — detects relative volume spikes + price jumps on 1h candles across the top 50 USDT-M perpetuals, executes with precision trailing stops, and auto-optimizes its own parameters.

[![Rust](https://img.shields.io/badge/Bot-Rust-orange?logo=rust)](src/)
[![React](https://img.shields.io/badge/Dashboard-React-blue?logo=react)](dashboard/)
[![Python](https://img.shields.io/badge/AutoResearch-Python-green?logo=python)](autoresearch/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                   Gainer Predator V4                │
├──────────────┬──────────────┬───────────────────────┤
│  Rust Bot    │  Dashboard   │  Auto-Research        │
│  (Scanner +  │  (Vite +     │  (Python              │
│   Executor)  │   React)     │   Orchestrator)       │
├──────────────┼──────────────┼───────────────────────┤
│ • Self-scan  │ • Live       │ • Karpathy-style      │
│   top 50     │   trade log  │   experiment loop     │
│   perps      │ • Equity     │ • Composite scoring   │
│ • RVol +     │   curve      │ • Auto keep/discard   │
│   jump       │ • Metrics    │ • Hypothesis          │
│   detection  │   grid       │   generation          │
│ • Trailing   │ • Hour       │                       │
│   stop from  │   heatmap    │                       │
│   HWM        │ • Experiment │                       │
│ • SSE events │   panel      │                       │
└──────┬───────┴──────┬───────┴───────────┬───────────┘
       │    HTTP :3001 │                   │
       │    SSE stream │   POST /api/      │
       └──────────────┘   experiment       │
              ▲                            │
              └────────────────────────────┘
```

## Strategy

Gainer Predator is **reactive, not predictive**. It rides momentum that already started:

| Parameter | Value | Description |
|-----------|-------|-------------|
| **Timeframe** | 1h | Validated against 5m/15m/30m — 1h wins decisively |
| **RVol Threshold** | ≥ 2.0x | Relative volume vs 20-period SMA |
| **Price Jump** | 0.5% – 15% | Hourly candle body change |
| **Good Hours (UTC)** | 1,2,5,6,9,10,11,13,14,21 | Filtered by historical performance |
| **Trailing Stop** | 0.5% from HWM | High Water Mark tracking via BookTicker WebSocket (50ms) |
| **Hard Stop Loss** | 1.5% | Emergency exit |
| **Max Hold** | 6 hours | Time-based exit |
| **Max Positions** | 3 | Concurrent trades |
| **Leverage** | 10x | Cross margin |
| **Position Size** | 20% of balance | Per trade |

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (1.75+)
- [Node.js](https://nodejs.org/) (18+)
- [Python](https://python.org/) (3.10+)
- Binance Futures account (testnet or mainnet)

### 1. Clone & Configure

```bash
git clone https://github.com/muze-ai-consulting/gainer-predator-v4.git
cd gainer-predator-v4
cp .env.example .env
```

Edit `.env` with your Binance API keys:

```env
# Testnet (recommended to start)
BINANCE_API_KEY=your_testnet_key
BINANCE_SECRET_KEY=your_testnet_secret
USE_TESTNET=true

# Strategy params (defaults are battle-tested)
RVOL_THRESHOLD=2.0
JUMP_MIN_PCT=0.5
JUMP_MAX_PCT=15.0
SCAN_INTERVAL_SECS=60
UNIVERSE_SIZE=50
```

### 2. Run the Bot

```bash
cargo run --release --bin trz_bot
```

The bot will:
- Connect to Binance Futures WebSocket
- Pre-heat 20 candles for volume baseline
- Start scanning every 60 seconds
- Serve HTTP API on port `3001`
- Stream events via SSE at `/api/stream`

### 3. Run the Dashboard

```bash
cd dashboard
npm install
npm run dev
```

Open [http://localhost:5173](http://localhost:5173) — you'll see live scan events immediately.

### 4. Run Auto-Research (Optional)

```bash
pip install requests
python -m autoresearch.engine
```

The research engine will:
1. Generate a parameter hypothesis
2. Push it to the bot via `POST /api/experiment`
3. Wait for N trades to complete
4. Compute a composite score
5. Keep or discard the change
6. Repeat

## API Reference

The bot exposes a REST + SSE API on port `3001`:

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/status` | Current positions, balance, uptime |
| `GET` | `/api/trades` | All completed trades (JSONL) |
| `GET` | `/api/metrics` | Win rate, PF, drawdown, score |
| `GET` | `/api/experiments` | Experiment history |
| `POST` | `/api/experiment` | Push new parameters (hot-reload) |
| `GET` | `/api/stream` | SSE stream of real-time events |

### SSE Event Types

```
scan_result    — Each scan cycle with candidates found
signal_detected — RVol + jump threshold met
trade_opened   — Position entered
trade_closed   — Position exited with PnL
```

### Hot-Reload Parameters

```bash
curl -X POST http://localhost:3001/api/experiment \
  -H "Content-Type: application/json" \
  -d '{
    "rvol_threshold": 2.5,
    "apex_retracement": 0.006,
    "stop_loss_pct": 0.02
  }'
```

Only the fields you send are updated — everything else stays the same.

## Project Structure

```
├── src/
│   ├── main.rs              # Entry point — spawns scanner + HTTP server
│   ├── scanner.rs           # Self-scanning module (top 50 perps, 1h candles)
│   ├── binance.rs           # Binance client — orders, WebSocket, trailing stop
│   ├── runtime_config.rs    # Hot-reloadable parameters (Arc<RwLock>)
│   ├── http_api.rs          # Axum HTTP server + SSE streaming
│   ├── trade_logger.rs      # JSONL trade logging + metrics computation
│   ├── config.rs            # Static config from .env
│   ├── models.rs            # Data structures
│   └── parser.rs            # Signal parsing utilities
├── dashboard/
│   ├── src/
│   │   ├── components/
│   │   │   ├── Header.tsx           # Status bar with connection indicator
│   │   │   ├── MetricsGrid.tsx      # 3x3 KPI cards (win rate, PnL, PF, etc.)
│   │   │   ├── LiveTradeLog.tsx     # Scrolling event feed (SSE)
│   │   │   ├── ActivePositions.tsx  # Current open positions with live PnL
│   │   │   ├── EquityCurve.tsx      # Cumulative PnL area chart
│   │   │   ├── TradesTable.tsx      # Sortable trade history table
│   │   │   ├── HourHeatmap.tsx      # UTC hour performance heatmap
│   │   │   └── ExperimentPanel.tsx  # Auto-research experiments + score chart
│   │   ├── hooks/
│   │   │   ├── useSSE.ts            # SSE connection hook
│   │   │   └── usePolling.ts        # Polling hook for REST endpoints
│   │   └── lib/
│   │       ├── api.ts               # API client functions
│   │       └── utils.ts             # Formatting utilities
│   └── package.json
├── autoresearch/
│   ├── engine.py            # Main orchestrator loop
│   ├── scoring.py           # Composite score computation
│   ├── hypothesis.py        # Parameter hypothesis generation
│   └── history.py           # Experiment history persistence
├── Cargo.toml
├── Dockerfile
└── .env.example
```

## Dashboard

The dashboard uses a dark terminal aesthetic inspired by trading terminals:

- **Tech stack**: Vite + React + TypeScript + Tailwind CSS + Framer Motion
- **Real-time**: SSE for instant trade events, polling for metrics
- **Responsive**: Works on desktop and tablet

### Components

| Component | Data Source | Update Frequency |
|-----------|-----------|-----------------|
| Live Trade Log | SSE `/api/stream` | Real-time |
| Metrics Grid | `GET /api/metrics` | Every 5s |
| Active Positions | `GET /api/status` | Every 3s |
| Equity Curve | `GET /api/trades` | Every 10s |
| Trades Table | `GET /api/trades` | Every 10s |
| Hour Heatmap | `GET /api/trades` | Every 10s |
| Experiment Panel | `GET /api/experiments` | Every 10s |

## Auto-Research Engine

Inspired by [Karpathy's approach](https://karpathy.ai/) to automated experimentation:

1. **Hypothesis**: Generates a parameter tweak (e.g., "increase RVol threshold from 2.0 to 2.5")
2. **Apply**: Pushes to bot via HTTP API (hot-reload, no restart needed)
3. **Observe**: Waits for N trades to complete
4. **Score**: Computes composite metric:
   ```
   Score = 0.30 × win_rate
         + 0.25 × normalized(avg_pnl)
         + 0.25 × normalized(profit_factor)
         + 0.20 × (1 - normalized(max_drawdown))
   ```
5. **Decide**: Keep if score improved, discard and rollback if not
6. **Repeat**: Next hypothesis

## Configuration

All strategy parameters can be set via environment variables or hot-reloaded at runtime:

| Env Variable | Default | Description |
|-------------|---------|-------------|
| `RVOL_THRESHOLD` | 2.0 | Minimum relative volume multiplier |
| `JUMP_MIN_PCT` | 0.5 | Minimum hourly price jump (%) |
| `JUMP_MAX_PCT` | 15.0 | Maximum hourly price jump (%) |
| `MAX_POSITIONS` | 3 | Max concurrent positions |
| `POSITION_SIZE_PCT` | 20.0 | Position size as % of balance |
| `APEX_RETRACEMENT` | 0.005 | Trailing stop distance (0.5%) |
| `STOP_LOSS_PCT` | 0.015 | Hard stop loss (1.5%) |
| `MAX_HOLD_SECS` | 21600 | Max hold time (6 hours) |
| `DEFAULT_LEVERAGE` | 10 | Leverage multiplier |
| `SCAN_INTERVAL_SECS` | 60 | Scan frequency |
| `UNIVERSE_SIZE` | 50 | Number of top pairs to scan |
| `USE_TESTNET` | true | Use Binance testnet |
| `HTTP_PORT` | 3001 | HTTP API port |

## Testnet First

**Always start on testnet.** The system is designed to run on Binance Futures Testnet first:

1. Get testnet keys at [testnet.binancefuture.com](https://testnet.binancefuture.com/)
2. Set `USE_TESTNET=true` in `.env`
3. Run, observe, tune with auto-research
4. When confident, switch to mainnet with real capital

## License

MIT

## Disclaimer

This software is for educational and research purposes. Trading cryptocurrencies involves substantial risk of loss. Past performance (including backtest results) does not guarantee future results. Use at your own risk. Always start with testnet and small amounts.
