### 5. HTTP Semantics, Security Policies, CVE Regression Matrix, Headers API, Cookies, HSTS, ALTSVC, Auth, Proxies, and WebSocket Disabled ABI

**Phase Name:** HTTP Semantics, Security Policies, CVE Regression Matrix, Headers API, Cookies, HSTS, ALTSVC, Auth, Proxies, and WebSocket Disabled ABI

**Implement Phase ID:** `impl-http-security`

**Verification Phases:**

- `check-http-security-cves`
  - Type: `check`
  - Fixed `bounce_target`: `impl-http-security`
  - Purpose: verify all curated CVEs have case mappings and run the native Rust CVE regression suite for both flavors.
  - Commands:
    ```bash
    python3 safe/scripts/verify-cve-coverage.py
    CARGO_TARGET_DIR=safe/target/check-http-security-openssl \
      cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test cve_regressions
    CARGO_TARGET_DIR=safe/target/check-http-security-gnutls \
      cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test cve_regressions
    ```

- `check-http-security-upstream`
  - Type: `check`
  - Fixed `bounce_target`: `impl-http-security`
  - Purpose: run upstream libtests that exercise HTTP redirects, proxy auth, cookies, HSTS, headers, credentials, content decoding, and WebSocket disabled-build ABI behavior.
  - Commands:
    ```bash
    bash safe/scripts/run-curated-libtests.sh --flavor openssl --tests 555 560 567 571 574 578 586 589 597 1500 1501 1502 1520 1521 1527 1530 1531 1533 1550 1553 1556 1557 1559 1565 1567 1591 1593 1596 1600 1601 1602 1603 1604 1605 1606 1607 1608 1609 1610 1611 1612 1614 1620 1621 1650 1651 1652 1653 1654 1655 1656 1660 1661
    bash safe/scripts/run-curated-libtests.sh --flavor gnutls --tests 555 560 567 571 574 578 586 589 597 1500 1501 1502 1520 1521 1527 1530 1531 1533 1550 1553 1556 1557 1559 1565 1567 1591 1593 1596 1600 1601 1602 1603 1604 1605 1606 1607 1608 1609 1610 1611 1612 1620 1621 1650 1651 1652 1653 1654 1655 1656 1660 1661
    ```

**Preexisting Inputs:**

- `safe/src/easy/perform.rs`, `safe/src/transfer/mod.rs`, `safe/src/multi/mod.rs`, `safe/src/multi/state.rs`, `safe/src/multi/poll.rs`, `safe/src/dns/mod.rs`, `safe/src/conn/cache.rs`, `safe/src/conn/filter.rs`, `safe/src/protocols/mod.rs`, `safe/src/protocols/file.rs`, and `safe/src/abi/connect_only.rs`.
- `safe/scripts/run-curated-libtests.sh`, `safe/scripts/run-link-compat.sh`, `safe/scripts/build-compat-consumers.sh`, `safe/compat/link-manifest.json`, and `safe/metadata/test-manifest.json`.
- `relevant_cves.json`, `all_cves.json`, `original/debian/patches/CVE-*.patch`, `safe/metadata/cve-manifest.json`, `safe/metadata/cve-to-test.json`, and `safe/tests/cve_cases/*.json`.
- `original/lib/http.c`, `original/lib/http1.c`, `original/lib/headers.c`, `original/lib/cookie.c`, `original/lib/hsts.c`, `original/lib/altsvc.c`, `original/lib/http_proxy.c`, `original/lib/http_digest.c`, `original/lib/http_ntlm.c`, `original/lib/http_aws_sigv4.c`, `original/lib/content_encoding.c`, `original/lib/ws.c`.
- Existing `safe/src/http/*.rs`, `safe/src/ws.rs`, `safe/src/rand.rs`, `safe/tests/cve_regressions.rs`.

**New Outputs:**

- Completed native HTTP/1.1 policy implementation.
- Expanded CVE case files and runtime tests where current shared reference-backend mappings are too broad.
- Ubuntu-compatible disabled WebSocket ABI behavior: `curl_ws_recv` and `curl_ws_send` return `CURLE_NOT_BUILT_IN`, `curl_ws_meta` returns `NULL`, and `ws://`/`wss://` are not routed or advertised.

**File Changes:**

- `safe/src/http/request.rs`: redirects, referer stripping, credential isolation, method rewriting, body/retry rules, proxy tunnel request targets.
- `safe/src/http/response.rs`: response/header limits, 1xx/CONNECT/trailer origins, content-length/range/chunked parsing.
- `safe/src/http/auth.rs`: Basic, Digest, NTLM/Negotiate boundaries, Bearer, AWS SigV4, and proxy auth reuse isolation.
- `safe/src/http/proxy.rs`: HTTP proxy tunneling, proxy headers, proxy credential separation, no-proxy matching.
- `safe/src/http/cookies.rs`: Netscape cookie jar loading/saving, PSL rejection, host-only/domain/path/secure matching, session clearing, and Set-Cookie parsing.
- `safe/src/http/hsts.rs`: HSTS file/callback handling, includeSubDomains, IP exclusion, expiry, persistence, and lookup.
- `safe/src/http/altsvc.rs`: ALTSVC persistence, host isolation, HSTS interaction, and origin constraints.
- `safe/src/http/headers_api.rs`: `curl_easy_header` and `curl_easy_nextheader` semantics.
- `safe/src/ws.rs`: preserve exported `curl_ws_meta`, `curl_ws_recv`, and `curl_ws_send` ABI symbols while matching Ubuntu's disabled-WebSocket build: recv/send return `CURLE_NOT_BUILT_IN` for all inputs and meta returns `NULL`.
- `safe/src/rand.rs`: cryptographic randomness for HTTP auth, TLS-adjacent nonce material, and any enabled third-party boundary that needs randomness, with deterministic test injection only behind test-only code. WebSocket mask-generation paths must not be reachable while `USE_WEBSOCKETS` is disabled.
- `safe/tests/cve_regressions.rs` and `safe/tests/cve_cases/*.json`: map every curated CVE to an executable or justified shared case.

**Implementation Details:**

- Rust memory safety is not enough for this phase. Each CVE class in `relevant_cves.json` must be mapped to explicit behavior: certificate/transport validation, credential leakage, redirects, cookies, HSTS, connection reuse, parser canonicalization, randomness, platform loading, resource limits, and API contracts.
- Any case currently labeled as `reference_backend_*` must either move to a native test when the native code owns the behavior or retain a written temporary justification that the behavior is still delegated in this phase. Phase 9 must remove any remaining reference-backend labels and replace unavoidable third-party coverage with boundary-specific non-reference case names, because Phase 9 scans whole detached source exports for these markers.
- WebSocket CVE cases must be rewritten around the disabled Ubuntu contract unless the package contract is intentionally changed to enable WebSockets. The disabled-contract tests must prove the ABI symbols remain exported, `curl_ws_recv`/`curl_ws_send` return `CURLE_NOT_BUILT_IN`, `curl_ws_meta` returns `NULL`, `ws://`/`wss://` are not in protocol/tooling lists, and no WebSocket handshake or mask-generation path is reachable.
- Response header memory must be bounded. Exceeding configured limits must produce libcurl-compatible errors rather than allocation growth.
- Auth and cookies must never leak across origins, redirects, proxies, or reused connections unless the corresponding libcurl option explicitly allows it.
- Implementer must commit this phase's work before yielding.

**Verification:**

- Both check phases must pass for both flavors.

**Success Criteria:**

- Every item listed under `New Outputs` is present, updated, or explicitly left unchanged because it already satisfied the plan.
- Every required `File Changes` and `Implementation Details` invariant for this phase is satisfied.
- Every verifier listed under `Verification Phases` passes exactly as written and bounces only to this phase on failure.
- All listed `Preexisting Inputs` are consumed in place; existing artifacts are not rediscovered, refetched, or regenerated from untracked sources unless this phase explicitly updates a derived safe artifact from them.

**Git Commit Requirement:**

- The implementer must commit this phase's work to git before yielding.
