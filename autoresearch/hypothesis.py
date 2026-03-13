"""Parameter space and mutation logic for auto-research."""

import random

# Each parameter: (name, ordered list of allowed values)
PARAM_SPACE = {
    "rvol_threshold": [1.5, 2.0, 2.5, 3.0, 3.5],
    "jump_min_pct": [0.3, 0.5, 0.8, 1.0],
    "jump_max_pct": [10.0, 12.0, 15.0, 20.0],
    "apex_retracement": [0.3, 0.5, 0.8, 1.0],
    "stop_loss_pct": [1.0, 1.5, 2.0, 2.5],
    "max_hold_secs": [10800, 21600, 32400, 43200],  # 3h, 6h, 9h, 12h
    "default_leverage": [5, 10, 15, 20],
}


def generate_hypothesis(current_params: dict) -> dict:
    """Generate a new hypothesis by mutating one parameter to an adjacent value.

    Hill climbing strategy: pick a random parameter, move to an adjacent value.
    Returns dict with 'param', 'old_value', 'new_value', 'description', 'new_params'.
    """
    param = random.choice(list(PARAM_SPACE.keys()))
    values = PARAM_SPACE[param]

    current_val = current_params.get(param)

    # Find closest match in allowed values
    if current_val is not None:
        closest_idx = min(range(len(values)), key=lambda i: abs(values[i] - current_val))
    else:
        closest_idx = len(values) // 2

    # Pick adjacent value (up or down)
    candidates = []
    if closest_idx > 0:
        candidates.append(closest_idx - 1)
    if closest_idx < len(values) - 1:
        candidates.append(closest_idx + 1)

    new_idx = random.choice(candidates)
    old_value = values[closest_idx]
    new_value = values[new_idx]

    # Build new params (only changed param)
    new_params = {param: new_value}

    # Human-readable descriptions
    if param == "max_hold_secs":
        old_h = old_value / 3600
        new_h = new_value / 3600
        desc = f"Change max_hold from {old_h:.0f}h to {new_h:.0f}h"
    else:
        desc = f"Change {param} from {old_value} to {new_value}"

    return {
        "param": param,
        "old_value": old_value,
        "new_value": new_value,
        "description": desc,
        "new_params": new_params,
    }
