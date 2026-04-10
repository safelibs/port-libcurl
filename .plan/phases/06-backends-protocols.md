# Phase Name
TLS Backends, HTTP/2, Remaining Protocol Engines, and Tracked HTTP Client Coverage

## Implement Phase ID
`impl-backends-protocols`

## Preexisting Inputs
- `original/lib/vtls/*.c`
- `original/lib/vssh/*.c`
- `original/lib/vquic/*.c`
- `original/lib/http2.c`
- `original/lib/file.c`
- `original/lib/ftp.c`
- `original/lib/imap.c`
- `original/lib/pop3.c`
- `original/lib/smtp.c`
- `original/lib/ldap.c`
- `original/lib/openldap.c`
- `original/lib/smb.c`
- `original/lib/telnet.c`
- `original/lib/tftp.c`
- `original/lib/dict.c`
- `original/lib/gopher.c`
- `original/lib/rtsp.c`
- `original/lib/mqtt.c`
- `original/lib/doh.c`
- `original/lib/idn.c`
- the tracked files under `original/tests/http/`
- `safe/scripts/run-upstream-tests.sh`
- `safe/scripts/run-http-client-tests.sh`
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

## New Outputs
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

## File Changes
- Port the flavor-specific TLS logic into small backend adapters with a shared Rust policy layer.
- Port the remaining non-HTTP protocol engines and backend integrations.
- Add tracked HTTP-client support for server push headers, multiplexing, pause/resume, TLS reuse, and WebSockets.

## Implementation Details
- Keep the backend boundary small. Policy, state, and reuse rules stay in Rust; backend modules perform only backend-specific cryptographic and certificate operations.
- Preserve `curl_global_sslset`, pinned public key behavior, ALPN, session-cache semantics, backend-specific error reporting, and certificate-info extraction.
- Cover the certificate-validation and pinning issues highlighted in `relevant_cves.json`, including OpenSSL and GnuTLS backend differences.
- Implement `curl_pushheader_byname` and `curl_pushheader_bynum` as part of the HTTP/2 server-push surface exercised by `h2-serverpush.c`.
- The source tree contains QUIC/HTTP/3 code, but Ubuntu 24.04 package builds do not currently enable the corresponding extra dependencies in `original/debian/control`. The safe port should preserve the Ubuntu package feature matrix first; HTTP/3 paths should only be exposed in a given flavor when that flavor is built with matching backend support.
- The tracked `tests/http/clients` programs are canonical existing inputs. The runner should provision only the dependencies needed by those tracked clients and should never fabricate the absent pytest tree.
- The phase-6 curated `runtests.pl` subset must contain only ids that upstream will execute without `-f`. Do not schedule the former-unit ids `1300`, `1309`, `1323`, `1602`, `1603`, `1604`, `1661`, or `2601` here; phase 7 discharges that coverage through `safe/tests/unit_port.rs`.
- Ensure all protocol handlers plug into the shared easy/multi/connection engine from earlier phases rather than bypassing it with protocol-local lifetimes.

## Verification Phases
### `check-backends-protocols-openssl`
- Type: `check`
- Bounce Target: `impl-backends-protocols`
- Purpose: validate the OpenSSL flavor across the remaining protocol and backend surface, including the tracked HTTP client programs.
- Commands it should run:
```bash
bash safe/scripts/run-upstream-tests.sh --flavor openssl --tests 1 105 506 586 1500 1900 1915 2304 3000 3025 3100
bash safe/scripts/run-http-client-tests.sh --flavor openssl --clients h2-download h2-serverpush h2-pausing h2-upgrade-extreme tls-session-reuse ws-data ws-pingpong
```

### `check-backends-protocols-gnutls`
- Type: `check`
- Bounce Target: `impl-backends-protocols`
- Purpose: validate the GnuTLS flavor across the same protocol and backend surface.
- Commands it should run:
```bash
bash safe/scripts/run-upstream-tests.sh --flavor gnutls --tests 1 105 506 586 1500 1900 1915 2304 3000 3025 3100
bash safe/scripts/run-http-client-tests.sh --flavor gnutls --clients h2-download h2-serverpush h2-pausing h2-upgrade-extreme tls-session-reuse ws-data ws-pingpong
```

## Success Criteria
- Every listed `Preexisting Input` is consumed as an existing artifact rather than rediscovered, regenerated, or refetched.
- Every listed `New Output` for this implement phase exists and is ready for downstream phases in the linear workflow.
- The verifier phase(s) `check-backends-protocols-openssl`, `check-backends-protocols-gnutls` pass exactly as written for `impl-backends-protocols`.

## Git Commit Requirement
The implementer must commit this phase's work to git before yielding. Ignored-only or untracked-only outputs are not acceptable.
