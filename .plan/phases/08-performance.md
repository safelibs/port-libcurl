# Phase Name
Performance Baseline, Benchmark Harness, and Regression Tuning

## Implement Phase ID
`impl-performance`

## Preexisting Inputs
- `original/lib/speedcheck.c`
- `original/tests/data/test1606`
- `original/src/tool_operate.c`
- `original/tests/http/clients/h2-download.c`
- `original/tests/http/clients/h2-pausing.c`
- `original/tests/http/clients/tls-session-reuse.c`
- `safe/scripts/build-compat-consumers.sh`
- `safe/scripts/run-http-client-tests.sh`
- `safe/scripts/http-fixtures.sh`
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/src/lib.rs`
- `safe/src/abi/generated.rs`
- `safe/include/curl/*.h`
- `safe/metadata/abi-manifest.json`
- `safe/metadata/test-manifest.json`
- `safe/metadata/cve-manifest.json`
- `safe/abi/libcurl-openssl.map`
- `safe/abi/libcurl-gnutls.map`
- `safe/scripts/generate-manifests.py`
- `safe/scripts/generate-bindings.py`
- `safe/scripts/verify-manifests.py`
- `safe/scripts/verify-public-headers.sh`
- `safe/scripts/verify-export-names.sh`
- `safe/scripts/verify-symbol-versions.sh`
- `safe/scripts/build-reference-curl.sh`
- `safe/debian/control`
- `safe/debian/changelog`
- `safe/debian/copyright`
- `safe/debian/README.*`
- `safe/debian/rules`
- `safe/debian/source/format`
- `safe/debian/*.install`
- `safe/debian/*.links`
- `safe/debian/*.docs`
- `safe/debian/*.examples`
- `safe/debian/*.lintian-overrides`
- `safe/debian/*.manpages`
- `safe/debian/*.symbols`
- `safe/debian/patches/series`
- `safe/c_shim/forwarders.c`
- `safe/src/alloc.rs`
- `safe/src/global.rs`
- `safe/src/version.rs`
- `safe/src/slist.rs`
- `safe/src/mime.rs`
- `safe/src/form.rs`
- `safe/src/urlapi.rs`
- `safe/src/share.rs`
- `safe/src/easy/mod.rs`
- `safe/src/easy/options.rs`
- `safe/src/easy/handle.rs`
- `safe/src/abi/public_types.rs`
- `safe/src/abi/easy.rs`
- `safe/src/abi/share.rs`
- `safe/src/abi/url.rs`
- `safe/c_shim/variadic.c`
- `safe/c_shim/mprintf.c`
- `safe/tests/public_abi.rs`
- `safe/tests/abi_layout.rs`
- `safe/tests/smoke/public_api_smoke.c`
- `safe/scripts/run-public-abi-smoke.sh`
- `safe/scripts/verify-abi-manifest.sh`
- `safe/scripts/vendor-compat-assets.sh`
- `safe/vendor/upstream/manifest.json`
- `safe/vendor/upstream/src/*`
- `safe/vendor/upstream/tests/*`
- `safe/vendor/upstream/lib/*`
- `safe/vendor/upstream/.pc/90_gnutls.patch/*`
- `safe/vendor/upstream/debian/tests/LDAP-bindata.c`
- `safe/compat/CMakeLists.txt`
- `safe/compat/generated-sources.cmake`
- `safe/scripts/export-tracked-tree.sh`
- `safe/scripts/run-curated-libtests.sh`
- `safe/scripts/run-link-compat.sh`
- `safe/scripts/run-upstream-tests.sh`
- `safe/scripts/run-curl-tool-smoke.sh`
- `safe/scripts/run-ldap-devpkg-test.sh`
- `safe/scripts/http-fixture.py`
- `safe/src/easy/perform.rs`
- `safe/src/multi/mod.rs`
- `safe/src/multi/state.rs`
- `safe/src/multi/poll.rs`
- `safe/src/conn/mod.rs`
- `safe/src/conn/cache.rs`
- `safe/src/conn/filter.rs`
- `safe/src/dns/mod.rs`
- `safe/src/transfer/mod.rs`
- `safe/src/abi/multi.rs`
- `safe/src/abi/connect_only.rs`
- `safe/src/http/mod.rs`
- `safe/src/http/request.rs`
- `safe/src/http/response.rs`
- `safe/src/http/proxy.rs`
- `safe/src/http/auth.rs`
- `safe/src/http/cookies.rs`
- `safe/src/http/hsts.rs`
- `safe/src/http/altsvc.rs`
- `safe/src/http/headers_api.rs`
- `safe/src/ws.rs`
- `safe/src/rand.rs`
- `safe/tests/cve_regressions.rs`
- `safe/tests/cve_cases/`
- `safe/metadata/cve-to-test.json`
- `safe/scripts/verify-cve-coverage.py`
- `safe/src/tls/mod.rs`
- `safe/src/tls/openssl.rs`
- `safe/src/tls/gnutls.rs`
- `safe/src/tls/certinfo.rs`
- `safe/src/vquic/mod.rs`
- `safe/src/ssh/mod.rs`
- `safe/src/protocols/mod.rs`
- `safe/src/protocols/file.rs`
- `safe/src/protocols/ftp.rs`
- `safe/src/protocols/imap.rs`
- `safe/src/protocols/pop3.rs`
- `safe/src/protocols/smtp.rs`
- `safe/src/protocols/ldap.rs`
- `safe/src/protocols/smb.rs`
- `safe/src/protocols/telnet.rs`
- `safe/src/protocols/tftp.rs`
- `safe/src/protocols/dict.rs`
- `safe/src/protocols/gopher.rs`
- `safe/src/protocols/rtsp.rs`
- `safe/src/protocols/mqtt.rs`
- `safe/src/doh.rs`
- `safe/src/idn.rs`
- `safe/tests/unit_port.rs`
- `safe/tests/unit_port_cases/`
- `safe/tests/port-map.json`
- `safe/compat/link-manifest.json`

## New Outputs
- `safe/benchmarks/README.md`
- `safe/benchmarks/scenarios.json`
- `safe/benchmarks/thresholds.json`
- `safe/benchmarks/harness/easy_loop.c`
- `safe/benchmarks/harness/multi_parallel.c`
- `safe/scripts/benchmark-local.sh`
- `safe/scripts/compare-benchmarks.py`
- `safe/docs/performance.md`

## File Changes
- Add a deterministic loopback benchmark harness that can run against either the original or safe implementation without changing the workload definition.
- Add explicit scenario and threshold files so the performance requirement is measurable and version-controlled.
- Tune the Rust implementation where the benchmark matrix shows material regressions.

## Implementation Details
- `safe/scripts/benchmark-local.sh` should use the shared local-fixture helpers from phase 3 so the original and safe implementations run against the same loopback HTTP/HTTPS setup.
- `safe/benchmarks/scenarios.json` should define at least these scenarios:
  - `easy-http1-reuse`
  - `multi-http1-parallel`
  - `h2-download-multiplex`
  - `tls-session-reuse`
- The benchmark harness should write structured JSON output per scenario, including at minimum median wall-clock time, run count, bytes transferred, and implementation/flavor metadata.
- `safe/benchmarks/thresholds.json` should record explicit maximum median regressions for each scenario. Reasonable initial budgets are:
  - `easy-http1-reuse`: 15%
  - `multi-http1-parallel`: 15%
  - `h2-download-multiplex`: 20%
  - `tls-session-reuse`: 15%
- `safe/scripts/compare-benchmarks.py` should fail if any required scenario is missing or exceeds its budget.
- `safe/docs/performance.md` should document methodology, local-fixture assumptions, scenario definitions, and the rule that performance tuning must not weaken compatibility or security.

## Verification Phases
### `check-performance-budgets`
- Type: `check`
- Bounce Target: `impl-performance`
- Purpose: benchmark the original and safe implementations under the same local workloads for both flavors and fail if the safe port exceeds the recorded regression budgets.
- Commands it should run:
```bash
rm -rf safe/.bench-output
mkdir -p safe/.bench-output
bash safe/scripts/benchmark-local.sh --implementation original --flavor openssl --matrix core --output-dir safe/.bench-output/original/openssl
bash safe/scripts/benchmark-local.sh --implementation safe --flavor openssl --matrix core --output-dir safe/.bench-output/safe/openssl
python3 safe/scripts/compare-benchmarks.py --baseline safe/.bench-output/original/openssl --candidate safe/.bench-output/safe/openssl --thresholds safe/benchmarks/thresholds.json
bash safe/scripts/benchmark-local.sh --implementation original --flavor gnutls --matrix core --output-dir safe/.bench-output/original/gnutls
bash safe/scripts/benchmark-local.sh --implementation safe --flavor gnutls --matrix core --output-dir safe/.bench-output/safe/gnutls
python3 safe/scripts/compare-benchmarks.py --baseline safe/.bench-output/original/gnutls --candidate safe/.bench-output/safe/gnutls --thresholds safe/benchmarks/thresholds.json
```

## Success Criteria
- Every listed `Preexisting Input` is consumed as an existing artifact rather than rediscovered, regenerated, or refetched.
- Every listed `New Output` for this implement phase exists and is ready for downstream phases in the linear workflow.
- The verifier phase(s) `check-performance-budgets` pass exactly as written for `impl-performance`.

## Git Commit Requirement
The implementer must commit this phase's work to git before yielding. Ignored-only or untracked-only outputs are not acceptable.
