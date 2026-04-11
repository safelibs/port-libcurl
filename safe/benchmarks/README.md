# Benchmarks

This directory defines the checked-in performance contract for the local loopback benchmark matrix.

- `scenarios.json` records the workload definitions and matrix membership.
- `thresholds.json` records the allowed median wall-clock regression budgets.
- `harness/` contains the small C benchmark consumers that can be compiled against either the reference or safe libcurl implementation without changing the workload definition.

Use `bash safe/scripts/benchmark-local.sh --implementation <original|safe> --flavor <openssl|gnutls> --matrix core --output-dir <dir>` to run the tracked matrix and emit per-scenario JSON results.

The thresholds are expected to evolve with the implementation, but changes should stay tied to an explicit workload definition and documented methodology in `safe/docs/performance.md`.
