### 10. Final Hardening, Unsafe Audit, and Full End-to-End Verification

**Phase Name:** Final Hardening, Unsafe Audit, and Full End-to-End Verification

**Implement Phase ID:** `impl-final-hardening`

**Verification Phases:**

- `check-final-hardening-full`
  - Type: `check`
  - Fixed `bounce_target`: `impl-final-hardening`
  - Purpose: rerun the full compatibility, security, packaging, dependent, and performance suite as the final linear workflow gate. The `Final Verification` section restates this command set and is not a separate generated phase.
  - Commands:
    ```bash
    bash scripts/check-layout.sh
    bash safe/scripts/verify-public-headers.sh --expected original/include/curl --actual safe/include/curl
    python3 safe/scripts/verify-manifests.py \
      --abi safe/metadata/abi-manifest.json \
      --tests safe/metadata/test-manifest.json \
      --cves safe/metadata/cve-manifest.json
    bash safe/scripts/verify-abi-manifest.sh safe/metadata/abi-manifest.json
    python3 safe/scripts/verify-cve-coverage.py
    test -f safe/debian/patches/series
    test "$(cat safe/debian/source/format)" = "3.0 (quilt)"
    ! rg -n "/home/[A-Za-z][A-Za-z0-9_-]*/" safe/metadata safe/vendor/upstream/manifest.json
    command -v nghttpx >/dev/null
    command -v python3 >/dev/null
    command -v cc >/dev/null
    command -v openssl >/dev/null
    command -v pkgconf >/dev/null
    command -v ldapsearch >/dev/null
    test -x /usr/sbin/slapd || command -v slapd >/dev/null
    pkgconf --exists ldap
    assert_dependent_safe_mode_executor() {
      command -v docker >/dev/null
      command -v git >/dev/null
      command -v jq >/dev/null
      test -c /dev/fuse
      docker info >/dev/null
      docker run --rm \
        --device /dev/fuse \
        --cap-add SYS_ADMIN \
        --security-opt apparmor:unconfined \
        ubuntu:24.04 \
        sh -ec 'test -c /dev/fuse'
    }
    assert_dependent_safe_mode_executor
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
    PY

    python3 - <<'PY'
    import stat
    import subprocess
    from pathlib import Path

    root = Path("safe/compat/config")
    expected_protocols = {"HTTP", "HTTPS", "GOPHERS", "LDAP", "LDAPS", "RTMP"}
    forbidden_protocols = {"WS", "WSS"}
    forbidden_text = ("/home/", "../original", "/original", ".reference", "libcurl-reference", "reference_backend")
    for flavor in ("openssl", "gnutls"):
        base = root / flavor
        required = [
            base / "lib" / "curl_config.h",
            base / "tests" / "config",
            base / "curl-config",
        ]
        missing = [path.as_posix() for path in required if not path.exists()]
        if missing:
            raise SystemExit(f"{flavor} missing final compatibility config artifacts: {missing}")
        if not (base / "curl-config").stat().st_mode & stat.S_IXUSR:
            raise SystemExit(f"{base / 'curl-config'} is not executable")
        combined = "\n".join(path.read_text(errors="ignore") for path in required)
        for needle in forbidden_text:
            if needle in combined:
                raise SystemExit(f"{flavor} compatibility config embeds forbidden reference/path marker: {needle}")
        if "#define USE_WEBSOCKETS" in (base / "lib" / "curl_config.h").read_text(errors="ignore"):
            raise SystemExit(f"{flavor} compatibility curl_config.h defines USE_WEBSOCKETS despite Ubuntu disabled-WebSocket contract")
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
            raise SystemExit(f"{flavor} compatibility curl-config missing protocols {sorted(missing_protocols)}")
        unexpected = forbidden_protocols & protocols
        if unexpected:
            raise SystemExit(f"{flavor} compatibility curl-config unexpectedly advertises WebSocket protocols: {sorted(unexpected)}")
    PY
    if rg -n 'safe/\.reference|\.reference/|reference_root|reference_config|reference_tests_config|reference_curl_config' \
      safe/scripts \
      -g '!build-reference-curl.sh' \
      -g '!benchmark-local.sh' \
      -g '!verify-*'; then
      echo "compatibility harness still depends on reference-sidecar configured metadata" >&2
      exit 1
    fi
    package_sidecar_forbidden='build-reference-curl|forwarders\.c|REFERENCE_LIBRARY|run_reference_build|libcurl-reference|safe/\.reference|(^|/)\.reference($|/)|\.reference/|reference_library_path|port_safe_resolve_reference_symbol|bridge_resolve_symbol|bridge_open_reference|reference_backend|reference backend|libcurl reference'
    if rg -n "$package_sidecar_forbidden" safe/build.rs safe/debian/rules safe/debian/*.install safe/debian/*.links scripts/build-debs.sh scripts/lib/build-deb-common.sh; then
      echo "final package build path still references transitional libcurl sidecar machinery" >&2
      exit 1
    fi
    package_consumer_sidecar_forbidden='safe/\.reference|(^|/)\.reference($|/)|\.reference/|libcurl-reference|reference_library_path|reference_root|reference_config|reference_curl_config|reference_tests_config|port_safe_resolve_reference_symbol|bridge_resolve_symbol|bridge_open_reference|REFERENCE_LIBRARY|run_reference_build|build-reference-curl|forwarders\.c|reference_backend|reference backend|libcurl reference'
    package_consumer_paths=()
    for path in \
      safe/scripts/run-public-abi-smoke.sh \
      safe/scripts/compat_harness.py \
      safe/scripts/export-tracked-tree.sh \
      safe/scripts/build-compat-consumers.sh \
      safe/scripts/run-link-compat.sh \
      safe/scripts/run-upstream-tests.sh \
      safe/scripts/run-curated-libtests.sh \
      safe/scripts/run-curl-tool-smoke.sh \
      safe/scripts/run-http-client-tests.sh \
      safe/scripts/run-websocket-disabled-smoke.sh \
      safe/scripts/run-ldap-devpkg-test.sh \
      safe/scripts/run-ldaps-functional-test.sh \
      safe/scripts/run-rtmp-functional-tests.sh \
      safe/debian/tests/control \
      safe/debian/tests/upstream-tests-openssl \
      safe/debian/tests/upstream-tests-gnutls \
      safe/debian/tests/curl-ldapi-test \
      safe/debian/tests/LDAP-bindata.c \
      scripts/run-upstream-tests.sh \
      scripts/run-port-tests.sh \
      scripts/run-validation-tests.sh \
      scripts/run-tests.sh \
      test-original.sh
    do
      test -e "$path" || { echo "missing final package-consuming path: $path" >&2; exit 1; }
      package_consumer_paths+=("$path")
    done
    if rg -n "$package_consumer_sidecar_forbidden" "${package_consumer_paths[@]}"; then
      echo "final package-consuming path still references transitional libcurl sidecar machinery" >&2
      exit 1
    fi
    assert_no_sidecar_outputs() {
      local label="$1"
      shift
      local path
      for path in "$@"; do
        [ -e "$path" ] || continue
        if find "$path" \( -type d -name .reference -o -name 'libcurl-reference-*' \) -print -quit | grep -q .; then
          echo "$label produced a .reference directory or libcurl-reference artifact under $path" >&2
          exit 1
        fi
        local -a marker_files=()
        mapfile -d '' marker_files < <(
          find "$path" -type f \
            ! -path '*/.git/*' \
            ! -path '*/.plan/*' \
            ! -path '*/safe/scripts/build-reference-curl.sh' \
            ! -path '*/safe/scripts/benchmark-local.sh' \
            ! -path '*/scripts/build-reference-curl.sh' \
            ! -path '*/scripts/benchmark-local.sh' \
            ! -path '*/safe/scripts/verify-*' \
            ! -path '*/scripts/verify-*' \
            -print0
        )
        if ((${#marker_files[@]})) && rg -a -n "$package_consumer_sidecar_forbidden" "${marker_files[@]}"; then
          echo "$label recorded sidecar resolver markers under $path" >&2
          exit 1
        fi
      done
    }
    rm -rf safe/.reference safe/.compat safe/target/public-abi safe/target/compat-consumers .work/validation

    test ! -e safe/src/easy/reference.rs
    test ! -e safe/c_shim/forwarders.c
    reference_forbidden='port_safe_resolve_reference_symbol|libcurl-reference|easy::reference|mod reference|load_reference|ReferenceRegistry|ReferenceHandle|reference_library_path|reference_backend|transitional bridge|build-reference-curl|REFERENCE_LIBRARY|BRIDGE_FLAVOR|bridge_open_reference|bridge_resolve_symbol|g_reference_handle|dlopen\('
    if rg -n "$reference_forbidden" safe/src safe/c_shim safe/build.rs safe/debian; then
      echo "unexpected transitional reference fallback remains" >&2
      exit 1
    fi
    if rg -n "$reference_forbidden|render_forwarders|forwarders\\.c" safe/scripts -g '!build-reference-curl.sh' -g '!benchmark-local.sh' -g '!verify-*'; then
      echo "unexpected safe-script dependency on transitional reference fallback remains" >&2
      exit 1
    fi
    if rg -n "reference_backend|reference backend|libcurl reference" safe/tests safe/metadata/cve-to-test.json; then
      echo "unexpected final CVE/test reference-backend placeholder remains" >&2
      exit 1
    fi
    python3 - <<'PY'
    import json
    from pathlib import Path

    mapping = json.loads(Path("safe/metadata/cve-to-test.json").read_text())
    bad = []
    for entry in mapping["mappings"]:
        case_file = entry.get("case_file", "")
        justification = entry.get("justification", "").lower()
        if "reference_backend" in case_file or "reference backend" in justification or "libcurl reference" in justification:
            bad.append(entry["cve_id"])
    if bad:
        raise SystemExit("final CVE mappings still depend on reference-backend placeholders: " + ", ".join(sorted(bad)))
    PY

    test -f safe/docs/unsafe-audit.md
    python3 - <<'PY'
    import re
    from pathlib import Path

    audit_path = Path("safe/docs/unsafe-audit.md")
    audit = audit_path.read_text()
    if not audit.strip():
        raise SystemExit("safe/docs/unsafe-audit.md is empty")

    boundary_line = re.compile(r'#\s*\[\s*(?:no_mangle|export_name)|\bunsafe\b|extern\s+"C"')
    required = {}
    for path in sorted(Path("safe/src").rglob("*.rs")):
        ids = []
        for lineno, line in enumerate(path.read_text(errors="ignore").splitlines(), 1):
            stripped = line.strip()
            if not stripped or stripped.startswith("//"):
                continue
            if boundary_line.search(line):
                ids.append(f"{path}:{lineno}")
        if ids:
            required[str(path)] = ids

    for path in sorted(Path("safe/c_shim").glob("*.c")):
        required[str(path)] = [f"{path}:c-shim"]

    missing = []
    for path, ids in required.items():
        pos = audit.find(path)
        if pos < 0:
            missing.append(f"{path}: missing audit section")
            continue
        next_heading = audit.find("\n##", pos + len(path))
        section = audit[pos:] if next_heading < 0 else audit[pos:next_heading]
        for marker in ("Purpose:", "Invariants:"):
            if marker not in section:
                missing.append(f"{path}: audit section missing {marker}")
        if "Necessity:" not in section and "Why unsafe remains:" not in section:
            missing.append(f"{path}: audit section missing Necessity or Why unsafe remains")
        for boundary_id in ids:
            if boundary_id not in section:
                missing.append(f"{boundary_id}: undocumented boundary id")

    documented_paths = set(re.findall(r"safe/(?:src/[^`\s:)]+\.rs|c_shim/[^`\s:)]+\.c)", audit))
    stale = sorted(path for path in documented_paths if not Path(path).exists())
    if stale:
        missing.extend(f"{path}: stale audit path" for path in stale)

    if missing:
        raise SystemExit("unsafe audit coverage failure:\n" + "\n".join(missing[:200]))
    print(f"unsafe audit covers {sum(len(v) for v in required.values())} Rust/C boundary ids across {len(required)} files")
    PY

    for flavor in openssl gnutls; do
      case "$flavor" in
        openssl)
          feature=openssl-flavor
          symbols=original/debian/libcurl4t64.symbols
          ;;
        gnutls)
          feature=gnutls-flavor
          symbols=original/debian/libcurl3t64-gnutls.symbols
          ;;
      esac
      artifact="safe/target/final-$flavor/debug/libport_libcurl_safe.so"
      CARGO_TARGET_DIR="safe/target/final-$flavor" \
        cargo test --manifest-path safe/Cargo.toml --no-default-features --features "$feature"
      CARGO_TARGET_DIR="safe/target/final-$flavor" \
        cargo build --manifest-path safe/Cargo.toml --no-default-features --features "$feature"
      CARGO_TARGET_DIR="safe/target/final-$flavor" \
        cargo clippy --manifest-path safe/Cargo.toml --no-default-features --features "$feature" --all-targets -- -D warnings
      test -f "$artifact"
      python3 safe/scripts/verify-protocol-feature-contract.py \
        --flavor "$flavor" \
        --artifact "$artifact" \
        --assert-routing
      bash safe/scripts/run-rtmp-functional-tests.sh \
        --flavor "$flavor" \
        --artifact "$artifact" \
        --schemes rtmp,rtmpe,rtmps,rtmpt,rtmpte,rtmpts \
        --require-download \
        --require-upload
      bash safe/scripts/run-ldaps-functional-test.sh \
        --flavor "$flavor" \
        --artifact "$artifact"
      if readelf -Wd "$artifact" | rg -n 'NEEDED.*libcurl-reference|(RPATH|RUNPATH).*(libcurl-reference|\.reference|/home/|\.\./original|/original|safe/target|(^|[:\[]|/)target/|\.compat|debian/build|/tmp/|/var/tmp/)|libcurl-reference|\.reference'; then
        echo "safe shared library has sidecar or local build/staging dependency/search path: $artifact" >&2
        exit 1
      fi
      bash safe/scripts/run-public-abi-smoke.sh --flavor "$flavor"
      assert_no_sidecar_outputs "final public ABI smoke for $flavor" safe/.reference safe/.compat "safe/target/public-abi/$flavor"
      bash safe/scripts/verify-export-names.sh --expected "$symbols" --flavor "$flavor" --artifact "$artifact"
      bash safe/scripts/verify-symbol-versions.sh --expected "$symbols" --flavor "$flavor" --artifact "$artifact"
      bash safe/scripts/build-compat-consumers.sh --flavor "$flavor" --all
      if jq -e '.. | strings | select(test("libcurl-reference|reference_library_path|reference_backend"))' "safe/.compat/$flavor/build-state.json"; then
        echo "compat safe staging still records transitional reference sidecar data for $flavor" >&2
        exit 1
      fi
      assert_no_sidecar_outputs "final compatibility staging for $flavor" safe/.reference safe/.compat safe/target/compat-consumers
      bash safe/scripts/run-link-compat.sh --flavor "$flavor" --set all-objects
      bash safe/scripts/run-upstream-tests.sh --flavor "$flavor" --require-all-runtests
      assert_no_sidecar_outputs "final upstream safe tests for $flavor" safe/.reference safe/.compat safe/target/compat-consumers
      bash safe/scripts/run-http-client-tests.sh --flavor "$flavor" --clients h2-download h2-pausing h2-serverpush h2-upgrade-extreme tls-session-reuse
      bash safe/scripts/run-websocket-disabled-smoke.sh --flavor "$flavor" --clients ws-data ws-pingpong
      assert_no_sidecar_outputs "final HTTP and disabled-WebSocket client tests for $flavor" safe/.reference safe/.compat safe/target/compat-consumers
    done

    export DEBIAN_FRONTEND=noninteractive
    export CARGO_NET_OFFLINE=true

    check_deb_payload_contract() {
      local deb_dir="$1"
      python3 safe/scripts/verify-package-payload-contract.py \
        --deb-dir "$deb_dir" \
        --require-safelibs-version
    }

    check_deb_control_contract() {
      local deb_dir="$1"
      local safe_control="$2"
      python3 safe/scripts/verify-debian-control-contract.py \
        --contract safe/metadata/debian-control-contract.json \
        --safe-control "$safe_control" \
        --original-control original/debian/control \
        --deb-dir "$deb_dir"
    }

    check_autopkgtest_control_contract() {
      python3 - <<'PY'
    from pathlib import Path

    def parse_control(path):
        stanzas = {}
        current = {}
        current_key = None
        for line in Path(path).read_text().splitlines():
            if not line.strip():
                if current:
                    stanzas[current["Tests"]] = current
                current = {}
                current_key = None
                continue
            if line[0].isspace():
                if current_key is None:
                    raise SystemExit(f"orphan continuation in {path}: {line}")
                current[current_key] += "\n" + line.rstrip()
                continue
            key, value = line.split(":", 1)
            current[key] = value.strip()
            current_key = key
        if current:
            stanzas[current["Tests"]] = current
        return stanzas

    expected_names = ["upstream-tests-openssl", "upstream-tests-gnutls", "curl-ldapi-test"]
    original = parse_control("original/debian/tests/control")
    safe = parse_control("safe/debian/tests/control")
    if sorted(safe) != sorted(expected_names):
        raise SystemExit(f"unexpected safe autopkgtest names: {sorted(safe)}")
    for name in expected_names:
        if name not in original:
            raise SystemExit(f"missing original autopkgtest reference: {name}")
        for field in ("Depends", "Restrictions"):
            if safe[name].get(field) != original[name].get(field):
                raise SystemExit(
                    f"{name} {field} drifted: expected {original[name].get(field)!r}, "
                    f"found {safe[name].get(field)!r}"
                )
    PY
    }

    check_installed_no_reference_sidecar() {
      local multiarch
      multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
      if find /usr -path '*libcurl-reference*' -print -quit | grep -q .; then
        echo "installed filesystem contains transitional libcurl reference sidecar" >&2
        exit 1
      fi
      local -a installed_artifacts
      mapfile -d '' installed_artifacts < <(find /usr/bin /usr/lib/"$multiarch" \( -name 'curl' -o -name 'libcurl*.so*' -o -name 'libcurl*.a' \) -print0)
      if ((${#installed_artifacts[@]})) && rg -a -n 'libcurl-reference|safe/\.reference|(^|/)\.reference($|/)|\.reference/|reference_library_path|port_safe_resolve_reference_symbol|bridge_resolve_symbol|bridge_open_reference' "${installed_artifacts[@]}"; then
        echo "installed curl/libcurl artifacts contain transitional reference sidecar marker" >&2
        exit 1
      fi
      while IFS= read -r -d '' link; do
        target="$(readlink "$link")"
        if printf '%s -> %s\n' "$link" "$target" | rg -n 'libcurl-reference|\.reference|/home/|\.\./original|/original|safe/target|(^|[[:space:]]|/)target/|\.compat|debian/build|/tmp/|/var/tmp/'; then
          echo "installed symlink points at a sidecar or local build/staging path: $link -> $target" >&2
          exit 1
        fi
      done < <(find /usr/bin /usr/lib/"$multiarch" \( -name 'curl' -o -name 'libcurl*.so*' -o -name 'libcurl*.a' \) -type l -print0)
      while IFS= read -r -d '' artifact; do
        if file "$artifact" | grep -Eq 'ELF .* (shared object|executable|pie executable)'; then
          if readelf -Wd "$artifact" | rg -n 'NEEDED.*libcurl-reference|(RPATH|RUNPATH).*(libcurl-reference|\.reference|/home/|\.\./original|/original|safe/target|(^|[:\[]|/)target/|\.compat|debian/build|/tmp/|/var/tmp/)|libcurl-reference|\.reference'; then
            echo "installed ELF contains sidecar or local build/staging dependency/search path: $artifact" >&2
            exit 1
          fi
        fi
      done < <(printf '%s\0' "${installed_artifacts[@]}")
    }

    run_detached_package_runtime_checks() {
      local src_dir="$1"
      local deb_dir="$2"
      local tmp_root="$3"
      mkdir -p "$tmp_root/autopkgtest-openssl" "$tmp_root/autopkgtest-gnutls" "$tmp_root/autopkgtest-ldap"
      (
        cd "$src_dir"
        sudo apt-get update
        sudo env DEBIAN_FRONTEND=noninteractive mk-build-deps -ir -t 'apt-get -y --no-install-recommends' debian/control
        sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
          "$deb_dir"/curl_*.deb \
          "$deb_dir"/libcurl4t64_*.deb \
          "$deb_dir"/libcurl4-openssl-dev_*.deb \
          "$deb_dir"/libcurl4-doc_*.deb \
          gcc libc-dev libldap-dev slapd ldap-utils openssl python3 pkgconf autoconf automake make
        dpkg-query -W -f='${Package} ${Version}\n' curl libcurl4t64 libcurl4-openssl-dev libcurl4-doc \
          | awk '$2 !~ /\+safelibs/ { print "non-safelibs package version: " $0 > "/dev/stderr"; bad=1 } END { exit bad }'
        test "$(dpkg-query -S /usr/bin/curl | cut -d: -f1)" = "curl"
        readelf -d /usr/bin/curl | grep -F 'Shared library: [libcurl.so.4]'
        curl_lib="$(ldd /usr/bin/curl | awk '/libcurl\.so\.4/ { print $3; exit }')"
        test -n "$curl_lib"
        dpkg -S "$curl_lib" | grep -E '^libcurl4t64(:|,)'
        multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
        test ! -e /usr/include/curl
        for header in curl.h curlver.h easy.h header.h mprintf.h multi.h options.h stdcheaders.h system.h typecheck-gcc.h urlapi.h websockets.h; do
          test -f "/usr/include/$multiarch/curl/$header"
        done
        check_installed_no_reference_sidecar
        test -e "/usr/lib/$multiarch/libcurl.so"
        test -f "/usr/lib/$multiarch/libcurl.a"
        command -v curl-config >/dev/null
        pkg-config --exists libcurl
        pkg_config_includedir="$(pkg-config --variable=includedir libcurl)"
        test "$pkg_config_includedir" = "/usr/include/$multiarch"
        pkg-config --cflags libcurl | grep -F -- "-I/usr/include/$multiarch"
        test -f /usr/share/aclocal/libcurl.m4
        bash scripts/verify-dev-tooling-contract.sh \
          --contract metadata/dev-tooling-contract.json \
          --flavor openssl \
          --expected-shared-lib libcurl.so.4
        bash scripts/run-rtmp-functional-tests.sh \
          --implementation packaged \
          --flavor openssl \
          --schemes rtmp,rtmpe,rtmps,rtmpt,rtmpte,rtmpts \
          --require-download \
          --require-upload
        bash scripts/run-ldaps-functional-test.sh \
          --implementation packaged \
          --flavor openssl
        AUTOPKGTEST_TMP="$tmp_root/autopkgtest-openssl" bash debian/tests/upstream-tests-openssl
        AUTOPKGTEST_TMP="$tmp_root/autopkgtest-ldap" bash debian/tests/curl-ldapi-test
        sudo env DEBIAN_FRONTEND=noninteractive apt-get purge -y libcurl4-openssl-dev
        sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
          "$deb_dir"/libcurl3t64-gnutls_*.deb \
          "$deb_dir"/libcurl4-gnutls-dev_*.deb
        dpkg-query -W -f='${Package} ${Version}\n' libcurl3t64-gnutls libcurl4-gnutls-dev \
          | awk '$2 !~ /\+safelibs/ { print "non-safelibs package version: " $0 > "/dev/stderr"; bad=1 } END { exit bad }'
        test ! -e /usr/include/curl
        for header in curl.h curlver.h easy.h header.h mprintf.h multi.h options.h stdcheaders.h system.h typecheck-gcc.h urlapi.h websockets.h; do
          test -f "/usr/include/$multiarch/curl/$header"
        done
        check_installed_no_reference_sidecar
        test -e "/usr/lib/$multiarch/libcurl-gnutls.so.4"
        test -e "/usr/lib/$multiarch/libcurl-gnutls.so.3"
        test -e "/usr/lib/$multiarch/libcurl-gnutls.so"
        test -f "/usr/lib/$multiarch/libcurl-gnutls.a"
        test -e "/usr/lib/$multiarch/libcurl.so"
        test -e "/usr/lib/$multiarch/libcurl.a"
        pkg_config_includedir="$(pkg-config --variable=includedir libcurl)"
        test "$pkg_config_includedir" = "/usr/include/$multiarch"
        pkg-config --cflags libcurl | grep -F -- "-I/usr/include/$multiarch"
        bash scripts/verify-dev-tooling-contract.sh \
          --contract metadata/dev-tooling-contract.json \
          --flavor gnutls \
          --expected-shared-lib libcurl-gnutls.so.4
        bash scripts/run-rtmp-functional-tests.sh \
          --implementation packaged \
          --flavor gnutls \
          --schemes rtmp,rtmpe,rtmps,rtmpt,rtmpte,rtmpts \
          --require-download \
          --require-upload
        bash scripts/run-ldaps-functional-test.sh \
          --implementation packaged \
          --flavor gnutls
        AUTOPKGTEST_TMP="$tmp_root/autopkgtest-gnutls" bash debian/tests/upstream-tests-gnutls
      )
      PACKAGED_CURL_BIN=/usr/bin/curl bash "$src_dir/scripts/run-curl-tool-smoke.sh" --implementation packaged
    }

    check_autopkgtest_control_contract
    root_cargo_home="$(mktemp -d)"
    export CARGO_HOME="$root_cargo_home"
    test "$CARGO_HOME" != "$HOME/.cargo"
    test -z "$(find "$CARGO_HOME" -mindepth 1 -maxdepth 1 -print -quit)"
    python3 safe/scripts/verify-cargo-source-policy.py \
      --manifest safe/Cargo.toml \
      --lock safe/Cargo.lock \
      --config safe/.cargo/config.toml \
      --vendor safe/vendor/cargo
    python3 safe/scripts/verify-package-no-refetch.py \
      --package-root safe \
      --root-build-script scripts/build-debs.sh \
      --root-build-helper scripts/lib/build-deb-common.sh
    python3 safe/scripts/verify-debian-control-contract.py \
      --contract safe/metadata/debian-control-contract.json \
      --safe-control safe/debian/control \
      --original-control original/debian/control
    python3 safe/scripts/verify-protocol-feature-contract.py \
      --contract safe/metadata/dev-tooling-contract.json \
      --package-root safe \
      --debian-control safe/debian/control \
      --check-source-deps-only
    rg -n -- '--locked' safe/debian/rules
    rg -n 'CARGO_NET_OFFLINE|--offline' safe/debian/rules
    rm -rf build dist safe/.reference safe/.compat .work/validation
    test -z "$(find "$CARGO_HOME" -mindepth 1 -maxdepth 1 -print -quit)"
    bash scripts/build-debs.sh
    git diff --exit-code -- safe/debian/changelog
    if find dist -maxdepth 1 -type f ! -name '*.deb' -print -quit | grep -q .; then
      echo "scripts/build-debs.sh must leave only .deb artifacts in dist/" >&2
      find dist -maxdepth 1 -type f ! -name '*.deb' -print >&2
      exit 1
    fi
    check_deb_payload_contract dist
    check_deb_control_contract dist safe/debian/control
    assert_no_sidecar_outputs "final root package build output" dist safe/.reference safe/.compat .work/validation
    rm -rf "$root_cargo_home"
    unset CARGO_HOME

    detached_tmp="$(mktemp -d)"
    bash safe/scripts/export-tracked-tree.sh --safe-only --dest "$detached_tmp/curl"
    test ! -e "$detached_tmp/original"
    test ! -e "$detached_tmp/curl/../original"
    detached_package_consumer_paths=()
    for path in \
      "$detached_tmp/curl/scripts/run-public-abi-smoke.sh" \
      "$detached_tmp/curl/scripts/compat_harness.py" \
      "$detached_tmp/curl/scripts/export-tracked-tree.sh" \
      "$detached_tmp/curl/scripts/build-compat-consumers.sh" \
      "$detached_tmp/curl/scripts/run-link-compat.sh" \
      "$detached_tmp/curl/scripts/run-upstream-tests.sh" \
      "$detached_tmp/curl/scripts/run-curated-libtests.sh" \
      "$detached_tmp/curl/scripts/run-curl-tool-smoke.sh" \
      "$detached_tmp/curl/scripts/run-http-client-tests.sh" \
      "$detached_tmp/curl/scripts/run-websocket-disabled-smoke.sh" \
      "$detached_tmp/curl/scripts/run-ldap-devpkg-test.sh" \
      "$detached_tmp/curl/scripts/run-ldaps-functional-test.sh" \
      "$detached_tmp/curl/scripts/run-rtmp-functional-tests.sh" \
      "$detached_tmp/curl/debian/tests/control" \
      "$detached_tmp/curl/debian/tests/upstream-tests-openssl" \
      "$detached_tmp/curl/debian/tests/upstream-tests-gnutls" \
      "$detached_tmp/curl/debian/tests/curl-ldapi-test" \
      "$detached_tmp/curl/debian/tests/LDAP-bindata.c"
    do
      test -e "$path" || { echo "missing final detached safe-only package-consuming path: $path" >&2; exit 1; }
      detached_package_consumer_paths+=("$path")
    done
    if rg -n "$package_consumer_sidecar_forbidden" "${detached_package_consumer_paths[@]}"; then
      echo "final detached safe-only export package-consuming path still references transitional libcurl sidecar machinery" >&2
      exit 1
    fi
    assert_no_sidecar_outputs "final detached safe-only export before package build" "$detached_tmp/curl"
    detached_cargo_home="$detached_tmp/cargo-home"
    mkdir -p "$detached_cargo_home"
    test "$detached_cargo_home" != "$HOME/.cargo"
    test -z "$(find "$detached_cargo_home" -mindepth 1 -maxdepth 1 -print -quit)"
    (
      cd "$detached_tmp/curl"
      export CARGO_HOME="$detached_cargo_home"
      python3 scripts/verify-cargo-source-policy.py \
        --manifest Cargo.toml \
        --lock Cargo.lock \
        --config .cargo/config.toml \
        --vendor vendor/cargo
      python3 scripts/verify-package-no-refetch.py --package-root .
      if rg -n "$package_sidecar_forbidden" build.rs debian/rules debian/*.install debian/*.links; then
        echo "final detached package build path still references transitional libcurl sidecar machinery" >&2
        exit 1
      fi
      python3 scripts/verify-debian-control-contract.py \
        --contract metadata/debian-control-contract.json \
        --safe-control debian/control
      python3 scripts/verify-protocol-feature-contract.py \
        --contract metadata/dev-tooling-contract.json \
        --package-root . \
        --debian-control debian/control \
        --check-source-deps-only
      rg -n -- '--locked' debian/rules
      rg -n 'CARGO_NET_OFFLINE|--offline' debian/rules
      detached_version="$(dpkg-parsechangelog -S Version)"
      case "$detached_version" in
        *+safelibs*) ;;
        *)
          echo "detached changelog version lacks +safelibs: $detached_version" >&2
          exit 1
          ;;
      esac
      sudo env DEBIAN_FRONTEND=noninteractive mk-build-deps -ir -t 'apt-get -y --no-install-recommends' debian/control
      test -z "$(find "$CARGO_HOME" -mindepth 1 -maxdepth 1 -print -quit)"
      CARGO_HOME="$CARGO_HOME" CARGO_NET_OFFLINE=true dpkg-buildpackage -us -uc -b
    )
    check_deb_payload_contract "$detached_tmp"
    check_deb_control_contract "$detached_tmp" "$detached_tmp/curl/debian/control"
    assert_no_sidecar_outputs "final detached package build output" "$detached_tmp" "$detached_tmp/curl"
    rm -rf dist
    mkdir -p dist
    cp "$detached_tmp"/*.deb dist/
    check_deb_payload_contract dist
    check_deb_control_contract dist "$detached_tmp/curl/debian/control"
    run_detached_package_runtime_checks "$detached_tmp/curl" "$detached_tmp" "$detached_tmp/runtime"
    assert_no_sidecar_outputs "final packaged autopkgtest and runtime smoke" "$detached_tmp/curl" "$detached_tmp/runtime"
    rm -rf safe/.reference safe/.compat .work/validation
    bash scripts/run-upstream-tests.sh
    assert_no_sidecar_outputs "final root upstream-test hook" safe/.reference safe/.compat dist .work/validation
    bash scripts/run-port-tests.sh
    assert_no_sidecar_outputs "final root port-test hook" safe/.reference safe/.compat dist .work/validation
    bash scripts/run-validation-tests.sh
    assert_no_sidecar_outputs "final root validation hook" safe/.reference safe/.compat dist .work/validation
    dep_tmp="$(mktemp -d)"
    bash safe/scripts/export-tracked-tree.sh --with-root-harness --dest "$dep_tmp/harness"
    test ! -e "$dep_tmp/original"
    test ! -e "$dep_tmp/harness/original"
    test ! -e "$dep_tmp/harness/safe/../original"
    detached_root_harness_package_consumer_paths=()
    for path in \
      "$dep_tmp/harness/test-original.sh" \
      "$dep_tmp/harness/scripts/run-upstream-tests.sh" \
      "$dep_tmp/harness/scripts/run-port-tests.sh" \
      "$dep_tmp/harness/scripts/run-validation-tests.sh" \
      "$dep_tmp/harness/scripts/run-tests.sh" \
      "$dep_tmp/harness/safe/scripts/run-public-abi-smoke.sh" \
      "$dep_tmp/harness/safe/scripts/compat_harness.py" \
      "$dep_tmp/harness/safe/scripts/export-tracked-tree.sh" \
      "$dep_tmp/harness/safe/scripts/build-compat-consumers.sh" \
      "$dep_tmp/harness/safe/scripts/run-link-compat.sh" \
      "$dep_tmp/harness/safe/scripts/run-upstream-tests.sh" \
      "$dep_tmp/harness/safe/scripts/run-curated-libtests.sh" \
      "$dep_tmp/harness/safe/scripts/run-curl-tool-smoke.sh" \
      "$dep_tmp/harness/safe/scripts/run-http-client-tests.sh" \
      "$dep_tmp/harness/safe/scripts/run-websocket-disabled-smoke.sh" \
      "$dep_tmp/harness/safe/scripts/run-ldap-devpkg-test.sh" \
      "$dep_tmp/harness/safe/scripts/run-ldaps-functional-test.sh" \
      "$dep_tmp/harness/safe/scripts/run-rtmp-functional-tests.sh" \
      "$dep_tmp/harness/safe/debian/tests/control" \
      "$dep_tmp/harness/safe/debian/tests/upstream-tests-openssl" \
      "$dep_tmp/harness/safe/debian/tests/upstream-tests-gnutls" \
      "$dep_tmp/harness/safe/debian/tests/curl-ldapi-test" \
      "$dep_tmp/harness/safe/debian/tests/LDAP-bindata.c"
    do
      test -e "$path" || { echo "missing final detached root-harness package-consuming path: $path" >&2; exit 1; }
      detached_root_harness_package_consumer_paths+=("$path")
    done
    if rg -n "$package_consumer_sidecar_forbidden" "${detached_root_harness_package_consumer_paths[@]}"; then
      echo "final detached root-harness package-consuming path still references transitional libcurl sidecar machinery" >&2
      exit 1
    fi
    assert_no_sidecar_outputs "final detached root-harness export before dependent safe mode" "$dep_tmp/harness"
    detached_dist_abs="$(pwd)/dist"
    (
      cd "$dep_tmp/harness"
      SAFE_MODE_ARTIFACT_ROOT="$dep_tmp/harness/.safe-mode-artifacts" \
        bash ./test-original.sh --implementation safe --safe-deb-dir "$detached_dist_abs"
    )
    assert_no_sidecar_outputs "final dependent safe-mode output" "$dep_tmp/harness" "$dep_tmp/harness/.safe-mode-artifacts"
    rm -rf "$dep_tmp"
    rm -rf "$detached_tmp"
    unset CARGO_HOME

    perf_root="$(mktemp -d)"
    for flavor in openssl gnutls; do
      bash safe/scripts/benchmark-local.sh --implementation original --flavor "$flavor" --matrix core --output-dir "$perf_root/original-$flavor"
      bash safe/scripts/benchmark-local.sh --implementation safe --flavor "$flavor" --matrix core --output-dir "$perf_root/safe-$flavor"
      python3 safe/scripts/compare-benchmarks.py \
        --baseline "$perf_root/original-$flavor" \
        --candidate "$perf_root/safe-$flavor" \
        --thresholds safe/benchmarks/thresholds.json \
        --output "$perf_root/comparison-$flavor.json"
    done
    cat "$perf_root"/comparison-*.json
    rm -rf "$perf_root"
    ```

**Preexisting Inputs:**

- `safe/debian/control`, `safe/debian/rules`, `safe/debian/changelog`, `safe/debian/source/format`, `safe/debian/patches/series`, `safe/.cargo/config.toml`, `safe/vendor/cargo/`, `safe/metadata/debian-control-contract.json`, `safe/metadata/dev-tooling-contract.json`, `safe/scripts/verify-cargo-source-policy.py`, `safe/scripts/verify-debian-control-contract.py`, `safe/scripts/verify-dev-tooling-contract.sh`, `safe/scripts/verify-package-no-refetch.py`, `safe/scripts/verify-package-payload-contract.py`, `safe/scripts/run-packaged-autopkgtests.sh`, `safe/scripts/run-rtmp-functional-tests.sh`, `safe/scripts/run-ldaps-functional-test.sh`, `scripts/build-debs.sh`, `scripts/lib/build-deb-common.sh`, `scripts/run-upstream-tests.sh`, `scripts/run-port-tests.sh`, `scripts/run-validation-tests.sh`, and `test-original.sh`.
- `safe/src/`, `safe/c_shim/`, `safe/scripts/`, `safe/metadata/`, `safe/tests/`, `safe/compat/`, `safe/benchmarks/`, `safe/debian/`, `.github/workflows/ci-release.yml`, `packaging/package.env`, `dependents.json`, `relevant_cves.json`, and `all_cves.json`.
- `original/include/curl/`, `original/libcurl.def`, `original/lib/libcurl.vers.in`, `original/lib/Makefile.inc`, `original/src/Makefile.am`, `original/src/Makefile.inc`, `original/curl-config.in`, `original/libcurl.pc.in`, `original/docs/libcurl/libcurl.m4`, `original/debian/control`, `original/debian/changelog`, `original/debian/rules`, `original/debian/source/format`, `original/debian/tests/`, `original/debian/patches/`, `original/tests/runtests.pl`, `original/tests/data/Makefile.inc`, `original/tests/data/DISABLED`, `original/tests/libtest/`, `original/tests/server/`, `original/tests/unit/`, and `original/tests/http/`.

**New Outputs:**

- Final confirmation that Phase 9's sidecar-removal invariants still hold after final fixes, with any Phase 10-introduced regressions removed before yielding.
- Final machine-verifiable unsafe-boundary documentation and reduced unsafe scope.
- Final package and verification fixes.

**File Changes:**

- Keep Phase 9's sidecar-removal outputs absent. Do not reintroduce `safe/c_shim/forwarders.c`, `safe/src/easy/reference.rs`, `mod reference`, `port_safe_resolve_reference_symbol`, `load_reference`, `ReferenceRegistry`, `ReferenceHandle`, `reference_library_path`, `reference_backend`, package installation directives for `libcurl-reference-*.so.4`, generator support for `forwarders.c`, or any package-consuming dependency on `safe/.reference`.
- Keep `safe/build.rs`, `safe/scripts/run-public-abi-smoke.sh`, `safe/scripts/compat_harness.py`, `safe/scripts/build-compat-consumers.sh`, root CI hooks, Debian autopkgtests, and dependent safe-mode harnesses sidecar-free while making final fixes. If a final verification failure exposes a sidecar marker introduced during Phase 10, remove that regression in Phase 10; do not defer or reassign Phase 9-owned source deletion to final hardening.
- Keep only justified permanent C/unsafe boundaries: C variadic shims, mprintf varargs if retained, generated public export thunks, libc/socket/TLS/nghttp2/ssh FFI, and OS callbacks.
- Add `safe/docs/unsafe-audit.md` documenting each remaining unsafe Rust boundary and C shim. The document must have one section per boundary file under `safe/src/**/*.rs` or `safe/c_shim/*.c`, include the file path, `Purpose:`, `Invariants:`, and `Necessity:` or `Why unsafe remains:`, and list every verifier boundary id reported by the final audit checker, using ids of the form `safe/src/path.rs:<line>` for Rust boundary lines and `safe/c_shim/name.c:c-shim` for C shim files.
- Keep `safe/metadata/cve-to-test.json`, `safe/tests/cve_cases/*`, and `safe/tests/cve_regressions.rs` free of the `reference_backend_*`, "reference backend", and "libcurl reference" placeholders removed in Phase 9. If final hardening adds or changes CVE coverage through an unavoidable third-party C boundary, name the case after that boundary, document the invariant explicitly, and keep the test independent of any libcurl sidecar.

**Implementation Details:**

- The final code must not depend on a C libcurl sidecar for runtime behavior. Third-party libraries such as OpenSSL, GnuTLS, nghttp2, Ubuntu's selected libssh backend, librtmp, zlib, brotli, zstd, libidn2, libkrb5/GSS-API, libc, and libpsl are acceptable FFI dependencies when wrapped with clear invariants.
- Phase 9 has already removed stale transitional sidecar files and non-benchmark markers. The final verifier must fail if `safe/src/easy/reference.rs`, `safe/c_shim/forwarders.c`, generator support for `forwarders.c`, `load_reference`, `ReferenceRegistry`, `reference_library_path`, `reference_backend`, or similar sidecar resolver names have been reintroduced outside the benchmark-only original-baseline helper paths or verifier/audit detector constants.
- Benchmark-only original baselines may still build an original libcurl from `safe/vendor/upstream/`, but that code path must stay confined to benchmark commands and must not be reachable from `safe/build.rs`, the packaged safe libraries, public ABI smoke tests, compatibility config artifact flow, compatibility consumer staging, link compatibility runs, upstream safe test runs, or dependent safe-package tests.
- Keep `unsafe` Rust isolated to ABI, allocator, pointer, callback, socket, and third-party library boundaries. Keep internal parsing, state machines, policy decisions, and data ownership in safe Rust.
- The unsafe audit must be generated or updated after the final unsafe reduction, not before it. It must cover the final post-reference-removal tree and must be kept in sync with line-level boundary ids from the final checker. Removing an unsafe block or C shim requires removing its stale audit entry; adding one requires adding the matching audit id and invariants in the same commit.
- All final verifier failures must bounce only to `impl-final-hardening`.
- Implementer must commit this phase's work before yielding.

**Verification:**

- `check-final-hardening-full` is the final and complete verification gate.

**Success Criteria:**

- Every item listed under `New Outputs` is present, updated, or explicitly left unchanged because it already satisfied the plan.
- Every required `File Changes` and `Implementation Details` invariant for this phase is satisfied.
- Every verifier listed under `Verification Phases` passes exactly as written and bounces only to this phase on failure.
- All listed `Preexisting Inputs` are consumed in place; existing artifacts are not rediscovered, refetched, or regenerated from untracked sources unless this phase explicitly updates a derived safe artifact from them.

**Git Commit Requirement:**

- The implementer must commit this phase's work to git before yielding.
