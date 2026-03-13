"""Experiment history logger — appends to experiments.jsonl."""

import json
import os
from datetime import datetime, timezone

EXPERIMENTS_FILE = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "experiments.jsonl"
)


def log_experiment(experiment: dict) -> None:
    """Append an experiment record to experiments.jsonl."""
    experiment.setdefault("timestamp", datetime.now(timezone.utc).isoformat())
    with open(EXPERIMENTS_FILE, "a") as f:
        f.write(json.dumps(experiment) + "\n")


def read_experiments() -> list[dict]:
    """Read all experiment records."""
    if not os.path.exists(EXPERIMENTS_FILE):
        return []
    experiments = []
    with open(EXPERIMENTS_FILE) as f:
        for line in f:
            line = line.strip()
            if line:
                try:
                    experiments.append(json.loads(line))
                except json.JSONDecodeError:
                    continue
    return experiments


def get_next_experiment_id() -> int:
    """Get next experiment ID (sequential)."""
    experiments = read_experiments()
    if not experiments:
        return 1
    return max(e.get("id", 0) for e in experiments) + 1
