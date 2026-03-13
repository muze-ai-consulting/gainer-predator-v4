"""Auto-Research Engine — Karpathy-style parameter optimization loop.

Usage:
    python autoresearch/engine.py [--bot-url http://localhost:3001] [--min-trades 10] [--timeout-hours 4]
"""

import argparse
import json
import time
import sys
import os
from datetime import datetime, timezone

import requests

# Add parent dir to path for imports
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from autoresearch.scoring import compute_score
from autoresearch.hypothesis import generate_hypothesis
from autoresearch.history import log_experiment, get_next_experiment_id


def fetch_status(bot_url: str) -> dict:
    """Get current bot status and config."""
    r = requests.get(f"{bot_url}/api/status", timeout=10)
    r.raise_for_status()
    return r.json()


def fetch_trades(bot_url: str, since_experiment_id: int | None = None) -> list[dict]:
    """Get trades, optionally filtered by experiment_id."""
    r = requests.get(f"{bot_url}/api/trades", timeout=10)
    r.raise_for_status()
    trades = r.json()
    if since_experiment_id is not None:
        trades = [t for t in trades if t.get("experiment_id") == since_experiment_id]
    return trades


def fetch_metrics(bot_url: str) -> dict:
    """Get current metrics."""
    r = requests.get(f"{bot_url}/api/metrics", timeout=10)
    r.raise_for_status()
    return r.json()


def apply_experiment(bot_url: str, params: dict) -> dict:
    """Apply new parameters to the bot via POST /api/experiment."""
    r = requests.post(f"{bot_url}/api/experiment", json=params, timeout=10)
    r.raise_for_status()
    return r.json()


def run_loop(bot_url: str, min_trades: int, timeout_hours: float):
    """Main auto-research loop."""
    print(f"[AutoResearch] Starting — bot at {bot_url}")
    print(f"[AutoResearch] Min trades per experiment: {min_trades}")
    print(f"[AutoResearch] Timeout per experiment: {timeout_hours}h")

    # Get baseline
    try:
        status = fetch_status(bot_url)
        baseline_config = status.get("config", {})
        print(f"[AutoResearch] Baseline config: {json.dumps(baseline_config, indent=2)}")
    except Exception as e:
        print(f"[AutoResearch] ERROR: Cannot connect to bot: {e}")
        return

    # Compute baseline score from existing trades
    try:
        all_trades = fetch_trades(bot_url)
        baseline_metrics = compute_score(all_trades)
        baseline_score = baseline_metrics["score"]
        print(f"[AutoResearch] Baseline score: {baseline_score:.4f} ({len(all_trades)} trades)")
    except Exception as e:
        print(f"[AutoResearch] WARNING: Cannot compute baseline: {e}")
        baseline_score = 0.0

    experiment_count = 0

    while True:
        experiment_count += 1
        exp_id = get_next_experiment_id()

        print(f"\n{'='*60}")
        print(f"[Experiment #{exp_id}]")

        # Generate hypothesis
        hypothesis = generate_hypothesis(baseline_config)
        print(f"[Hypothesis] {hypothesis['description']}")

        # Apply new params
        new_params = hypothesis["new_params"]
        new_params["experiment_id"] = exp_id

        try:
            apply_experiment(bot_url, new_params)
            print(f"[Applied] {json.dumps(new_params)}")
        except Exception as e:
            print(f"[ERROR] Failed to apply experiment: {e}")
            time.sleep(60)
            continue

        # Wait for trades
        start_time = time.time()
        timeout_secs = timeout_hours * 3600
        trades_collected = 0

        print(f"[Waiting] Collecting {min_trades} trades (timeout: {timeout_hours}h)...")

        while True:
            elapsed = time.time() - start_time
            if elapsed > timeout_secs:
                print(f"[Timeout] {elapsed/3600:.1f}h elapsed, got {trades_collected} trades")
                break

            try:
                exp_trades = fetch_trades(bot_url, since_experiment_id=exp_id)
                trades_collected = len(exp_trades)

                if trades_collected >= min_trades:
                    print(f"[Collected] {trades_collected} trades in {elapsed/60:.0f}m")
                    break
            except Exception:
                pass

            # Poll every 30 seconds
            remaining = timeout_secs - elapsed
            wait = min(30, remaining)
            if wait > 0:
                time.sleep(wait)

        # Evaluate
        try:
            exp_trades = fetch_trades(bot_url, since_experiment_id=exp_id)
            exp_metrics = compute_score(exp_trades)
            new_score = exp_metrics["score"]
        except Exception as e:
            print(f"[ERROR] Cannot compute score: {e}")
            new_score = 0.0
            exp_metrics = {}

        # Decision: KEEP or DISCARD
        improved = new_score > baseline_score
        decision = "KEEP" if improved else "DISCARD"

        print(f"[Score] Baseline: {baseline_score:.4f} → New: {new_score:.4f}")
        print(f"[Decision] {'✅ KEEP' if improved else '❌ DISCARD'}")

        # Log experiment
        experiment_record = {
            "id": exp_id,
            "description": hypothesis["description"],
            "param": hypothesis["param"],
            "old_value": hypothesis["old_value"],
            "new_value": hypothesis["new_value"],
            "status": decision,
            "baseline_score": baseline_score,
            "new_score": new_score,
            "trades_collected": len(exp_trades) if exp_trades else 0,
            "metrics": exp_metrics,
            "timestamp": datetime.now(timezone.utc).isoformat(),
        }
        log_experiment(experiment_record)

        if improved:
            # Update baseline
            baseline_score = new_score
            baseline_config[hypothesis["param"]] = hypothesis["new_value"]
            print(f"[Baseline Updated] score={baseline_score:.4f}")
        else:
            # Revert to baseline params
            revert_params = {hypothesis["param"]: hypothesis["old_value"]}
            try:
                apply_experiment(bot_url, revert_params)
                print(f"[Reverted] {hypothesis['param']} back to {hypothesis['old_value']}")
            except Exception as e:
                print(f"[ERROR] Failed to revert: {e}")

        # Brief pause between experiments
        print(f"[AutoResearch] Sleeping 60s before next experiment...")
        time.sleep(60)


def main():
    parser = argparse.ArgumentParser(description="Gainer Predator Auto-Research Engine")
    parser.add_argument("--bot-url", default="http://localhost:3001", help="Bot API URL")
    parser.add_argument("--min-trades", type=int, default=10, help="Min trades per experiment")
    parser.add_argument("--timeout-hours", type=float, default=4.0, help="Max hours per experiment")
    args = parser.parse_args()

    run_loop(args.bot_url, args.min_trades, args.timeout_hours)


if __name__ == "__main__":
    main()
