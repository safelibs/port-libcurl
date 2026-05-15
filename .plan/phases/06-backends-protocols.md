### 6. TLS Backends, HTTP/2, SSH, Remaining Protocols, and Tracked HTTP Client Programs

**Phase Name:** TLS Backends, HTTP/2, SSH, Remaining Protocols, and Tracked HTTP Client Programs

**Implement Phase ID:** `impl-backends-protocols`

**Verification Phases:**

- `check-backends-http-clients`
  - Type: `check`
  - Fixed `bounce_target`: `impl-backends-protocols`
  - Purpose: run the 5 tracked non-WebSocket HTTP/TLS client programs against safe libraries and local fixtures, then prove the 2 tracked WebSocket client programs compile and take the original disabled-WebSocket path.
  - Commands:
    ```bash
    command -v nghttpx >/dev/null
    bash safe/scripts/run-http-client-tests.sh --flavor openssl --clients h2-download h2-pausing h2-serverpush h2-upgrade-extreme tls-session-reuse
    bash safe/scripts/run-http-client-tests.sh --flavor gnutls --clients h2-download h2-pausing h2-serverpush h2-upgrade-extreme tls-session-reuse
    bash safe/scripts/run-websocket-disabled-smoke.sh --flavor openssl --clients ws-data ws-pingpong
    bash safe/scripts/run-websocket-disabled-smoke.sh --flavor gnutls --clients ws-data ws-pingpong
    ```

- `check-backends-protocol-matrix`
  - Type: `check`
  - Fixed `bounce_target`: `impl-backends-protocols`
  - Purpose: exercise TLS, HTTP/2, SSH, RTMP, `gophers`, LDAPS, and all other Ubuntu-advertised non-HTTP protocols through contract probes, functional RTMP/LDAPS fixtures, upstream libtests, and CVE regressions.
  - Commands:
    ```bash
    CARGO_TARGET_DIR=safe/target/check-backends-openssl \
      cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test cve_regressions
    CARGO_TARGET_DIR=safe/target/check-backends-openssl \
      cargo build --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor
    CARGO_TARGET_DIR=safe/target/check-backends-gnutls \
      cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test cve_regressions
    CARGO_TARGET_DIR=safe/target/check-backends-gnutls \
      cargo build --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor
    python3 safe/scripts/verify-protocol-feature-contract.py \
      --flavor openssl \
      --artifact safe/target/check-backends-openssl/debug/libport_libcurl_safe.so \
      --assert-routing
    python3 safe/scripts/verify-protocol-feature-contract.py \
      --flavor gnutls \
      --artifact safe/target/check-backends-gnutls/debug/libport_libcurl_safe.so \
      --assert-routing
    command -v python3 >/dev/null
    command -v cc >/dev/null
    command -v openssl >/dev/null
    command -v pkgconf >/dev/null
    command -v ldapsearch >/dev/null
    test -x /usr/sbin/slapd || command -v slapd >/dev/null
    pkgconf --exists ldap
    bash safe/scripts/run-rtmp-functional-tests.sh \
      --flavor openssl \
      --artifact safe/target/check-backends-openssl/debug/libport_libcurl_safe.so \
      --schemes rtmp,rtmpe,rtmps,rtmpt,rtmpte,rtmpts \
      --require-download \
      --require-upload
    bash safe/scripts/run-rtmp-functional-tests.sh \
      --flavor gnutls \
      --artifact safe/target/check-backends-gnutls/debug/libport_libcurl_safe.so \
      --schemes rtmp,rtmpe,rtmps,rtmpt,rtmpte,rtmpts \
      --require-download \
      --require-upload
    bash safe/scripts/run-ldaps-functional-test.sh \
      --flavor openssl \
      --artifact safe/target/check-backends-openssl/debug/libport_libcurl_safe.so
    bash safe/scripts/run-ldaps-functional-test.sh \
      --flavor gnutls \
      --artifact safe/target/check-backends-gnutls/debug/libport_libcurl_safe.so
    bash safe/scripts/run-curated-libtests.sh --flavor openssl --tests 3010 3026 3100 3101 3102 3200 1900 1903 1906 1907 1911 1915 1916 1919 1934 1935 1936 1937 1938 1947 1948 1955 1956 1957 1958 1964 1970 1971 1972 1974 1975 2302 2304 2404 2502 2600 2601 2602 2603
    bash safe/scripts/run-curated-libtests.sh --flavor gnutls --tests 3010 3026 3100 3101 3102 3200 1900 1903 1906 1907 1911 1915 1916 1919 1934 1935 1936 1937 1938 1947 1948 1955 1956 1957 1958 1964 1970 1971 1972 1974 1975 2302 2304 2404 2502 2600 2601 2602 2603
    ```

**Preexisting Inputs:**

- `safe/src/http/request.rs`, `safe/src/http/response.rs`, `safe/src/http/auth.rs`, `safe/src/http/proxy.rs`, `safe/src/http/cookies.rs`, `safe/src/http/hsts.rs`, `safe/src/http/altsvc.rs`, `safe/src/http/headers.rs`, `safe/src/ws.rs`, `safe/src/rand.rs`, `safe/tests/cve_regressions.rs`, `safe/tests/cve_cases/`, `safe/metadata/cve-manifest.json`, and `safe/metadata/cve-to-test.json`.
- `safe/scripts/run-curated-libtests.sh`, `safe/scripts/run-http-client-tests.sh`, `safe/scripts/run-websocket-disabled-smoke.sh`, `safe/compat/config/openssl/curl-config`, `safe/compat/config/gnutls/curl-config`, and `safe/metadata/test-manifest.json`.
- `original/lib/version.c`, `original/lib/url.c`, `original/lib/urldata.h`, `original/debian/rules`, and `original/debian/control` for the exact Ubuntu protocol, feature, backend, and dependency contract.
- `original/lib/vtls/*.c`, `original/lib/vssh/*.c`, `original/lib/vquic/*.c`, `original/lib/http2.c`, `original/lib/doh.c`, `original/lib/idn.c`, `original/lib/content_encoding.c`, and protocol implementations for file, FTP, IMAP, POP3, SMTP, LDAP, SMB, TELNET, TFTP, DICT, GOPHER, RTSP, MQTT, and RTMP.
- `original/lib/gopher.c`, `original/lib/gopher.h`, `original/lib/curl_rtmp.c`, and `original/lib/curl_rtmp.h`.
- Existing `safe/src/version.rs`, `safe/src/tls/*.rs`, `safe/src/ssh/mod.rs`, `safe/src/vquic/mod.rs`, `safe/src/doh.rs`, `safe/src/idn.rs`, `safe/src/protocols/*.rs`, `safe/c_shim/http2_transport.c`, `safe/c_shim/tls_backend.c`, `safe/c_shim/ssh_backend.c`.

**New Outputs:**

- Completed flavor-correct TLS support for OpenSSL and GnuTLS.
- HTTP/2 behavior sufficient for tracked non-WebSocket clients, plus disabled-WebSocket compile/runtime smoke coverage for `ws-data` and `ws-pingpong`.
- Native or explicitly justified protocol engines for the full Ubuntu-advertised protocol list, including `gophers`, `ldap`, `ldaps`, and all six RTMP URL schemes.
- `safe/src/protocols/rtmp.rs` or an equivalent module implementing RTMP-family routing through a justified `librtmp` FFI boundary.
- `safe/scripts/verify-protocol-feature-contract.py`, which validates `curl_version_info`, feature bits/names, advertised protocol names, default routing, and registration/routing probes for both flavors without using sibling `original/`; it is complemented by the functional RTMP and LDAPS fixtures for advertised protocols that cannot be proven by routing alone.
- `safe/scripts/rtmp-fixture.py` and `safe/scripts/run-rtmp-functional-tests.sh`, which provide deterministic local functional coverage for every advertised RTMP URL scheme without external network access.
- `safe/scripts/run-ldaps-functional-test.sh`, which starts a local TLS-enabled LDAP fixture and verifies successful `ldaps://` query behavior plus certificate/CA failure modes for artifact and installed-package modes.
- Reduced transport sidecar dependency.

**File Changes:**

- `safe/src/tls/openssl.rs` and `safe/src/tls/gnutls.rs`: certificate verification, CA path/bundle/blob, hostname, pinned public keys, ALPN, certinfo, session reuse, and error mapping.
- `safe/src/tls/certinfo.rs`: public certinfo list allocation and lifetime.
- `safe/src/version.rs`: expose the exact Ubuntu-derived protocol and feature contract for the active flavor. `curl_version_info_data.protocols` must list `dict`, `file`, `ftp`, `ftps`, `gopher`, `gophers`, `http`, `https`, `imap`, `imaps`, `ldap`, `ldaps`, `mqtt`, `pop3`, `pop3s`, `rtmp`, `rtmpe`, `rtmps`, `rtmpt`, `rtmpte`, `rtmpts`, `rtsp`, `scp`, `sftp`, `smb`, `smbs`, `smtp`, `smtps`, `telnet`, and `tftp`. `feature_names` and the `features` bitmask must include `alt-svc`, `AsynchDNS`, `brotli`, `GSS-API`, `HSTS`, `HTTP2`, `HTTPS-proxy`, `IDN`, `IPv6`, `Kerberos`, `Largefile`, `libz`, `NTLM`, `PSL`, `SPNEGO`, `SSL`, `threadsafe`, `TLS-SRP`, `UnixSockets`, and `zstd`, with version fields populated for zlib, brotli, zstd, libidn2, nghttp2, libpsl, the selected TLS backend, and SSH.
- `safe/src/ssh/mod.rs`: SCP/SFTP auth and transport semantics through Ubuntu's selected `libssh` backend, matching `original/debian/rules`, unless a documented verifier-backed substitute preserves the same public `curl_version_info`, `curl-config`, `libcurl.pc`, static link, and runtime behavior.
- `safe/src/vquic/mod.rs`: QUIC option surface and clear not-built-in behavior unless native support is implemented.
- `safe/src/doh.rs`: DNS-over-HTTPS option behavior and safe fallback.
- `safe/src/idn.rs`: IDNA conversion consistent with libcurl 8.5.0 behavior.
- `safe/src/http/response.rs` or a dedicated content-decoding module: implement gzip/deflate, brotli, and zstd decoding consistently with the advertised `libz`, `brotli`, and `zstd` features.
- `safe/src/http/cookies.rs` and `safe/src/idn.rs`: use or wrap libpsl and libidn2 behavior so the advertised `PSL` and `IDN` features are true at runtime.
- `safe/src/protocols/mod.rs`: route every advertised scheme to a concrete handler. `gophers` must route to the Gopher handler with TLS enabled, not to the unknown/unsupported path. `ldap` and `ldaps` must remain advertised and routed. `rtmp`, `rtmpe`, `rtmps`, `rtmpt`, `rtmpte`, and `rtmpts` must route to the RTMP handler with correct default ports and TLS/HTTP-tunnel semantics.
- `safe/src/protocols/mod.rs`: under the Ubuntu-compatible default configuration, do not route `ws` or `wss`; those schemes must behave like the original build without `--enable-websockets` and must not appear in `curl_version_info`, `curl-config`, or `libcurl.pc`.
- `safe/src/protocols/gopher.rs`: accept both `gopher` and `gophers`, use default port 70 for both, and use the transfer TLS path for `gophers`.
- `safe/src/protocols/rtmp.rs`: implement the RTMP family through `librtmp` or a native Rust implementation. The initial acceptable boundary is `librtmp` FFI that validates URLs, pointers, buffer lengths, callbacks, and ownership; maps `RTMP_SetupURL`, `RTMP_Connect1`, `RTMP_ConnectStream`, `RTMP_Read`, `RTMP_Write`, and close/free failures to libcurl-compatible errors; and supports uploads/downloads sufficiently for `rtmp://`, `rtmpe://`, `rtmps://`, `rtmpt://`, `rtmpte://`, and `rtmpts://`.
- `safe/src/protocols/*.rs`: only protocols omitted by the Ubuntu package, such as HTTP/3/QUIC unless implemented and advertised, may return `CURLE_UNSUPPORTED_PROTOCOL` or `CURLE_NOT_BUILT_IN`. No scheme in the explicit Ubuntu-advertised list may fail with either unsupported/not-built-in from the initial routing/protocol-selection layer.
- `safe/tests/public_abi.rs` and any version/protocol smoke tests: update expected protocol and feature lists to the exact Ubuntu-derived contract rather than the current partial safe inventory.
- `safe/scripts/run-websocket-disabled-smoke.sh`: build the tracked `ws-data` and `ws-pingpong` clients from vendored upstream sources using `safe/compat/config/<flavor>/`, run them with no arguments or dummy arguments as needed, require a non-zero exit, and require stderr to contain the original `websockets not enabled in libcurl` diagnostic. The script must fail if the clients enter their functional WebSocket branch, if `USE_WEBSOCKETS` is defined, or if `curl-config --protocols` advertises `WS` or `WSS`.
- `safe/scripts/rtmp-fixture.py`: implement a local deterministic RTMP/RTMPT server sufficient for the advertised libcurl contract. It must support the RTMP handshake, AMF command sequence needed for connect/createStream/play and publish or an equivalent write-command path, deterministic sentinel media bytes for download assertions, upload/write capture for byte-for-byte assertions, TLS wrapping for `rtmps`/`rtmpts`, HTTP tunnel endpoints for `rtmpt`/`rtmpte`/`rtmpts`, and encrypted RTMP-family coverage for `rtmpe`/`rtmpte` without delegating success to an unsupported-protocol or connection-failure path.
- `safe/scripts/run-rtmp-functional-tests.sh`: compile a small C probe against either `--artifact <safe shared library>` or installed `libcurl`, run it against `safe/scripts/rtmp-fixture.py`, require success for all six schemes, assert that at least one download/read and at least one upload/write or publish-command path are observed by the fixture, verify the TLS and HTTP-tunnel fixture logs for their respective schemes, and fail on `CURLE_UNSUPPORTED_PROTOCOL`, `CURLE_NOT_BUILT_IN`, early connection failure, timeout-only success, or missing fixture observations.
- `safe/scripts/run-ldaps-functional-test.sh`: compile a small C probe against either `--artifact <safe shared library>` or installed `libcurl`, start an isolated local OpenLDAP `slapd` instance on high ports with temporary database, known entries, and a tracked or generated local CA/server certificate, then run `ldaps://localhost:<port>/...` queries. It must require success when `CURLOPT_CAINFO` trusts the fixture CA, require certificate verification failure when the CA is absent or the hostname/CA is wrong, and record that the query returned the expected LDAP attributes through libcurl's write callback.
- `safe/c_shim/http2_transport.c`, `safe/c_shim/tls_backend.c`, `safe/c_shim/ssh_backend.c`: shrink to unavoidable C-library FFI boundaries or replace with Rust FFI wrappers.
- `safe/scripts/verify-protocol-feature-contract.py`: build or load a small C probe against the selected safe artifact, compare `curl_version_info_data.protocols`, `feature_names`, feature bitmask, version fields, and routing behavior against the hard-coded original-derived Ubuntu contract. With `--assert-routing`, it must call `curl_easy_perform` on every advertised scheme at least far enough to prove the scheme is registered: closed-port network probes for ordinary TCP protocols may accept connection, timeout, DNS, authentication, or protocol-level failures, but must fail the check on `CURLE_UNSUPPORTED_PROTOCOL` or `CURLE_NOT_BUILT_IN`; `file://` must use a temporary local file; `gophers://`, `ldaps://`, and every RTMP variant must be included in this probe set. Passing this routing probe is not sufficient for RTMP-family or LDAPS compatibility; `run-rtmp-functional-tests.sh` and `run-ldaps-functional-test.sh` are mandatory verifier commands.

**Implementation Details:**

- `curl_version_info`, `curl-config --protocols`, and `libcurl.pc:supported_protocols` must agree with actually implemented/advertised protocol support, accounting for the original libcurl distinction that `curl_version_info_data.protocols` lists all six RTMP URL schemes while configure-style tooling surfaces list the aggregate `RTMP` protocol once.
- The advertised protocol set for both flavors is fixed in this plan. It includes `gophers`, `ldap`, `ldaps`, `rtmp`, `rtmpe`, `rtmps`, `rtmpt`, `rtmpte`, and `rtmpts`; omitting any of them is a compatibility failure even if upstream tests do not exercise that route.
- WebSocket ABI functions and tracked HTTP/WebSocket clients remain in scope only as disabled-build compatibility checks. `ws` and `wss` are not part of the Ubuntu-advertised protocol list, must not be routed, and must not be added unless the package configuration is deliberately changed to match an original build with `--enable-websockets` and all protocol, tooling, CVE, client, package, and dev-tooling contracts are updated in the same phase.
- RTMP support must be present because `original/debian/control` Build-Depends on `librtmp-dev`, Ubuntu package descriptions advertise RTMP, `original/lib/version.c` advertises the six RTMP schemes under `USE_LIBRTMP`, and `original/lib/url.c` routes them. Phase 6 must provide functional local RTMP-family coverage; a closed-port RTMP smoke, DNS failure, timeout, or registration-only probe is not sufficient for any advertised RTMP scheme.
- LDAPS support must be present because `original/lib/version.c` advertises `ldaps` when OpenLDAP and TLS are enabled and `original/lib/url.c` routes it. The existing `curl-ldapi-test` Unix-socket LDAP autopkgtest remains required, but it is not LDAPS coverage; Phase 6 must add the local TLS LDAP fixture and prove successful `ldaps://` transfer plus certificate/CA enforcement for both flavors.
- For protocols still delegated to third-party C libraries, unsafe boundaries must validate pointers, lengths, callbacks, and ownership.
- Do not implement HTTP/3/QUIC unless the Ubuntu 24.04 package advertises it; unsupported protocol behavior must match libcurl's configured build.
- Implementer must commit this phase's work before yielding.

**Verification:**

- Both backend checks must pass before broad object relinking.

**Success Criteria:**

- Every item listed under `New Outputs` is present, updated, or explicitly left unchanged because it already satisfied the plan.
- Every required `File Changes` and `Implementation Details` invariant for this phase is satisfied.
- Every verifier listed under `Verification Phases` passes exactly as written and bounces only to this phase on failure.
- All listed `Preexisting Inputs` are consumed in place; existing artifacts are not rediscovered, refetched, or regenerated from untracked sources unless this phase explicitly updates a derived safe artifact from them.

**Git Commit Requirement:**

- The implementer must commit this phase's work to git before yielding.
