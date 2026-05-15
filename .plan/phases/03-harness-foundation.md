### 3. Compatibility Harness Foundation, Vendored Upstream Assets, Object Capture, and Fixture Scripts

**Phase Name:** Compatibility Harness Foundation, Vendored Upstream Assets, Object Capture, and Fixture Scripts

**Implement Phase ID:** `impl-harness-foundation`

**Verification Phases:**

- `check-harness-vendor-export`
  - Type: `check`
  - Fixed `bounce_target`: `impl-harness-foundation`
  - Purpose: prove the vendored compatibility assets and detached exports consume tracked inputs only.
  - Commands:
    ```bash
    bash safe/scripts/vendor-compat-assets.sh --check
    git diff --exit-code -- safe/vendor/upstream safe/metadata/test-manifest.json safe/compat/config
    python3 - <<'PY'
    import os
    import stat
    import subprocess
    from pathlib import Path

    root = Path("safe/compat/config")
    expected_protocols = {"HTTP", "HTTPS", "GOPHERS", "LDAP", "LDAPS", "RTMP"}
    forbidden_protocols = {"WS", "WSS"}
    for flavor in ("openssl", "gnutls"):
        base = root / flavor
        required = [
            base / "lib" / "curl_config.h",
            base / "tests" / "config",
            base / "curl-config",
        ]
        missing = [path.as_posix() for path in required if not path.exists()]
        if missing:
            raise SystemExit(f"{flavor} missing compatibility config artifacts: {missing}")
        mode = (base / "curl-config").stat().st_mode
        if not mode & stat.S_IXUSR:
            raise SystemExit(f"{base / 'curl-config'} is not executable")
        combined = "\n".join(path.read_text(errors="ignore") for path in required)
        for needle in ("/home/", "../original", "/original", ".reference", "libcurl-reference"):
            if needle in combined:
                raise SystemExit(f"{flavor} compatibility config embeds forbidden path or sidecar marker: {needle}")
        curl_config_h = (base / "lib" / "curl_config.h").read_text(errors="ignore")
        if "#define USE_WEBSOCKETS" in curl_config_h:
            raise SystemExit(f"{flavor} compatibility curl_config.h must keep USE_WEBSOCKETS undefined to match Ubuntu")
        result = subprocess.run(
            [(base / "curl-config").as_posix(), "--protocols"],
            text=True,
            capture_output=True,
            check=False,
        )
        if result.returncode != 0:
            raise SystemExit(f"{flavor} curl-config --protocols failed: {result.stderr}")
        protocols = set(result.stdout.split())
        missing_protocols = expected_protocols - protocols
        if missing_protocols:
            raise SystemExit(f"{flavor} curl-config --protocols missing {sorted(missing_protocols)}")
        unexpected = forbidden_protocols & protocols
        if unexpected:
            raise SystemExit(f"{flavor} curl-config --protocols must not advertise WebSocket schemes: {sorted(unexpected)}")
    PY
    python3 safe/scripts/verify-manifests.py \
      --abi safe/metadata/abi-manifest.json \
      --tests safe/metadata/test-manifest.json \
      --cves safe/metadata/cve-manifest.json
    tmpdir="$(mktemp -d)"
    bash safe/scripts/export-tracked-tree.sh --safe-only --dest "$tmpdir/safe-only"
    test -f "$tmpdir/safe-only/Cargo.toml"
    test -f "$tmpdir/safe-only/debian/control"
    test ! -e "$tmpdir/safe-only/../original"
    bash safe/scripts/export-tracked-tree.sh --with-root-harness --dest "$tmpdir/with-root-harness"
    test -f "$tmpdir/with-root-harness/safe/Cargo.toml"
    test -f "$tmpdir/with-root-harness/dependents.json"
    test -f "$tmpdir/with-root-harness/test-original.sh"
    rm -rf "$tmpdir"
    ```

- `check-harness-consumer-build`
  - Type: `check`
  - Fixed `bounce_target`: `impl-harness-foundation`
  - Purpose: build the curl tool, a representative curated libtest, a representative tracked HTTP client, and object metadata against the safe libraries for both flavors.
  - Commands:
    ```bash
    bash safe/scripts/build-compat-consumers.sh --flavor openssl --target src:curl --target libtest:lib500 --target http-client:h2-download
    bash safe/scripts/build-compat-consumers.sh --flavor gnutls --target src:curl --target libtest:lib500 --target http-client:h2-download
    jq -e '.targets[] | select(.target_id=="src:curl") | .object_paths | length > 0' safe/.compat/openssl/build-state.json
    jq -e '.targets[] | select(.target_id=="libtest:lib500") | .object_paths | length > 0' safe/.compat/openssl/build-state.json
    jq -e '.targets[] | select(.target_id=="http-client:h2-download") | .object_paths | length > 0' safe/.compat/openssl/build-state.json
    jq -e '.targets[] | select(.target_id=="src:curl") | .object_paths | length > 0' safe/.compat/gnutls/build-state.json
    jq -e '.targets[] | select(.target_id=="libtest:lib500") | .object_paths | length > 0' safe/.compat/gnutls/build-state.json
    jq -e '.targets[] | select(.target_id=="http-client:h2-download") | .object_paths | length > 0' safe/.compat/gnutls/build-state.json
    bash safe/scripts/run-curl-tool-smoke.sh --implementation compat --flavor openssl
    bash safe/scripts/run-curl-tool-smoke.sh --implementation compat --flavor gnutls
    ```

**Preexisting Inputs:**

- `safe/src/alloc.rs`, `safe/src/global.rs`, `safe/src/version.rs`, `safe/src/slist.rs`, `safe/src/share.rs`, `safe/src/urlapi.rs`, `safe/src/mime.rs`, `safe/src/form.rs`, `safe/src/easy/`, `safe/c_shim/variadic.c`, `safe/c_shim/mprintf.c`, `safe/tests/public_abi.rs`, `safe/tests/abi_layout.rs`, and `safe/tests/smoke/public_api_smoke.c`.
- `safe/include/curl/`, `safe/metadata/abi-manifest.json`, `safe/metadata/test-manifest.json`, `safe/metadata/cve-manifest.json`, `safe/abi/libcurl-openssl.map`, and `safe/abi/libcurl-gnutls.map`.
- `safe/scripts/compat_harness.py`, `safe/scripts/vendor-compat-assets.sh`, `safe/scripts/export-tracked-tree.sh`, `safe/scripts/build-compat-consumers.sh`, `safe/scripts/run-upstream-tests.sh`, `safe/scripts/run-curated-libtests.sh`, `safe/scripts/run-link-compat.sh`, `safe/scripts/run-curl-tool-smoke.sh`, `safe/scripts/run-http-client-tests.sh`, `safe/scripts/run-ldap-devpkg-test.sh`, `safe/scripts/http-fixture.py`, `safe/scripts/http-fixtures.sh`.
- `safe/vendor/upstream/manifest.json` and the tracked `safe/vendor/upstream/` source tree.
- `original/configure`, `original/configure.ac`, `original/curl-config.in`, `original/lib/curl_config.h.in`, `original/tests/config.in`, `original/debian/rules`, and tracked files under `original/.pc/90_gnutls.patch/` as the source inputs for the committed compatibility config artifacts.

**New Outputs:**

- Updated vendored inventory and harness scripts if gaps are found.
- A non-mutating `--check` mode for `safe/scripts/vendor-compat-assets.sh` that verifies `safe/vendor/upstream/` and its manifest against tracked original inputs without rewriting them.
- Committed per-flavor compatibility config artifacts under `safe/compat/config/openssl/` and `safe/compat/config/gnutls/`, each containing `lib/curl_config.h`, `tests/config`, and an executable `curl-config`.
- Stable `safe/.compat/<flavor>/build-state.json` schema produced at runtime by checks.
- Updated `safe/compat/link-manifest.json` if relink coverage is incomplete.

**File Changes:**

- `safe/scripts/compat_harness.py`: preserve source fingerprinting, non-mutating vendored-asset verification, vendoring for implementer use, safe-only export, root-harness export, reference build staging while the transitional bridge still exists, worktree sync, generated-source handling, object compilation, link argument expansion, and build-state output. For configured upstream metadata, copy only from `safe/compat/config/<flavor>/` into `safe/.compat/<flavor>/worktree`; do not read `safe/.reference/<flavor>/source/upstream/lib/curl_config.h`, `tests/config`, or `curl-config`. Remove the current compatibility-harness rewrite that turns `/* #undef USE_WEBSOCKETS */` into `#define USE_WEBSOCKETS 1`; WebSocket client sources must compile their original disabled branch under the safe Ubuntu contract.
- `safe/scripts/vendor-compat-assets.sh`: support `--check` for verifiers and the existing mutating vendor/update mode for implementers. Checkers must use `--check`; only implementers may rewrite `safe/vendor/upstream/`.
- `safe/compat/config/{openssl,gnutls}/`: maintain tracked derived configured files generated from the tracked Ubuntu configure inputs, Debian flavor flags, `original/curl-config.in`, `original/tests/config.in`, `original/lib/curl_config.h.in`, and the pre-`90_gnutls.patch` OpenSSL inputs where applicable. These files are generated or refreshed in this phase, committed to git, and consumed unchanged by later phases; later checkers must not run `configure` or rebuild them from `.reference`.
- `safe/scripts/build-compat-consumers.sh`: build selected or all manifest targets with explicit flavor.
- `safe/scripts/run-link-compat.sh`: relink captured original objects to the safe library and run the appropriate runtime adapter.
- `safe/scripts/run-upstream-tests.sh`: drive vendored `runtests.pl` through safe-built `curl` and safe libraries, with `--require-all-runtests` support.
- `safe/scripts/http-fixture.py` and `safe/scripts/http-fixtures.sh`: provide deterministic loopback HTTP fixtures shared by curl-tool, HTTP-client, CVE, and benchmark checks.

**Implementation Details:**

- The harness must never compile consumers from dirty `original/` output directories. It must sync from `safe/vendor/upstream/` into `safe/.compat/<flavor>/worktree`.
- The harness must treat `safe/compat/config/<flavor>/` as the sole source for configured upstream metadata. `lib/curl_config.h` must keep `USE_WEBSOCKETS` undefined, the tracked WebSocket client programs must compile their disabled branch, and the paired `curl-config --protocols` output must preserve the Ubuntu tooling contract and omit `WS` and `WSS`.
- `safe/scripts/build-reference-curl.sh` may build sidecar references only from `safe/vendor/upstream/`; it must not read sibling `original/`.
- The OpenSSL worktree must restore pre-`90_gnutls.patch` tracked files from `safe/vendor/upstream/.pc/90_gnutls.patch/`.
- `safe/.compat/` and `safe/.reference/` remain generated output and must not be required by package builds.
- Implementer must commit this phase's work before yielding.

**Verification:**

- The two harness checks must pass before transfer, HTTP, and link phases depend on object build-state files.

**Success Criteria:**

- Every item listed under `New Outputs` is present, updated, or explicitly left unchanged because it already satisfied the plan.
- Every required `File Changes` and `Implementation Details` invariant for this phase is satisfied.
- Every verifier listed under `Verification Phases` passes exactly as written and bounces only to this phase on failure.
- All listed `Preexisting Inputs` are consumed in place; existing artifacts are not rediscovered, refetched, or regenerated from untracked sources unless this phase explicitly updates a derived safe artifact from them.

**Git Commit Requirement:**

- The implementer must commit this phase's work to git before yielding.
