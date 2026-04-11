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
- `multi-http1-parallel`: parallel HTTP/1.1 downloads through the multi API
- `h2-download-multiplex`: multiplexed HTTP/2 downloads over the local TLS proxy
- `tls-session-reuse`: fresh sequential HTTPS requests that force new connections while reusing the TLS session cache

The workload definitions live in `safe/benchmarks/scenarios.json`, and the allowed median regression budgets live in `safe/benchmarks/thresholds.json`. Those files are version-controlled so any benchmark change is explicit and reviewable.

The current thresholds are intentionally uneven. The HTTP/1.1 reuse and HTTP/2 budgets stay tight because the safe path is already close to the reference on those workloads. The `tls-session-reuse` budget is temporarily wider because the safe easy/TLS path still carries measurable per-request overhead under repeated fresh OpenSSL connects even after enabling `TCP_NODELAY` and trimming hot-path bookkeeping. That scenario remains tracked so future phases can ratchet the budget back down with concrete data.

## Guardrails

Performance tuning is allowed only when it preserves the public compatibility contract and the security posture of the port. Do not trade away ABI behavior, certificate validation semantics, protocol correctness, or CVE coverage to make a benchmark faster.
