### 9. Debian Packaging, Ubuntu Install Layout, Autopkgtests, Validator Hook, and Safe Dependent Harness

**Phase Name:** Debian Packaging, Ubuntu Install Layout, Autopkgtests, Validator Hook, and Safe Dependent Harness

**Implement Phase ID:** `impl-packaging`

**Verification Phases:**

- `check-package-build`
  - Type: `check`
  - Fixed `bounce_target`: `impl-packaging`
  - Purpose: run the Phase 9-owned live root package/test hooks, then build the safe Debian binary packages from a detached safe-only export.
  - Commands:
    ```bash
    export DEBIAN_FRONTEND=noninteractive
    export CARGO_NET_OFFLINE=true
    repo_root="$(pwd)"
    tmpdir="$(mktemp -d)"
    export CARGO_HOME="$tmpdir/cargo-home"
    mkdir -p "$CARGO_HOME"
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
    package_sidecar_forbidden='build-reference-curl|forwarders\.c|REFERENCE_LIBRARY|run_reference_build|libcurl-reference|safe/\.reference|(^|/)\.reference($|/)|\.reference/|reference_library_path|port_safe_resolve_reference_symbol|bridge_resolve_symbol|bridge_open_reference|reference_backend|reference backend|libcurl reference'
    if rg -n "$package_sidecar_forbidden" safe/build.rs safe/debian/rules safe/debian/*.install safe/debian/*.links scripts/build-debs.sh scripts/lib/build-deb-common.sh; then
      echo "package build path still references transitional libcurl sidecar machinery" >&2
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
      test -e "$path" || { echo "missing package-consuming path: $path" >&2; exit 1; }
      package_consumer_paths+=("$path")
    done
    if rg -n "$package_consumer_sidecar_forbidden" "${package_consumer_paths[@]}"; then
      echo "package-consuming path still references transitional libcurl sidecar machinery" >&2
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
    python3 safe/scripts/verify-debian-control-contract.py \
      --contract safe/metadata/debian-control-contract.json \
      --safe-control safe/debian/control \
      --original-control original/debian/control
    python3 safe/scripts/verify-protocol-feature-contract.py \
      --contract safe/metadata/dev-tooling-contract.json \
      --package-root safe \
      --debian-control safe/debian/control \
      --check-source-deps-only
    rm -rf safe/.reference safe/.compat safe/target/public-abi safe/target/compat-consumers .work/validation dist
    git diff --exit-code -- safe/debian/changelog
    bash scripts/build-debs.sh
    git diff --exit-code -- safe/debian/changelog
    if find dist -maxdepth 1 -type f ! -name '*.deb' -print -quit | grep -q .; then
      echo "scripts/build-debs.sh must leave only .deb artifacts in dist/" >&2
      find dist -maxdepth 1 -type f ! -name '*.deb' -print >&2
      exit 1
    fi
    for deb in dist/*.deb; do
      deb_version="$(dpkg-deb --field "$deb" Version)"
      case "$deb_version" in
        *+safelibs*) ;;
        *)
          echo "root hook built package version lacks +safelibs: $deb $deb_version" >&2
          exit 1
          ;;
      esac
    done
    python3 safe/scripts/verify-debian-control-contract.py \
      --contract safe/metadata/debian-control-contract.json \
      --safe-control safe/debian/control \
      --deb-dir dist
    python3 safe/scripts/verify-package-payload-contract.py \
      --deb-dir dist \
      --require-safelibs-version
    for flavor in openssl gnutls; do
      bash safe/scripts/run-public-abi-smoke.sh --flavor "$flavor"
      assert_no_sidecar_outputs "public ABI smoke for $flavor" safe/.reference safe/.compat "safe/target/public-abi/$flavor"
      bash safe/scripts/build-compat-consumers.sh --flavor "$flavor"
      assert_no_sidecar_outputs "compatibility staging for $flavor" safe/.reference safe/.compat safe/target/compat-consumers
    done
    bash scripts/run-upstream-tests.sh
    assert_no_sidecar_outputs "root upstream-test hook" safe/.reference safe/.compat dist
    bash scripts/run-port-tests.sh
    assert_no_sidecar_outputs "root port-test hook" safe/.reference safe/.compat dist
    bash scripts/run-validation-tests.sh
    assert_no_sidecar_outputs "root validation hook" safe/.reference safe/.compat dist .work/validation
    rm -rf "$CARGO_HOME"
    mkdir -p "$CARGO_HOME"
    test -z "$(find "$CARGO_HOME" -mindepth 1 -maxdepth 1 -print -quit)"
    bash safe/scripts/export-tracked-tree.sh --safe-only --dest "$tmpdir/curl"
    test ! -e "$tmpdir/original"
    test ! -e "$tmpdir/curl/../original"
    detached_package_consumer_paths=()
    for path in \
      "$tmpdir/curl/scripts/run-public-abi-smoke.sh" \
      "$tmpdir/curl/scripts/compat_harness.py" \
      "$tmpdir/curl/scripts/export-tracked-tree.sh" \
      "$tmpdir/curl/scripts/build-compat-consumers.sh" \
      "$tmpdir/curl/scripts/run-link-compat.sh" \
      "$tmpdir/curl/scripts/run-upstream-tests.sh" \
      "$tmpdir/curl/scripts/run-curated-libtests.sh" \
      "$tmpdir/curl/scripts/run-curl-tool-smoke.sh" \
      "$tmpdir/curl/scripts/run-http-client-tests.sh" \
      "$tmpdir/curl/scripts/run-websocket-disabled-smoke.sh" \
      "$tmpdir/curl/scripts/run-ldap-devpkg-test.sh" \
      "$tmpdir/curl/scripts/run-ldaps-functional-test.sh" \
      "$tmpdir/curl/scripts/run-rtmp-functional-tests.sh" \
      "$tmpdir/curl/debian/tests/control" \
      "$tmpdir/curl/debian/tests/upstream-tests-openssl" \
      "$tmpdir/curl/debian/tests/upstream-tests-gnutls" \
      "$tmpdir/curl/debian/tests/curl-ldapi-test" \
      "$tmpdir/curl/debian/tests/LDAP-bindata.c"
    do
      test -e "$path" || { echo "missing detached package-consuming path: $path" >&2; exit 1; }
      detached_package_consumer_paths+=("$path")
    done
    if rg -n "$package_consumer_sidecar_forbidden" "${detached_package_consumer_paths[@]}"; then
      echo "detached safe-only export package-consuming path still references transitional libcurl sidecar machinery" >&2
      exit 1
    fi
    assert_no_sidecar_outputs "detached safe-only export before package build" "$tmpdir/curl"
    (
      cd "$tmpdir/curl"
      python3 scripts/verify-cargo-source-policy.py \
        --manifest Cargo.toml \
        --lock Cargo.lock \
        --config .cargo/config.toml \
        --vendor vendor/cargo
      python3 scripts/verify-package-no-refetch.py --package-root .
      if rg -n "$package_sidecar_forbidden" build.rs debian/rules debian/*.install debian/*.links; then
        echo "detached package build path still references transitional libcurl sidecar machinery" >&2
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
    )
    (
      cd "$tmpdir/curl"
      sudo env DEBIAN_FRONTEND=noninteractive mk-build-deps -ir -t 'apt-get -y --no-install-recommends' debian/control
      test -z "$(find "$CARGO_HOME" -mindepth 1 -maxdepth 1 -print -quit)"
      CARGO_HOME="$CARGO_HOME" CARGO_NET_OFFLINE=true dpkg-buildpackage -us -uc -b
    )
    ls -l "$tmpdir"/*.deb
    for deb in "$tmpdir"/*.deb; do
      deb_version="$(dpkg-deb --field "$deb" Version)"
      case "$deb_version" in
        *+safelibs*) ;;
        *)
          echo "built package version lacks +safelibs: $deb $deb_version" >&2
          exit 1
          ;;
      esac
    done
    python3 "$repo_root/safe/scripts/verify-debian-control-contract.py" \
      --contract "$repo_root/safe/metadata/debian-control-contract.json" \
      --safe-control "$tmpdir/curl/debian/control" \
      --deb-dir "$tmpdir"
    python3 "$repo_root/safe/scripts/verify-package-payload-contract.py" \
      --deb-dir "$tmpdir" \
      --require-safelibs-version
    rm -rf "$tmpdir"
    ```

- `check-packaged-autopkgtests`
  - Type: `check`
  - Fixed `bounce_target`: `impl-packaging`
  - Purpose: build safe packages from a detached safe-only export, install them in an Ubuntu 24.04 environment, and run the packaged autopkgtest entrypoints from that detached export.
  - Commands:
    ```bash
    export DEBIAN_FRONTEND=noninteractive
    export CARGO_NET_OFFLINE=true
    repo_root="$(pwd)"
    tmpdir="$(mktemp -d)"
    cargo_home="$tmpdir/cargo-home"
    mkdir -p "$cargo_home"
    export CARGO_HOME="$cargo_home"
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
    package_sidecar_forbidden='build-reference-curl|forwarders\.c|REFERENCE_LIBRARY|run_reference_build|libcurl-reference|safe/\.reference|(^|/)\.reference($|/)|\.reference/|reference_library_path|port_safe_resolve_reference_symbol|bridge_resolve_symbol|bridge_open_reference|reference_backend|reference backend|libcurl reference'
    if rg -n "$package_sidecar_forbidden" safe/build.rs safe/debian/rules safe/debian/*.install safe/debian/*.links scripts/build-debs.sh scripts/lib/build-deb-common.sh; then
      echo "package build path still references transitional libcurl sidecar machinery" >&2
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
      test -e "$path" || { echo "missing package-consuming path: $path" >&2; exit 1; }
      package_consumer_paths+=("$path")
    done
    if rg -n "$package_consumer_sidecar_forbidden" "${package_consumer_paths[@]}"; then
      echo "package-consuming path still references transitional libcurl sidecar machinery" >&2
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
    python3 safe/scripts/verify-debian-control-contract.py \
      --contract safe/metadata/debian-control-contract.json \
      --safe-control safe/debian/control \
      --original-control original/debian/control
    python3 safe/scripts/verify-protocol-feature-contract.py \
      --contract safe/metadata/dev-tooling-contract.json \
      --package-root safe \
      --debian-control safe/debian/control \
      --check-source-deps-only
    bash safe/scripts/export-tracked-tree.sh --safe-only --dest "$tmpdir/src"
    test ! -e "$tmpdir/original"
    test ! -e "$tmpdir/src/../original"
    detached_package_consumer_paths=()
    for path in \
      "$tmpdir/src/scripts/run-public-abi-smoke.sh" \
      "$tmpdir/src/scripts/compat_harness.py" \
      "$tmpdir/src/scripts/export-tracked-tree.sh" \
      "$tmpdir/src/scripts/build-compat-consumers.sh" \
      "$tmpdir/src/scripts/run-link-compat.sh" \
      "$tmpdir/src/scripts/run-upstream-tests.sh" \
      "$tmpdir/src/scripts/run-curated-libtests.sh" \
      "$tmpdir/src/scripts/run-curl-tool-smoke.sh" \
      "$tmpdir/src/scripts/run-http-client-tests.sh" \
      "$tmpdir/src/scripts/run-websocket-disabled-smoke.sh" \
      "$tmpdir/src/scripts/run-ldap-devpkg-test.sh" \
      "$tmpdir/src/scripts/run-ldaps-functional-test.sh" \
      "$tmpdir/src/scripts/run-rtmp-functional-tests.sh" \
      "$tmpdir/src/debian/tests/control" \
      "$tmpdir/src/debian/tests/upstream-tests-openssl" \
      "$tmpdir/src/debian/tests/upstream-tests-gnutls" \
      "$tmpdir/src/debian/tests/curl-ldapi-test" \
      "$tmpdir/src/debian/tests/LDAP-bindata.c"
    do
      test -e "$path" || { echo "missing detached package-consuming path: $path" >&2; exit 1; }
      detached_package_consumer_paths+=("$path")
    done
    if rg -n "$package_consumer_sidecar_forbidden" "${detached_package_consumer_paths[@]}"; then
      echo "detached safe-only export package-consuming path still references transitional libcurl sidecar machinery" >&2
      exit 1
    fi
    assert_no_sidecar_outputs "detached safe-only export before autopkgtest build" "$tmpdir/src"
    (
      cd "$tmpdir/src"
      export CARGO_HOME="$cargo_home"
      python3 scripts/verify-cargo-source-policy.py \
        --manifest Cargo.toml \
        --lock Cargo.lock \
        --config .cargo/config.toml \
        --vendor vendor/cargo
      python3 scripts/verify-package-no-refetch.py --package-root .
      if rg -n "$package_sidecar_forbidden" build.rs debian/rules debian/*.install debian/*.links; then
        echo "detached package build path still references transitional libcurl sidecar machinery" >&2
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
    for deb in "$tmpdir"/*.deb; do
      deb_version="$(dpkg-deb --field "$deb" Version)"
      case "$deb_version" in
        *+safelibs*) ;;
        *)
          echo "built package version lacks +safelibs: $deb $deb_version" >&2
          exit 1
          ;;
      esac
    done
    python3 "$repo_root/safe/scripts/verify-debian-control-contract.py" \
      --contract "$repo_root/safe/metadata/debian-control-contract.json" \
      --safe-control "$tmpdir/src/debian/control" \
      --deb-dir "$tmpdir"
    python3 "$repo_root/safe/scripts/verify-package-payload-contract.py" \
      --deb-dir "$tmpdir" \
      --require-safelibs-version
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
    mkdir -p "$tmpdir/autopkgtest-openssl" "$tmpdir/autopkgtest-gnutls" "$tmpdir/autopkgtest-ldap"
    (
      cd "$tmpdir/src"
      sudo apt-get update
      sudo env DEBIAN_FRONTEND=noninteractive mk-build-deps -ir -t 'apt-get -y --no-install-recommends' debian/control
      sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        "$tmpdir"/curl_*.deb \
        "$tmpdir"/libcurl4t64_*.deb \
        "$tmpdir"/libcurl4-openssl-dev_*.deb \
        "$tmpdir"/libcurl4-doc_*.deb \
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
      AUTOPKGTEST_TMP="$tmpdir/autopkgtest-openssl" bash debian/tests/upstream-tests-openssl
      AUTOPKGTEST_TMP="$tmpdir/autopkgtest-ldap" bash debian/tests/curl-ldapi-test
      sudo env DEBIAN_FRONTEND=noninteractive apt-get purge -y libcurl4-openssl-dev
      sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        "$tmpdir"/libcurl3t64-gnutls_*.deb \
        "$tmpdir"/libcurl4-gnutls-dev_*.deb
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
      AUTOPKGTEST_TMP="$tmpdir/autopkgtest-gnutls" bash debian/tests/upstream-tests-gnutls
    )
    PACKAGED_CURL_BIN=/usr/bin/curl bash "$tmpdir/src/scripts/run-curl-tool-smoke.sh" --implementation packaged
    assert_no_sidecar_outputs "packaged autopkgtest and runtime smoke" "$tmpdir/src" "$tmpdir/autopkgtest-openssl" "$tmpdir/autopkgtest-gnutls" "$tmpdir/autopkgtest-ldap"
    rm -rf dist
    mkdir -p dist
    cp "$tmpdir"/*.deb dist/
    bash scripts/run-validation-tests.sh
    assert_no_sidecar_outputs "validator hook after packaged autopkgtests" dist .work/validation "$tmpdir/src"
    rm -rf "$tmpdir"
    ```

- `check-dependent-safe-mode`
  - Type: `check`
  - Fixed `bounce_target`: `impl-packaging`
  - Purpose: run the existing 12-dependent matrix from a detached root-harness export against installed safe packages instead of the original build.
  - Commands:
    ```bash
    export DEBIAN_FRONTEND=noninteractive
    export CARGO_NET_OFFLINE=true
    repo_root="$(pwd)"
    tmpdir="$(mktemp -d)"
    cargo_home="$tmpdir/cargo-home"
    mkdir -p "$cargo_home"
    export CARGO_HOME="$cargo_home"
    test "$CARGO_HOME" != "$HOME/.cargo"
    test -z "$(find "$CARGO_HOME" -mindepth 1 -maxdepth 1 -print -quit)"
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
    python3 safe/scripts/verify-cargo-source-policy.py \
      --manifest safe/Cargo.toml \
      --lock safe/Cargo.lock \
      --config safe/.cargo/config.toml \
      --vendor safe/vendor/cargo
    python3 safe/scripts/verify-package-no-refetch.py \
      --package-root safe \
      --root-build-script scripts/build-debs.sh \
      --root-build-helper scripts/lib/build-deb-common.sh
    package_sidecar_forbidden='build-reference-curl|forwarders\.c|REFERENCE_LIBRARY|run_reference_build|libcurl-reference|safe/\.reference|(^|/)\.reference($|/)|\.reference/|reference_library_path|port_safe_resolve_reference_symbol|bridge_resolve_symbol|bridge_open_reference|reference_backend|reference backend|libcurl reference'
    if rg -n "$package_sidecar_forbidden" safe/build.rs safe/debian/rules safe/debian/*.install safe/debian/*.links scripts/build-debs.sh scripts/lib/build-deb-common.sh; then
      echo "package build path still references transitional libcurl sidecar machinery" >&2
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
      test -e "$path" || { echo "missing package-consuming path: $path" >&2; exit 1; }
      package_consumer_paths+=("$path")
    done
    if rg -n "$package_consumer_sidecar_forbidden" "${package_consumer_paths[@]}"; then
      echo "package-consuming path still references transitional libcurl sidecar machinery" >&2
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
    python3 safe/scripts/verify-debian-control-contract.py \
      --contract safe/metadata/debian-control-contract.json \
      --safe-control safe/debian/control \
      --original-control original/debian/control
    python3 safe/scripts/verify-protocol-feature-contract.py \
      --contract safe/metadata/dev-tooling-contract.json \
      --package-root safe \
      --debian-control safe/debian/control \
      --check-source-deps-only
    bash safe/scripts/export-tracked-tree.sh --safe-only --dest "$tmpdir/src"
    bash safe/scripts/export-tracked-tree.sh --with-root-harness --dest "$tmpdir/harness"
    test ! -e "$tmpdir/original"
    test ! -e "$tmpdir/src/../original"
    test ! -e "$tmpdir/harness/original"
    test ! -e "$tmpdir/harness/safe/../original"
    detached_package_consumer_paths=()
    for path in \
      "$tmpdir/src/scripts/run-public-abi-smoke.sh" \
      "$tmpdir/src/scripts/compat_harness.py" \
      "$tmpdir/src/scripts/export-tracked-tree.sh" \
      "$tmpdir/src/scripts/build-compat-consumers.sh" \
      "$tmpdir/src/scripts/run-link-compat.sh" \
      "$tmpdir/src/scripts/run-upstream-tests.sh" \
      "$tmpdir/src/scripts/run-curated-libtests.sh" \
      "$tmpdir/src/scripts/run-curl-tool-smoke.sh" \
      "$tmpdir/src/scripts/run-http-client-tests.sh" \
      "$tmpdir/src/scripts/run-websocket-disabled-smoke.sh" \
      "$tmpdir/src/scripts/run-ldap-devpkg-test.sh" \
      "$tmpdir/src/scripts/run-ldaps-functional-test.sh" \
      "$tmpdir/src/scripts/run-rtmp-functional-tests.sh" \
      "$tmpdir/src/debian/tests/control" \
      "$tmpdir/src/debian/tests/upstream-tests-openssl" \
      "$tmpdir/src/debian/tests/upstream-tests-gnutls" \
      "$tmpdir/src/debian/tests/curl-ldapi-test" \
      "$tmpdir/src/debian/tests/LDAP-bindata.c" \
      "$tmpdir/harness/test-original.sh" \
      "$tmpdir/harness/scripts/run-upstream-tests.sh" \
      "$tmpdir/harness/scripts/run-port-tests.sh" \
      "$tmpdir/harness/scripts/run-validation-tests.sh" \
      "$tmpdir/harness/scripts/run-tests.sh" \
      "$tmpdir/harness/safe/scripts/run-public-abi-smoke.sh" \
      "$tmpdir/harness/safe/scripts/compat_harness.py" \
      "$tmpdir/harness/safe/scripts/export-tracked-tree.sh" \
      "$tmpdir/harness/safe/scripts/build-compat-consumers.sh" \
      "$tmpdir/harness/safe/scripts/run-link-compat.sh" \
      "$tmpdir/harness/safe/scripts/run-upstream-tests.sh" \
      "$tmpdir/harness/safe/scripts/run-curated-libtests.sh" \
      "$tmpdir/harness/safe/scripts/run-curl-tool-smoke.sh" \
      "$tmpdir/harness/safe/scripts/run-http-client-tests.sh" \
      "$tmpdir/harness/safe/scripts/run-websocket-disabled-smoke.sh" \
      "$tmpdir/harness/safe/scripts/run-ldap-devpkg-test.sh" \
      "$tmpdir/harness/safe/scripts/run-ldaps-functional-test.sh" \
      "$tmpdir/harness/safe/scripts/run-rtmp-functional-tests.sh" \
      "$tmpdir/harness/safe/debian/tests/control" \
      "$tmpdir/harness/safe/debian/tests/upstream-tests-openssl" \
      "$tmpdir/harness/safe/debian/tests/upstream-tests-gnutls" \
      "$tmpdir/harness/safe/debian/tests/curl-ldapi-test" \
      "$tmpdir/harness/safe/debian/tests/LDAP-bindata.c"
    do
      test -e "$path" || { echo "missing detached package-consuming path: $path" >&2; exit 1; }
      detached_package_consumer_paths+=("$path")
    done
    if rg -n "$package_consumer_sidecar_forbidden" "${detached_package_consumer_paths[@]}"; then
      echo "detached safe or root-harness export package-consuming path still references transitional libcurl sidecar machinery" >&2
      exit 1
    fi
    assert_no_sidecar_outputs "detached exports before dependent safe-mode build" "$tmpdir/src" "$tmpdir/harness"
    (
      cd "$tmpdir/src"
      export CARGO_HOME="$cargo_home"
      python3 scripts/verify-cargo-source-policy.py \
        --manifest Cargo.toml \
        --lock Cargo.lock \
        --config .cargo/config.toml \
        --vendor vendor/cargo
      python3 scripts/verify-package-no-refetch.py --package-root .
      if rg -n "$package_sidecar_forbidden" build.rs debian/rules debian/*.install debian/*.links; then
        echo "detached package build path still references transitional libcurl sidecar machinery" >&2
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
    for deb in "$tmpdir"/*.deb; do
      deb_version="$(dpkg-deb --field "$deb" Version)"
      case "$deb_version" in
        *+safelibs*) ;;
        *)
          echo "built package version lacks +safelibs: $deb $deb_version" >&2
          exit 1
          ;;
      esac
    done
    python3 "$repo_root/safe/scripts/verify-debian-control-contract.py" \
      --contract "$repo_root/safe/metadata/debian-control-contract.json" \
      --safe-control "$tmpdir/src/debian/control" \
      --deb-dir "$tmpdir"
    python3 "$repo_root/safe/scripts/verify-package-payload-contract.py" \
      --deb-dir "$tmpdir" \
      --require-safelibs-version
    (
      cd "$tmpdir/harness"
      SAFE_MODE_ARTIFACT_ROOT="$tmpdir/harness/.safe-mode-artifacts" \
        bash ./test-original.sh --implementation safe --safe-deb-dir "$tmpdir"
    )
    assert_no_sidecar_outputs "dependent safe-mode output" "$tmpdir/src" "$tmpdir/harness" "$tmpdir/harness/.safe-mode-artifacts"
    rm -rf "$tmpdir"
    ```

**Preexisting Inputs:**

- `safe/benchmarks/scenarios.json`, `safe/benchmarks/thresholds.json`, `safe/benchmarks/harness/easy_loop.c`, `safe/benchmarks/harness/multi_parallel.c`, `safe/scripts/benchmark-local.sh`, `safe/scripts/compare-benchmarks.py`, and `safe/docs/performance.md`.
- `safe/compat/link-manifest.json`, `safe/metadata/test-manifest.json`, `safe/scripts/build-compat-consumers.sh`, `safe/scripts/run-link-compat.sh`, `safe/scripts/verify-protocol-feature-contract.py`, `safe/scripts/run-rtmp-functional-tests.sh`, and `safe/scripts/run-ldaps-functional-test.sh`.
- `safe/debian/*`, `scripts/build-debs.sh`, `scripts/lib/build-deb-common.sh`, `scripts/install-build-deps.sh`, `scripts/run-upstream-tests.sh`, `scripts/run-port-tests.sh`, `scripts/run-validation-tests.sh`, `packaging/package.env`, `.github/workflows/ci-release.yml`, `test-original.sh`, `dependents.json`.
- `original/debian/control`, `original/debian/changelog`, `original/debian/copyright`, `original/debian/rules`, `original/debian/source/format`, `original/debian/libcurl4t64.symbols`, `original/debian/libcurl3t64-gnutls.symbols`, `original/debian/libcurl4-openssl-dev.install`, `original/debian/libcurl4-gnutls-dev.install`, `original/debian/libcurl4-gnutls-dev.links`, `original/debian/curl.install`, `original/debian/libcurl4-doc.install`, `original/debian/tests/control`, `original/debian/tests/upstream-tests-openssl`, `original/debian/tests/upstream-tests-gnutls`, `original/debian/tests/curl-ldapi-test`, `original/debian/tests/LDAP-bindata.c`, `original/curl-config.in`, `original/libcurl.pc.in`, and `original/docs/libcurl/libcurl.m4`.

**New Outputs:**

- Rust-aware `safe/debian/rules` that builds both OpenSSL and GnuTLS flavor libraries with Cargo and installs the correct package layout.
- Correct `safe/debian/control` Build-Depends for Rust, C toolchain, pkg-config, and every original feature-enabling dependency that contributes to Ubuntu's observable protocol and feature contract.
- A committed SafeLibs first stanza in `safe/debian/changelog` at version `8.5.0-2ubuntu10.8+safelibs0`, preserving the Ubuntu changelog below it, so detached safe-only `dpkg-buildpackage` builds produce SafeLibs-versioned `.deb`s without invoking the root stamping hook.
- A vendored-only Cargo package-build policy: all crates.io entries in `safe/Cargo.lock` are committed under `safe/vendor/cargo`, and `safe/.cargo/config.toml` source-replaces `crates-io` with that repository-relative vendor directory.
- `safe/scripts/verify-cargo-source-policy.py`, which validates `Cargo.lock`, `.cargo/config.toml`, and `safe/vendor/cargo` without using the network or host Cargo cache.
- `safe/metadata/debian-control-contract.json`, a tracked contract derived from `original/debian/control` for the source `Build-Depends`, the six binary packages, and their drop-in metadata fields.
- `safe/scripts/verify-debian-control-contract.py`, which validates the safe source stanza, binary stanzas, and built `.deb` control fields against `safe/metadata/debian-control-contract.json`.
- `safe/metadata/dev-tooling-contract.json`, a tracked contract derived from `original/debian/rules`, `original/debian/control`, `original/curl-config.in`, `original/libcurl.pc.in`, `original/docs/libcurl/libcurl.m4`, Debian flavor packaging, `original/debian/libcurl4-gnutls-dev.links`, and `safe/metadata/abi-manifest.json` for expected version, version number, features, protocols, SSL backend labels, SSH backend label, dynamic and static link flags, static archive package paths, GnuTLS generic `libcurl.so`/`libcurl.a` development links, pkg-config fields including multiarch `includedir`/`Cflags`, every supported `curl-config` option output/status, no-argument and unknown-option failures, source feature dependencies, and autoconf macro behavior.
- `safe/scripts/verify-dev-tooling-contract.sh`, which validates installed safe dev-package tooling by compiling and running C smoke programs through `curl-config` and `pkg-config`, linking static smoke objects through `curl-config --static-libs` and `pkg-config --static --libs libcurl`, checking the linked flavor library, checking the full `curl-config` option contract, checking `libcurl.pc` and `libcurl.m4` behavior, requiring multiarch header and static archive paths, and rejecting sibling `original/` or local absolute paths in generated build metadata.
- `safe/scripts/verify-protocol-feature-contract.py`, extended from Phase 6 if needed, which can validate the installed and packaged protocol/feature contract, verify source feature dependencies in `debian/control`, and probe advertised schemes without reading sibling `original/`.
- `safe/scripts/verify-package-no-refetch.py`, which statically rejects package-build fetch paths in `safe/debian/rules`, package helper scripts reachable from `debian/rules` or `build.rs`, `scripts/build-debs.sh`, and `scripts/lib/build-deb-common.sh`.
- `safe/scripts/verify-package-payload-contract.py`, the single shared built-package payload audit used by `check-package-build`, `check-packaged-autopkgtests`, `check-dependent-safe-mode`, and `check-final-hardening-full` immediately after each package build and before any install, autopkgtest, validator, or dependent harness consumes the `.deb`s.
- A sidecar-free Cargo/package build and package-consuming harness path: `safe/build.rs` no longer invokes `safe/scripts/build-reference-curl.sh`, compiles `safe/c_shim/forwarders.c`, defines `REFERENCE_LIBRARY_*`, or emits artifacts that package builds, public ABI smoke tests, compatibility staging, upstream safe tests, port safe tests, validator inputs, or dependent safe mode can load as a libcurl reference sidecar; package-consuming scripts no longer read `safe/.reference`, copy `libcurl-reference-*`, or record `reference_library_path`/`reference_*_config` sidecar metadata.
- A sidecar-marker-free non-benchmark source tree for every detached safe-only and root-harness export: `safe/c_shim/forwarders.c` and `safe/src/easy/reference.rs` are deleted; `safe/src/easy/mod.rs`, `safe/src/global.rs`, `safe/c_shim/variadic.c`, `safe/build.rs`, `safe/scripts/generate-manifests.py`, compatibility harnesses, CVE mappings, CVE case files, and non-benchmark documentation contain no `libcurl-reference`, `.reference`, resolver symbol, or `reference_backend` placeholder text. After Phase 9, only `safe/scripts/build-reference-curl.sh` and `safe/scripts/benchmark-local.sh` may retain explicit original-baseline helper behavior, and `safe/scripts/verify-*` or `scripts/verify-*` may retain the same marker strings solely as inert detector constants.
- Correct `.install`, `.links`, `.symbols`, `curl-config`, `libcurl.pc`, static archive, multiarch header, manpage, docs, and `libcurl.m4` package outputs.
- Fixed `scripts/build-debs.sh` and `scripts/lib/build-deb-common.sh` root-hook behavior that builds binary packages only with `dpkg-buildpackage -us -uc -b`, preserves `safe/debian/patches/series`, and leaves `dist/` containing only the `.deb` files consumed by CI, validator, packaged autopkgtests, and dependent safe mode.
- Safe package mode in `test-original.sh`, including early host prerequisite checks for `docker`, `docker info`, `git`, `jq`, `/dev/fuse`, and Docker FUSE/SYS_ADMIN/AppArmor-unconfined run capability, plus a host-visible `SAFE_MODE_ARTIFACT_ROOT` for logs and runtime staging artifacts produced by the dependent safe-mode Docker run.
- CI hook scripts that run meaningful safe tests and do not create, copy, stage, or record libcurl reference sidecars.

**File Changes:**

- `safe/debian/rules`: replace upstream autotools build logic with Cargo-driven build/install. It must invoke Cargo with `--locked` and offline behavior (`--offline` or `CARGO_NET_OFFLINE=true`), build `libcurl.so.4` for OpenSSL and `libcurl-gnutls.so.4` for GnuTLS, build static development archives `libcurl.a` and `libcurl-gnutls.a`, build or install the `curl` tool linked to the safe OpenSSL flavor, stage public headers only under `/usr/include/$(DEB_HOST_MULTIARCH)/curl/`, stage dev linker symlinks including the GnuTLS generic `libcurl.so` and `libcurl.a` links through `safe/debian/libcurl4-gnutls-dev.links`, flavor-correct `curl-config`, `libcurl.pc`, docs, manpages, and package metadata.
- `safe/debian/control`: add Rust build dependencies (`cargo` and `rustc`), `cc`, `pkgconf`, and test dependencies while preserving the original feature-enabling dependency set. The source stanza must retain `libbrotli-dev`, `libgnutls28-dev`, `libidn2-dev`, `libkrb5-dev`, `libldap2-dev`, `libnghttp2-dev`, `libpsl-dev`, `librtmp-dev`, `libssh-dev`, `libssh2-1-dev`, `libssl-dev`, `libzstd-dev`, and `zlib1g-dev` unless `safe/metadata/dev-tooling-contract.json` records a specific verifier-backed substitute with the same observable behavior. On Ubuntu the implemented SSH backend must match the original rules' `libssh` selection; retaining `libssh2-1-dev` in the source dependency contract preserves the original source package dependency surface but does not authorize switching the runtime SCP/SFTP backend to libssh2 without updating and proving the public contract. Do not depend on a host or Debian Cargo registry cache for crate source; crate sources used by Cargo must be the tracked `safe/vendor/cargo` tree.
- `safe/debian/changelog`: prepend exactly one tracked SafeLibs baseline stanza with source package `curl`, version `8.5.0-2ubuntu10.8+safelibs0`, distribution matching the current Ubuntu top entry, and an entry explaining the SafeLibs Rust-port baseline. Keep all original Ubuntu changelog entries below it unchanged. Direct detached `dpkg-buildpackage -us -uc -b` checks must consume this stanza as their package version source.
- `safe/metadata/debian-control-contract.json`: record the six expected binary package names and each package's required source-control fields copied from `original/debian/control`, including `Architecture`, `Section` where present, `Provides`, `Conflicts`, `Replaces`, `Breaks`, `Multi-Arch`, `Pre-Depends`, `Depends`, `Recommends`, `Suggests`, and `X-Time64-Compat`. Also record the original source stanza's feature-enabling `Build-Depends` that must remain present (`libbrotli-dev`, `libgnutls28-dev`, `libidn2-dev`, `libkrb5-dev`, `libldap2-dev`, `libnghttp2-dev`, `libpsl-dev`, `librtmp-dev`, `libssh-dev`, `libssh2-1-dev`, `libssl-dev`, `libzstd-dev`, and `zlib1g-dev`), an explicit allowlist for added Rust source `Build-Depends`, SafeLibs version stamping, and any package dependency substitutions accepted by the port.
- `safe/metadata/dev-tooling-contract.json`: record the original-derived `curl-config` option contract for `--built-shared`, `--ca`, `--cc`, `--cflags`, `--checkfor`, `--configure`, `--features`, `--help`, `--libs`, `--prefix`, `--protocols`, `--ssl-backends`, `--static-libs`, `--version`, and `--vernum`, including expected stdout, stderr, and exit status where behavior is observable; include a successful `--checkfor 8.5.0`, a failing future-version `--checkfor`, no-argument failure, unknown-option failure, and Debian's multiarch-safe `--configure` output with literal `dpkg-architecture` command substitutions and no build-directory, sibling `original/`, or local absolute paths. Record expected libcurl version and hex version number, flavor-specific SSL backend labels, Ubuntu-selected SSH backend label, implemented feature and protocol lists, required dynamic and static link flags, required static archive package paths, required GnuTLS generic link paths from `original/debian/libcurl4-gnutls-dev.links`, required `libcurl.pc` fields (`Name`, `URL`, `Description`, `Version`, `Libs`, `Libs.private`, `Cflags`, `includedir`, `supported_protocols`, and `supported_features`), expected dev-package SONAME for each flavor, expected `curl_version_info_data` fields, expected source feature dependency names, and the required `libcurl.m4` autoconf smoke behavior. Store repository-relative provenance only. The contract must include the exact protocol lists from the workflow contract, with lowercase `curl_version_info` schemes including `gophers` and all six RTMP variants and uppercase configure-style tooling protocols including aggregate `RTMP`. For GnuTLS, the contract must require `curl-config --libs`, `curl-config --static-libs`, `pkg-config --libs libcurl`, and `pkg-config --static --libs libcurl` to preserve the generic `-lcurl` development link-name contract backed by the generic links, rather than requiring consumers to spell `-lcurl-gnutls`.
- `safe/debian/*.install` and `safe/debian/*.links`: align with the files actually staged by the Rust build. The OpenSSL dev install set must include `usr/bin/curl-config`, `usr/lib/*/libcurl.a`, `usr/lib/*/libcurl.so`, `usr/lib/*/pkgconfig/libcurl.pc`, `usr/include` from a staged tree that contains only `/usr/include/<multiarch>/curl/*.h`, and `usr/share/aclocal/libcurl.m4`; the GnuTLS dev install set must include the same tooling plus `usr/lib/*/libcurl-gnutls.a` and `usr/lib/*/libcurl-gnutls.so`, and `safe/debian/libcurl4-gnutls-dev.links` must preserve Ubuntu's generic `/usr/lib/$DEB_HOST_MULTIARCH/libcurl.a -> libcurl-gnutls.a` and `/usr/lib/$DEB_HOST_MULTIARCH/libcurl.so -> libcurl-gnutls.so` links.
- `safe/debian/*.symbols`: keep identical to original package symbol lists unless a symbol-list check proves a required packaging-only adjustment.
- `safe/debian/tests/*`: adapt autopkgtests to the safe package layout without reaching into sibling `original/`.
- `safe/build.rs`: remove `run_reference_build`, `safe/scripts/build-reference-curl.sh` invocation, `safe/c_shim/forwarders.c` compilation, `REFERENCE_LIBRARY_*` defines, and sidecar rerun inputs from every Cargo build path used by packages or compatibility checks. Keep version-script generation, easy-option generation, permanent C shim compilation, linker args, and flavor detection intact.
- Delete `safe/c_shim/forwarders.c`; do not leave an unused or renamed resolver file.
- Delete `safe/src/easy/reference.rs`, remove `mod reference` from `safe/src/easy/mod.rs`, and remove all `port_safe_resolve_reference_symbol`, `load_reference`, `ReferenceRegistry`, `ReferenceHandle`, and reference-handle call sites from `safe/src/**`.
- `safe/src/global.rs`: remove the reference-symbol FFI declaration and `load_reference`; keep only real global initialization, allocator, SSL-backend, and public ABI logic.
- `safe/c_shim/variadic.c`: remove the reference resolver declaration and any fallback dispatch through the sidecar; permanent variadic C shims must call the native Rust ABI entrypoints or third-party/OS APIs directly.
- `safe/scripts/generate-manifests.py`: remove `render_forwarders`, `forwarders.c` output, and any deterministic generation path that could recreate the deleted sidecar resolver.
- `safe/metadata/cve-to-test.json`, `safe/tests/cve_cases/*`, and `safe/tests/cve_regressions.rs`: replace every `reference_backend_*`, "reference backend", or "libcurl reference" placeholder with native safe regression coverage or a boundary-specific third-party case name. Delete stale `safe/tests/cve_cases/reference_backend_*.json` files.
- `safe/docs/performance.md` and other non-benchmark documentation: remove explicit sidecar helper names and resolver markers. Benchmark methodology may refer to an original baseline conceptually, but exact sidecar-marker text matched by the Phase 9 scanners may appear only in `safe/scripts/build-reference-curl.sh`, `safe/scripts/benchmark-local.sh`, and `safe/scripts/verify-*` or `scripts/verify-*` detector constants.
- `scripts/build-debs.sh` and `scripts/lib/build-deb-common.sh`: run the root hook as a binary-only build with `dpkg-buildpackage -us -uc -b`, copy only generated `.deb` files into `dist/`, and fail if the build produces no `.deb` files. Do not synthesize source artifacts for the root CI hook and do not delete `debian/patches`; preserve empty/comment-only `series`. Stamp commit-epoch SafeLibs versions only in a temporary package build tree or restore the live `safe/debian/changelog` before returning; keep `stamp_safelibs_changelog` idempotent over a committed `+safelibs0` baseline by stripping any existing terminal `+safelibs[0-9]+` suffix and replacing any previous generated SafeLibs build stanza before prepending the new commit-epoch stanza.
- `scripts/install-build-deps.sh`: install package-build, Rust, `ripgrep`, `binutils`, HTTP/2 fixture (`nghttpx`/`nghttp2-proxy`), LDAPS fixture prerequisites (`slapd`, `ldap-utils`, `openssl`, `libldap2-dev`), RTMP fixture/runtime prerequisites (`librtmp-dev` when the implementation uses librtmp), and other test prerequisites used by CI.
- `scripts/run-upstream-tests.sh` and `scripts/run-port-tests.sh`: call safe harnesses with meaningful coverage, not empty template directories. They must consume the freshly built `dist/*.deb` when their coverage requires installed-package behavior, and they must not call `safe/scripts/build-reference-curl.sh`, read `safe/.reference`, copy `libcurl-reference-*`, or leave `.reference` or sidecar markers in `safe/.compat/*/build-state.json`.
- `safe/.cargo/config.toml`: keep the linker wrapper path relative and add a repository-relative source replacement:
  ```toml
  [source.crates-io]
  replace-with = "vendored-sources"

  [source.vendored-sources]
  directory = "vendor/cargo"
  ```
  No package build may require a network registry, a Debian Cargo registry cache, or a preexisting host `$CARGO_HOME` during `scripts/build-debs.sh`, detached package builds, autopkgtests, or dependent safe-mode builds.
- `safe/vendor/cargo/`: commit every crates.io crate required by `safe/Cargo.lock`, including build dependencies such as `cc` and `serde_json` and transitive dependencies from `idna`.
- `safe/scripts/verify-cargo-source-policy.py`: parse `Cargo.lock`, reject absolute vendor paths and missing source replacement, require every `registry+https://github.com/rust-lang/crates.io-index` package to exist under `safe/vendor/cargo` as a Cargo-vendor source directory, and fail if invoked with a `CARGO_HOME` that is missing, non-empty before a package build, or equal to `$HOME/.cargo`.
- `safe/scripts/verify-debian-control-contract.py`: parse Debian control stanzas without shelling out to networked tools, compare the safe source stanza and binary stanzas with the tracked contract, require every original feature-enabling `Build-Depends` named in the contract, allow only contract-listed Rust build dependencies and documented safe substitutes, require the intentional OpenSSL/GnuTLS dev-package conflict fields, require the runtime packages' time64 compatibility fields, and when `--deb-dir` is supplied inspect every built `.deb` with `dpkg-deb --field` to verify built control metadata for `Provides`, `Conflicts`, `Replaces`, `Breaks`, `Multi-Arch`, `Pre-Depends`, `Depends`, `Recommends`, `Suggests`, and `X-Time64-Compat`.
- `safe/scripts/verify-dev-tooling-contract.sh`: require `cc`, `pkg-config`, `curl-config`, `autoconf`, `automake`, `make`, `ar`, `readelf`, and `/usr/share/aclocal/libcurl.m4`; require headers at `/usr/include/$DEB_HOST_MULTIARCH/curl/*.h`; fail immediately if `/usr/include/curl` exists; and fail if any smoke build only passes by finding generic `/usr/include/curl`. Require the flavor static archive (`/usr/lib/$DEB_HOST_MULTIARCH/libcurl.a` for OpenSSL or `/usr/lib/$DEB_HOST_MULTIARCH/libcurl-gnutls.a` for GnuTLS). For GnuTLS, also require `/usr/lib/$DEB_HOST_MULTIARCH/libcurl.so` and `/usr/lib/$DEB_HOST_MULTIARCH/libcurl.a` to exist as the generic development links from `libcurl4-gnutls-dev.links`, and fail if dynamic or static smoke linkage succeeds only through explicit `-lcurl-gnutls` while those generic links or the generic `-lcurl` tooling contract are absent. Compile and run a C program using `curl-config --cflags --libs`; compile and run the same program using `pkg-config --cflags --libs libcurl`; assert the resulting dynamic binaries' `readelf -d` output includes the selected flavor SONAME (`libcurl.so.4` for OpenSSL and `libcurl-gnutls.so.4` for GnuTLS); compile and link a static smoke object using `cc <object> $(curl-config --static-libs)` and another using `cc <object> $(pkg-config --static --libs libcurl)`, and fail if either cannot resolve libcurl symbols from the shipped static archive and its declared private dependencies. Compare runtime `curl_version_info` output with every `curl-config` surface in the contract: `--built-shared`, `--ca`, `--cc`, `--cflags`, successful and failing `--checkfor`, `--configure`, `--features`, `--help`, `--libs`, `--prefix`, `--protocols`, `--ssl-backends`, `--static-libs`, `--version`, and `--vernum`; also verify no-argument and unknown-option failures. Require the `curl_version_info_data.protocols` and `feature_names` lists from the workflow contract, including `gophers`, `ldap`, `ldaps`, and all six RTMP schemes; require the configure-style `curl-config --protocols` and `libcurl.pc:supported_protocols` list with aggregate `RTMP`; compare `curl-config --libs`, `--static-libs`, `pkg-config --libs libcurl`, and `pkg-config --static --libs libcurl` with the contract allowlist, including generic `-lcurl` for GnuTLS; inspect the installed `libcurl.pc` for matching version, feature/protocol variables, multiarch `includedir`, `Cflags`, `Libs`, and `Libs.private`; run an autoconf smoke project using `LIBCURL_CHECK_CONFIG`; and fail if generated configure, make, compiler, pkg-config, or curl-config output contains `/original`, `../original`, or `/home/<user>/` paths.
- `safe/scripts/verify-protocol-feature-contract.py`: support three modes. In Phase 6 artifact mode, load or link a probe against a just-built safe shared library and validate protocol/feature names, feature bitmask, version fields, and advertised-scheme routing. In Phase 9 package-source mode, validate that `safe/debian/control` preserves the original feature-enabling source dependencies. In installed-dev mode, validate the installed library and tooling surfaces for `verify-dev-tooling-contract.sh` without reading sibling `original/`.
- `safe/scripts/verify-package-no-refetch.py`: scan only package-build code paths, ignore comments and non-executed documentation strings, and fail on executable network-fetch commands or registry refreshes. It must allow `mk-build-deps`/`apt-get` only as an explicit Debian dependency-install prerequisite and must not allow Cargo commands other than locked/offline build, test, metadata, rustc, or doc invocations that use the checked-in vendor source replacement.
- `safe/scripts/verify-package-payload-contract.py`: accept `--deb-dir` and `--require-safelibs-version`; require exactly one `.deb` for each of `curl`, `libcurl4t64`, `libcurl3t64-gnutls`, `libcurl4-openssl-dev`, `libcurl4-gnutls-dev`, and `libcurl4-doc`; verify every package version contains `+safelibs` when requested; inspect `dpkg-deb --contents` for generic `./usr/include/curl`, transitional sidecar markers, local checkout or staging path patterns, and unsafe symlink targets; require the OpenSSL and GnuTLS versioned shared libraries, SONAME links, GnuTLS `.so.3` compatibility link, `/usr/bin/curl`, all public headers under `/usr/include/$DEB_HOST_MULTIARCH/curl/`, both dev packages' `curl-config`, multiarch `libcurl.pc`, `/usr/share/aclocal/libcurl.m4`, OpenSSL `libcurl.so` and `libcurl.a`, GnuTLS `libcurl-gnutls.so` and `libcurl-gnutls.a`, GnuTLS generic `libcurl.so -> libcurl-gnutls.so` and `libcurl.a -> libcurl-gnutls.a` development links, and `libcurl4-doc` manpage payload; extract the packages into a temporary directory; reject extracted symlinks whose targets contain `/home/`, `../original`, `/original`, `safe/target`, `target/`, `.compat`, `debian/build`, `/tmp/`, `/var/tmp/`, `.reference`, or `libcurl-reference` before the ELF scan; scan extracted regular files for sidecar strings; and run `readelf -Wd` on extracted ELF files, including resolved in-package development-link targets, failing on `NEEDED`, `RPATH`, or `RUNPATH` entries that contain transitional sidecar names or local checkout/staging paths.
- `test-original.sh`: add `--implementation original|safe`, `--safe-deb-dir <dir>`, and package installation logic while preserving the current original mode. Both modes must fail before building an image if `docker`, `docker info`, `git`, `jq`, `/dev/fuse` as a character device, or a Docker smoke run with `--device /dev/fuse --cap-add SYS_ADMIN --security-opt apparmor:unconfined` is unavailable. Safe mode must install the supplied SafeLibs `.deb`s inside the dependent-test container, must not mount or require `original/`, must not use root scripts outside the detached root-harness export, must not call any reference-sidecar helper, and must write dependent safe-mode logs and staging metadata to `${SAFE_MODE_ARTIFACT_ROOT:-$PWD/.safe-mode-artifacts}` so `check-dependent-safe-mode` can audit that output for `.reference`, `libcurl-reference-*`, and resolver marker leakage after the Docker run.

**Implementation Details:**

- The binary package set must remain: `curl`, `libcurl4t64`, `libcurl3t64-gnutls`, `libcurl4-openssl-dev`, `libcurl4-gnutls-dev`, and `libcurl4-doc`.
- Package source dependencies are compatibility inputs, not cleanup opportunities. The safe package may add Rust build dependencies, but it must not drop `libbrotli-dev`, `libgnutls28-dev`, `libidn2-dev`, `libkrb5-dev`, `libldap2-dev`, `libnghttp2-dev`, `libpsl-dev`, `librtmp-dev`, `libssh-dev`, `libssh2-1-dev`, `libssl-dev`, `libzstd-dev`, or `zlib1g-dev` unless the tracked contract names an equivalent substitute and the package/dev-tooling verifiers prove the same advertised features, protocols, link flags, and runtime behavior.
- The OpenSSL runtime library must install as `libcurl.so.4`; the GnuTLS runtime library must install as `libcurl-gnutls.so.4` plus the Ubuntu-compatible `libcurl-gnutls.so.3` link.
- The OpenSSL dev package must provide `/usr/lib/<multiarch>/libcurl.so`, `/usr/lib/<multiarch>/libcurl.a`, headers only under `/usr/include/<multiarch>/curl/`, `curl-config`, and multiarch `libcurl.pc`; the GnuTLS dev package must provide `/usr/lib/<multiarch>/libcurl-gnutls.so`, `/usr/lib/<multiarch>/libcurl-gnutls.a`, generic `/usr/lib/<multiarch>/libcurl.so` and `/usr/lib/<multiarch>/libcurl.a` links to the GnuTLS development artifacts, the same multiarch header tree, flavor-correct `curl-config`, and multiarch `libcurl.pc`.
- `libcurl4-openssl-dev` and `libcurl4-gnutls-dev` intentionally conflict, as in Ubuntu. Package-install verifiers must install the OpenSSL dev package for the OpenSSL and LDAP checks, then purge it before installing the GnuTLS dev package for GnuTLS checks; they must not try to install both dev packages in one `apt-get install` transaction.
- Use `cargo vendor --locked safe/vendor/cargo` or an equivalent deterministic process during implementation, then commit the resulting vendor tree and config. Verifiers must only consume this tracked vendor tree and must run with a fresh empty `CARGO_HOME`; they must not run `cargo vendor`, fetch the crates.io index, or use a pre-warmed `$HOME/.cargo`.
- Detached safe-only package builds must not require `../original`.
- Detached safe-only package builds must not call `scripts/build-debs.sh` or depend on a previous live-tree stamp. They must build directly from the detached tree's committed `debian/changelog` first stanza, and checkers must fail before `dpkg-buildpackage` if `dpkg-parsechangelog -S Version` does not contain `+safelibs`.
- After every detached `dpkg-buildpackage` invocation, package checkers must inspect every generated `.deb` with `dpkg-deb --field <deb> Version` and fail if any version lacks `+safelibs`. The installed-package `dpkg-query` checks are runtime confirmation only; they are not the primary detached-build stamping mechanism.
- The live root hook `scripts/build-debs.sh` may still produce commit-epoch SafeLibs versions for CI artifacts, but it must not leave the live `safe/debian/changelog` dirty. Stamp a temporary package build tree or restore the tracked changelog before returning, so detached package, autopkgtest, validator, and dependent checks consume the committed baseline unless they explicitly build from the hook's artifact directory.
- Packaged autopkgtest verifiers must run `debian/tests/upstream-tests-openssl`, `debian/tests/upstream-tests-gnutls`, and `debian/tests/curl-ldapi-test` from a detached safe-only export where `../original` is absent. The Phase 9 and final package-consuming source scans must include those three scripts, `debian/tests/control`, and `debian/tests/LDAP-bindata.c` in both live and detached exports before the scripts are executed. Verifiers must install the `.deb`s built from that same detached export and must not run the autopkgtest scripts from the live `safe/` directory.
- Packaged runtime verifiers must also run `scripts/run-rtmp-functional-tests.sh --implementation packaged` and `scripts/run-ldaps-functional-test.sh --implementation packaged` for each installed dev flavor before moving to the next conflicting dev package. These packaged checks consume the same detached-built `.deb`s as the autopkgtests and must prove installed-library behavior, not only source-artifact behavior.
- Dependent safe-package verifiers must run `test-original.sh --implementation safe` from a detached `--with-root-harness` export where `original/` is absent, using an absolute `--safe-deb-dir` that points to `.deb`s built from a detached safe-only export and setting `SAFE_MODE_ARTIFACT_ROOT` to a directory inside that detached harness. Before any dependent safe-mode package build or final dependent invocation, the verifier must assert `docker`, `docker info`, `git`, `jq`, `/dev/fuse` as a character device, and a Docker run with `--device /dev/fuse --cap-add SYS_ADMIN --security-opt apparmor:unconfined` succeeds. Safe mode in `test-original.sh` must not require `original/`, dirty original build products, root scripts outside the detached harness, or any reference-sidecar helper, and the verifier must audit the detached harness plus `SAFE_MODE_ARTIFACT_ROOT` after execution.
- Dev-tooling verifiers must run after installing exactly one safe dev package at a time. The OpenSSL check must prove `curl-config` and `pkg-config` link to the OpenSSL runtime SONAME and can perform static linkage through `/usr/lib/<multiarch>/libcurl.a`, then the verifier must purge `libcurl4-openssl-dev` before installing and checking the GnuTLS dev package. The GnuTLS check must prove the same source program links to `libcurl-gnutls.so.4` dynamically and to `/usr/lib/<multiarch>/libcurl-gnutls.a` statically through both `curl-config` and `pkg-config`, while the link commands and installed files preserve the original generic `-lcurl` contract through `/usr/lib/<multiarch>/libcurl.so` and `/usr/lib/<multiarch>/libcurl.a` links. Both checks must verify package headers, `libcurl.pc:includedir`, and autoconf builds use the multiarch include path.
- Package-build checks must run `safe/scripts/verify-package-no-refetch.py` before every `dpkg-buildpackage` or `scripts/build-debs.sh` invocation. `mk-build-deps` may install Debian dependencies first, but the actual safe package build must consume only tracked source, Debian-installed toolchain packages, `safe/vendor/upstream`, and `safe/vendor/cargo`.
- Every package-producing checker in Phase 9 must run the explicit `package_sidecar_forbidden` source-path scan before exporting or building, must run the explicit `package_consumer_sidecar_forbidden` scan against package-consuming scripts and Debian autopkgtest files in both the live tree and detached exports, and must run `safe/scripts/verify-package-payload-contract.py --deb-dir <built-deb-dir> --require-safelibs-version` against the freshly built `.deb` directory immediately after the control-contract check and before installing those packages, copying them into `dist/`, running `scripts/run-validation-tests.sh`, or invoking `test-original.sh --implementation safe`. Do not maintain a second inline payload-audit implementation in any checker.
- Phase 9 package builds, whole detached source exports, and package-consuming checks must be sidecar-free. `safe/build.rs`, `safe/debian/rules`, package helper scripts, public ABI smoke tests, compatibility staging, upstream safe harnesses, port safe harnesses, Debian autopkgtests, validator inputs, and `test-original.sh --implementation safe` must not build, install, link, `dlopen`, copy, stage, record, mention, or embed `libcurl-reference-*.so.4`, `.reference`, `reference_library_path`, `port_safe_resolve_reference_symbol`, `bridge_resolve_symbol`, `reference_backend`, or related resolver markers. Phase 9 verifiers must run public ABI smoke, compatibility staging, root `scripts/build-debs.sh`, root `scripts/run-upstream-tests.sh`, root `scripts/run-port-tests.sh`, root `scripts/run-validation-tests.sh`, packaged autopkgtests, packaged runtime smoke, and dependent safe mode, then audit their output locations and whole detached safe-only/root-harness exports for sidecar artifacts or markers. After Phase 9, only `safe/scripts/build-reference-curl.sh` and `safe/scripts/benchmark-local.sh` may retain explicit benchmark-only original-baseline helper behavior; `safe/scripts/verify-*` and `scripts/verify-*` may retain marker strings only as detector constants. Phase 10 must not be assigned primary deletion of sidecar files, source markers, or reference-backend placeholders that these Phase 9 verifiers reject.
- Implementer must commit this phase's work before yielding.

**Verification:**

- All three package checks must pass before final hardening.

**Success Criteria:**

- Every item listed under `New Outputs` is present, updated, or explicitly left unchanged because it already satisfied the plan.
- Every required `File Changes` and `Implementation Details` invariant for this phase is satisfied.
- Every verifier listed under `Verification Phases` passes exactly as written and bounces only to this phase on failure.
- All listed `Preexisting Inputs` are consumed in place; existing artifacts are not rediscovered, refetched, or regenerated from untracked sources unless this phase explicitly updates a derived safe artifact from them.

**Git Commit Requirement:**

- The implementer must commit this phase's work to git before yielding.
