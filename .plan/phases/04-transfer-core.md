# Phase Name
Easy Perform, Multi Engine, Conncache, Resolver Ownership, Share Locking, and Transfer Loop

## Implement Phase ID
`impl-transfer-core`

## Preexisting Inputs
- `original/lib/easy.c`
- `original/lib/multi.c`
- `original/lib/multihandle.h`
- `original/lib/conncache.c`
- `original/lib/connect.c`
- `original/lib/cfilters.h`
- `original/lib/transfer.c`
- `original/lib/share.c`
- `original/lib/speedcheck.c`
- `original/lib/hostip.c`
- `original/lib/hostip4.c`
- `original/lib/hostip6.c`
- `original/lib/hostsyn.c`
- `safe/src/easy/handle.rs`
- `safe/src/global.rs`
- `safe/src/alloc.rs`
- `safe/scripts/run-curated-libtests.sh`
- `safe/scripts/run-link-compat.sh`
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
- `safe/src/version.rs`
- `safe/src/slist.rs`
- `safe/src/mime.rs`
- `safe/src/form.rs`
- `safe/src/urlapi.rs`
- `safe/src/share.rs`
- `safe/src/easy/mod.rs`
- `safe/src/easy/options.rs`
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
- `safe/scripts/run-upstream-tests.sh`
- `safe/scripts/run-curl-tool-smoke.sh`
- `safe/scripts/run-http-client-tests.sh`
- `safe/scripts/run-ldap-devpkg-test.sh`
- `safe/scripts/http-fixtures.sh`
- `safe/scripts/http-fixture.py`

## New Outputs
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

## File Changes
- Port `curl_easy_perform` onto a Rust-owned multi/transfer engine instead of a C fallback.
- Port the multi-handle state machine and wakeup/timer plumbing from `original/lib/multi.c` and `original/lib/multihandle.h`.
- Port the connection-cache, resolver ownership model, and connection-filter chain.
- Port share-handle lock callbacks and the shared-resource plumbing required by DNS, cookies, HSTS, and SSL session reuse.

## Implementation Details
- Preserve the upstream easy-perform behavior that internally uses a private multi handle, as implemented in `original/lib/easy.c`.
- Mirror `MSTATE_*` from `original/lib/multihandle.h` with an explicit Rust enum and state-transition functions so behavior stays inspectable and testable.
- The connection-cache key must include all fields that affect identity and reuse safety, including host, port, proxy/tunnel state, `conn_to` overrides, TLS peer identity, authentication context, and share-handle state needed to avoid the CVE classes around incorrect reuse.
- Recreate the connection-filter chain from `original/lib/cfilters.h` using Rust trait objects or enums, with unsafe code only at the raw socket and backend boundaries.
- Implement `curl_multi_init`, `curl_multi_cleanup`, `curl_multi_add_handle`, `curl_multi_remove_handle`, `curl_multi_fdset`, `curl_multi_perform`, `curl_multi_wait`, `curl_multi_poll`, `curl_multi_timeout`, `curl_multi_wakeup`, `curl_multi_info_read`, `curl_multi_socket`, `curl_multi_socket_all`, `curl_multi_socket_action`, `curl_multi_assign`, `curl_multi_get_handles`, and `curl_multi_strerror`.
- Implement the transport-facing portions of `curl_easy_pause`, `curl_easy_recv`, `curl_easy_send`, and `curl_easy_upkeep`.
- Preserve low-speed and timeout semantics from `original/lib/speedcheck.c` and `original/tests/data/test1606`, not just callback wiring.
- Ensure share-handle locking callbacks and shared-data selections from `curl_share_setopt` remain ABI-compatible even if some shared-resource implementations are completed in later phases.

## Verification Phases
### `check-transfer-core-curated`
- Type: `check`
- Bounce Target: `impl-transfer-core`
- Purpose: validate the easy/multi lifecycle, timer handling, poll/wakeup logic, timeout behavior, and connection reuse semantics with focused upstream libtests.
- Commands it should run:
```bash
bash safe/scripts/run-curated-libtests.sh --flavor openssl 530 582 1506 1550 1554 1557 1591 1597 1606 2402 2404
bash safe/scripts/run-curated-libtests.sh --flavor gnutls 530 582 1506 1550 1554 1557 1591 1597 1606 2402 2404
```

### `check-transfer-core-link`
- Type: `check`
- Bounce Target: `impl-transfer-core`
- Purpose: verify that original object files using easy and multi APIs can be relinked against the safe library without recompilation and that the relinked executables still run correctly.
- Commands it should run:
```bash
bash safe/scripts/run-link-compat.sh --flavor openssl --tests lib500 lib501 lib530 lib582 lib1550 lib1606 lib2402
bash safe/scripts/run-link-compat.sh --flavor gnutls --tests lib500 lib501 lib530 lib582 lib1550 lib1606 lib2402
```

## Success Criteria
- Every listed `Preexisting Input` is consumed as an existing artifact rather than rediscovered, regenerated, or refetched.
- Every listed `New Output` for this implement phase exists and is ready for downstream phases in the linear workflow.
- The verifier phase(s) `check-transfer-core-curated`, `check-transfer-core-link` pass exactly as written for `impl-transfer-core`.

## Git Commit Requirement
The implementer must commit this phase's work to git before yielding. Ignored-only or untracked-only outputs are not acceptable.
