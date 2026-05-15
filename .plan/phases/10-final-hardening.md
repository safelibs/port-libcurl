# Phase Name
Remove Temporary C Fallbacks, Audit Unsafe Boundaries, and Run the Full No-Exclusions Matrix

## Implement Phase ID
`impl-final-hardening`

## Preexisting Inputs
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
- `safe/scripts/build-compat-consumers.sh`
- `safe/scripts/run-curated-libtests.sh`
- `safe/scripts/run-link-compat.sh`
- `safe/scripts/run-upstream-tests.sh`
- `safe/scripts/run-curl-tool-smoke.sh`
- `safe/scripts/run-http-client-tests.sh`
- `safe/scripts/run-ldap-devpkg-test.sh`
- `safe/scripts/http-fixtures.sh`
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
- `safe/benchmarks/README.md`
- `safe/benchmarks/scenarios.json`
- `safe/benchmarks/thresholds.json`
- `safe/benchmarks/harness/easy_loop.c`
- `safe/benchmarks/harness/multi_parallel.c`
- `safe/scripts/benchmark-local.sh`
- `safe/scripts/compare-benchmarks.py`
- `safe/docs/performance.md`
- `safe/Cargo.lock`
- `safe/.cargo/config.toml`
- `safe/vendor/cargo/*`
- `safe/debian/patches/*.patch`
- `safe/debian/tests/control`
- `safe/debian/tests/upstream-tests-openssl`
- `safe/debian/tests/upstream-tests-gnutls`
- `safe/debian/tests/curl-ldapi-test`
- `safe/debian/tests/LDAP-bindata.c`
- `safe/libcurl.pc`
- `safe/curl-config`
- `safe/docs/libcurl/libcurl.m4`
- `safe/scripts/verify-autopkgtest-contract.sh`
- `safe/scripts/verify-package-control-contract.py`
- `safe/scripts/verify-package-install-layout.sh`
- `safe/scripts/verify-devpkg-tooling-contract.sh`
- `safe/scripts/run-packaged-autopkgtests.sh`
- `test-original.sh`
- `safe/debian/*`

## New Outputs
- final Rust-owned libcurl core with no direct dependency on the original C library
- `safe/docs/unsafe-audit.md`
- finalized `safe/docs/performance.md`
- finalized `safe/metadata/abi-manifest.json`
- finalized `safe/metadata/test-manifest.json`
- finalized `safe/metadata/cve-manifest.json`
- `safe/scripts/audit-final-build-independence.sh`

## File Changes
- Delete the temporary all-symbol C fallback bridge.
- Tighten or eliminate avoidable `unsafe` blocks.
- Add an explicit unsafe-boundary audit document.
- Delete the transitional reference-build dependency from the final library and package build.
- Fix the remaining compatibility, package, and performance issues found by the full matrix.

## Implementation Details
- By the end of this phase, the only C code that should remain is the unavoidable ABI layer for varargs, the `curl_mprintf*` family, and boundary glue required by libc, TLS/SSH backends, or OS callbacks.
- `safe/docs/unsafe-audit.md` should document every remaining `unsafe` block and classify it as one of: C ABI boundary, libc/socket call, TLS/SSH backend FFI, or raw-pointer adaptation required by a callback signature.
- `run-upstream-tests.sh --require-all-runtests --no-exclusion-keywords` must execute every ordered `TESTCASES` token not disabled by the tracked vendored `tests/data/DISABLED` rules for the selected flavor, preserve the duplicate `test1190` invocation, refuse to pass `-f`, report the disabled-token set explicitly, and fail if it uses `nonflaky-test`, `TEST_NF`, `~flaky`, `~timing-dependent`, or any comparable extra exclusion mechanism. The unconditional former-unit ids `1300`, `1309`, `1323`, `1602`, `1603`, `1604`, `1661`, and `2601` are not forced through `runtests.pl`; they are discharged by the required Rust unit-port suite instead.
- `build-compat-consumers.sh --all` in the final matrix must prove that every manifest-recorded compatibility target builds for the selected flavor. Targets whose manifest marks them as libcurl consumers must compile and link against the safe library; auxiliary helper targets such as `chkhostname` and the 10 server helpers must preserve their original upstream non-libcurl link lines and still build successfully.
- `run-link-compat.sh --all-objects` in the final matrix must relink the complete runnable object manifest without recompilation and execute every relinked consumer under its declared runtime adapter; a link-only success is insufficient.
- `run-curl-tool-smoke.sh` must remain part of the final matrix so the compatibility-built tool is exercised for both flavors and the packaged OpenSSL `curl` binary is exercised after `dpkg-buildpackage`.
- `run-http-client-tests.sh --all` must execute all 7 tracked `tests/http/clients` programs for each flavor.
- `cargo test --test unit_port` must execute the full Rust port of all 46 original unit source ids, not only the 3 upstream `UNITPROGS`.
- `python3 safe/scripts/verify-cve-coverage.py --manifest safe/metadata/cve-manifest.json --mapping safe/metadata/cve-to-test.json --cases-dir safe/tests/cve_cases` must remain part of the final matrix so the security-manifest-to-regression mapping contract cannot silently regress after phase 5.
- `safe/scripts/compare-benchmarks.py` must still pass in the final matrix; performance verification cannot disappear after the dedicated performance phase.
- Package-related commands in the final matrix must run against a detached `safe/`-only export such as `/tmp/libcurl-safe-final-check`, not the live repo tree, so the final package proof remains self-contained.
- The package-build subblock in the final matrix must assume the same prepared Ubuntu 24.04 executor contract as phase 9, plus `rust-clippy` for the required `cargo clippy` commands, and it must run `mk-build-deps -ir -t 'apt-get -y --no-install-recommends' debian/control` in the detached export before `dpkg-buildpackage`.
- `python3 /tmp/libcurl-safe-final-check/scripts/verify-package-control-contract.py --expected-control original/debian/control --actual-control /tmp/libcurl-safe-final-check/debian/control --package-root /tmp/libcurl-safe-final-check --require-source-build-deps cargo:native rustc:native` must remain part of the final matrix so the package stanza contract is verified both before install-layout checks and after substvars expansion into the built `.deb` metadata, while also proving that the detached safe source package declares its Rust build-tool requirements explicitly.
- `bash /tmp/libcurl-safe-final-check/scripts/verify-package-install-layout.sh --package-root /tmp/libcurl-safe-final-check` must remain part of the final matrix so the actual `.deb` payloads are checked for the required runtime-library symlinks, public headers, packaged `curl` binary/manpage, development metadata files, and `libcurl4-doc` docs/examples/manpages instead of being treated as valid merely because the package build completed.
- `bash /tmp/libcurl-safe-final-check/scripts/verify-devpkg-tooling-contract.sh --package-root /tmp/libcurl-safe-final-check` must remain part of the final matrix so packaged `curl-config`, packaged `libcurl.pc`, and installed `usr/share/aclocal/libcurl.m4` are revalidated after the last cleanup and package rebuild.
- `safe/scripts/audit-final-build-independence.sh` must fail if `safe/c_shim/forwarders.c` still exists, if `safe/Cargo.toml`, `safe/build.rs`, `safe/debian/rules`, or any other file consumed directly by the final library or package build still refers to `forwarders.c`, `safe/scripts/build-reference-curl.sh`, `safe/.reference/`, or `libcurl-reference-*`, or if `readelf -d` on either final flavor library or the packaged `/usr/bin/curl` reports `DT_NEEDED`, `RPATH`, or `RUNPATH` entries that point at the transitional reference build.
- The final audit must inspect both flavor-specific Rust library outputs and the packaged OpenSSL `curl` binary; a surviving transitional dependency in any one of them is a phase failure even if the functional tests still pass.
- Freeze the manifests only after the full matrix passes; those manifests become the maintenance contract for later changes.

## Verification Phases
### `check-final-full-matrix`
- Type: `check`
- Bounce Target: `impl-final-hardening`
- Purpose: run the full ABI, package, link-and-run, security, benchmark, upstream, HTTP-client, unit-port, and downstream compatibility matrix for both flavors after all temporary fallback bridges are removed.
- Commands it should run:
```bash
bash -lc '
set -euo pipefail
rm -rf /tmp/libcurl-safe-final-check
bash safe/scripts/export-tracked-tree.sh --safe-only --dest /tmp/libcurl-safe-final-check
test -f /tmp/libcurl-safe-final-check/Cargo.lock
test -f /tmp/libcurl-safe-final-check/.cargo/config.toml
test -d /tmp/libcurl-safe-final-check/vendor/cargo
rg -n "replace-with *= *\"vendored-sources\"|directory *= *\"vendor/cargo\"" /tmp/libcurl-safe-final-check/.cargo/config.toml >/dev/null
rg -n "cargo:native" /tmp/libcurl-safe-final-check/debian/control >/dev/null
rg -n "rustc:native" /tmp/libcurl-safe-final-check/debian/control >/dev/null
rg -n "CARGO_NET_OFFLINE=true|--offline" /tmp/libcurl-safe-final-check/debian/rules >/dev/null
rg -n -- "--locked" /tmp/libcurl-safe-final-check/debian/rules >/dev/null
test "$(cat /tmp/libcurl-safe-final-check/debian/source/format)" = "3.0 (quilt)"
test -f /tmp/libcurl-safe-final-check/debian/patches/series
while IFS= read -r patch; do
  case "$patch" in
    ''|'#'*) continue ;;
  esac
  test -f "/tmp/libcurl-safe-final-check/debian/patches/$patch"
done </tmp/libcurl-safe-final-check/debian/patches/series
cd /tmp/libcurl-safe-final-check
mk-build-deps -ir -t 'apt-get -y --no-install-recommends' debian/control
dpkg-buildpackage -us -uc -b
'
python3 safe/scripts/verify-manifests.py --abi safe/metadata/abi-manifest.json --tests safe/metadata/test-manifest.json --cves safe/metadata/cve-manifest.json
bash safe/scripts/verify-public-headers.sh --expected original/include/curl --actual safe/include/curl
bash safe/scripts/verify-export-names.sh --expected original/libcurl.def --flavor openssl
bash safe/scripts/verify-export-names.sh --expected original/libcurl.def --flavor gnutls
bash safe/scripts/verify-symbol-versions.sh --expected original/debian/libcurl4t64.symbols --flavor openssl
bash safe/scripts/verify-symbol-versions.sh --expected original/debian/libcurl3t64-gnutls.symbols --flavor gnutls
bash safe/scripts/build-compat-consumers.sh --flavor openssl --all
bash safe/scripts/build-compat-consumers.sh --flavor gnutls --all
cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test public_abi
cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test public_abi
cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test abi_layout
cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test abi_layout
bash safe/scripts/run-public-abi-smoke.sh --flavor openssl
bash safe/scripts/run-public-abi-smoke.sh --flavor gnutls
bash safe/scripts/run-link-compat.sh --flavor openssl --all-objects
bash safe/scripts/run-link-compat.sh --flavor gnutls --all-objects
bash safe/scripts/run-upstream-tests.sh --flavor openssl --require-all-runtests --no-exclusion-keywords
bash safe/scripts/run-upstream-tests.sh --flavor gnutls --require-all-runtests --no-exclusion-keywords
bash safe/scripts/run-curl-tool-smoke.sh --implementation compat --flavor openssl
bash safe/scripts/run-curl-tool-smoke.sh --implementation compat --flavor gnutls
bash /tmp/libcurl-safe-final-check/scripts/run-curl-tool-smoke.sh --implementation packaged --flavor openssl --package-root /tmp/libcurl-safe-final-check
bash safe/scripts/run-http-client-tests.sh --flavor openssl --all
bash safe/scripts/run-http-client-tests.sh --flavor gnutls --all
cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test unit_port
cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test unit_port
python3 safe/scripts/verify-cve-coverage.py --manifest safe/metadata/cve-manifest.json --mapping safe/metadata/cve-to-test.json --cases-dir safe/tests/cve_cases
cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test cve_regressions
cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test cve_regressions
rm -rf safe/.bench-output/final
mkdir -p safe/.bench-output/final
bash safe/scripts/benchmark-local.sh --implementation original --flavor openssl --matrix core --output-dir safe/.bench-output/final/original/openssl
bash safe/scripts/benchmark-local.sh --implementation safe --flavor openssl --matrix core --output-dir safe/.bench-output/final/safe/openssl
python3 safe/scripts/compare-benchmarks.py --baseline safe/.bench-output/final/original/openssl --candidate safe/.bench-output/final/safe/openssl --thresholds safe/benchmarks/thresholds.json
bash safe/scripts/benchmark-local.sh --implementation original --flavor gnutls --matrix core --output-dir safe/.bench-output/final/original/gnutls
bash safe/scripts/benchmark-local.sh --implementation safe --flavor gnutls --matrix core --output-dir safe/.bench-output/final/safe/gnutls
python3 safe/scripts/compare-benchmarks.py --baseline safe/.bench-output/final/original/gnutls --candidate safe/.bench-output/final/safe/gnutls --thresholds safe/benchmarks/thresholds.json
bash /tmp/libcurl-safe-final-check/scripts/run-ldap-devpkg-test.sh --flavor openssl --package-root /tmp/libcurl-safe-final-check
bash /tmp/libcurl-safe-final-check/scripts/run-ldap-devpkg-test.sh --flavor gnutls --package-root /tmp/libcurl-safe-final-check
bash /tmp/libcurl-safe-final-check/scripts/verify-devpkg-tooling-contract.sh --package-root /tmp/libcurl-safe-final-check
python3 /tmp/libcurl-safe-final-check/scripts/verify-package-control-contract.py --expected-control original/debian/control --actual-control /tmp/libcurl-safe-final-check/debian/control --package-root /tmp/libcurl-safe-final-check --require-source-build-deps cargo:native rustc:native
bash /tmp/libcurl-safe-final-check/scripts/verify-package-install-layout.sh --package-root /tmp/libcurl-safe-final-check
bash /tmp/libcurl-safe-final-check/scripts/verify-autopkgtest-contract.sh --expected-control original/debian/tests/control --actual-control /tmp/libcurl-safe-final-check/debian/tests/control
bash /tmp/libcurl-safe-final-check/scripts/run-packaged-autopkgtests.sh --package-root /tmp/libcurl-safe-final-check --test upstream-tests-openssl
bash /tmp/libcurl-safe-final-check/scripts/run-packaged-autopkgtests.sh --package-root /tmp/libcurl-safe-final-check --test upstream-tests-gnutls
bash /tmp/libcurl-safe-final-check/scripts/run-packaged-autopkgtests.sh --package-root /tmp/libcurl-safe-final-check --test curl-ldapi-test
bash ./test-original.sh --implementation safe
bash /tmp/libcurl-safe-final-check/scripts/audit-final-build-independence.sh --package-root /tmp/libcurl-safe-final-check
cargo clippy --manifest-path safe/Cargo.toml --all-targets --no-default-features --features openssl-flavor -- -D warnings
cargo clippy --manifest-path safe/Cargo.toml --all-targets --no-default-features --features gnutls-flavor -- -D warnings
```

## Success Criteria
- Every listed `Preexisting Input` is consumed as an existing artifact rather than rediscovered, regenerated, or refetched.
- Every listed `New Output` for this implement phase exists and is ready for downstream phases in the linear workflow.
- The verifier phase(s) `check-final-full-matrix` pass exactly as written for `impl-final-hardening`.

## Git Commit Requirement
The implementer must commit this phase's work to git before yielding. Ignored-only or untracked-only outputs are not acceptable.
