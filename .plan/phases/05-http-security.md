# Phase Name
HTTP, Redirect, Cookies/HSTS, Headers API, Authentication, WebSockets, and CVE Regressions

## Implement Phase ID
`impl-http-security`

## Preexisting Inputs
- `original/lib/http.c`
- `original/lib/http1.c`
- `original/lib/http_proxy.c`
- `original/lib/headers.c`
- `original/lib/cookie.c`
- `original/lib/hsts.c`
- `original/lib/altsvc.c`
- `original/lib/http_digest.c`
- `original/lib/http_ntlm.c`
- `original/lib/http_negotiate.c`
- `original/lib/content_encoding.c`
- `original/lib/ws.c`
- `original/lib/rand.c`
- `relevant_cves.json`
- `original/debian/patches/CVE-*.patch`
- `safe/metadata/cve-manifest.json`
- `safe/scripts/run-curated-libtests.sh`
- `safe/scripts/run-http-client-tests.sh`
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/src/lib.rs`
- `safe/src/abi/generated.rs`
- `safe/include/curl/*.h`
- `safe/metadata/abi-manifest.json`
- `safe/metadata/test-manifest.json`
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
- `safe/scripts/run-link-compat.sh`
- `safe/scripts/run-upstream-tests.sh`
- `safe/scripts/run-curl-tool-smoke.sh`
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

## New Outputs
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

## File Changes
- Port HTTP and proxy request construction, response parsing, redirect following, header lookup, cookies, HSTS, alt-svc, and WebSocket framing into Rust.
- Add an explicit CVE-to-regression mapping generated from the curated JSON and the Debian patch files.
- Replace the relevant temporary C fallbacks for HTTP and WebSocket behavior.

## Implementation Details
- Redirect and credential-forwarding rules must become explicit typed policy rather than scattered flag checks, so the port closes the credential-leakage classes represented in `relevant_cves.json`.
- Connection reuse must become authentication-aware and proxy-aware, so the port closes the reuse classes represented by CVEs such as `CVE-2026-3784` and `CVE-2026-1965`.
- Port cookie and HSTS state into Rust data structures that preserve upstream behavior while making origin scoping, persistence, PSL checks, and serialization rules explicit and testable.
- Preserve `curl_easy_header` and `curl_easy_nextheader` semantics for `struct curl_header`, including pointer lifetime, origin filtering, request/response selection, and anchor handling.
- Port `curl_ws_recv`, `curl_ws_send`, and `curl_ws_meta` while replacing weak randomness or predictable mask generation with strong OS-backed entropy and explicit failure handling.
- `safe/metadata/cve-to-test.json` should map every curated CVE from `safe/metadata/cve-manifest.json` either to a dedicated regression case or to a specific shared regression case with written justification. No curated CVE should remain unmapped by the end of this phase.
- `safe/scripts/verify-cve-coverage.py` must fail if any curated CVE is missing from `safe/metadata/cve-to-test.json`, if a mapping points to a nonexistent file under `safe/tests/cve_cases/`, or if a shared-case mapping omits its written justification.
- `safe/tests/cve_regressions.rs` must consume `safe/metadata/cve-to-test.json` directly or from a generated compile-time artifact and fail if the mapping artifact and the implemented regression cases drift out of sync.

## Verification Phases
### `check-http-security-curated`
- Type: `check`
- Bounce Target: `impl-http-security`
- Purpose: validate HTTP request/response handling, redirect policy, headers API, cookies, HSTS, auth, and related easy-handle behavior with focused upstream tests.
- Commands it should run:
```bash
bash safe/scripts/run-curated-libtests.sh --flavor openssl 659 1526 1900 1905 1915 1970 1971 1972 1973 1974 1975 2304 2305
bash safe/scripts/run-curated-libtests.sh --flavor gnutls 659 1526 1900 1905 1915 1970 1971 1972 1973 1974 1975 2304 2305
```

### `check-http-security-websockets`
- Type: `check`
- Bounce Target: `impl-http-security`
- Purpose: verify the tracked WebSocket client programs against the Rust implementation.
- Commands it should run:
```bash
bash safe/scripts/run-http-client-tests.sh --flavor openssl --clients ws-data ws-pingpong
bash safe/scripts/run-http-client-tests.sh --flavor gnutls --clients ws-data ws-pingpong
```

### `check-http-security-cve-map`
- Type: `check`
- Bounce Target: `impl-http-security`
- Purpose: verify that every curated CVE in the manifest is mapped to an implemented regression case before the regression suite is accepted.
- Commands it should run:
```bash
python3 safe/scripts/verify-cve-coverage.py --manifest safe/metadata/cve-manifest.json --mapping safe/metadata/cve-to-test.json --cases-dir safe/tests/cve_cases
```

### `check-http-security-cves`
- Type: `check`
- Bounce Target: `impl-http-security`
- Purpose: verify that the CVE regression suite covers all curated security cases and passes in both flavors.
- Commands it should run:
```bash
python3 safe/scripts/verify-cve-coverage.py --manifest safe/metadata/cve-manifest.json --mapping safe/metadata/cve-to-test.json --cases-dir safe/tests/cve_cases
cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test cve_regressions
cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test cve_regressions
```

## Success Criteria
- Every listed `Preexisting Input` is consumed as an existing artifact rather than rediscovered, regenerated, or refetched.
- Every listed `New Output` for this implement phase exists and is ready for downstream phases in the linear workflow.
- The verifier phase(s) `check-http-security-curated`, `check-http-security-websockets`, `check-http-security-cve-map`, `check-http-security-cves` pass exactly as written for `impl-http-security`.

## Git Commit Requirement
The implementer must commit this phase's work to git before yielding. Ignored-only or untracked-only outputs are not acceptable.
