# Performance

The performance baseline for this port is intentionally narrow and reproducible: it runs a fixed loopback matrix against both the reference and safe implementations, for both supported TLS flavors, under the same local HTTP and HTTPS fixtures.

## Methodology

`safe/scripts/benchmark-local.sh` is the entrypoint used by the verifier. It:

- ensures the reference library exists through `safe/scripts/build-reference-curl.sh`
- stages the safe library through `safe/scripts/build-compat-consumers.sh` when benchmarking the safe implementation
- provisions the shared loopback fixture with `safe/scripts/http-fixtures.sh`
- places `nghttpx` in front of that fixture for the HTTPS and HTTP/2 scenarios so both implementations see the same TLS and ALPN surface
- compiles the checked-in C harnesses under `safe/benchmarks/harness/` against the selected implementation without changing the workload definition in `safe/benchmarks/scenarios.json`

Each scenario writes one JSON file with the median wall-clock time, sample count, bytes transferred, and implementation/flavor metadata. `safe/scripts/compare-benchmarks.py` compares those results against `safe/benchmarks/thresholds.json` and fails if any required scenario is missing or regresses beyond budget.

## Scenarios

The `core` matrix currently tracks four scenarios:

- `easy-http1-reuse`: sequential HTTP/1.1 GETs over a reused easy handle
- `multi-http1-parallel`: a single wave of parallel HTTP/1.1 downloads through the multi API over a benchmark-specific 1 MiB loopback asset
- `h2-download-multiplex`: multiplexed HTTP/2 downloads over the local TLS proxy against a benchmark-specific 64 MiB loopback asset
- `tls-session-reuse`: fresh sequential HTTPS requests that force new connections while reusing the TLS session cache

The workload definitions live in `safe/benchmarks/scenarios.json`, and the allowed median regression budgets live in `safe/benchmarks/thresholds.json`. Those files are version-controlled so any benchmark change is explicit and reviewable.

The current thresholds stay aligned with the checked-in matrix: 15% for the HTTP/1.1 easy, HTTP/1.1 multi, and TLS session reuse scenarios, and 20% for the HTTP/2 multiplex workload. The benchmark runner adds dedicated loopback payloads for the multi and H2 scenarios so those measurements reflect sustained transfer behavior instead of scheduler granularity or incidental connection-churn differences. The multi benchmark still uses one fully parallel wave rather than repeated connection churn so it measures the multi API's steady-state loopback download behavior instead of exercising an untracked connection-pool policy difference.

## Guardrails

Performance tuning is allowed only when it preserves the public compatibility contract and the security posture of the port. Do not trade away ABI behavior, certificate validation semantics, protocol correctness, or CVE coverage to make a benchmark faster.
