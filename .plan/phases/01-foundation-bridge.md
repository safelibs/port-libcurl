# Phase Name
Foundation, Manifests, ABI Maps, Package Skeleton, and Temporary Forwarder Bridge

## Implement Phase ID
`impl-foundation`

## Preexisting Inputs
- `original/include/curl/*.h`
- `original/include/curl/curlver.h`
- `original/libcurl.def`
- `original/lib/libcurl.vers.in`
- `original/lib/Makefile.inc`
- `original/src/Makefile.am`
- `original/src/Makefile.inc`
- the tracked files under `original/src/`
- `original/curl-config.in`
- `original/libcurl.pc.in`
- `original/debian/control`
- `original/debian/changelog`
- `original/debian/copyright`
- `original/debian/README.*`
- `original/debian/rules`
- `original/debian/source/format`
- `original/debian/*.install`
- `original/debian/*.links`
- `original/debian/*.docs`
- `original/debian/*.examples`
- `original/debian/*.lintian-overrides`
- `original/debian/*.manpages`
- `original/debian/libcurl4t64.symbols`
- `original/debian/libcurl3t64-gnutls.symbols`
- `original/debian/tests/*`
- `original/debian/patches/*.patch`
- `original/debian/patches/series`
- the tracked files under `original/.pc/90_gnutls.patch/`
- the tracked files under `original/tests/`
- `original/tests/data/Makefile.inc`
- `original/tests/libtest/Makefile.inc`
- `original/tests/server/Makefile.inc`
- `original/tests/unit/Makefile.inc`
- `original/tests/http/clients/Makefile.inc`
- `original/tests/http/config.ini.in`
- `original/tests/http/README.md`
- `original/debian/tests/LDAP-bindata.c`
- `dependents.json`
- `relevant_cves.json`

## New Outputs
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

## File Changes
- Create the `safe/` Rust package root with explicit `openssl-flavor` and `gnutls-flavor` features.
- Copy every installed public header from `original/include/curl/` into `safe/include/curl/` without semantic edits, including `curlver.h`, `stdcheaders.h`, `system.h`, and `typecheck-gcc.h`.
- Create manifest-generation scripts that extract ABI, test, dependent, and CVE metadata from the existing workspace files.
- Create Debian packaging skeleton files in `safe/debian/` by adapting the existing package layout instead of inventing a new one.
- Create an explicit safe-local quilt directory rooted at `safe/debian/patches/series` so detached package builds never depend on the original patch stack.
- Add a temporary exhaustive exported-symbol forwarder layer so every public symbol exists from the first build.
- Add a reference-build helper that can rebuild tracked original libcurl trees per flavor for temporary bridging without using dirty workspace outputs.

## Implementation Details
- `safe/metadata/abi-manifest.json` should record symbol names, symbol versions, sonames, shared-library filenames, header hashes, public function declarations, public struct names, every public enum discriminant and ABI-relevant macro alias exposed by the installed headers, version strings from `original/include/curl/curlver.h`, and option metadata derived from `original/include/curl/options.h` plus `original/lib/easyoptions.c`.
- `safe/metadata/test-manifest.json` should record:
  - the raw ordered `TESTCASES` token list from `original/tests/data/Makefile.inc`, including the duplicate `test1190`
  - the tracked `original/tests/data/DISABLED` contents, preserving unconditional and `%if`-conditional blocks and recording for each test id whether it must run via `runtests.pl`, is an intentional upstream-disabled skip for the selected flavor, or is additionally discharged elsewhere such as `safe/tests/unit_port.rs`
  - the canonical on-disk `original/tests/data/test*` file list
  - all 256 `noinst_PROGRAMS` entries from `original/tests/libtest/Makefile.inc`, preserving the names `chkhostname`, `libauthretry`, `libntlmconnect`, and `libprereq`
  - all 46 unit source ids from `original/tests/unit/Makefile.inc`
  - the currently enabled `UNITPROGS` subset
  - the 7 tracked HTTP client programs from `original/tests/http/clients/Makefile.inc`
  - the 10 tracked server-helper programs from `original/tests/server/Makefile.inc`
  - the tracked `original/src/` tool sources needed to build `curl`
  - stable target ids plus the exact compatibility-consumer build metadata extracted from `original/src/Makefile.am`, `original/src/Makefile.inc`, `original/tests/libtest/Makefile.am`, `original/tests/libtest/Makefile.inc`, `original/tests/server/Makefile.am`, `original/tests/server/Makefile.inc`, `original/tests/http/clients/Makefile.am`, and `original/tests/http/clients/Makefile.inc`, including common `AM_CPPFLAGS`, `AM_CFLAGS`, `AM_LDFLAGS`, `LDADD`, and `LIBS`; target-specific `*_CPPFLAGS`, `*_CFLAGS`, `*_LDFLAGS`, `*_LDADD`, and `*_DEPENDENCIES`; generated-source rules such as `tool_hugehelp.c` and `lib1521.c`; shared-source variant mappings such as `lib526.c` -> `lib526/lib527/lib532`, `lib544.c` -> `lib544/lib545`, `lib547.c` -> `lib547/lib548`, and `lib670.c` -> `lib670/lib671/lib672/lib673`; and a per-target role flag that distinguishes actual libcurl consumers from auxiliary non-libcurl helpers such as `chkhostname` and the server programs
  - the exact manifest-backed vendor inventory for `safe/vendor/upstream/`, including all tracked files under `original/src/` and `original/tests/`, the tracked files under `original/.pc/90_gnutls.patch/`, `original/debian/tests/LDAP-bindata.c`, and every tracked `original/lib/` helper source/header referenced by `CURLX_CFILES`, `CURLX_HFILES`, or the recorded include-dependency closure of the vendored consumer sources
  - the Debian test scripts and dependent names from `dependents.json`
  - an explicit note that `original/tests/http/README.md` references pytest assets that are not present in the tracked workspace
- `safe/metadata/cve-manifest.json` should copy the curated contents of `relevant_cves.json` and map each of the 21 Debian CVE patch files to its corresponding CVE id when possible.
- `safe/build.rs` should generate Linux version scripts from the Debian `.symbols` files, not only from `original/lib/libcurl.vers.in`, so the resulting namespaces remain `CURL_OPENSSL_4` and `CURL_GNUTLS_3`.
- `safe/scripts/verify-symbol-versions.sh` must validate the exported namespaces and the ELF SONAMEs `libcurl.so.4` and `libcurl-gnutls.so.4`, not only symbol spellings.
- Copy `original/debian/source/format` to `safe/debian/source/format`, preserving `3.0 (quilt)`, and create `safe/debian/patches/series` as a tracked empty or comment-only safe-local patch series. The original Debian patch stack remains reference input only until a later phase deliberately adds a safe-local quilt patch.
- Rust `#[repr(C)]` ABI scaffolding should be generated from the copied headers by `safe/scripts/generate-bindings.py` and checked into `safe/src/abi/generated.rs` with explicit target-conditioned branches for the Ubuntu package architectures, so package builds do not depend on libclang or a live header parser while `system.h`-controlled type layouts remain correct on non-amd64 builds.
- `safe/scripts/build-reference-curl.sh` should export tracked source files only and recreate per-flavor reference builds under `safe/.reference/`. It must name any helper shared libraries or archives `libcurl-reference-<flavor>.*` rather than reusing the public package filenames, and it must not depend on existing dirty `original/` build outputs.
- The temporary `safe/c_shim/forwarders.c` may bridge into a tracked-source reference C build during early phases, but it must be explicitly marked transitional and removed in the final phase.
- This phase should not edit tracked files under `original/`.

## Verification Phases
### `check-foundation-build`
- Type: `check`
- Bounce Target: `impl-foundation`
- Purpose: verify that the Rust package skeleton, copied public headers, version scripts, and temporary exhaustive ABI bridge build for both flavors without consuming dirty `original/` outputs.
- Commands it should run:
```bash
cargo build --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor
cargo build --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor
bash safe/scripts/verify-public-headers.sh --expected original/include/curl --actual safe/include/curl
bash safe/scripts/verify-export-names.sh --expected original/libcurl.def --flavor openssl
bash safe/scripts/verify-export-names.sh --expected original/libcurl.def --flavor gnutls
bash safe/scripts/verify-symbol-versions.sh --expected original/debian/libcurl4t64.symbols --flavor openssl
bash safe/scripts/verify-symbol-versions.sh --expected original/debian/libcurl3t64-gnutls.symbols --flavor gnutls
test "$(cat safe/debian/source/format)" = "3.0 (quilt)"
test -f safe/debian/patches/series
```

### `check-foundation-manifests`
- Type: `check`
- Bounce Target: `impl-foundation`
- Purpose: verify that the ABI, test, and CVE manifests exactly reflect the prepared workspace artifacts and preserve known quirks such as the duplicate `test1190` and missing pytest HTTP tree.
- Commands it should run:
```bash
python3 safe/scripts/verify-manifests.py \
  --abi safe/metadata/abi-manifest.json \
  --tests safe/metadata/test-manifest.json \
  --cves safe/metadata/cve-manifest.json
```

## Success Criteria
- Every listed `Preexisting Input` is consumed as an existing artifact rather than rediscovered, regenerated, or refetched.
- Every listed `New Output` for this implement phase exists and is ready for downstream phases in the linear workflow.
- The verifier phase(s) `check-foundation-build`, `check-foundation-manifests` pass exactly as written for `impl-foundation`.

## Git Commit Requirement
The implementer must commit this phase's work to git before yielding. Ignored-only or untracked-only outputs are not acceptable.
