#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import pathlib
import sys


def load_results(directory: pathlib.Path) -> dict[str, dict]:
    results: dict[str, dict] = {}
    for path in sorted(directory.glob("*.json")):
      data = json.loads(path.read_text(encoding="utf-8"))
      scenario_id = data.get("scenario_id")
      if not scenario_id:
          continue
      results[scenario_id] = data
    return results


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=pathlib.Path, required=True)
    parser.add_argument("--candidate", type=pathlib.Path, required=True)
    parser.add_argument("--thresholds", type=pathlib.Path, required=True)
    args = parser.parse_args()

    thresholds = json.loads(args.thresholds.read_text(encoding="utf-8"))
    budgets = thresholds["scenarios"]
    baseline_results = load_results(args.baseline)
    candidate_results = load_results(args.candidate)

    failures: list[str] = []
    for scenario_id, budget_record in budgets.items():
        baseline = baseline_results.get(scenario_id)
        candidate = candidate_results.get(scenario_id)
        if baseline is None:
            failures.append(f"missing baseline result for {scenario_id}")
            continue
        if candidate is None:
            failures.append(f"missing candidate result for {scenario_id}")
            continue

        baseline_median = float(baseline["median_wall_time_ms"])
        candidate_median = float(candidate["median_wall_time_ms"])
        if baseline_median <= 0.0:
            failures.append(f"baseline median must be positive for {scenario_id}")
            continue

        regression_pct = ((candidate_median - baseline_median) / baseline_median) * 100.0
        budget_pct = float(budget_record["max_median_regression_pct"])
        print(
            f"{scenario_id}: baseline={baseline_median:.3f}ms "
            f"candidate={candidate_median:.3f}ms regression={regression_pct:+.2f}% "
            f"budget={budget_pct:.2f}%"
        )
        if regression_pct > budget_pct:
            failures.append(
                f"{scenario_id} exceeded budget: regression {regression_pct:.2f}% > {budget_pct:.2f}%"
            )

    if failures:
        for failure in failures:
            print(f"error: {failure}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
