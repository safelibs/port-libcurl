### 1. Foundation Refresh, Manifests, Headers, ABI Maps, and Transitional Bridge

**Phase Name:** Foundation Refresh, Manifests, Headers, ABI Maps, and Transitional Bridge

**Implement Phase ID:** `impl-foundation-refresh`

**Verification Phases:**

- `check-foundation-manifests`
  - Type: `check`
  - Fixed `bounce_target`: `impl-foundation-refresh`
  - Purpose: ensure existing generated manifests, headers, vendor inventory, and ABI maps are deterministic and match the canonical original inputs.
  - Commands:
    ```bash
    bash scripts/check-layout.sh
    bash safe/scripts/verify-public-headers.sh --expected original/include/curl --actual safe/include/curl
    python3 safe/scripts/verify-manifests.py \
      --abi safe/metadata/abi-manifest.json \
      --tests safe/metadata/test-manifest.json \
      --cves safe/metadata/cve-manifest.json
    python3 safe/scripts/verify-cve-coverage.py
    test -f safe/debian/patches/series
    test "$(cat safe/debian/source/format)" = "3.0 (quilt)"
    ! rg -n "/home/[A-Za-z][A-Za-z0-9_-]*/" safe/metadata safe/vendor/upstream/manifest.json
    python3 - <<'PY'
    import json
    from pathlib import Path

    cves = json.loads(Path("safe/metadata/cve-manifest.json").read_text())
    tests = json.loads(Path("safe/metadata/test-manifest.json").read_text())
    control = Path("original/debian/tests/control").read_text()
    assert len(cves["curated_relevant_cves"]["cves"]) == 107
    assert len(cves["debian_patch_mappings"]) == 21
    expected_autopkgtests = ["upstream-tests-openssl", "upstream-tests-gnutls", "curl-ldapi-test"]
    assert tests["debian_tests"]["autopkgtest_names"] == expected_autopkgtests
    for name in expected_autopkgtests:
        assert f"Tests: {name}" in control
    PY
    ```

- `check-foundation-build`
  - Type: `check`
  - Fixed `bounce_target`: `impl-foundation-refresh`
  - Purpose: verify both flavor builds produce a shared library with the expected exported symbol set and version namespace.
  - Commands:
    ```bash
    CARGO_TARGET_DIR=safe/target/check-foundation-openssl \
      cargo build --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor
    CARGO_TARGET_DIR=safe/target/check-foundation-gnutls \
      cargo build --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor
    bash safe/scripts/verify-export-names.sh --expected original/debian/libcurl4t64.symbols --flavor openssl \
      --artifact safe/target/check-foundation-openssl/debug/libport_libcurl_safe.so
    bash safe/scripts/verify-export-names.sh --expected original/debian/libcurl3t64-gnutls.symbols --flavor gnutls \
      --artifact safe/target/check-foundation-gnutls/debug/libport_libcurl_safe.so
    bash safe/scripts/verify-symbol-versions.sh --expected original/debian/libcurl4t64.symbols --flavor openssl \
      --artifact safe/target/check-foundation-openssl/debug/libport_libcurl_safe.so
    bash safe/scripts/verify-symbol-versions.sh --expected original/debian/libcurl3t64-gnutls.symbols --flavor gnutls \
      --artifact safe/target/check-foundation-gnutls/debug/libport_libcurl_safe.so
    ```

**Preexisting Inputs:**

- `original/include/curl/curl.h`, `original/include/curl/curlver.h`, `original/include/curl/easy.h`, `original/include/curl/header.h`, `original/include/curl/mprintf.h`, `original/include/curl/multi.h`, `original/include/curl/options.h`, `original/include/curl/stdcheaders.h`, `original/include/curl/system.h`, `original/include/curl/typecheck-gcc.h`, `original/include/curl/urlapi.h`, and `original/include/curl/websockets.h`.
- `original/libcurl.def`, `original/lib/libcurl.vers.in`, `original/debian/libcurl4t64.symbols`, and `original/debian/libcurl3t64-gnutls.symbols`.
- `original/lib/Makefile.inc`, `original/tests/data/Makefile.inc`, `original/tests/data/DISABLED`, `original/tests/libtest/Makefile.inc`, `original/tests/unit/Makefile.inc`, `original/tests/http/clients/Makefile.inc`, `original/tests/server/Makefile.inc`, and `original/debian/tests/control`.
- `dependents.json`, `relevant_cves.json`, `all_cves.json`, `original/debian/patches/series`, and the tracked `original/debian/patches/CVE-*.patch` files.
- Existing `safe/Cargo.toml`, `safe/build.rs`, `safe/src/lib.rs`, `safe/src/abi/generated.rs`, `safe/src/abi/public_types.rs`, `safe/include/curl/`, `safe/abi/libcurl-openssl.map`, `safe/abi/libcurl-gnutls.map`, `safe/metadata/abi-manifest.json`, `safe/metadata/test-manifest.json`, `safe/metadata/cve-manifest.json`, `safe/scripts/generate-manifests.py`, `safe/scripts/generate-bindings.py`, `safe/scripts/verify-manifests.py`, `safe/scripts/verify-public-headers.sh`, `safe/scripts/verify-export-names.sh`, `safe/scripts/verify-symbol-versions.sh`, `safe/scripts/verify-cve-coverage.py`, `safe/c_shim/`, `safe/debian/source/format`, `safe/debian/patches/series`, and `safe/vendor/upstream/manifest.json`.

**New Outputs:**

- Updated `safe/metadata/abi-manifest.json`, `safe/metadata/test-manifest.json`, and `safe/metadata/cve-manifest.json` if deterministic regeneration shows drift.
- Updated `safe/src/abi/generated.rs` and `safe/src/abi/public_types.rs` if header-derived ABI scaffolding is stale.
- Updated `safe/abi/libcurl-openssl.map` and `safe/abi/libcurl-gnutls.map` if Debian `.symbols` inputs changed.
- Updated `safe/vendor/upstream/manifest.json` if tracked vendor inputs differ from the manifest.
- Updated verifier scripts if they currently miss a required invariant.

**File Changes:**

- Keep `safe/include/curl/*.h` byte-for-byte aligned with `original/include/curl/*.h`.
- Keep `safe/metadata/abi-manifest.json` recording 12 header hashes, 93 public function declarations, 93 symbols per flavor, 1483 enum discriminants, 313 macro aliases, 25 public structs, shared library names, version strings, and option metadata.
- Keep `safe/metadata/test-manifest.json` recording 1677 ordered testcase tokens, 1676 unique testcase files, the duplicate `test1190`, 256 libtests, 46 unit sources, the 3 enabled unit IDs, 7 HTTP clients, 10 server helpers, all 3 Debian autopkgtest stanzas (`upstream-tests-openssl`, `upstream-tests-gnutls`, `curl-ldapi-test`), the LDAP test source, curl tool sources, and the vendor inventory.
- `safe/metadata/test-manifest.json` must expose `debian_tests.autopkgtest_names` exactly as `["upstream-tests-openssl", "upstream-tests-gnutls", "curl-ldapi-test"]`; the existing dependent inventory and LDAP source-script fields are not a substitute for the autopkgtest-name contract.
- Keep `safe/metadata/cve-manifest.json` recording 107 entries under `curated_relevant_cves.cves` and 21 entries under `debian_patch_mappings`.
- Normalize generated metadata provenance to repository-relative paths; `safe/metadata/*` and `safe/vendor/upstream/manifest.json` must not contain absolute local checkout paths copied from `relevant_cves.json`, `all_cves.json`, or generator process state.
- Keep `safe/debian/source/format` as `3.0 (quilt)` and `safe/debian/patches/series` tracked.
- Ensure `safe/build.rs` never depends on dirty files under `original/`; it must consume `safe/debian/*.symbols`, `safe/metadata/abi-manifest.json`, `safe/vendor/upstream/manifest.json`, and tracked `safe/` sources.

**Implementation Details:**

- Treat current generator outputs as first-class source artifacts, not throwaway build results.
- Update `safe/scripts/generate-manifests.py` only if the deterministic manifest check is wrong or incomplete. It must parse Makefile variables structurally and preserve duplicate testcase tokens.
- Update `safe/scripts/generate-bindings.py` only if ABI structs, constants, callbacks, or architecture-conditioned type aliases are incomplete.
- Preserve the `safe/.cargo/cc-linker-wrapper.sh` workaround if needed for rustc/version-script interactions, but keep it relative-path only.
- Do not touch tracked files under `original/`.
- Implementer must commit this phase's work before yielding.

**Verification:**

- Run the two check phases exactly as listed.
- Inspect failures for stale derived artifacts first. Regenerate or patch the safe artifact; do not edit original inputs.

**Success Criteria:**

- Every item listed under `New Outputs` is present, updated, or explicitly left unchanged because it already satisfied the plan.
- Every required `File Changes` and `Implementation Details` invariant for this phase is satisfied.
- Every verifier listed under `Verification Phases` passes exactly as written and bounces only to this phase on failure.
- All listed `Preexisting Inputs` are consumed in place; existing artifacts are not rediscovered, refetched, or regenerated from untracked sources unless this phase explicitly updates a derived safe artifact from them.

**Git Commit Requirement:**

- The implementer must commit this phase's work to git before yielding.
