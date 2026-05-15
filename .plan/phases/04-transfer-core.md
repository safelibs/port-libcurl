### 4. Easy/Multi Transfer Core, DNS, Connection Cache, Connect-Only, Callbacks, and Basic Protocol Routing

**Phase Name:** Easy/Multi Transfer Core, DNS, Connection Cache, Connect-Only, Callbacks, and Basic Protocol Routing

**Implement Phase ID:** `impl-transfer-core`

**Verification Phases:**

- `check-transfer-core`
  - Type: `check`
  - Fixed `bounce_target`: `impl-transfer-core`
  - Purpose: verify the safe easy and multi transfer loop for basic HTTP/1.1, file, callbacks, timeout, connect-only, and selected upstream libtests.
  - Commands:
    ```bash
    CARGO_TARGET_DIR=safe/target/check-transfer-openssl \
      cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test public_abi
    CARGO_TARGET_DIR=safe/target/check-transfer-gnutls \
      cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test public_abi
    bash safe/scripts/run-curated-libtests.sh --flavor openssl --tests 500 501 502 504 509 510 511 512 518 519 520 521 523 524
    bash safe/scripts/run-curated-libtests.sh --flavor gnutls --tests 500 501 502 504 509 510 511 512 518 519 520 521 523 524
    ```

- `check-transfer-link-smoke`
  - Type: `check`
  - Fixed `bounce_target`: `impl-transfer-core`
  - Purpose: prove objects compiled against the original libcurl link lines relink and run against safe for representative easy/multi transfer tests.
  - Commands:
    ```bash
    bash safe/scripts/run-link-compat.sh --flavor openssl --target lib500 --target lib526 --target lib547 --target lib670 --target curl
    bash safe/scripts/run-link-compat.sh --flavor gnutls --target lib500 --target lib526 --target lib547 --target lib670 --target curl
    ```

**Preexisting Inputs:**

- `safe/scripts/compat_harness.py`, `safe/scripts/build-compat-consumers.sh`, `safe/scripts/run-curated-libtests.sh`, `safe/scripts/run-link-compat.sh`, `safe/scripts/run-upstream-tests.sh`, `safe/scripts/run-curl-tool-smoke.sh`, `safe/scripts/http-fixture.py`, and `safe/scripts/http-fixtures.sh`.
- `safe/compat/config/openssl/lib/curl_config.h`, `safe/compat/config/openssl/tests/config`, `safe/compat/config/openssl/curl-config`, `safe/compat/config/gnutls/lib/curl_config.h`, `safe/compat/config/gnutls/tests/config`, and `safe/compat/config/gnutls/curl-config`.
- `safe/vendor/upstream/manifest.json`, `safe/vendor/upstream/`, `safe/compat/link-manifest.json`, and `safe/metadata/test-manifest.json`.
- `original/lib/easy.c`, `original/lib/multi.c`, `original/lib/transfer.c`, `original/lib/url.c`, `original/lib/connect.c`, `original/lib/conncache.c`, `original/lib/cfilters.h`, `original/lib/hostip*.c`, `original/lib/select.c`, `original/lib/progress.c`, `original/lib/speedcheck.c`, `original/lib/file.c`.
- Existing `safe/src/easy/perform.rs`, `safe/src/multi/mod.rs`, `safe/src/multi/state.rs`, `safe/src/multi/poll.rs`, `safe/src/transfer/mod.rs`, `safe/src/dns/mod.rs`, `safe/src/conn/cache.rs`, `safe/src/conn/filter.rs`, `safe/src/protocols/mod.rs`, `safe/src/protocols/file.rs`, and `safe/src/abi/connect_only.rs`.

**New Outputs:**

- Completed native transfer planning and execution for easy and multi HTTP/1.1 plus file/local protocol paths required by early libtests.
- Reduced reference-backed fallback use for easy/multi transfer paths.
- Tests for callbacks, progress, low-speed, timeout, pause, connect-only send/recv, socket callbacks, timers, wakeup, and multi info messages.

**File Changes:**

- `safe/src/easy/perform.rs`: track option state, callbacks, error buffers, read/write/header callbacks, request metadata, and transfer info.
- `safe/src/transfer/mod.rs`: implement URL parsing, DNS resolution, connection setup, request execution, response parsing, callbacks, body upload/download, ranges, redirects where not HTTP-security-specific, low-speed timeouts, connect-only sessions, and `CURLINFO_*`.
- `safe/src/multi/mod.rs`: implement add/remove, scheduling, worker lifecycle, fdset/wait/poll/wakeup, socket/timer callbacks, info queue, cleanup, and connection cache handoff.
- `safe/src/dns/mod.rs`: implement `CURLOPT_RESOLVE`, `CURLOPT_CONNECT_TO`, IPv4/IPv6, DNS ownership, and error mapping.
- `safe/src/conn/cache.rs`: implement cache keys that isolate scheme, host, port, proxy, TLS/backend, credentials, and share state.
- `safe/src/protocols/file.rs`: complete local file transfer compatibility for upload/download cases.

**Implementation Details:**

- Native transfer code must not expose Rust panics across C ABI. Convert internal errors to libcurl `CURLcode` or `CURLMcode` and write error buffer text where libcurl would.
- Multi callbacks must prevent recursive API misuse and preserve message ordering.
- Connection reuse must be conservative: if a reuse key is uncertain, create a fresh connection rather than leaking credentials or state.
- Connect-only mode must honor pause/unpause and `CURLE_AGAIN`.
- Implementer must commit this phase's work before yielding.

**Verification:**

- Run both transfer checks. Any failure must be fixed in this phase, not deferred to HTTP/security phases unless the failing test is explicitly HTTP-policy-specific.

**Success Criteria:**

- Every item listed under `New Outputs` is present, updated, or explicitly left unchanged because it already satisfied the plan.
- Every required `File Changes` and `Implementation Details` invariant for this phase is satisfied.
- Every verifier listed under `Verification Phases` passes exactly as written and bounces only to this phase on failure.
- All listed `Preexisting Inputs` are consumed in place; existing artifacts are not rediscovered, refetched, or regenerated from untracked sources unless this phase explicitly updates a derived safe artifact from them.

**Git Commit Requirement:**

- The implementer must commit this phase's work to git before yielding.
