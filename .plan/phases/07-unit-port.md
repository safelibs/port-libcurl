# Phase Name
Rust Unit Port and Broad Link/Object Compatibility

## Implement Phase ID
`impl-unit-port`

## Preexisting Inputs
- `safe/metadata/test-manifest.json`
- `safe/scripts/build-compat-consumers.sh`
- `safe/scripts/run-link-compat.sh`
- the tracked files under `original/tests/unit/`
- `original/tests/unit/Makefile.inc`
- `original/tests/libtest/first.c`
- `original/tests/libtest/test.h`
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/src/lib.rs`
- `safe/src/abi/generated.rs`
- `safe/include/curl/*.h`
- `safe/metadata/abi-manifest.json`
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

## New Outputs
- `safe/tests/unit_port.rs`
- `safe/tests/unit_port_cases/`
- `safe/tests/port-map.json`
- `safe/compat/link-manifest.json`

## File Changes
- Port the 46 internal unit source ids to Rust integration tests while preserving numeric ids and explicit source-to-port mappings.
- Extend the relink harness from targeted link-and-run tests to a broad curated object matrix derived from tracked source files.

## Implementation Details
- `safe/tests/port-map.json` should map each original `unitNNNN.c` source file to its Rust integration-test location and note whether the unit was part of upstream `UNITPROGS` or only present as a source input.
- `safe/tests/unit_port.rs` must execute the logical content of all 46 original unit ids, not just the 3 upstream-enabled `UNITPROGS`.
- `safe/compat/link-manifest.json` should define at least:
  - a curated broad set for phase-7 verification
  - a final `all-objects` set used in phase 10
- Each manifest set should refer to stable target ids already defined in `safe/metadata/test-manifest.json`; compile flags, generated-source rules, and translation-unit membership must come from that earlier manifest and the per-flavor compatibility-build state rather than being duplicated or rediscovered here.
- The link manifest should be derived from the tracked-target metadata in `safe/metadata/test-manifest.json`, not from ad hoc scans of build directories or prebuilt `.o` files.
- Each manifest entry must declare the relink target id, the target/object ids from `safe/metadata/test-manifest.json`, flavor applicability, and a runtime adapter such as `libtest`, `curl-tool-smoke`, `http-client`, or `ldap-devpkg`, plus any required test ids or client names. `safe/scripts/run-link-compat.sh` must first ensure that `safe/scripts/build-compat-consumers.sh` has emitted the matching per-flavor build-state, then resolve the actual `.o` paths from that state, execute the adapter after relinking, and fail if any selected entry lacks build metadata or runtime metadata.
- The final `all-objects` set must contain only runnable entries. Pure link-only diagnostics are allowed in non-final exploratory sets, but they must not satisfy the final link-compatibility proof.

## Verification Phases
### `check-unit-port`
- Type: `check`
- Bounce Target: `impl-unit-port`
- Purpose: run the Rust port of every original unit source id for both flavors.
- Commands it should run:
```bash
cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test unit_port
cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test unit_port
```

### `check-link-compat-curated`
- Type: `check`
- Bounce Target: `impl-unit-port`
- Purpose: validate the generalized link-compat harness across a broad tracked object-file set built from original consumer sources, including execution of the relinked binaries.
- Commands it should run:
```bash
bash safe/scripts/run-link-compat.sh --flavor openssl --all-curated
bash safe/scripts/run-link-compat.sh --flavor gnutls --all-curated
```

## Success Criteria
- Every listed `Preexisting Input` is consumed as an existing artifact rather than rediscovered, regenerated, or refetched.
- Every listed `New Output` for this implement phase exists and is ready for downstream phases in the linear workflow.
- The verifier phase(s) `check-unit-port`, `check-link-compat-curated` pass exactly as written for `impl-unit-port`.

## Git Commit Requirement
The implementer must commit this phase's work to git before yielding. Ignored-only or untracked-only outputs are not acceptable.
