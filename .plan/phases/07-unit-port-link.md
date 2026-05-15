### 7. Ported Upstream Unit Tests and Broad Object Link Compatibility

**Phase Name:** Ported Upstream Unit Tests and Broad Object Link Compatibility

**Implement Phase ID:** `impl-unit-port-link`

**Verification Phases:**

- `check-unit-port`
  - Type: `check`
  - Fixed `bounce_target`: `impl-unit-port-link`
  - Purpose: verify all 46 upstream unit-source cases are represented by Rust tests and source-marker checks.
  - Commands:
    ```bash
    CARGO_TARGET_DIR=safe/target/check-unit-port-openssl \
      cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test unit_port
    CARGO_TARGET_DIR=safe/target/check-unit-port-gnutls \
      cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test unit_port
    python3 - <<'PY'
    import json
    from pathlib import Path
    manifest = json.loads(Path("safe/metadata/test-manifest.json").read_text())
    cases = sorted(p.stem for p in Path("safe/tests/unit_port_cases").glob("unit*.json"))
    expected = sorted(manifest["units"]["source_ids"])
    assert cases == expected, (len(cases), len(expected))
    PY
    ```

- `check-link-compat-broad`
  - Type: `check`
  - Fixed `bounce_target`: `impl-unit-port-link`
  - Purpose: relink and run every runnable object-compatibility entry for both flavors.
  - Commands:
    ```bash
    python3 - <<'PY'
    import json
    from pathlib import Path

    tests = json.loads(Path("safe/metadata/test-manifest.json").read_text())
    manifest = json.loads(Path("safe/compat/link-manifest.json").read_text())
    all_entries = {entry["id"] for entry in manifest["entries"]}
    target_ids = {entry["target_id"] for entry in manifest["entries"]}
    selected = set(manifest["sets"]["all-objects"]["entries"])
    targets = tests["compatibility_build"]["targets"]
    expected = {
        target["target_id"]
        for target in targets
        if target.get("role") == "libcurl-consumer"
    }
    expected.add("src:curl")
    helper_ids = {
        target["target_id"]
        for target in targets
        if target.get("role") == "helper"
    }
    required = {"http-client:ws-data", "http-client:ws-pingpong", "src:curl"}
    if len(expected) != 263:
        raise SystemExit(f"expected current metadata to derive 263 link targets, found {len(expected)}")
    if not required <= expected:
        raise SystemExit(f"derived link target set is missing required entries: {sorted(required - expected)}")
    if target_ids != expected:
        missing = sorted(expected - target_ids)[:20]
        extra = sorted(target_ids - expected)[:20]
        raise SystemExit(f"link manifest target coverage drifted; missing={missing} extra={extra}")
    if target_ids & helper_ids:
        raise SystemExit(f"link manifest includes helper-only targets: {sorted(target_ids & helper_ids)[:20]}")
    if len(manifest["entries"]) != len(expected):
        raise SystemExit(f"expected {len(expected)} object link entries, found {len(manifest['entries'])}")
    if selected != all_entries:
        missing = sorted(all_entries - selected)[:20]
        extra = sorted(selected - all_entries)[:20]
        raise SystemExit(f"all-objects set drifted; missing={missing} extra={extra}")
    if len(manifest["sets"].get("curated-broad", {}).get("entries", [])) >= len(selected):
        raise SystemExit("curated-broad is no longer the small smoke set; keep broad checks on all-objects explicitly")
    PY
    bash safe/scripts/build-compat-consumers.sh --flavor openssl --all
    bash safe/scripts/build-compat-consumers.sh --flavor gnutls --all
    bash safe/scripts/run-link-compat.sh --flavor openssl --set all-objects
    bash safe/scripts/run-link-compat.sh --flavor gnutls --set all-objects
    ```

**Preexisting Inputs:**

- `safe/src/version.rs`, `safe/src/tls/openssl.rs`, `safe/src/tls/gnutls.rs`, `safe/src/tls/certinfo.rs`, `safe/src/ssh/mod.rs`, `safe/src/vquic/mod.rs`, `safe/src/doh.rs`, `safe/src/idn.rs`, `safe/src/protocols/mod.rs`, `safe/src/protocols/rtmp.rs`, `safe/c_shim/http2_transport.c`, `safe/c_shim/tls_backend.c`, and `safe/c_shim/ssh_backend.c`.
- `safe/scripts/verify-protocol-feature-contract.py`, `safe/scripts/rtmp-fixture.py`, `safe/scripts/run-rtmp-functional-tests.sh`, `safe/scripts/run-ldaps-functional-test.sh`, `safe/scripts/run-http-client-tests.sh`, and `safe/scripts/run-websocket-disabled-smoke.sh`.
- `original/tests/unit/*.c`, `original/tests/unit/Makefile.inc`, `original/tests/libtest/first.c`, `original/tests/libtest/test.h`, `safe/tests/port-map.json`, `safe/tests/unit_port_cases/*.json`, `safe/tests/unit_port.rs`, `safe/compat/link-manifest.json`.

**New Outputs:**

- Complete unit-port case inventory under `safe/tests/unit_port_cases/`.
- Updated `safe/tests/unit_port.rs` for all upstream units.
- Updated `safe/tests/port-map.json` if source markers or coverage drift.
- Updated `safe/compat/link-manifest.json` to cover every object relink case derived from `safe/metadata/test-manifest.json`: every `role == "libcurl-consumer"` target plus `src:curl`, with all `role == "helper"` targets excluded. The `all-objects` set must equal the full manifest entry set and currently contains exactly 263 entries; `curated-broad` remains only a small smoke subset and must not be used for broad compatibility proof.

**File Changes:**

- `safe/tests/unit_port.rs`: add native Rust tests for unit cases and assert source markers still exist in the vendored/original inputs.
- `safe/tests/unit_port_cases/*.json`: one case per upstream unit source id with source path, kind, upstream status, Rust test name, summary, and source markers.
- `safe/tests/port-map.json`: inventory must match `safe/metadata/test-manifest.json`.
- `safe/compat/link-manifest.json`: derive entries from `safe/metadata/test-manifest.json`, include all 262 current libcurl-consumer targets plus `src:curl`, include `http-client:ws-data` and `http-client:ws-pingpong` as disabled-WebSocket relink entries, include `libtest:libauthretry`, `libtest:libntlmconnect`, and `libtest:libprereq` instead of silently omitting them, exclude helper-only targets such as `chkhostname` and the server helpers, and keep `sets.all-objects.entries` exactly equal to the complete `entries[].id` set.

**Implementation Details:**

- Unit-port tests must check Rust behavior directly, not merely compile old C units.
- Source-marker checks ensure future upstream drift is noticed.
- Broad link compatibility must use objects compiled from vendored upstream C sources and relink them against safe libraries through `safe/scripts/run-link-compat.sh --set all-objects`. The small curated smoke subset is not a broad proof and must not be used by the broad or final link verifiers. If a target requires a special runtime fixture, add the fixture adapter to `safe/scripts/run-link-compat.sh`; do not remove a `libcurl-consumer` target from `all-objects` to avoid writing the adapter.
- Implementer must commit this phase's work before yielding.

**Verification:**

- Both unit and broad link checks must pass.

**Success Criteria:**

- Every item listed under `New Outputs` is present, updated, or explicitly left unchanged because it already satisfied the plan.
- Every required `File Changes` and `Implementation Details` invariant for this phase is satisfied.
- Every verifier listed under `Verification Phases` passes exactly as written and bounces only to this phase on failure.
- All listed `Preexisting Inputs` are consumed in place; existing artifacts are not rediscovered, refetched, or regenerated from untracked sources unless this phase explicitly updates a derived safe artifact from them.

**Git Commit Requirement:**

- The implementer must commit this phase's work to git before yielding.
