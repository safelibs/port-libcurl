# Phase Name
Compatibility Harness Foundation, Upstream Asset Vendoring, Consumer Build Scaffolding, Link Harness, and Fixture Helpers

## Implement Phase ID
`impl-harness-foundation`

## Preexisting Inputs
- `safe/metadata/test-manifest.json`
- `safe/scripts/build-reference-curl.sh`
- `safe/include/curl/*.h`
- `safe/c_shim/forwarders.c`
- `original/src/Makefile.am`
- `original/src/Makefile.inc`
- the tracked files under `original/src/`
- the tracked files under `original/tests/`
- the tracked files under `original/.pc/90_gnutls.patch/`
- `original/debian/tests/LDAP-bindata.c`
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/src/lib.rs`
- `safe/src/abi/generated.rs`
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

## New Outputs
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

## File Changes
- Vendor the tracked upstream compatibility-source assets required by the tool/test/package harnesses into `safe/vendor/upstream/`.
- Add a compatibility-consumer build system that compiles manifest-recorded libcurl consumers against the safe library while preserving the original upstream link lines for auxiliary helper targets instead of reusing the upstream libcurl build directly.
- Add manifest-driven runners for curated libtests, full `runtests.pl`, HTTP client programs, object-file relinking and execution, the Debian LDAP dev-package compile test, and the build-state handoff that lets relink checks reuse the exact objects emitted by the compatibility build.
- Extract reusable loopback HTTP fixture logic from the root harness into `safe/scripts/` so later benchmarking, HTTP-client tests, and safe-package validation share one implementation.

## Implementation Details
- `safe/scripts/vendor-compat-assets.sh` should copy the exact manifest-backed compatibility-source inventory into `safe/vendor/upstream/`, preserving relative paths from the upstream tree. That inventory must include all tracked files under `original/src/` and `original/tests/`, the tracked files under `original/.pc/90_gnutls.patch/`, `original/debian/tests/LDAP-bindata.c`, and every tracked `original/lib/` helper source/header named by `CURLX_CFILES`, `CURLX_HFILES`, or the manifest-recorded include-dependency closure of the vendored consumer sources. The script must write `safe/vendor/upstream/manifest.json` with source path, destination path, git-tracked status, and content hash for every copied file, and it must fail if any required file is missing or untracked.
- `safe/compat/CMakeLists.txt` should compile, at minimum:
  - the upstream `curl` tool from `safe/vendor/upstream/src/*.c`
  - the vendored `CURLX_CFILES` and `CURLX_HFILES` helper set under `safe/vendor/upstream/lib/`
  - the 10 server helpers from `safe/vendor/upstream/tests/server/*.c`
  - all 256 `noinst_PROGRAMS` entries from the vendored `tests/libtest/Makefile.inc`
  - the 7 tracked HTTP client programs from `safe/vendor/upstream/tests/http/clients/*.c`
  - the vendored Debian LDAP dev-package test source from `safe/vendor/upstream/debian/tests/LDAP-bindata.c`
- The compatibility build should consume the exact consumer target metadata already recorded in `safe/metadata/test-manifest.json` as its primary input. The vendored `Makefile.am` and `Makefile.inc` files under `safe/vendor/upstream/` are validation baselines only; later phases must not rediscover compile flags, generated-source rules, or target membership by reparsing them once the manifest exists.
- `safe/scripts/export-tracked-tree.sh` must support exactly two explicit modes. `--safe-only --dest <dir>` exports only tracked files from `safe/` into `<dir>/` for detached package/autopkgtest builds. `--with-root-harness --dest <dir>` exports tracked files from `safe/` into `<dir>/safe/` and tracked root `dependents.json` into `<dir>/dependents.json` for the Docker dependent harness. Both modes must mirror the current `git ls-files` discipline and fail if they would include untracked files or if required tracked inputs are missing.
- `safe/scripts/build-compat-consumers.sh` must prepare isolated per-flavor work trees from `safe/vendor/upstream/`, reconstruct the OpenSSL variant from the vendored `.pc/90_gnutls.patch/` files, generate any required `curl_config.h`-style configure-time headers inside those work trees, and then swap only the library/header link target over to the safe build for manifest entries marked as actual libcurl consumers so the consumer compile surface matches upstream. For manifest entries marked as auxiliary helpers, it must preserve the original upstream non-libcurl link line rather than injecting the safe library. It must read exact per-target source, flag, and link-role metadata from `safe/metadata/test-manifest.json`, must not read `original/` or any dirty build outputs at runtime, and must emit a machine-readable per-flavor build-state file at `safe/.compat/<flavor>/build-state.json` recording each target id, generated-source output, resolved compile/link arguments, object-file path, and final executable path.
- `safe/scripts/run-upstream-tests.sh` should:
  - export a detached tracked `safe/` tree using `safe/scripts/export-tracked-tree.sh --safe-only`
  - build the manifest-recorded compatibility target set from `safe/vendor/upstream/` using the safe headers and each target's recorded upstream link contract
  - run `safe/vendor/upstream/tests/runtests.pl` for either a selected subset or the full manifest-backed list
  - in `--require-all-runtests` mode, execute every ordered `TESTCASES` token not disabled by the vendored `tests/data/DISABLED` rules applicable to the selected flavor, preserve the manifest order and duplicate tokens such as the duplicated `test1190`, and fail if any enabled token is silently omitted
  - in `--require-all-runtests` mode, honor the vendored `tests/data/DISABLED` file exactly as upstream `runtests.pl` does, refuse to pass `-f`, and emit the disabled-token set separately so the workflow can prove those ids were intentionally skipped rather than forgotten
  - reject reduced keyword filters in full-suite mode
- `safe/scripts/run-curated-libtests.sh` should be a stable wrapper for phase-specific libtest subsets so later verifiers do not embed ad hoc build logic.
- `safe/scripts/run-curl-tool-smoke.sh` should support both `--implementation compat` and `--implementation packaged`. It should execute the compatibility-built or packaged `curl` binary, as requested, against the shared loopback fixtures, covering at least download, upload, redirect following, and header output through public CLI options.
- `safe/scripts/run-http-client-tests.sh` should provision only the dependencies needed by the tracked HTTP client programs, using `safe/vendor/upstream/tests/http/config.ini.in` and `safe/vendor/upstream/tests/http/README.md` as guidance, and must not fabricate the absent pytest fixture tree.
- `safe/scripts/run-link-compat.sh` must consume the per-flavor build-state emitted by `safe/scripts/build-compat-consumers.sh`, reuse the already-built consumer `.o` files that were compiled from the manifest-recorded upstream metadata against the original-compatible public headers, relink those `.o` files against the safe shared libraries without recompiling the source, and then execute each relinked binary under the correct runtime contract for that target.
- The relink harness should reuse the existing runtime adapters instead of inventing new per-check logic: libtests should run with the same server/fixture setup used by `runtests.pl`, the `curl` tool should delegate to `safe/scripts/run-curl-tool-smoke.sh`, tracked HTTP clients should delegate to `safe/scripts/run-http-client-tests.sh`, and the LDAP dev-package consumer should delegate to `safe/scripts/run-ldap-devpkg-test.sh`. The harness must fail if a selected relink target has no declared runnable path or if the relinked executable exits nonzero.
- Once the vendored tree exists, later phases must consume `safe/vendor/upstream/` rather than `original/src/`, `original/tests/`, or `original/.pc/90_gnutls.patch/` when building the safe package, compatibility consumers, or packaged autopkgtests.
- Because the vendored `safe/vendor/upstream/src/Makefile.am` hardcodes `libcurl-gnutls.la` while the OpenSSL baseline is reconstructed from the vendored `.pc/90_gnutls.patch/` files, the compatibility build must select the correct source variant per flavor and parameterize the linked safe library instead of relying on the system `curl`.

## Verification Phases
### `check-harness-foundation-build`
- Type: `check`
- Bounce Target: `impl-harness-foundation`
- Purpose: confirm that the vendored safe-local compatibility build can compile every manifest-recorded compatibility target for both flavors, linking libcurl consumers against the safe library while preserving upstream non-libcurl link lines for auxiliary helpers such as `chkhostname` and the server programs.
- Commands it should run:
```bash
bash safe/scripts/build-compat-consumers.sh --flavor openssl --all
bash safe/scripts/build-compat-consumers.sh --flavor gnutls --all
```

### `check-harness-foundation-smoke`
- Type: `check`
- Bounce Target: `impl-harness-foundation`
- Purpose: verify that the harness scripts themselves work against the transitional bridge by running a small set of tracked libtests, relink-and-run checks, and upstream test ids.
- Commands it should run:
```bash
bash safe/scripts/run-curated-libtests.sh --flavor openssl 500 501 506
bash safe/scripts/run-curated-libtests.sh --flavor gnutls 500 501 506
bash safe/scripts/run-link-compat.sh --flavor openssl --tests lib500 lib501
bash safe/scripts/run-link-compat.sh --flavor gnutls --tests lib500 lib501
bash safe/scripts/run-upstream-tests.sh --flavor openssl --tests 1 506
bash safe/scripts/run-upstream-tests.sh --flavor gnutls --tests 1 506
bash safe/scripts/run-curl-tool-smoke.sh --implementation compat --flavor openssl
bash safe/scripts/run-curl-tool-smoke.sh --implementation compat --flavor gnutls
```

## Success Criteria
- Every listed `Preexisting Input` is consumed as an existing artifact rather than rediscovered, regenerated, or refetched.
- Every listed `New Output` for this implement phase exists and is ready for downstream phases in the linear workflow.
- The verifier phase(s) `check-harness-foundation-build`, `check-harness-foundation-smoke` pass exactly as written for `impl-harness-foundation`.

## Git Commit Requirement
The implementer must commit this phase's work to git before yielding. Ignored-only or untracked-only outputs are not acceptable.
