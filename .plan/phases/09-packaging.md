# Phase Name
Debian Packaging, Ubuntu Install Layout, Autopkgtests, and Root Dependent Harness

## Implement Phase ID
`impl-packaging`

## Preexisting Inputs
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/include/curl/*.h`
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
- `safe/debian/patches/*.patch`
- `safe/vendor/upstream/manifest.json`
- `safe/vendor/upstream/src/*`
- `safe/vendor/upstream/tests/*`
- `safe/vendor/upstream/lib/*`
- `safe/vendor/upstream/.pc/90_gnutls.patch/*`
- `safe/vendor/upstream/debian/tests/LDAP-bindata.c`
- `safe/compat/CMakeLists.txt`
- `safe/scripts/export-tracked-tree.sh`
- `safe/scripts/build-compat-consumers.sh`
- `safe/scripts/run-upstream-tests.sh`
- `safe/scripts/run-curl-tool-smoke.sh`
- `safe/scripts/run-ldap-devpkg-test.sh`
- `safe/scripts/http-fixtures.sh`
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
- `original/debian/*.symbols`
- `original/debian/tests/*`
- `original/debian/patches/*.patch`
- `original/debian/patches/series`
- the tracked files under `original/.pc/90_gnutls.patch/`
- `original/libcurl.pc.in`
- `original/curl-config.in`
- `original/docs/libcurl/libcurl.m4`
- `original/docs/curl-config.1`
- `original/docs/curl.1`
- `test-original.sh`
- `dependents.json`
- `safe/src/lib.rs`
- `safe/src/abi/generated.rs`
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
- `safe/compat/generated-sources.cmake`
- `safe/scripts/run-curated-libtests.sh`
- `safe/scripts/run-link-compat.sh`
- `safe/scripts/run-http-client-tests.sh`
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
- `safe/tests/unit_port.rs`
- `safe/tests/unit_port_cases/`
- `safe/tests/port-map.json`
- `safe/compat/link-manifest.json`
- `safe/benchmarks/README.md`
- `safe/benchmarks/scenarios.json`
- `safe/benchmarks/thresholds.json`
- `safe/benchmarks/harness/easy_loop.c`
- `safe/benchmarks/harness/multi_parallel.c`
- `safe/scripts/benchmark-local.sh`
- `safe/scripts/compare-benchmarks.py`
- `safe/docs/performance.md`

## New Outputs
- `safe/Cargo.lock`
- `safe/.cargo/config.toml`
- `safe/vendor/cargo/*`
- finalized `safe/debian/control`
- finalized `safe/debian/changelog`
- finalized `safe/debian/copyright`
- finalized `safe/debian/README.*`
- finalized `safe/debian/rules`
- finalized `safe/debian/source/format`
- finalized `safe/debian/*.install`
- finalized `safe/debian/*.links`
- finalized `safe/debian/*.docs`
- `safe/debian/*.examples`
- `safe/debian/*.lintian-overrides`
- finalized `safe/debian/*.manpages`
- finalized `safe/debian/*.symbols`
- finalized `safe/debian/patches/series`
- `safe/debian/patches/*.patch`
- `safe/debian/tests/control`
- `safe/debian/tests/upstream-tests-openssl`
- `safe/debian/tests/upstream-tests-gnutls`
- `safe/debian/tests/curl-ldapi-test`
- `safe/debian/tests/LDAP-bindata.c`
- `safe/libcurl.pc`
- `safe/curl-config`
- `safe/docs/libcurl/libcurl.m4`
- `safe/scripts/verify-autopkgtest-contract.sh`
- `safe/scripts/verify-package-control-contract.py`
- `safe/scripts/verify-package-install-layout.sh`
- `safe/scripts/verify-devpkg-tooling-contract.sh`
- `safe/scripts/run-packaged-autopkgtests.sh`
- modified `test-original.sh`

## File Changes
- Finalize the Debian package layout under `safe/debian/`.
- Make the safe Debian source package self-contained by consuming only vendored compatibility-source assets inside `safe/`.
- Check in the Cargo lockfile, Cargo source configuration, and vendored crate sources needed for detached offline Debian builds.
- Package the safe library flavors under the same names and install paths as the current Ubuntu packages.
- Build the `curl` binary package as the OpenSSL-flavor compatibility consumer compiled against the safe library, matching the current Ubuntu package graph, while the compatibility harness continues to compile the tool against both flavors.
- Preserve the Debian autopkgtest names, control metadata, and entrypoint semantics while retargeting them to the safe package build.
- Finalize the safe-local quilt contract by keeping `safe/debian/source/format` at `3.0 (quilt)` and making `safe/debian/patches/series` the only active patch list for detached safe-package builds.
- Preserve the Debian developer-tooling contract for `curl-config`, `libcurl.pc`, and `usr/share/aclocal/libcurl.m4` in both dev packages.
- Add an explicit package-payload layout verifier so required installed files and symlinks fail deterministically instead of being inferred from archive listings.
- Modify `test-original.sh` so it can export a root-harness workspace containing tracked `safe/` plus `dependents.json`, install safe-source build-dependencies from that detached source tree inside Docker, build and install safe `.deb` packages from that workspace, and then run the downstream matrix without copying ad hoc shared libraries into `/usr/local/lib`.

## Implementation Details
- Keep the binary package names and binary dependency graph compatible with the current Debian control file so downstream package resolution stays unchanged. Preserve the emitted package identities and runtime/development dependency behavior, not the original C package's source `Build-Depends` stanza verbatim.
- Preserve the exact binary package set `curl`, `libcurl4t64`, `libcurl3t64-gnutls`, `libcurl4-openssl-dev`, `libcurl4-gnutls-dev`, and `libcurl4-doc`.
- `safe/debian/control` must preserve `Source: curl` and those six binary package stanzas, but its source stanza `Build-Depends` must be rewritten to match the Rust build path actually used by `safe/debian/rules`. At minimum the final source stanza must include `cargo:native` and `rustc:native`; it must also include every remaining native tool and library build dependency still required by the OpenSSL/GnuTLS builds and by any package-time compatibility-consumer build steps.
- `rust-clippy` remains a verifier-only executor prerequisite for phase 10 and must not be added to `safe/debian/control` `Build-Depends` unless `safe/debian/rules` actually invokes Clippy during package builds.
- Preserve the symbol files, public headers, include path `/usr/include/$(DEB_HOST_MULTIARCH)/curl`, `curl-config`, `libcurl.pc`, `docs/libcurl/libcurl.m4`, `debian/changelog`, `debian/source/format`, and the docs/manpages expected by the current dev packages.
- `safe/debian/source/format` must remain `3.0 (quilt)`. `safe/debian/patches/series` must always exist, must list only safe-local patch filenames relative to `safe/debian/patches/`, and must be the only quilt series consumed by detached safe-package builds. If no safe-local patches are required, keep the series empty or comment-only rather than reaching into `original/debian/patches/`.
- Preserve Debian’s single packaged `curl` binary that depends on `libcurl4t64`; do not invent a second GnuTLS-linked `curl` binary package.
- `safe/debian/rules`, `safe/debian/tests/*`, `safe/compat/CMakeLists.txt`, and every script they call during package build or packaged autopkgtests must consume only files inside the detached `safe/` source export. Any upstream compatibility-source asset that those steps still need must come from `safe/vendor/upstream/` or from a copied safe-local file such as `safe/debian/tests/LDAP-bindata.c`, never from `original/`, `../original`, or a sibling checkout.
- The self-containment checker's textual path audit must reject any package-time reference containing the path segment `original/`, including bare forms such as `original/include/curl/curl.h`, relative forms such as `../original/tests/...`, and absolute forms such as `/work/original/...`; it is not limited to `src/`, `tests/`, `debian/tests/`, or `.pc/90_gnutls.patch/`.
- `safe/vendor/upstream/` must preserve the relative layout required by the compatibility build, including `src/`, `tests/`, the tracked helper files under `lib/`, the tracked `.pc/90_gnutls.patch/` subtree needed to reconstruct the OpenSSL variant, and `debian/tests/LDAP-bindata.c`, so package-time paths stay stable after export.
- `safe/Cargo.lock`, `safe/.cargo/config.toml`, and `safe/vendor/cargo/` must be tracked inside `safe/`. `.cargo/config.toml` must redirect crates.io to `vendor/cargo`, and `safe/debian/rules` must invoke Cargo with `--locked --offline` and package-local `CARGO_HOME` plus flavor-specific `CARGO_TARGET_DIR` values so detached `dpkg-buildpackage` runs without network access or host-global Cargo state.
- Every phase-9 verifier that runs `dpkg-buildpackage` must assume a prepared Ubuntu 24.04 executor that already contains `build-essential`, `ca-certificates`, `devscripts`, `equivs`, `dpkg-dev`, `fakeroot`, `pkgconf`, `python3`, and `ripgrep`, and the checker command block itself must run `mk-build-deps -ir -t 'apt-get -y --no-install-recommends' debian/control` in the detached export immediately before `dpkg-buildpackage`.
- `safe/scripts/export-tracked-tree.sh --safe-only --dest <dir>` must create a detached source tree containing everything `dpkg-buildpackage`, the packaged autopkgtests, and the packaged-tool smoke checks need, with no dependency on files outside that destination.
- `safe/scripts/export-tracked-tree.sh --with-root-harness --dest <dir>` must create a Docker input tree with `<dir>/safe/` containing only tracked files from `safe/` and `<dir>/dependents.json` copied from the tracked root inventory. It must not include `original/`, `.git/`, or any other sibling path.
- `curl-config`, `libcurl.pc`, and `docs/libcurl/libcurl.m4` should start from the existing templates and Debian packaging behavior instead of being recreated from scratch. Preserve Debian’s multiarch and patchstamp behavior.
- `safe/docs/libcurl/libcurl.m4` must be installed into both development packages at `/usr/share/aclocal/libcurl.m4`, preserving the macro names and invocation shape expected by downstream autoconf consumers.
- Packaged `curl-config` must preserve the observable Debian rewrites from `original/debian/rules`: `--static-libs` must keep a runtime `krb5-config --libs gssapi` invocation instead of hardcoding its output; `--configure` must retain literal backquoted `dpkg-architecture -qDEB_HOST_MULTIARCH` and `dpkg-architecture -qDEB_BUILD_GNU_TYPE` substitutions rather than embedding host-specific values; and the emitted flags must omit `-fdebug-prefix-map`, `-ffile-prefix-map`, and `-D_DEB_HOST_ARCH`.
- Packaged `libcurl.pc` must continue to install as `usr/lib/*/pkgconfig/libcurl.pc` in both development packages and remain usable through `pkgconf --cflags --libs libcurl` against the installed package roots for both flavors.
- `safe/scripts/verify-package-control-contract.py` must compare `safe/debian/control` against `original/debian/control` for the six binary package stanzas above and fail on any drift in package presence or in the fields `Architecture`, `Multi-Arch`, `Depends`, `Pre-Depends`, `Recommends`, `Suggests`, `Provides`, `Conflicts`, `Breaks`, and `Replaces`. The same checker must also inspect the safe source stanza and fail unless the explicit Rust build-tool dependencies required above are present.
- The same package-control verifier must also inspect the built `.deb` files with `dpkg-deb -f` or `dpkg-deb -I` and fail if the emitted package metadata no longer matches the intended control-file contract after substvars expansion.
- `safe/scripts/verify-package-install-layout.sh` must inspect the built `.deb` payloads by extracting them into temporary roots, resolve `DEB_HOST_MULTIARCH` with `dpkg-architecture`, and fail unless the expected installed paths are present. At minimum it must assert `/usr/bin/curl` and `/usr/share/man/man1/curl.1.gz` in the `curl` package; `usr/lib/$DEB_HOST_MULTIARCH/libcurl.so.4*` in `libcurl4t64`; `usr/lib/$DEB_HOST_MULTIARCH/libcurl-gnutls.so.4*` plus the compatibility symlink `usr/lib/$DEB_HOST_MULTIARCH/libcurl-gnutls.so.3` in `libcurl3t64-gnutls`; every header from `safe/include/curl/*.h` under `/usr/include/$DEB_HOST_MULTIARCH/curl/`; `curl-config`, `libcurl.pc`, `usr/share/aclocal/libcurl.m4`, and the expected static/shared development symlinks in both dev packages; and the docs, examples, manpages, and documented symlink set from `safe/debian/libcurl4-doc.docs`, `safe/debian/libcurl4-doc.examples`, `safe/debian/libcurl4-doc.manpages`, and `safe/debian/libcurl4-doc.links` inside `libcurl4-doc`. It must account for debhelper’s compressed manpage outputs and symlinks. A plain `dpkg-deb -c` listing is not sufficient proof.
- `safe/scripts/verify-devpkg-tooling-contract.sh` must inspect both built dev packages, fail unless each package contains `curl-config`, `libcurl.pc`, and `usr/share/aclocal/libcurl.m4`, extract them into isolated temp roots, run the packaged `curl-config --version --cflags --libs --static-libs --configure`, run `pkgconf --cflags --libs libcurl` against the extracted pkg-config roots, and run `aclocal` plus `autoconf` on a tiny `configure.ac` that invokes `LIBCURL_CHECK_CONFIG` to prove the installed `libcurl.m4` works for real autoconf consumers.
- `safe/debian/tests/control` must preserve the exact three `Tests` names from `original/debian/tests/control`, the same `Restrictions` values, and the same `Depends` shapes: `curl, @builddeps@` for `upstream-tests-openssl`, `@builddeps@` for `upstream-tests-gnutls`, and `gcc, libc-dev, libcurl4-openssl-dev | libcurl-dev, libldap-dev, slapd, pkgconf` for `curl-ldapi-test`.
- Port `original/debian/tests/upstream-tests-openssl` to `safe/debian/tests/upstream-tests-openssl` while preserving its semantics: set `DEB_BUILD_PROFILES="pkg.curl.openssl-only"`, keep `VERBOSE=1`, clear `TESTS_FAILS_ON_IPV6_ONLY_MACHINES`, force `/usr/bin/curl`, and drive `dh_update_autotools_config`, `dh_autoreconf`, `debian/rules override_dh_auto_configure`, `override_dh_auto_build`, and `override_dh_auto_test`.
- Port `original/debian/tests/upstream-tests-gnutls` to `safe/debian/tests/upstream-tests-gnutls` while preserving its semantics: set `DEB_BUILD_PROFILES="pkg.curl.gnutls-only"`, keep `VERBOSE=1`, clear `TESTS_FAILS_ON_IPV6_ONLY_MACHINES`, and drive the same `debian/rules` targets against the in-tree GnuTLS build rather than an installed `curl`.
- Port `original/debian/tests/curl-ldapi-test` to `safe/debian/tests/curl-ldapi-test` and copy the vendored `safe/vendor/upstream/debian/tests/LDAP-bindata.c` to `safe/debian/tests/LDAP-bindata.c`, preserving the `pkgconf --cflags --libs ldap libcurl` compile-and-run contract against the installed development package.
- The packaged autopkgtest entrypoints may delegate to `safe/scripts/run-upstream-tests.sh`, `safe/scripts/run-curl-tool-smoke.sh`, and `safe/scripts/run-ldap-devpkg-test.sh`, but the actual `safe/debian/tests/upstream-tests-openssl`, `safe/debian/tests/upstream-tests-gnutls`, and `safe/debian/tests/curl-ldapi-test` files must remain the canonical entrypoints that the verifier executes directly from the detached `safe/` export.
- `safe/scripts/verify-autopkgtest-contract.sh` must compare `safe/debian/tests/control` against `original/debian/tests/control` and fail on any drift in test names, `Depends`, or `Restrictions`.
- `safe/scripts/run-packaged-autopkgtests.sh` must install the just-built safe packages, resolve the selected test from `safe/debian/tests/control`, expand any `@builddeps@` token against `safe/debian/control`, install the resulting autopkgtest dependency set, prepare the detached safe-source-tree environment expected by autopkgtest, and then execute the named script from `safe/debian/tests/` directly with an `AUTOPKGTEST_TMP` workspace rather than reimplementing the test logic elsewhere.
- Modify `test-original.sh` to add an `--implementation original|safe` mode. The `safe` mode must invoke `safe/scripts/export-tracked-tree.sh --with-root-harness --dest <host-dir>` before `docker run`, bind-mount that host export at `/work`, reuse the same Ubuntu 24.04 package-build baseline as the package verifiers by ensuring the image already includes `build-essential`, `ca-certificates`, `dpkg-dev`, `fakeroot`, `pkgconf`, and `python3` alongside the downstream-matrix tools, run `apt-get update`, install `devscripts` and `equivs` in the container, run `mk-build-deps -ir -t 'apt-get -y --no-install-recommends' /work/safe/debian/control`, then `cd /work/safe && dpkg-buildpackage -us -uc -b`, install the resulting safe `.deb` packages inside the container, and then run the existing dependent inventory unchanged from `/work/dependents.json`. It must not mount the whole repository or expect `/work/original` in safe mode.
- Preserve an `original` baseline mode in `test-original.sh`, including the current use of `original/.pc/90_gnutls.patch/` to reconstruct the original OpenSSL tree. The new `safe` mode must not rely on that quilt state or on `apt-get build-dep curl` as a proxy for the safe source package's Rust build dependencies.

## Verification Phases
### `check-packaging-self-contained-source`
- Type: `check`
- Bounce Target: `impl-packaging`
- Purpose: prove that a detached export of `safe/` alone contains every build/test asset needed by the Debian package and autopkgtests, and fail if package-time files still reference `original/` or another out-of-tree compatibility-source path.
- Commands it should run:
```bash
bash -lc '
set -euo pipefail
repo_root=$PWD
rm -rf /tmp/libcurl-safe-pkgcheck
bash safe/scripts/export-tracked-tree.sh --safe-only --dest /tmp/libcurl-safe-pkgcheck
scan_roots=(
  /tmp/libcurl-safe-pkgcheck/debian
  /tmp/libcurl-safe-pkgcheck/compat
  /tmp/libcurl-safe-pkgcheck/scripts/build-compat-consumers.sh
  /tmp/libcurl-safe-pkgcheck/scripts/run-upstream-tests.sh
  /tmp/libcurl-safe-pkgcheck/scripts/run-curl-tool-smoke.sh
  /tmp/libcurl-safe-pkgcheck/scripts/run-http-client-tests.sh
  /tmp/libcurl-safe-pkgcheck/scripts/run-ldap-devpkg-test.sh
  /tmp/libcurl-safe-pkgcheck/scripts/run-packaged-autopkgtests.sh
  /tmp/libcurl-safe-pkgcheck/scripts/http-fixtures.sh
  /tmp/libcurl-safe-pkgcheck/scripts/http-fixture.py
)
if rg -n '(^|[^[:alnum:]_-])original/' "${scan_roots[@]}"; then
  echo "found forbidden original/ reference in safe package-time files" >&2
  exit 1
fi
if rg -n --fixed-strings "$repo_root/" \
  "${scan_roots[@]}"; then
  echo "found forbidden absolute repo path in safe package-time files" >&2
  exit 1
fi
test -f /tmp/libcurl-safe-pkgcheck/Cargo.lock
test -f /tmp/libcurl-safe-pkgcheck/.cargo/config.toml
test -d /tmp/libcurl-safe-pkgcheck/vendor/cargo
rg -n 'replace-with *= *"vendored-sources"|directory *= *"vendor/cargo"' /tmp/libcurl-safe-pkgcheck/.cargo/config.toml >/dev/null
rg -n 'cargo:native' /tmp/libcurl-safe-pkgcheck/debian/control >/dev/null
rg -n 'rustc:native' /tmp/libcurl-safe-pkgcheck/debian/control >/dev/null
rg -n 'CARGO_NET_OFFLINE=true|--offline' /tmp/libcurl-safe-pkgcheck/debian/rules >/dev/null
rg -n -- '--locked' /tmp/libcurl-safe-pkgcheck/debian/rules >/dev/null
test "$(cat /tmp/libcurl-safe-pkgcheck/debian/source/format)" = "3.0 (quilt)"
test -f /tmp/libcurl-safe-pkgcheck/debian/patches/series
while IFS= read -r patch; do
  case "$patch" in
    ''|'#'*) continue ;;
  esac
  test -f "/tmp/libcurl-safe-pkgcheck/debian/patches/$patch"
done </tmp/libcurl-safe-pkgcheck/debian/patches/series
cd /tmp/libcurl-safe-pkgcheck
mk-build-deps -ir -t 'apt-get -y --no-install-recommends' debian/control
dpkg-buildpackage -us -uc -b
'
```

### `check-packaging-control-contract`
- Type: `check`
- Bounce Target: `impl-packaging`
- Purpose: verify that the detached safe-source export preserves the current Ubuntu binary package contract in both `debian/control` and the built `.deb` metadata.
- Commands it should run:
```bash
bash -lc '
set -euo pipefail
repo_root=$PWD
rm -rf /tmp/libcurl-safe-pkgcheck
bash safe/scripts/export-tracked-tree.sh --safe-only --dest /tmp/libcurl-safe-pkgcheck
cd /tmp/libcurl-safe-pkgcheck
mk-build-deps -ir -t 'apt-get -y --no-install-recommends' debian/control
dpkg-buildpackage -us -uc -b
python3 scripts/verify-package-control-contract.py --expected-control "$repo_root/original/debian/control" --actual-control debian/control --package-root . --require-source-build-deps cargo:native rustc:native
'
```

### `check-packaging-install-layout`
- Type: `check`
- Bounce Target: `impl-packaging`
- Purpose: verify that the detached safe-source export builds the correct Ubuntu binary packages, preserves the required Ubuntu install paths and symlink layout for the runtime, dev, tool, and doc packages, and still supports the existing dev-package compile test.
- Commands it should run:
```bash
bash -lc '
set -euo pipefail
rm -rf /tmp/libcurl-safe-pkgcheck
bash safe/scripts/export-tracked-tree.sh --safe-only --dest /tmp/libcurl-safe-pkgcheck
cd /tmp/libcurl-safe-pkgcheck
mk-build-deps -ir -t 'apt-get -y --no-install-recommends' debian/control
dpkg-buildpackage -us -uc -b
bash scripts/verify-package-install-layout.sh --package-root .
bash scripts/run-curl-tool-smoke.sh --implementation packaged --flavor openssl --package-root .
bash scripts/run-ldap-devpkg-test.sh --flavor openssl --package-root .
bash scripts/run-ldap-devpkg-test.sh --flavor gnutls --package-root .
'
```

### `check-packaging-devpkg-tooling`
- Type: `check`
- Bounce Target: `impl-packaging`
- Purpose: verify the packaged developer-tooling contract for both dev packages from the detached safe-source export, including executable `curl-config`, usable `libcurl.pc`, and installable `libcurl.m4`.
- Commands it should run:
```bash
bash -lc '
set -euo pipefail
rm -rf /tmp/libcurl-safe-pkgcheck
bash safe/scripts/export-tracked-tree.sh --safe-only --dest /tmp/libcurl-safe-pkgcheck
cd /tmp/libcurl-safe-pkgcheck
mk-build-deps -ir -t 'apt-get -y --no-install-recommends' debian/control
dpkg-buildpackage -us -uc -b
bash scripts/verify-devpkg-tooling-contract.sh --package-root .
'
```

### `check-packaging-autopkgtests`
- Type: `check`
- Bounce Target: `impl-packaging`
- Purpose: verify that the detached safe-source export preserves the Debian autopkgtest contract and that the actual packaged autopkgtest entrypoints execute successfully.
- Commands it should run:
```bash
bash -lc '
set -euo pipefail
repo_root=$PWD
rm -rf /tmp/libcurl-safe-pkgcheck
bash safe/scripts/export-tracked-tree.sh --safe-only --dest /tmp/libcurl-safe-pkgcheck
cd /tmp/libcurl-safe-pkgcheck
mk-build-deps -ir -t 'apt-get -y --no-install-recommends' debian/control
dpkg-buildpackage -us -uc -b
bash scripts/verify-autopkgtest-contract.sh --expected-control "$repo_root/original/debian/tests/control" --actual-control debian/tests/control
bash scripts/run-packaged-autopkgtests.sh --package-root . --test upstream-tests-openssl
bash scripts/run-packaged-autopkgtests.sh --package-root . --test upstream-tests-gnutls
bash scripts/run-packaged-autopkgtests.sh --package-root . --test curl-ldapi-test
'
```

### `check-packaging-dependents`
- Type: `check`
- Bounce Target: `impl-packaging`
- Purpose: verify that the root Docker-based dependent harness can build and install the safe packages and that every dependent still compiles and runs.
- Commands it should run:
```bash
bash ./test-original.sh --implementation safe
```

## Success Criteria
- Every listed `Preexisting Input` is consumed as an existing artifact rather than rediscovered, regenerated, or refetched.
- Every listed `New Output` for this implement phase exists and is ready for downstream phases in the linear workflow.
- The verifier phase(s) `check-packaging-self-contained-source`, `check-packaging-control-contract`, `check-packaging-install-layout`, `check-packaging-devpkg-tooling`, `check-packaging-autopkgtests`, `check-packaging-dependents` pass exactly as written for `impl-packaging`.

## Git Commit Requirement
The implementer must commit this phase's work to git before yielding. Ignored-only or untracked-only outputs are not acceptable.
