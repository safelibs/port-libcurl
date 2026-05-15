### 8. Performance Baseline, Benchmark Harness, and Regression Tuning

**Phase Name:** Performance Baseline, Benchmark Harness, and Regression Tuning

**Implement Phase ID:** `impl-performance`

**Verification Phases:**

- `check-performance`
  - Type: `check`
  - Fixed `bounce_target`: `impl-performance`
  - Purpose: compare safe versus original performance for core scenarios and enforce threshold policy without weakening compatibility or security.
  - Commands:
    ```bash
    command -v nghttpx >/dev/null
    perf_root="$(mktemp -d)"
    for flavor in openssl gnutls; do
      bash safe/scripts/benchmark-local.sh --implementation original --flavor "$flavor" --matrix core --output-dir "$perf_root/original-$flavor"
      bash safe/scripts/benchmark-local.sh --implementation safe --flavor "$flavor" --matrix core --output-dir "$perf_root/safe-$flavor"
      python3 safe/scripts/compare-benchmarks.py \
        --baseline "$perf_root/original-$flavor" \
        --candidate "$perf_root/safe-$flavor" \
        --thresholds safe/benchmarks/thresholds.json \
        --output "$perf_root/comparison-$flavor.json"
    done
    cat "$perf_root"/comparison-*.json
    ```

**Preexisting Inputs:**

- `safe/tests/unit_port.rs`, `safe/tests/unit_port_cases/`, `safe/tests/port-map.json`, `safe/compat/link-manifest.json`, `safe/metadata/test-manifest.json`, `safe/scripts/build-compat-consumers.sh`, and `safe/scripts/run-link-compat.sh`.
- `safe/benchmarks/scenarios.json`, `safe/benchmarks/thresholds.json`, `safe/benchmarks/harness/easy_loop.c`, `safe/benchmarks/harness/multi_parallel.c`, `safe/scripts/benchmark-local.sh`, `safe/scripts/compare-benchmarks.py`.

**New Outputs:**

- Updated benchmark harness or thresholds only if the current harness is wrong or unstable.
- `safe/docs/performance.md` documenting baseline scenarios, current results, and justified optimizations.
- Performance fixes in transfer, HTTP, TLS, or multi modules.

**File Changes:**

- `safe/benchmarks/scenarios.json`: maintain the core scenarios `easy-http1-reuse`, `multi-http1-parallel`, `h2-download-multiplex`, and `tls-session-reuse`.
- `safe/benchmarks/thresholds.json`: keep threshold values explicit and conservative.
- `safe/scripts/benchmark-local.sh`: ensure original and safe builds use the same local fixtures and link behavior.
- `safe/scripts/compare-benchmarks.py`: produce deterministic pass/fail JSON.
- `safe/docs/performance.md`: record methodology and tradeoffs.

**Implementation Details:**

- Performance work must not remove security checks or broaden connection reuse keys.
- Optimize allocation churn, header parsing, callback buffering, connection pooling, and HTTP/2 scheduling only after compatibility tests are green.
- Implementer must commit this phase's work before yielding.

**Verification:**

- `check-performance` must pass. If a threshold fails, document the exact technical reason and fix it in the same phase before yielding.

**Success Criteria:**

- Every item listed under `New Outputs` is present, updated, or explicitly left unchanged because it already satisfied the plan.
- Every required `File Changes` and `Implementation Details` invariant for this phase is satisfied.
- Every verifier listed under `Verification Phases` passes exactly as written and bounces only to this phase on failure.
- All listed `Preexisting Inputs` are consumed in place; existing artifacts are not rediscovered, refetched, or regenerated from untracked sources unless this phase explicitly updates a derived safe artifact from them.

**Git Commit Requirement:**

- The implementer must commit this phase's work to git before yielding.
