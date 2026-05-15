# libcurl Safe Port Plan

## Context

This workspace contains an upstream-style libcurl 8.5.0 source snapshot under `original/` and no Rust implementation yet under `safe/`. The goal is to create a standard Rust package in `safe/` that can replace Ubuntu 24.04’s libcurl packages without breaking compile-time, link-time, or runtime compatibility for existing C consumers, precompiled objects, Debian autopkgtests, or the downstream software matrix already captured in this repository.

The public ABI surface is large but well-bounded. `original/libcurl.def` exports 93 public `curl_*` symbols, and the Linux package ABI further constrains them through:

- `original/debian/libcurl4t64.symbols`
- `original/debian/libcurl3t64-gnutls.symbols`
- `original/lib/libcurl.vers.in`
- `original/include/curl/*.h`

The canonical installed headers live in:

- `original/include/curl/curl.h`
- `original/include/curl/curlver.h`
- `original/include/curl/easy.h`
- `original/include/curl/header.h`
- `original/include/curl/mprintf.h`
- `original/include/curl/multi.h`
- `original/include/curl/options.h`
- `original/include/curl/stdcheaders.h`
- `original/include/curl/system.h`
- `original/include/curl/typecheck-gcc.h`
- `original/include/curl/urlapi.h`
- `original/include/curl/websockets.h`

Those headers define both opaque handles and layout-sensitive public structs that must stay source- and binary-compatible. The especially important layout-sensitive structs include:

- `struct curl_httppost` in `original/include/curl/curl.h`
- `struct curl_ssl_backend` in `original/include/curl/curl.h`
- `struct curl_tlssessioninfo` in `original/include/curl/curl.h`
- `struct curl_version_info_data` in `original/include/curl/curl.h`
- `struct curl_blob` in `original/include/curl/easy.h`
- `struct curl_waitfd` in `original/include/curl/multi.h`
- `struct curl_header` in `original/include/curl/header.h`
- `struct curl_ws_frame` in `original/include/curl/websockets.h`
- `struct curl_easyoption` in `original/include/curl/options.h`

Several public entrypoints are variadic, which means the final implementation necessarily needs a permanent C ABI layer even after the core logic moves to Rust:

- `curl_easy_setopt`
- `curl_easy_getinfo`
- `curl_multi_setopt`
- `curl_share_setopt`
- `curl_formadd`
- the `curl_mprintf*` family

Internally, the C implementation revolves around a few central state structures and state machines:

- `struct UrlState`, `struct UserDefined`, and `struct Curl_easy` in `original/lib/urldata.h`
- `struct Curl_multi` and `MSTATE_*` transitions in `original/lib/multihandle.h`
- global initialization and easy-handle lifecycle in `original/lib/easy.c`
- option parsing and dispatch in `original/lib/setopt.c`, `original/lib/getinfo.c`, and `original/lib/easyoptions.c`
- multi-handle scheduling and timers in `original/lib/multi.c`
- connection reuse and identity in `original/lib/conncache.c`, `original/lib/connect.c`, and `original/lib/cfilters.h`
- URL API behavior in `original/lib/urlapi.c`
- MIME and legacy form support in `original/lib/mime.c` and `original/lib/formdata.c`
- headers API in `original/lib/headers.c`
- WebSockets in `original/lib/ws.c`

The implementation footprint is substantial. `original/lib/Makefile.inc` enumerates:

- 130 core library C files
- 13 `vauth/` C files
- 14 `vtls/` C files
- 4 `vquic/` C files
- 3 `vssh/` C files

That is 164 library implementation files before counting headers. The command-line tool is also part of the compatibility surface: `original/src/Makefile.inc` lists 42 `CURL_CFILES` in `src/` plus 8 reused `CURLX_CFILES` from `../lib/`, and the tracked Debian-patched `original/src/Makefile.am` currently links `curl` against `libcurl-gnutls.la` while the OpenSSL baseline is reconstructed from `original/.pc/90_gnutls.patch/`. The safe port therefore cannot treat the `curl` tool as optional test-only code; the compatibility harness must compile it against both safe flavors, and the final package set must still ship the single OpenSSL-linked `curl` binary package that Ubuntu 24.04 ships today.

The repository uses multiple build systems, but Debian packaging is the compatibility contract that matters for Ubuntu 24.04:

- autotools inputs under `original/configure.ac`, `original/Makefile.am`, `original/lib/Makefile.inc`, `original/src/Makefile.am`, and `original/tests/Makefile.am`
- an upstream CMake build in `original/CMakeLists.txt`
- Debian packaging in `original/debian/control`, `original/debian/changelog`, `original/debian/copyright`, `original/debian/source/format`, `original/debian/rules`, `original/debian/*.install`, `original/debian/*.links`, `original/debian/*.docs`, `original/debian/*.examples`, `original/debian/*.lintian-overrides`, `original/debian/*.manpages`, `original/debian/*.symbols`, `original/debian/tests/*`, and `original/debian/patches/*`

`original/debian/rules` builds two library flavors in separate build trees:

- OpenSSL package path producing `libcurl4t64` and `libcurl4-openssl-dev`
- GnuTLS package path producing `libcurl3t64-gnutls` and `libcurl4-gnutls-dev`

The package rules also preserve Debian-specific behavior for:

- symbol namespaces `CURL_OPENSSL_4` and `CURL_GNUTLS_3`
- `curl-config`, including the architecture-independent `--configure` rewriting and runtime `krb5-config` `--static-libs` behavior from `original/debian/rules`
- `libcurl.pc`
- `docs/libcurl/libcurl.m4` installed at `/usr/share/aclocal/libcurl.m4`
- headers under `/usr/include/$(DEB_HOST_MULTIARCH)/curl`
- package names and dependencies

For the Rust port, Ubuntu compatibility applies to the emitted binary packages and installed developer/runtime behavior, not to the original C package's source `Build-Depends` verbatim. `original/debian/control`'s source stanza describes the current autotools C build; the safe plan must preserve the six binary package stanzas and their observable metadata while making the Rust source-build requirements explicit enough that a detached `safe/` export can be built without guessing at toolchain or crate provisioning.

There is an important packaging-specific baseline artifact already in the workspace: `test-original.sh` reconstructs the original two-flavor build behavior by exporting only tracked sources and restoring pre-`90_gnutls.patch` files from `original/.pc/90_gnutls.patch/` for the OpenSSL baseline tree. The safe implementation must preserve that original baseline mode while adding a safe-package mode that does not depend on quilt state or dirty build outputs.

`original/debian/source/format` currently declares `3.0 (quilt)`. The safe package should preserve that source format, but it must do so with its own tracked `safe/debian/patches/series` and any safe-local patches it actually needs. The original Debian patch stack remains a reference input for behavior, packaging semantics, and regression design; it must not become an implicit patch source for detached safe-package builds.

Because Debian packaging and autopkgtests rebuild the tool and test consumers from source, the final `safe/` Debian source package cannot reach back into sibling `original/` paths for those assets. The workflow must vendor the tracked upstream compatibility-source inputs needed by the safe package into `safe/vendor/upstream/` early enough that later `dpkg-buildpackage` and packaged-autopkgtest checks can run from a detached export of `safe/` alone.

The test surface already exists and must be consumed in place instead of rediscovered:

- `original/tests/data/Makefile.inc` defines 1677 ordered `TESTCASES` tokens and 1676 unique test ids because `test1190` appears twice
- `original/tests/data/DISABLED` is part of the tracked upstream harness contract. `original/tests/runtests.pl` skips ids listed there unless `-f` is passed, so the final "no exclusions" proof must honor that tracked file exactly and must not add ad hoc skips or force disabled ids through `runtests.pl`
- `original/tests/libtest/Makefile.inc` defines 256 `noinst_PROGRAMS` entries, including the four nonnumeric compatibility consumers `chkhostname`, `libauthretry`, `libntlmconnect`, and `libprereq`
- `original/tests/unit/Makefile.inc` contains 46 `unitNNNN_SOURCES` definitions and 3 enabled upstream `UNITPROGS`
- `original/tests/http/clients/Makefile.inc` lists 7 tracked HTTP/WebSocket client programs
- `original/tests/server/Makefile.inc` defines 10 server-helper programs
- `original/tests/Makefile.am` defines both `full-test` and `nonflaky-test`; the latter is explicitly weaker and cannot be the final proof
- `original/debian/tests/control` defines 3 Debian autopkgtests with fixed contract details:
  - `upstream-tests-openssl` depends on `curl, @builddeps@`, sets `DEB_BUILD_PROFILES="pkg.curl.openssl-only"`, and drives `debian/rules override_dh_auto_configure`, `override_dh_auto_build`, and `override_dh_auto_test` while forcing `/usr/bin/curl`
  - `upstream-tests-gnutls` depends on `@builddeps@`, sets `DEB_BUILD_PROFILES="pkg.curl.gnutls-only"`, and drives the same `debian/rules` targets against the in-tree GnuTLS build rather than the installed `curl`
  - `curl-ldapi-test` depends on `gcc, libc-dev, libcurl4-openssl-dev | libcurl-dev, libldap-dev, slapd, pkgconf` and compiles `debian/tests/LDAP-bindata.c`
- `test-original.sh` smoke-tests 12 representative downstream dependents from `dependents.json`

The downstream dependent inventory in `dependents.json` is already curated and must be consumed as-is. The current matrix contains:

- Git
- CMake
- PHP cURL extension
- PycURL
- R curl package
- GDAL
- OSTree
- librepo
- HTSlib
- pacman/libalpm
- HTTPDirFS
- fwupd

The root harness is also part of the compatibility contract. `test-original.sh` currently:

- builds a Docker image for Ubuntu 24.04
- requires `docker`, `git`, and `jq` on the host
- requires `/dev/fuse` to exercise HTTPDirFS
- exports only tracked files with `git ls-files`
- builds both original flavors locally
- provisions loopback HTTP fixtures
- compiles or runs each dependent strictly through libcurl’s public API

The safe-mode dependent harness must use a separate tracked-source export from the detached `--safe-only` package export: it needs tracked `safe/` plus root `dependents.json`, but it must not mount `original/` into Docker when exercising the safe implementation.

The tracked HTTP test assets require care. `original/tests/http/README.md` describes a larger pytest-based suite, but the tracked tree under `original/tests/http/` contains only:

- `Makefile*`
- `config.ini.in`
- `README.md`
- the 7 C client programs

The workflow must therefore use the tracked HTTP client programs as the canonical HTTP extension test surface and must not invent or regenerate the missing pytest fixture tree.

Security is a first-class objective independent of memory safety. `relevant_cves.json` records 107 curated non-memory-corruption CVEs under its `cves` array that still matter after a Rust rewrite, including categories such as:

- certificate and transport validation
- connection reuse and authentication isolation
- cookies, HSTS, and origin policy
- credential leakage
- parsing and canonicalization
- randomness and nonce generation
- platform loading and packaging
- resource exhaustion and API contracts

The repository also already carries 21 Debian CVE patch files under `original/debian/patches/CVE-*.patch`. The plan must use these artifacts as prepared inputs and convert them into explicit regression coverage rather than treating Rust’s memory safety as sufficient.

Allocator compatibility is another hard ABI requirement. `curl_global_init_mem` allows callers to replace libcurl’s allocator family before any other use. Any memory allocated by the Rust port and returned across the C ABI boundary, including strings or arrays returned by functions such as `curl_easy_escape`, `curl_url_get`, `curl_version`, `curl_maprintf`, `curl_mvaprintf`, and `curl_multi_get_handles`, must be managed through a runtime-switchable allocation facade that honors libcurl’s allocator contract.

The current workspace is also dirty from prior builds. Files such as `original/config.status`, `original/lib/*.o`, `original/tests/*/.deps`, untracked executables like `original/tests/libtest/lib500`, and untracked HTTP-client binaries like `original/tests/http/clients/ws-data` are present, but they are not canonical inputs for planning or verification. The plan must consistently consume tracked source artifacts from `original/`, `dependents.json`, `relevant_cves.json`, and `test-original.sh`, while treating generated build products as disposable noise.

## Generated Workflow Contract

- The generated workflow must be strictly linear. Do not use `parallel_groups`.
- The generated workflow YAML must be self-contained and inline-only. Do not use top-level `include`, or any phase-level `prompt_file`, `workflow_file`, `workflow_dir`, `checks`, or similar YAML indirection.
- The final `safe/` Debian source package must be self-contained. If the safe build still needs tracked upstream compatibility assets, an earlier phase must vendor them under `safe/vendor/upstream/`; no later build, package, or autopkgtest step may read them from `original/`, `../original`, or any other sibling directory.
- Use only fixed `bounce_target` values. Do not use `bounce_targets` lists or verifier-controlled routing.
- Every verifier must be an explicit top-level `check` phase.
- Every verifier must live with the implement phase it verifies and must bounce only to that implement phase.
- If a verifier needs to run tests, builds, package commands, Docker commands, benchmark commands, ABI comparisons, or review commands, those commands must be written directly into the checker instructions. Do not model them as separate non-agentic workflow phases.
- Checker commands must be runnable exactly as written. If a command depends on `cd`, that `cd` must be in the same shell block as the dependent commands or the commands must use paths valid from the repository root.
- Any verifier that proves Debian package builds or packaged autopkgtests must first export a detached source tree containing tracked files from `safe/` only, then run `dpkg-buildpackage` and the autopkgtest entrypoints inside that detached tree so accidental `original/` dependencies fail closed. Those package-oriented verifiers must run in an explicit Ubuntu 24.04 package-build environment whose prerequisite package set is named in the plan rather than implied; at minimum that baseline environment must already contain `build-essential`, `ca-certificates`, `devscripts`, `equivs`, `dpkg-dev`, `fakeroot`, `pkgconf`, `python3`, and `ripgrep`, and the verifier commands must then run `mk-build-deps -ir -t 'apt-get -y --no-install-recommends' <safe-root>/debian/control` before `dpkg-buildpackage` so the detached safe source package installs its own declared `Build-Depends`.
- Any verifier that runs `cargo clippy` must run in an executor that already contains the Ubuntu 24.04 `rust-clippy` package or an equivalent Clippy component. This is a verifier prerequisite only, not a `safe/debian/control` `Build-Depends` requirement, unless `safe/debian/rules` itself invokes Clippy.
- The safe Debian source package must preserve `safe/debian/source/format` as `3.0 (quilt)` and carry an explicit tracked `safe/debian/patches/series`. If the safe package needs no local quilt patches, that `series` file must still exist and remain empty or comment-only; if patches are needed, every listed patch must live under `safe/debian/patches/` and be exported with the detached safe source tree. `original/debian/patches/*.patch` and `original/debian/patches/series` remain reference inputs only and must not be treated as the active safe patch stack.
- No verifier may invoke a script, manifest, or generated source that is first created in a later phase. Compatibility-harness artifacts must therefore be produced before any verifier uses them.
- Consume the existing prepared artifacts in place. The workflow must treat the following as canonical inputs and must not refetch or regenerate them from scratch:
  - `original/include/curl/*.h`
  - `original/libcurl.def`
  - `original/lib/libcurl.vers.in`
  - `original/lib/Makefile.inc`
  - `original/src/Makefile.am`
  - `original/src/Makefile.inc`
  - `original/curl-config.in`
  - `original/libcurl.pc.in`
  - `original/docs/libcurl/libcurl.m4`
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
  - `original/tests/runtests.pl`
  - `original/tests/data/Makefile.inc`
  - `original/tests/data/DISABLED`
  - `original/tests/data/test*`
  - the tracked files under `original/tests/libtest/`
  - the tracked files under `original/tests/server/`
  - the tracked files under `original/tests/unit/`
  - the tracked files under `original/tests/http/`
  - `original/tests/http/config.ini.in`
  - the Perl/Python helper files under `original/tests/`
  - `dependents.json`
  - `relevant_cves.json`
  - `test-original.sh`
- Preserve the consume-existing-artifacts contract explicitly. The workflow must not refetch upstream tarballs, rerun dependent discovery to rebuild `dependents.json`, regenerate the CVE inventory, or fabricate the missing pytest HTTP fixture tree referenced only by `original/tests/http/README.md`.
- Treat the current generated build products under `original/` as non-canonical. Files such as `original/config.status`, `original/lib/*.o`, `original/tests/*/.deps`, untracked executables like `original/tests/libtest/lib500`, and similar outputs must not be used as workflow inputs.
- Keep `original/` as the reference snapshot and place new Rust code, new build logic, vendored upstream compatibility assets required by the final safe package, new compatibility harnesses, new package files, and new benchmark/test assets under `safe/`, except for the root harness file `test-original.sh`, which must be modified in place.
- The workflow must preserve the existing `test-original.sh` baseline behavior for the original implementation while adding a safe-package mode. Safe packaging must not depend on quilt state under `original/.pc/`, even though original-baseline mode still uses it.
- `safe/scripts/export-tracked-tree.sh` must expose two fixed modes with no implicit path discovery: `--safe-only --dest <dir>` exports only tracked files from `safe/` into `<dir>/` for detached package/autopkgtest builds, while `--with-root-harness --dest <dir>` exports tracked files from `safe/` into `<dir>/safe/` plus tracked root `dependents.json` into `<dir>/dependents.json` for `test-original.sh --implementation safe`.
- `test-original.sh --implementation safe` must create the `--with-root-harness` export on the host and mount that export into Docker instead of mounting the whole repository, so any remaining dependency on `original/`, `.git/`, or another sibling path fails closed.
- The generated workflow must preserve exact upstream consumer build metadata. `safe/metadata/test-manifest.json` must carry the common and target-specific consumer `CPPFLAGS`, `CFLAGS`, `LDFLAGS`, `LDADD`, `LIBS`, `DEPENDENCIES`, generated-source rules, shared-source/different-define mappings extracted from the tracked upstream build files, and a per-target link-role flag that distinguishes actual libcurl consumers from auxiliary helper targets. `safe/scripts/run-link-compat.sh` must relink only object files emitted by `safe/scripts/build-compat-consumers.sh` from that metadata.
- The final generated workflow must include an explicit performance implementation phase and an explicit performance check phase. Performance cannot be left to a generic cleanup step.
- The generated workflow must verify flavor-specific public C ABI smoke separately. Any checker that compiles `safe/tests/smoke/public_api_smoke.c` must use isolated per-flavor build artifacts through distinct `CARGO_TARGET_DIR` values or a dedicated flavor-aware runner; it must not build both flavors into one shared `safe/target/debug` tree and run only once.
- The generated workflow must include an explicit Debian autopkgtest contract verifier that checks `safe/debian/tests/control` against `original/debian/tests/control` for the three existing test names, `Depends` stanzas, and `Restrictions` values, and it must run the actual `safe/debian/tests/upstream-tests-openssl`, `safe/debian/tests/upstream-tests-gnutls`, and `safe/debian/tests/curl-ldapi-test` entrypoints rather than shadow reimplementations.
- The generated workflow must make packaged autopkgtest dependency installation explicit. `safe/scripts/run-packaged-autopkgtests.sh` must resolve the selected test from `safe/debian/tests/control`, expand `@builddeps@` against `safe/debian/control`, install the resulting packages together with the just-built safe `.deb`s inside the prepared package-build environment, and only then execute the actual entrypoint from `safe/debian/tests/`.
- The generated workflow must include an explicit safe-source self-containment verifier that exports tracked files from `safe/` alone, builds that detached tree with `dpkg-buildpackage`, and fails if `safe/debian/rules`, `safe/debian/tests/*`, `safe/compat/*`, or the package/autopkgtest scripts they call still reference `original/`, `../original`, or other out-of-tree compatibility-source paths.
- The generated workflow must distinguish the binary package contract from the safe source package's build requirements. `safe/debian/control` must preserve the six Ubuntu binary package stanzas as the compatibility contract, but its source stanza `Build-Depends` must explicitly cover the Rust build path actually used by `safe/debian/rules`; at minimum the final plan must require `cargo:native` and `rustc:native`, plus every remaining native tool or library build dependency still needed by the OpenSSL/GnuTLS feature matrix and packaged compatibility consumers.
- The generated workflow must make detached Rust package builds offline and self-contained. By the packaging phase, the tracked `safe/` tree must contain a checked-in `Cargo.lock`, a checked-in `.cargo/config.toml` that redirects crates.io to a checked-in `safe/vendor/cargo/` tree, and `safe/debian/rules` must invoke Cargo with `--locked --offline` or equivalent exported `CARGO_NET_OFFLINE=true` semantics so `dpkg-buildpackage` does not depend on crates.io or a developer-global Cargo cache.
- The generated workflow must include an explicit Debian package-control verifier that checks the binary package stanzas in `safe/debian/control` against `original/debian/control` for the package set `curl`, `libcurl4t64`, `libcurl3t64-gnutls`, `libcurl4-openssl-dev`, `libcurl4-gnutls-dev`, and `libcurl4-doc`, and also inspects the built `.deb` metadata to confirm that `Architecture`, `Multi-Arch`, `Depends`, `Pre-Depends`, `Recommends`, `Suggests`, `Provides`, `Conflicts`, `Breaks`, and `Replaces` still match the intended contract after substvars expansion.
- The generated workflow must include an explicit Debian package-install-layout verifier that extracts or inspects the built `.deb` payloads and fails unless the required runtime-library sonames and symlinks, packaged `curl` binary and manpage, public headers under `/usr/include/$(DEB_HOST_MULTIARCH)/curl`, development-package `curl-config`, `libcurl.pc`, and `usr/share/aclocal/libcurl.m4`, and the `libcurl4-doc` docs/examples/manpages/symlink set are present at the expected Ubuntu install paths. A bare `dpkg-deb -c` listing is not sufficient proof.
- The generated workflow must include an explicit Debian dev-package tooling verifier that checks both built development packages for the installed files `usr/bin/curl-config`, `usr/lib/*/pkgconfig/libcurl.pc`, and `usr/share/aclocal/libcurl.m4`; verifies that packaged `curl-config` preserves Debian’s architecture-independent `--configure` rewriting and runtime `krb5-config` `--static-libs` behavior from `original/debian/rules`; verifies `pkgconf --cflags --libs libcurl` against each packaged dev-root; and proves `usr/share/aclocal/libcurl.m4` works for a real `aclocal`/autoconf consumer.
- The generated workflow must make the `test-original.sh --implementation safe` Docker provisioning path explicit. Safe mode must reuse an Ubuntu 24.04 image that already names the baseline package-build prerequisites `build-essential`, `ca-certificates`, `dpkg-dev`, `fakeroot`, `pkgconf`, and `python3` in addition to the downstream-matrix tools, then run `apt-get update`, install `devscripts` and `equivs`, run `mk-build-deps -ir -t 'apt-get -y --no-install-recommends' /work/safe/debian/control` against the mounted detached `safe/` export, and only then run `dpkg-buildpackage -us -uc -b` in `/work/safe`; it must not rely on `apt-get build-dep curl` or on hard-coded guesses about Rust packages.
- The final generated workflow must include an explicit independence audit after the temporary forwarder bridge is removed. That audit must fail if `safe/c_shim/forwarders.c` still exists or if the final OpenSSL/GnuTLS libraries or packaged binaries retain `DT_NEEDED`, `RPATH`, or `RUNPATH` references into the transitional reference-build area such as `safe/.reference/` or `libcurl-reference-*`.
- Every implement prompt in the final generated workflow must instruct the agent to commit its work to git before yielding.
- The generated workflow must preserve phase-to-phase artifact flow by explicitly consuming earlier outputs such as:
  - `safe/metadata/abi-manifest.json`
  - `safe/metadata/test-manifest.json`
  - `safe/metadata/cve-manifest.json`
  - `safe/metadata/cve-to-test.json`
  - `safe/scripts/verify-cve-coverage.py`
  - `safe/abi/libcurl-openssl.map`
  - `safe/abi/libcurl-gnutls.map`
  - `safe/scripts/build-reference-curl.sh`
  - `safe/scripts/vendor-compat-assets.sh`
  - `safe/vendor/upstream/manifest.json`
  - `safe/vendor/upstream/src/*`
  - `safe/vendor/upstream/tests/*`
  - `safe/vendor/upstream/lib/*`
  - `safe/vendor/upstream/.pc/90_gnutls.patch/*`
  - `safe/vendor/upstream/debian/tests/LDAP-bindata.c`
  - `safe/scripts/run-public-abi-smoke.sh`
  - `safe/scripts/export-tracked-tree.sh`
  - `safe/compat/CMakeLists.txt`
  - `safe/compat/generated-sources.cmake`
  - `safe/compat/link-manifest.json`
  - `safe/scripts/build-compat-consumers.sh`
  - `safe/scripts/run-curated-libtests.sh`
  - `safe/scripts/run-link-compat.sh`
  - `safe/scripts/run-upstream-tests.sh`
  - `safe/scripts/run-curl-tool-smoke.sh`
  - `safe/scripts/run-http-client-tests.sh`
  - `safe/scripts/run-ldap-devpkg-test.sh`
  - `safe/scripts/verify-autopkgtest-contract.sh`
  - `safe/scripts/verify-package-control-contract.py`
  - `safe/scripts/verify-package-install-layout.sh`
  - `safe/scripts/verify-devpkg-tooling-contract.sh`
  - `safe/scripts/run-packaged-autopkgtests.sh`
  - `safe/Cargo.lock`
  - `safe/.cargo/config.toml`
  - `safe/vendor/cargo/*`
  - `safe/debian/patches/series`
  - `safe/debian/patches/*.patch`
  - `safe/scripts/http-fixtures.sh`
  - `safe/tests/port-map.json`
  - `safe/benchmarks/scenarios.json`
  - `safe/benchmarks/thresholds.json`
  - `safe/debian/*`
  - `safe/libcurl.pc`
  - `safe/curl-config`
  - `safe/docs/libcurl/libcurl.m4`
  - the modified `test-original.sh`
- The final generated workflow must require a full end-to-end verification pass that executes the entire tracked original test surface and all downstream compatibility checks. At minimum, the final verifier must cover:
  - every ordered `TESTCASES` token from `original/tests/data/Makefile.inc`, with the duplicate `test1190` preserved, by executing every token not disabled by the tracked `original/tests/data/DISABLED` rules for the selected flavor and by reporting the disabled-token set explicitly instead of silently omitting it
  - every manifest-recorded compatibility target, including every `noinst_PROGRAMS` entry from `original/tests/libtest/Makefile.inc`, with libcurl consumers building against the safe library and auxiliary helper targets preserving their upstream non-libcurl link lines
  - every tracked HTTP client program from `original/tests/http/clients/Makefile.inc`
  - every original unit source id from `original/tests/unit/Makefile.inc`
  - flavor-isolated public C smoke of `safe/tests/smoke/public_api_smoke.c` for both the OpenSSL and GnuTLS builds
  - execution of the full relink manifest in `safe/compat/link-manifest.json`, where each selected original object-file consumer reuses the object outputs built from the manifest-recorded upstream compile metadata, is relinked without recompilation, and then executes successfully under its declared runtime adapter for both flavors
  - both package flavors’ symbol/version contracts
  - explicit verification that every curated CVE in `safe/metadata/cve-manifest.json` is represented in `safe/metadata/cve-to-test.json` and that the mapped regression cases exist
  - the Debian autopkgtest control contract plus the actual `upstream-tests-openssl`, `upstream-tests-gnutls`, and `curl-ldapi-test` entrypoints from `safe/debian/tests/`
  - the Debian package-control contract for `curl`, `libcurl4t64`, `libcurl3t64-gnutls`, `libcurl4-openssl-dev`, `libcurl4-gnutls-dev`, and `libcurl4-doc`, validated against both `safe/debian/control` and the built `.deb` metadata
  - the Debian package-install-layout contract for the built `.deb` payloads, covering the runtime-library soname/symlink files, packaged `curl` binary/manpage, public headers, development metadata files, and `libcurl4-doc` docs/examples/manpages at the expected Ubuntu paths
  - the safe source-package build contract, including explicit Rust toolchain `Build-Depends` in `safe/debian/control` and checked-in `Cargo.lock`, `.cargo/config.toml`, and `safe/vendor/cargo/` artifacts that keep detached package builds offline
  - the Debian dev-package tooling contract for both `libcurl4-openssl-dev` and `libcurl4-gnutls-dev`, covering packaged `curl-config`, `libcurl.pc`, and installed `usr/share/aclocal/libcurl.m4`
  - runtime smoke of the compatibility-built `curl` tool for both flavors and the packaged OpenSSL `curl` binary
  - the full dependent matrix from `dependents.json`
  - the benchmark matrix defined in `safe/benchmarks/scenarios.json`, compared against the original implementation using `safe/benchmarks/thresholds.json`
  - `cargo clippy --all-targets -D warnings` for both feature flavors on an executor with `rust-clippy` explicitly provisioned
  - an explicit final audit that `safe/c_shim/forwarders.c` is gone and that neither final flavor library nor packaged binary depends on the transitional reference build
- The final generated workflow must not accept `test-nonflaky`, `TEST_NF`, `~flaky`, `~timing-dependent`, keyword-only subsets, or any other reduced matrix as the final proof. If package-build-time tests remain reduced for practical reasons, the final project verifier must still run the full suite separately and explicitly. For this contract, the tracked upstream `tests/data/DISABLED` file remains authoritative; "no exclusions" forbids extra ad hoc reductions, not the upstream skip file that `runtests.pl` already honors.

## Implementation Phases

### 1. Foundation, Manifests, ABI Maps, Package Skeleton, and Temporary Forwarder Bridge

- **Phase Name**: Foundation, Manifests, ABI Maps, Package Skeleton, and Temporary Forwarder Bridge
- **Implement Phase ID**: `impl-foundation`
- **Verification Phases**:
  - `check-foundation-build` — type `check`, bounce_target `impl-foundation`; purpose: verify that the Rust package skeleton, copied public headers, version scripts, and temporary exhaustive ABI bridge build for both flavors without consuming dirty `original/` outputs. Commands it should run:
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
  - `check-foundation-manifests` — type `check`, bounce_target `impl-foundation`; purpose: verify that the ABI, test, and CVE manifests exactly reflect the prepared workspace artifacts and preserve known quirks such as the duplicate `test1190` and missing pytest HTTP tree. Commands it should run:
    ```bash
    python3 safe/scripts/verify-manifests.py \
      --abi safe/metadata/abi-manifest.json \
      --tests safe/metadata/test-manifest.json \
      --cves safe/metadata/cve-manifest.json
    ```
- **Preexisting Inputs**:
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
- **New Outputs**:
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
- **File Changes**:
  - Create the `safe/` Rust package root with explicit `openssl-flavor` and `gnutls-flavor` features.
  - Copy every installed public header from `original/include/curl/` into `safe/include/curl/` without semantic edits, including `curlver.h`, `stdcheaders.h`, `system.h`, and `typecheck-gcc.h`.
  - Create manifest-generation scripts that extract ABI, test, dependent, and CVE metadata from the existing workspace files.
  - Create Debian packaging skeleton files in `safe/debian/` by adapting the existing package layout instead of inventing a new one.
  - Create an explicit safe-local quilt directory rooted at `safe/debian/patches/series` so detached package builds never depend on the original patch stack.
  - Add a temporary exhaustive exported-symbol forwarder layer so every public symbol exists from the first build.
  - Add a reference-build helper that can rebuild tracked original libcurl trees per flavor for temporary bridging without using dirty workspace outputs.
- **Implementation Details**:
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
- **Verification**:
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
  python3 safe/scripts/verify-manifests.py --abi safe/metadata/abi-manifest.json --tests safe/metadata/test-manifest.json --cves safe/metadata/cve-manifest.json
  ```

### 2. Public ABI, Global State, Allocator Contract, Varargs, MIME/Form, URL API, and Opaque Handles

- **Phase Name**: Public ABI, Global State, Allocator Contract, Varargs, MIME/Form, URL API, and Opaque Handles
- **Implement Phase ID**: `impl-public-abi`
- **Verification Phases**:
  - `check-public-abi-smoke` — type `check`, bounce_target `impl-public-abi`; purpose: confirm that non-transport public APIs compile and execute against the Rust implementation using only installed headers and the safe shared library. Commands it should run:
    ```bash
    cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test public_abi
    cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test public_abi
    bash safe/scripts/run-public-abi-smoke.sh --flavor openssl
    bash safe/scripts/run-public-abi-smoke.sh --flavor gnutls
    ```
  - `check-public-abi-layout` — type `check`, bounce_target `impl-public-abi`; purpose: verify public layout and option-table compatibility against the phase-1 ABI manifest. Commands it should run:
    ```bash
    cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test abi_layout
    cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test abi_layout
    bash safe/scripts/verify-abi-manifest.sh safe/metadata/abi-manifest.json
    ```
- **Preexisting Inputs**:
  - `safe/metadata/abi-manifest.json`
  - `safe/src/abi/generated.rs`
  - `safe/c_shim/forwarders.c`
  - `original/lib/easy.c`
  - `original/lib/version.c`
  - `original/lib/setopt.c`
  - `original/lib/getinfo.c`
  - `original/lib/easyoptions.c`
  - `original/lib/easygetopt.c`
  - `original/lib/urlapi.c`
  - `original/lib/share.c`
  - `original/lib/mime.c`
  - `original/lib/formdata.c`
  - `original/lib/strerror.c`
  - `original/include/curl/curl.h`
  - `original/include/curl/easy.h`
  - `original/include/curl/options.h`
  - `original/include/curl/urlapi.h`
  - `original/include/curl/mprintf.h`
- **New Outputs**:
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
- **File Changes**:
  - Replace temporary forwarders for non-I/O public functions with Rust implementations.
  - Add a runtime-switchable allocator facade driven by `curl_global_init_mem`.
  - Add typed Rust dispatchers for varargs APIs behind thin C shims.
  - Add MIME, legacy form, URL API, slist, share-handle, and version-reporting support with ABI-compatible public structs and allocation behavior.
- **Implementation Details**:
  - Implement `curl_global_init`, `curl_global_init_mem`, `curl_global_cleanup`, `curl_global_trace`, `curl_global_sslset`, `curl_free`, `curl_getenv`, `curl_getdate`, `curl_strequal`, `curl_strnequal`, `curl_version`, and `curl_version_info` in Rust, preserving the process-global semantics from `original/lib/easy.c` and `original/lib/version.c`.
  - The allocation facade in `safe/src/alloc.rs` must default to libc allocators and switch to user-provided callbacks exactly once `curl_global_init_mem` succeeds. Any memory returned through the public ABI, including strings from `curl_easy_escape`, `curl_url_get`, `curl_version`, `curl_maprintf`, and `curl_mvaprintf`, plus arrays from `curl_multi_get_handles`, must use this facade.
  - Model `CURL`, `CURLSH`, and `CURLU` as Rust-owned opaque state with C-visible pointers only at the ABI boundary.
  - Generate the `curl_easyoption` table from `original/lib/easyoptions.c` and preserve aliases and type tags from `original/include/curl/options.h`.
  - Keep permanent C varargs shims for `curl_easy_setopt`, `curl_easy_getinfo`, `curl_multi_setopt`, `curl_share_setopt`, and `curl_formadd`. The shim should inspect option metadata and route to type-specific Rust setters and getters.
  - Keep permanent C implementations for the `curl_mprintf*` family because preserving `va_list` semantics there is simpler and safer than re-implementing them directly in Rust. The allocating variants in that family must route through the safe allocator facade rather than raw `malloc`.
  - Implement the object-model and non-transport portions of `curl_easy_init`, `curl_easy_cleanup`, `curl_easy_reset`, `curl_easy_duphandle`, `curl_share_init`, `curl_share_cleanup`, `curl_share_strerror`, `curl_url`, `curl_url_cleanup`, `curl_url_dup`, `curl_url_get`, `curl_url_set`, `curl_url_strerror`, `curl_easy_option_by_name`, `curl_easy_option_by_id`, `curl_easy_option_next`, `curl_mime_*`, `curl_formget`, and `curl_formfree`.
  - Public layout verification must include at minimum `curl_httppost`, `curl_blob`, `curl_waitfd`, `curl_header`, `curl_ws_frame`, `curl_ssl_backend`, `curl_tlssessioninfo`, `curl_version_info_data`, and every other public struct recorded in `safe/metadata/abi-manifest.json`.
  - `safe/scripts/run-public-abi-smoke.sh` must build exactly one flavor per invocation using an isolated `CARGO_TARGET_DIR` such as `safe/target/public-abi/<flavor>`, compile `safe/tests/smoke/public_api_smoke.c` against that flavor’s headers and library directory, and run it with `LD_LIBRARY_PATH` restricted to that same flavor output so the OpenSSL and GnuTLS smoke checks cannot accidentally share artifacts.
- **Verification**:
  ```bash
  cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test public_abi
  cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test public_abi
  cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test abi_layout
  cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test abi_layout
  bash safe/scripts/run-public-abi-smoke.sh --flavor openssl
  bash safe/scripts/run-public-abi-smoke.sh --flavor gnutls
  bash safe/scripts/verify-abi-manifest.sh safe/metadata/abi-manifest.json
  ```

### 3. Compatibility Harness Foundation, Upstream Asset Vendoring, Consumer Build Scaffolding, Link Harness, and Fixture Helpers

- **Phase Name**: Compatibility Harness Foundation, Upstream Asset Vendoring, Consumer Build Scaffolding, Link Harness, and Fixture Helpers
- **Implement Phase ID**: `impl-harness-foundation`
- **Verification Phases**:
  - `check-harness-foundation-build` — type `check`, bounce_target `impl-harness-foundation`; purpose: confirm that the vendored safe-local compatibility build can compile every manifest-recorded compatibility target for both flavors, linking libcurl consumers against the safe library while preserving upstream non-libcurl link lines for auxiliary helpers such as `chkhostname` and the server programs. Commands it should run:
    ```bash
    bash safe/scripts/build-compat-consumers.sh --flavor openssl --all
    bash safe/scripts/build-compat-consumers.sh --flavor gnutls --all
    ```
  - `check-harness-foundation-smoke` — type `check`, bounce_target `impl-harness-foundation`; purpose: verify that the harness scripts themselves work against the transitional bridge by running a small set of tracked libtests, relink-and-run checks, and upstream test ids. Commands it should run:
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
- **Preexisting Inputs**:
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
  - outputs from phases 1 and 2
- **New Outputs**:
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
- **File Changes**:
  - Vendor the tracked upstream compatibility-source assets required by the tool/test/package harnesses into `safe/vendor/upstream/`.
  - Add a compatibility-consumer build system that compiles manifest-recorded libcurl consumers against the safe library while preserving the original upstream link lines for auxiliary helper targets instead of reusing the upstream libcurl build directly.
  - Add manifest-driven runners for curated libtests, full `runtests.pl`, HTTP client programs, object-file relinking and execution, the Debian LDAP dev-package compile test, and the build-state handoff that lets relink checks reuse the exact objects emitted by the compatibility build.
  - Extract reusable loopback HTTP fixture logic from the root harness into `safe/scripts/` so later benchmarking, HTTP-client tests, and safe-package validation share one implementation.
- **Implementation Details**:
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
- **Verification**:
  ```bash
  bash safe/scripts/build-compat-consumers.sh --flavor openssl --all
  bash safe/scripts/build-compat-consumers.sh --flavor gnutls --all
  bash safe/scripts/run-curated-libtests.sh --flavor openssl 500 501 506
  bash safe/scripts/run-curated-libtests.sh --flavor gnutls 500 501 506
  bash safe/scripts/run-link-compat.sh --flavor openssl --tests lib500 lib501
  bash safe/scripts/run-link-compat.sh --flavor gnutls --tests lib500 lib501
  bash safe/scripts/run-upstream-tests.sh --flavor openssl --tests 1 506
  bash safe/scripts/run-upstream-tests.sh --flavor gnutls --tests 1 506
  bash safe/scripts/run-curl-tool-smoke.sh --implementation compat --flavor openssl
  bash safe/scripts/run-curl-tool-smoke.sh --implementation compat --flavor gnutls
  ```

### 4. Easy Perform, Multi Engine, Conncache, Resolver Ownership, Share Locking, and Transfer Loop

- **Phase Name**: Easy Perform, Multi Engine, Conncache, Resolver Ownership, Share Locking, and Transfer Loop
- **Implement Phase ID**: `impl-transfer-core`
- **Verification Phases**:
  - `check-transfer-core-curated` — type `check`, bounce_target `impl-transfer-core`; purpose: validate the easy/multi lifecycle, timer handling, poll/wakeup logic, timeout behavior, and connection reuse semantics with focused upstream libtests. Commands it should run:
    ```bash
    bash safe/scripts/run-curated-libtests.sh --flavor openssl 530 582 1506 1550 1554 1557 1591 1597 1606 2402 2404
    bash safe/scripts/run-curated-libtests.sh --flavor gnutls 530 582 1506 1550 1554 1557 1591 1597 1606 2402 2404
    ```
  - `check-transfer-core-link` — type `check`, bounce_target `impl-transfer-core`; purpose: verify that original object files using easy and multi APIs can be relinked against the safe library without recompilation and that the relinked executables still run correctly. Commands it should run:
    ```bash
    bash safe/scripts/run-link-compat.sh --flavor openssl --tests lib500 lib501 lib530 lib582 lib1550 lib1606 lib2402
    bash safe/scripts/run-link-compat.sh --flavor gnutls --tests lib500 lib501 lib530 lib582 lib1550 lib1606 lib2402
    ```
- **Preexisting Inputs**:
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
  - outputs from phases 1 through 3
- **New Outputs**:
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
- **File Changes**:
  - Port `curl_easy_perform` onto a Rust-owned multi/transfer engine instead of a C fallback.
  - Port the multi-handle state machine and wakeup/timer plumbing from `original/lib/multi.c` and `original/lib/multihandle.h`.
  - Port the connection-cache, resolver ownership model, and connection-filter chain.
  - Port share-handle lock callbacks and the shared-resource plumbing required by DNS, cookies, HSTS, and SSL session reuse.
- **Implementation Details**:
  - Preserve the upstream easy-perform behavior that internally uses a private multi handle, as implemented in `original/lib/easy.c`.
  - Mirror `MSTATE_*` from `original/lib/multihandle.h` with an explicit Rust enum and state-transition functions so behavior stays inspectable and testable.
  - The connection-cache key must include all fields that affect identity and reuse safety, including host, port, proxy/tunnel state, `conn_to` overrides, TLS peer identity, authentication context, and share-handle state needed to avoid the CVE classes around incorrect reuse.
  - Recreate the connection-filter chain from `original/lib/cfilters.h` using Rust trait objects or enums, with unsafe code only at the raw socket and backend boundaries.
  - Implement `curl_multi_init`, `curl_multi_cleanup`, `curl_multi_add_handle`, `curl_multi_remove_handle`, `curl_multi_fdset`, `curl_multi_perform`, `curl_multi_wait`, `curl_multi_poll`, `curl_multi_timeout`, `curl_multi_wakeup`, `curl_multi_info_read`, `curl_multi_socket`, `curl_multi_socket_all`, `curl_multi_socket_action`, `curl_multi_assign`, `curl_multi_get_handles`, and `curl_multi_strerror`.
  - Implement the transport-facing portions of `curl_easy_pause`, `curl_easy_recv`, `curl_easy_send`, and `curl_easy_upkeep`.
  - Preserve low-speed and timeout semantics from `original/lib/speedcheck.c` and `original/tests/data/test1606`, not just callback wiring.
  - Ensure share-handle locking callbacks and shared-data selections from `curl_share_setopt` remain ABI-compatible even if some shared-resource implementations are completed in later phases.
- **Verification**:
  ```bash
  bash safe/scripts/run-curated-libtests.sh --flavor openssl 530 582 1506 1550 1554 1557 1591 1597 1606 2402 2404
  bash safe/scripts/run-curated-libtests.sh --flavor gnutls 530 582 1506 1550 1554 1557 1591 1597 1606 2402 2404
  bash safe/scripts/run-link-compat.sh --flavor openssl --tests lib500 lib501 lib530 lib582 lib1550 lib1606 lib2402
  bash safe/scripts/run-link-compat.sh --flavor gnutls --tests lib500 lib501 lib530 lib582 lib1550 lib1606 lib2402
  ```

### 5. HTTP, Redirect, Cookies/HSTS, Headers API, Authentication, WebSockets, and CVE Regressions

- **Phase Name**: HTTP, Redirect, Cookies/HSTS, Headers API, Authentication, WebSockets, and CVE Regressions
- **Implement Phase ID**: `impl-http-security`
- **Verification Phases**:
  - `check-http-security-curated` — type `check`, bounce_target `impl-http-security`; purpose: validate HTTP request/response handling, redirect policy, headers API, cookies, HSTS, auth, and related easy-handle behavior with focused upstream tests. Commands it should run:
    ```bash
    bash safe/scripts/run-curated-libtests.sh --flavor openssl 659 1526 1900 1905 1915 1970 1971 1972 1973 1974 1975 2304 2305
    bash safe/scripts/run-curated-libtests.sh --flavor gnutls 659 1526 1900 1905 1915 1970 1971 1972 1973 1974 1975 2304 2305
    ```
  - `check-http-security-websockets` — type `check`, bounce_target `impl-http-security`; purpose: verify the tracked WebSocket client programs against the Rust implementation. Commands it should run:
    ```bash
    bash safe/scripts/run-http-client-tests.sh --flavor openssl --clients ws-data ws-pingpong
    bash safe/scripts/run-http-client-tests.sh --flavor gnutls --clients ws-data ws-pingpong
    ```
  - `check-http-security-cve-map` — type `check`, bounce_target `impl-http-security`; purpose: verify that every curated CVE in the manifest is mapped to an implemented regression case before the regression suite is accepted. Commands it should run:
    ```bash
    python3 safe/scripts/verify-cve-coverage.py --manifest safe/metadata/cve-manifest.json --mapping safe/metadata/cve-to-test.json --cases-dir safe/tests/cve_cases
    ```
  - `check-http-security-cves` — type `check`, bounce_target `impl-http-security`; purpose: verify that the CVE regression suite covers all curated security cases and passes in both flavors. Commands it should run:
    ```bash
    python3 safe/scripts/verify-cve-coverage.py --manifest safe/metadata/cve-manifest.json --mapping safe/metadata/cve-to-test.json --cases-dir safe/tests/cve_cases
    cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test cve_regressions
    cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test cve_regressions
    ```
- **Preexisting Inputs**:
  - `original/lib/http.c`
  - `original/lib/http1.c`
  - `original/lib/http_proxy.c`
  - `original/lib/headers.c`
  - `original/lib/cookie.c`
  - `original/lib/hsts.c`
  - `original/lib/altsvc.c`
  - `original/lib/http_digest.c`
  - `original/lib/http_ntlm.c`
  - `original/lib/http_negotiate.c`
  - `original/lib/content_encoding.c`
  - `original/lib/ws.c`
  - `original/lib/rand.c`
  - `relevant_cves.json`
  - `original/debian/patches/CVE-*.patch`
  - `safe/metadata/cve-manifest.json`
  - `safe/scripts/run-curated-libtests.sh`
  - `safe/scripts/run-http-client-tests.sh`
  - outputs from phases 1 through 4
- **New Outputs**:
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
- **File Changes**:
  - Port HTTP and proxy request construction, response parsing, redirect following, header lookup, cookies, HSTS, alt-svc, and WebSocket framing into Rust.
  - Add an explicit CVE-to-regression mapping generated from the curated JSON and the Debian patch files.
  - Replace the relevant temporary C fallbacks for HTTP and WebSocket behavior.
- **Implementation Details**:
  - Redirect and credential-forwarding rules must become explicit typed policy rather than scattered flag checks, so the port closes the credential-leakage classes represented in `relevant_cves.json`.
  - Connection reuse must become authentication-aware and proxy-aware, so the port closes the reuse classes represented by CVEs such as `CVE-2026-3784` and `CVE-2026-1965`.
  - Port cookie and HSTS state into Rust data structures that preserve upstream behavior while making origin scoping, persistence, PSL checks, and serialization rules explicit and testable.
  - Preserve `curl_easy_header` and `curl_easy_nextheader` semantics for `struct curl_header`, including pointer lifetime, origin filtering, request/response selection, and anchor handling.
  - Port `curl_ws_recv`, `curl_ws_send`, and `curl_ws_meta` while replacing weak randomness or predictable mask generation with strong OS-backed entropy and explicit failure handling.
  - `safe/metadata/cve-to-test.json` should map every curated CVE from `safe/metadata/cve-manifest.json` either to a dedicated regression case or to a specific shared regression case with written justification. No curated CVE should remain unmapped by the end of this phase.
  - `safe/scripts/verify-cve-coverage.py` must fail if any curated CVE is missing from `safe/metadata/cve-to-test.json`, if a mapping points to a nonexistent file under `safe/tests/cve_cases/`, or if a shared-case mapping omits its written justification.
  - `safe/tests/cve_regressions.rs` must consume `safe/metadata/cve-to-test.json` directly or from a generated compile-time artifact and fail if the mapping artifact and the implemented regression cases drift out of sync.
- **Verification**:
  ```bash
  bash safe/scripts/run-curated-libtests.sh --flavor openssl 659 1526 1900 1905 1915 1970 1971 1972 1973 1974 1975 2304 2305
  bash safe/scripts/run-curated-libtests.sh --flavor gnutls 659 1526 1900 1905 1915 1970 1971 1972 1973 1974 1975 2304 2305
  bash safe/scripts/run-http-client-tests.sh --flavor openssl --clients ws-data ws-pingpong
  bash safe/scripts/run-http-client-tests.sh --flavor gnutls --clients ws-data ws-pingpong
  python3 safe/scripts/verify-cve-coverage.py --manifest safe/metadata/cve-manifest.json --mapping safe/metadata/cve-to-test.json --cases-dir safe/tests/cve_cases
  cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test cve_regressions
  cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test cve_regressions
  ```

### 6. TLS Backends, HTTP/2, Remaining Protocol Engines, and Tracked HTTP Client Coverage

- **Phase Name**: TLS Backends, HTTP/2, Remaining Protocol Engines, and Tracked HTTP Client Coverage
- **Implement Phase ID**: `impl-backends-protocols`
- **Verification Phases**:
  - `check-backends-protocols-openssl` — type `check`, bounce_target `impl-backends-protocols`; purpose: validate the OpenSSL flavor across the remaining protocol and backend surface, including the tracked HTTP client programs. Commands it should run:
    ```bash
    bash safe/scripts/run-upstream-tests.sh --flavor openssl --tests 1 105 506 586 1500 1900 1915 2304 3000 3025 3100
    bash safe/scripts/run-http-client-tests.sh --flavor openssl --clients h2-download h2-serverpush h2-pausing h2-upgrade-extreme tls-session-reuse ws-data ws-pingpong
    ```
  - `check-backends-protocols-gnutls` — type `check`, bounce_target `impl-backends-protocols`; purpose: validate the GnuTLS flavor across the same protocol and backend surface. Commands it should run:
    ```bash
    bash safe/scripts/run-upstream-tests.sh --flavor gnutls --tests 1 105 506 586 1500 1900 1915 2304 3000 3025 3100
    bash safe/scripts/run-http-client-tests.sh --flavor gnutls --clients h2-download h2-serverpush h2-pausing h2-upgrade-extreme tls-session-reuse ws-data ws-pingpong
    ```
- **Preexisting Inputs**:
  - `original/lib/vtls/*.c`
  - `original/lib/vssh/*.c`
  - `original/lib/vquic/*.c`
  - `original/lib/http2.c`
  - `original/lib/file.c`
  - `original/lib/ftp.c`
  - `original/lib/imap.c`
  - `original/lib/pop3.c`
  - `original/lib/smtp.c`
  - `original/lib/ldap.c`
  - `original/lib/openldap.c`
  - `original/lib/smb.c`
  - `original/lib/telnet.c`
  - `original/lib/tftp.c`
  - `original/lib/dict.c`
  - `original/lib/gopher.c`
  - `original/lib/rtsp.c`
  - `original/lib/mqtt.c`
  - `original/lib/doh.c`
  - `original/lib/idn.c`
  - the tracked files under `original/tests/http/`
  - `safe/scripts/run-upstream-tests.sh`
  - `safe/scripts/run-http-client-tests.sh`
  - outputs from phases 1 through 5
- **New Outputs**:
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
- **File Changes**:
  - Port the flavor-specific TLS logic into small backend adapters with a shared Rust policy layer.
  - Port the remaining non-HTTP protocol engines and backend integrations.
  - Add tracked HTTP-client support for server push headers, multiplexing, pause/resume, TLS reuse, and WebSockets.
- **Implementation Details**:
  - Keep the backend boundary small. Policy, state, and reuse rules stay in Rust; backend modules perform only backend-specific cryptographic and certificate operations.
  - Preserve `curl_global_sslset`, pinned public key behavior, ALPN, session-cache semantics, backend-specific error reporting, and certificate-info extraction.
  - Cover the certificate-validation and pinning issues highlighted in `relevant_cves.json`, including OpenSSL and GnuTLS backend differences.
  - Implement `curl_pushheader_byname` and `curl_pushheader_bynum` as part of the HTTP/2 server-push surface exercised by `h2-serverpush.c`.
  - The source tree contains QUIC/HTTP/3 code, but Ubuntu 24.04 package builds do not currently enable the corresponding extra dependencies in `original/debian/control`. The safe port should preserve the Ubuntu package feature matrix first; HTTP/3 paths should only be exposed in a given flavor when that flavor is built with matching backend support.
  - The tracked `tests/http/clients` programs are canonical existing inputs. The runner should provision only the dependencies needed by those tracked clients and should never fabricate the absent pytest tree.
  - The phase-6 curated `runtests.pl` subset must contain only ids that upstream will execute without `-f`. Do not schedule the former-unit ids `1300`, `1309`, `1323`, `1602`, `1603`, `1604`, `1661`, or `2601` here; phase 7 discharges that coverage through `safe/tests/unit_port.rs`.
  - Ensure all protocol handlers plug into the shared easy/multi/connection engine from earlier phases rather than bypassing it with protocol-local lifetimes.
- **Verification**:
  ```bash
  bash safe/scripts/run-upstream-tests.sh --flavor openssl --tests 1 105 506 586 1500 1900 1915 2304 3000 3025 3100
  bash safe/scripts/run-http-client-tests.sh --flavor openssl --clients h2-download h2-serverpush h2-pausing h2-upgrade-extreme tls-session-reuse ws-data ws-pingpong
  bash safe/scripts/run-upstream-tests.sh --flavor gnutls --tests 1 105 506 586 1500 1900 1915 2304 3000 3025 3100
  bash safe/scripts/run-http-client-tests.sh --flavor gnutls --clients h2-download h2-serverpush h2-pausing h2-upgrade-extreme tls-session-reuse ws-data ws-pingpong
  ```

### 7. Rust Unit Port and Broad Link/Object Compatibility

- **Phase Name**: Rust Unit Port and Broad Link/Object Compatibility
- **Implement Phase ID**: `impl-unit-port`
- **Verification Phases**:
  - `check-unit-port` — type `check`, bounce_target `impl-unit-port`; purpose: run the Rust port of every original unit source id for both flavors. Commands it should run:
    ```bash
    cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test unit_port
    cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test unit_port
    ```
  - `check-link-compat-curated` — type `check`, bounce_target `impl-unit-port`; purpose: validate the generalized link-compat harness across a broad tracked object-file set built from original consumer sources, including execution of the relinked binaries. Commands it should run:
    ```bash
    bash safe/scripts/run-link-compat.sh --flavor openssl --all-curated
    bash safe/scripts/run-link-compat.sh --flavor gnutls --all-curated
    ```
- **Preexisting Inputs**:
  - `safe/metadata/test-manifest.json`
  - `safe/scripts/build-compat-consumers.sh`
  - `safe/scripts/run-link-compat.sh`
  - the tracked files under `original/tests/unit/`
  - `original/tests/unit/Makefile.inc`
  - `original/tests/libtest/first.c`
  - `original/tests/libtest/test.h`
  - outputs from phases 1 through 6
- **New Outputs**:
  - `safe/tests/unit_port.rs`
  - `safe/tests/unit_port_cases/`
  - `safe/tests/port-map.json`
  - `safe/compat/link-manifest.json`
- **File Changes**:
  - Port the 46 internal unit source ids to Rust integration tests while preserving numeric ids and explicit source-to-port mappings.
  - Extend the relink harness from targeted link-and-run tests to a broad curated object matrix derived from tracked source files.
- **Implementation Details**:
  - `safe/tests/port-map.json` should map each original `unitNNNN.c` source file to its Rust integration-test location and note whether the unit was part of upstream `UNITPROGS` or only present as a source input.
  - `safe/tests/unit_port.rs` must execute the logical content of all 46 original unit ids, not just the 3 upstream-enabled `UNITPROGS`.
  - `safe/compat/link-manifest.json` should define at least:
    - a curated broad set for phase-7 verification
    - a final `all-objects` set used in phase 10
  - Each manifest set should refer to stable target ids already defined in `safe/metadata/test-manifest.json`; compile flags, generated-source rules, and translation-unit membership must come from that earlier manifest and the per-flavor compatibility-build state rather than being duplicated or rediscovered here.
  - The link manifest should be derived from the tracked-target metadata in `safe/metadata/test-manifest.json`, not from ad hoc scans of build directories or prebuilt `.o` files.
  - Each manifest entry must declare the relink target id, the target/object ids from `safe/metadata/test-manifest.json`, flavor applicability, and a runtime adapter such as `libtest`, `curl-tool-smoke`, `http-client`, or `ldap-devpkg`, plus any required test ids or client names. `safe/scripts/run-link-compat.sh` must first ensure that `safe/scripts/build-compat-consumers.sh` has emitted the matching per-flavor build-state, then resolve the actual `.o` paths from that state, execute the adapter after relinking, and fail if any selected entry lacks build metadata or runtime metadata.
  - The final `all-objects` set must contain only runnable entries. Pure link-only diagnostics are allowed in non-final exploratory sets, but they must not satisfy the final link-compatibility proof.
- **Verification**:
  ```bash
  cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test unit_port
  cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test unit_port
  bash safe/scripts/run-link-compat.sh --flavor openssl --all-curated
  bash safe/scripts/run-link-compat.sh --flavor gnutls --all-curated
  ```

### 8. Performance Baseline, Benchmark Harness, and Regression Tuning

- **Phase Name**: Performance Baseline, Benchmark Harness, and Regression Tuning
- **Implement Phase ID**: `impl-performance`
- **Verification Phases**:
  - `check-performance-budgets` — type `check`, bounce_target `impl-performance`; purpose: benchmark the original and safe implementations under the same local workloads for both flavors and fail if the safe port exceeds the recorded regression budgets. Commands it should run:
    ```bash
    rm -rf safe/.bench-output
    mkdir -p safe/.bench-output
    bash safe/scripts/benchmark-local.sh --implementation original --flavor openssl --matrix core --output-dir safe/.bench-output/original/openssl
    bash safe/scripts/benchmark-local.sh --implementation safe --flavor openssl --matrix core --output-dir safe/.bench-output/safe/openssl
    python3 safe/scripts/compare-benchmarks.py --baseline safe/.bench-output/original/openssl --candidate safe/.bench-output/safe/openssl --thresholds safe/benchmarks/thresholds.json
    bash safe/scripts/benchmark-local.sh --implementation original --flavor gnutls --matrix core --output-dir safe/.bench-output/original/gnutls
    bash safe/scripts/benchmark-local.sh --implementation safe --flavor gnutls --matrix core --output-dir safe/.bench-output/safe/gnutls
    python3 safe/scripts/compare-benchmarks.py --baseline safe/.bench-output/original/gnutls --candidate safe/.bench-output/safe/gnutls --thresholds safe/benchmarks/thresholds.json
    ```
- **Preexisting Inputs**:
  - `original/lib/speedcheck.c`
  - `original/tests/data/test1606`
  - `original/src/tool_operate.c`
  - `original/tests/http/clients/h2-download.c`
  - `original/tests/http/clients/h2-pausing.c`
  - `original/tests/http/clients/tls-session-reuse.c`
  - `safe/scripts/build-compat-consumers.sh`
  - `safe/scripts/run-http-client-tests.sh`
  - `safe/scripts/http-fixtures.sh`
  - outputs from phases 1 through 7
- **New Outputs**:
  - `safe/benchmarks/README.md`
  - `safe/benchmarks/scenarios.json`
  - `safe/benchmarks/thresholds.json`
  - `safe/benchmarks/harness/easy_loop.c`
  - `safe/benchmarks/harness/multi_parallel.c`
  - `safe/scripts/benchmark-local.sh`
  - `safe/scripts/compare-benchmarks.py`
  - `safe/docs/performance.md`
- **File Changes**:
  - Add a deterministic loopback benchmark harness that can run against either the original or safe implementation without changing the workload definition.
  - Add explicit scenario and threshold files so the performance requirement is measurable and version-controlled.
  - Tune the Rust implementation where the benchmark matrix shows material regressions.
- **Implementation Details**:
  - `safe/scripts/benchmark-local.sh` should use the shared local-fixture helpers from phase 3 so the original and safe implementations run against the same loopback HTTP/HTTPS setup.
  - `safe/benchmarks/scenarios.json` should define at least these scenarios:
    - `easy-http1-reuse`
    - `multi-http1-parallel`
    - `h2-download-multiplex`
    - `tls-session-reuse`
  - The benchmark harness should write structured JSON output per scenario, including at minimum median wall-clock time, run count, bytes transferred, and implementation/flavor metadata.
  - `safe/benchmarks/thresholds.json` should record explicit maximum median regressions for each scenario. Reasonable initial budgets are:
    - `easy-http1-reuse`: 15%
    - `multi-http1-parallel`: 15%
    - `h2-download-multiplex`: 20%
    - `tls-session-reuse`: 15%
  - `safe/scripts/compare-benchmarks.py` should fail if any required scenario is missing or exceeds its budget.
  - `safe/docs/performance.md` should document methodology, local-fixture assumptions, scenario definitions, and the rule that performance tuning must not weaken compatibility or security.
- **Verification**:
  ```bash
  rm -rf safe/.bench-output
  mkdir -p safe/.bench-output
  bash safe/scripts/benchmark-local.sh --implementation original --flavor openssl --matrix core --output-dir safe/.bench-output/original/openssl
  bash safe/scripts/benchmark-local.sh --implementation safe --flavor openssl --matrix core --output-dir safe/.bench-output/safe/openssl
  python3 safe/scripts/compare-benchmarks.py --baseline safe/.bench-output/original/openssl --candidate safe/.bench-output/safe/openssl --thresholds safe/benchmarks/thresholds.json
  bash safe/scripts/benchmark-local.sh --implementation original --flavor gnutls --matrix core --output-dir safe/.bench-output/original/gnutls
  bash safe/scripts/benchmark-local.sh --implementation safe --flavor gnutls --matrix core --output-dir safe/.bench-output/safe/gnutls
  python3 safe/scripts/compare-benchmarks.py --baseline safe/.bench-output/original/gnutls --candidate safe/.bench-output/safe/gnutls --thresholds safe/benchmarks/thresholds.json
  ```

### 9. Debian Packaging, Ubuntu Install Layout, Autopkgtests, and Root Dependent Harness

- **Phase Name**: Debian Packaging, Ubuntu Install Layout, Autopkgtests, and Root Dependent Harness
- **Implement Phase ID**: `impl-packaging`
- **Verification Phases**:
  - `check-packaging-self-contained-source` — type `check`, bounce_target `impl-packaging`; purpose: prove that a detached export of `safe/` alone contains every build/test asset needed by the Debian package and autopkgtests, and fail if package-time files still reference `original/` or another out-of-tree compatibility-source path. Commands it should run:
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
  - `check-packaging-control-contract` — type `check`, bounce_target `impl-packaging`; purpose: verify that the detached safe-source export preserves the current Ubuntu binary package contract in both `debian/control` and the built `.deb` metadata. Commands it should run:
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
  - `check-packaging-install-layout` — type `check`, bounce_target `impl-packaging`; purpose: verify that the detached safe-source export builds the correct Ubuntu binary packages, preserves the required Ubuntu install paths and symlink layout for the runtime, dev, tool, and doc packages, and still supports the existing dev-package compile test. Commands it should run:
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
  - `check-packaging-devpkg-tooling` — type `check`, bounce_target `impl-packaging`; purpose: verify the packaged developer-tooling contract for both dev packages from the detached safe-source export, including executable `curl-config`, usable `libcurl.pc`, and installable `libcurl.m4`. Commands it should run:
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
  - `check-packaging-autopkgtests` — type `check`, bounce_target `impl-packaging`; purpose: verify that the detached safe-source export preserves the Debian autopkgtest contract and that the actual packaged autopkgtest entrypoints execute successfully. Commands it should run:
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
  - `check-packaging-dependents` — type `check`, bounce_target `impl-packaging`; purpose: verify that the root Docker-based dependent harness can build and install the safe packages and that every dependent still compiles and runs. Commands it should run:
    ```bash
    bash ./test-original.sh --implementation safe
    ```
- **Preexisting Inputs**:
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
  - outputs from phases 1 through 8
- **New Outputs**:
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
- **File Changes**:
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
- **Implementation Details**:
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
- **Verification**:
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
  python3 scripts/verify-package-control-contract.py --expected-control "$repo_root/original/debian/control" --actual-control debian/control --package-root . --require-source-build-deps cargo:native rustc:native
  bash scripts/verify-package-install-layout.sh --package-root .
  bash scripts/run-curl-tool-smoke.sh --implementation packaged --flavor openssl --package-root .
  bash scripts/run-ldap-devpkg-test.sh --flavor openssl --package-root .
  bash scripts/run-ldap-devpkg-test.sh --flavor gnutls --package-root .
  bash scripts/verify-devpkg-tooling-contract.sh --package-root .
  bash scripts/verify-autopkgtest-contract.sh --expected-control "$repo_root/original/debian/tests/control" --actual-control debian/tests/control
  bash scripts/run-packaged-autopkgtests.sh --package-root . --test upstream-tests-openssl
  bash scripts/run-packaged-autopkgtests.sh --package-root . --test upstream-tests-gnutls
  bash scripts/run-packaged-autopkgtests.sh --package-root . --test curl-ldapi-test
  '
  bash ./test-original.sh --implementation safe
  ```

### 10. Remove Temporary C Fallbacks, Audit Unsafe Boundaries, and Run the Full No-Exclusions Matrix

- **Phase Name**: Remove Temporary C Fallbacks, Audit Unsafe Boundaries, and Run the Full No-Exclusions Matrix
- **Implement Phase ID**: `impl-final-hardening`
- **Verification Phases**:
  - `check-final-full-matrix` — type `check`, bounce_target `impl-final-hardening`; purpose: run the full ABI, package, link-and-run, security, benchmark, upstream, HTTP-client, unit-port, and downstream compatibility matrix for both flavors after all temporary fallback bridges are removed. Commands it should run:
    ```bash
    bash -lc '
    set -euo pipefail
    rm -rf /tmp/libcurl-safe-final-check
    bash safe/scripts/export-tracked-tree.sh --safe-only --dest /tmp/libcurl-safe-final-check
    test -f /tmp/libcurl-safe-final-check/Cargo.lock
    test -f /tmp/libcurl-safe-final-check/.cargo/config.toml
    test -d /tmp/libcurl-safe-final-check/vendor/cargo
    rg -n "replace-with *= *\"vendored-sources\"|directory *= *\"vendor/cargo\"" /tmp/libcurl-safe-final-check/.cargo/config.toml >/dev/null
    rg -n "cargo:native" /tmp/libcurl-safe-final-check/debian/control >/dev/null
    rg -n "rustc:native" /tmp/libcurl-safe-final-check/debian/control >/dev/null
    rg -n "CARGO_NET_OFFLINE=true|--offline" /tmp/libcurl-safe-final-check/debian/rules >/dev/null
    rg -n -- "--locked" /tmp/libcurl-safe-final-check/debian/rules >/dev/null
    test "$(cat /tmp/libcurl-safe-final-check/debian/source/format)" = "3.0 (quilt)"
    test -f /tmp/libcurl-safe-final-check/debian/patches/series
    while IFS= read -r patch; do
      case "$patch" in
        ''|'#'*) continue ;;
      esac
      test -f "/tmp/libcurl-safe-final-check/debian/patches/$patch"
    done </tmp/libcurl-safe-final-check/debian/patches/series
    cd /tmp/libcurl-safe-final-check
    mk-build-deps -ir -t 'apt-get -y --no-install-recommends' debian/control
    dpkg-buildpackage -us -uc -b
    '
    python3 safe/scripts/verify-manifests.py --abi safe/metadata/abi-manifest.json --tests safe/metadata/test-manifest.json --cves safe/metadata/cve-manifest.json
    bash safe/scripts/verify-public-headers.sh --expected original/include/curl --actual safe/include/curl
    bash safe/scripts/verify-export-names.sh --expected original/libcurl.def --flavor openssl
    bash safe/scripts/verify-export-names.sh --expected original/libcurl.def --flavor gnutls
    bash safe/scripts/verify-symbol-versions.sh --expected original/debian/libcurl4t64.symbols --flavor openssl
    bash safe/scripts/verify-symbol-versions.sh --expected original/debian/libcurl3t64-gnutls.symbols --flavor gnutls
    bash safe/scripts/build-compat-consumers.sh --flavor openssl --all
    bash safe/scripts/build-compat-consumers.sh --flavor gnutls --all
    cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test public_abi
    cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test public_abi
    cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test abi_layout
    cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test abi_layout
    bash safe/scripts/run-public-abi-smoke.sh --flavor openssl
    bash safe/scripts/run-public-abi-smoke.sh --flavor gnutls
    bash safe/scripts/run-link-compat.sh --flavor openssl --all-objects
    bash safe/scripts/run-link-compat.sh --flavor gnutls --all-objects
    bash safe/scripts/run-upstream-tests.sh --flavor openssl --require-all-runtests --no-exclusion-keywords
    bash safe/scripts/run-upstream-tests.sh --flavor gnutls --require-all-runtests --no-exclusion-keywords
    bash safe/scripts/run-curl-tool-smoke.sh --implementation compat --flavor openssl
    bash safe/scripts/run-curl-tool-smoke.sh --implementation compat --flavor gnutls
    bash /tmp/libcurl-safe-final-check/scripts/run-curl-tool-smoke.sh --implementation packaged --flavor openssl --package-root /tmp/libcurl-safe-final-check
    bash safe/scripts/run-http-client-tests.sh --flavor openssl --all
    bash safe/scripts/run-http-client-tests.sh --flavor gnutls --all
    cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test unit_port
    cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test unit_port
    python3 safe/scripts/verify-cve-coverage.py --manifest safe/metadata/cve-manifest.json --mapping safe/metadata/cve-to-test.json --cases-dir safe/tests/cve_cases
    cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test cve_regressions
    cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test cve_regressions
    rm -rf safe/.bench-output/final
    mkdir -p safe/.bench-output/final
    bash safe/scripts/benchmark-local.sh --implementation original --flavor openssl --matrix core --output-dir safe/.bench-output/final/original/openssl
    bash safe/scripts/benchmark-local.sh --implementation safe --flavor openssl --matrix core --output-dir safe/.bench-output/final/safe/openssl
    python3 safe/scripts/compare-benchmarks.py --baseline safe/.bench-output/final/original/openssl --candidate safe/.bench-output/final/safe/openssl --thresholds safe/benchmarks/thresholds.json
    bash safe/scripts/benchmark-local.sh --implementation original --flavor gnutls --matrix core --output-dir safe/.bench-output/final/original/gnutls
    bash safe/scripts/benchmark-local.sh --implementation safe --flavor gnutls --matrix core --output-dir safe/.bench-output/final/safe/gnutls
    python3 safe/scripts/compare-benchmarks.py --baseline safe/.bench-output/final/original/gnutls --candidate safe/.bench-output/final/safe/gnutls --thresholds safe/benchmarks/thresholds.json
    bash /tmp/libcurl-safe-final-check/scripts/run-ldap-devpkg-test.sh --flavor openssl --package-root /tmp/libcurl-safe-final-check
    bash /tmp/libcurl-safe-final-check/scripts/run-ldap-devpkg-test.sh --flavor gnutls --package-root /tmp/libcurl-safe-final-check
    bash /tmp/libcurl-safe-final-check/scripts/verify-devpkg-tooling-contract.sh --package-root /tmp/libcurl-safe-final-check
    python3 /tmp/libcurl-safe-final-check/scripts/verify-package-control-contract.py --expected-control original/debian/control --actual-control /tmp/libcurl-safe-final-check/debian/control --package-root /tmp/libcurl-safe-final-check --require-source-build-deps cargo:native rustc:native
    bash /tmp/libcurl-safe-final-check/scripts/verify-package-install-layout.sh --package-root /tmp/libcurl-safe-final-check
    bash /tmp/libcurl-safe-final-check/scripts/verify-autopkgtest-contract.sh --expected-control original/debian/tests/control --actual-control /tmp/libcurl-safe-final-check/debian/tests/control
    bash /tmp/libcurl-safe-final-check/scripts/run-packaged-autopkgtests.sh --package-root /tmp/libcurl-safe-final-check --test upstream-tests-openssl
    bash /tmp/libcurl-safe-final-check/scripts/run-packaged-autopkgtests.sh --package-root /tmp/libcurl-safe-final-check --test upstream-tests-gnutls
    bash /tmp/libcurl-safe-final-check/scripts/run-packaged-autopkgtests.sh --package-root /tmp/libcurl-safe-final-check --test curl-ldapi-test
    bash ./test-original.sh --implementation safe
    bash /tmp/libcurl-safe-final-check/scripts/audit-final-build-independence.sh --package-root /tmp/libcurl-safe-final-check
    cargo clippy --manifest-path safe/Cargo.toml --all-targets --no-default-features --features openssl-flavor -- -D warnings
    cargo clippy --manifest-path safe/Cargo.toml --all-targets --no-default-features --features gnutls-flavor -- -D warnings
    ```
- **Preexisting Inputs**:
  - every output from phases 1 through 9
  - `safe/c_shim/forwarders.c`
  - `safe/tests/public_abi.rs`
  - `safe/tests/abi_layout.rs`
  - `safe/tests/unit_port.rs`
  - `safe/tests/cve_regressions.rs`
  - `safe/scripts/run-public-abi-smoke.sh`
  - `safe/scripts/run-curl-tool-smoke.sh`
  - `safe/scripts/verify-autopkgtest-contract.sh`
  - `safe/scripts/verify-package-install-layout.sh`
  - `safe/scripts/verify-devpkg-tooling-contract.sh`
  - `safe/scripts/run-packaged-autopkgtests.sh`
  - `safe/benchmarks/scenarios.json`
  - `safe/benchmarks/thresholds.json`
  - `safe/debian/*`
  - modified `test-original.sh`
- **New Outputs**:
  - final Rust-owned libcurl core with no direct dependency on the original C library
  - `safe/docs/unsafe-audit.md`
  - finalized `safe/docs/performance.md`
  - finalized `safe/metadata/abi-manifest.json`
  - finalized `safe/metadata/test-manifest.json`
  - finalized `safe/metadata/cve-manifest.json`
  - `safe/scripts/audit-final-build-independence.sh`
- **File Changes**:
  - Delete the temporary all-symbol C fallback bridge.
  - Tighten or eliminate avoidable `unsafe` blocks.
  - Add an explicit unsafe-boundary audit document.
  - Delete the transitional reference-build dependency from the final library and package build.
  - Fix the remaining compatibility, package, and performance issues found by the full matrix.
- **Implementation Details**:
  - By the end of this phase, the only C code that should remain is the unavoidable ABI layer for varargs, the `curl_mprintf*` family, and boundary glue required by libc, TLS/SSH backends, or OS callbacks.
  - `safe/docs/unsafe-audit.md` should document every remaining `unsafe` block and classify it as one of: C ABI boundary, libc/socket call, TLS/SSH backend FFI, or raw-pointer adaptation required by a callback signature.
  - `run-upstream-tests.sh --require-all-runtests --no-exclusion-keywords` must execute every ordered `TESTCASES` token not disabled by the tracked vendored `tests/data/DISABLED` rules for the selected flavor, preserve the duplicate `test1190` invocation, refuse to pass `-f`, report the disabled-token set explicitly, and fail if it uses `nonflaky-test`, `TEST_NF`, `~flaky`, `~timing-dependent`, or any comparable extra exclusion mechanism. The unconditional former-unit ids `1300`, `1309`, `1323`, `1602`, `1603`, `1604`, `1661`, and `2601` are not forced through `runtests.pl`; they are discharged by the required Rust unit-port suite instead.
  - `build-compat-consumers.sh --all` in the final matrix must prove that every manifest-recorded compatibility target builds for the selected flavor. Targets whose manifest marks them as libcurl consumers must compile and link against the safe library; auxiliary helper targets such as `chkhostname` and the 10 server helpers must preserve their original upstream non-libcurl link lines and still build successfully.
  - `run-link-compat.sh --all-objects` in the final matrix must relink the complete runnable object manifest without recompilation and execute every relinked consumer under its declared runtime adapter; a link-only success is insufficient.
  - `run-curl-tool-smoke.sh` must remain part of the final matrix so the compatibility-built tool is exercised for both flavors and the packaged OpenSSL `curl` binary is exercised after `dpkg-buildpackage`.
  - `run-http-client-tests.sh --all` must execute all 7 tracked `tests/http/clients` programs for each flavor.
  - `cargo test --test unit_port` must execute the full Rust port of all 46 original unit source ids, not only the 3 upstream `UNITPROGS`.
  - `python3 safe/scripts/verify-cve-coverage.py --manifest safe/metadata/cve-manifest.json --mapping safe/metadata/cve-to-test.json --cases-dir safe/tests/cve_cases` must remain part of the final matrix so the security-manifest-to-regression mapping contract cannot silently regress after phase 5.
  - `safe/scripts/compare-benchmarks.py` must still pass in the final matrix; performance verification cannot disappear after the dedicated performance phase.
  - Package-related commands in the final matrix must run against a detached `safe/`-only export such as `/tmp/libcurl-safe-final-check`, not the live repo tree, so the final package proof remains self-contained.
  - The package-build subblock in the final matrix must assume the same prepared Ubuntu 24.04 executor contract as phase 9, plus `rust-clippy` for the required `cargo clippy` commands, and it must run `mk-build-deps -ir -t 'apt-get -y --no-install-recommends' debian/control` in the detached export before `dpkg-buildpackage`.
  - `python3 /tmp/libcurl-safe-final-check/scripts/verify-package-control-contract.py --expected-control original/debian/control --actual-control /tmp/libcurl-safe-final-check/debian/control --package-root /tmp/libcurl-safe-final-check --require-source-build-deps cargo:native rustc:native` must remain part of the final matrix so the package stanza contract is verified both before install-layout checks and after substvars expansion into the built `.deb` metadata, while also proving that the detached safe source package declares its Rust build-tool requirements explicitly.
  - `bash /tmp/libcurl-safe-final-check/scripts/verify-package-install-layout.sh --package-root /tmp/libcurl-safe-final-check` must remain part of the final matrix so the actual `.deb` payloads are checked for the required runtime-library symlinks, public headers, packaged `curl` binary/manpage, development metadata files, and `libcurl4-doc` docs/examples/manpages instead of being treated as valid merely because the package build completed.
  - `bash /tmp/libcurl-safe-final-check/scripts/verify-devpkg-tooling-contract.sh --package-root /tmp/libcurl-safe-final-check` must remain part of the final matrix so packaged `curl-config`, packaged `libcurl.pc`, and installed `usr/share/aclocal/libcurl.m4` are revalidated after the last cleanup and package rebuild.
  - `safe/scripts/audit-final-build-independence.sh` must fail if `safe/c_shim/forwarders.c` still exists, if `safe/Cargo.toml`, `safe/build.rs`, `safe/debian/rules`, or any other file consumed directly by the final library or package build still refers to `forwarders.c`, `safe/scripts/build-reference-curl.sh`, `safe/.reference/`, or `libcurl-reference-*`, or if `readelf -d` on either final flavor library or the packaged `/usr/bin/curl` reports `DT_NEEDED`, `RPATH`, or `RUNPATH` entries that point at the transitional reference build.
  - The final audit must inspect both flavor-specific Rust library outputs and the packaged OpenSSL `curl` binary; a surviving transitional dependency in any one of them is a phase failure even if the functional tests still pass.
  - Freeze the manifests only after the full matrix passes; those manifests become the maintenance contract for later changes.
- **Verification**:
  ```bash
  bash -lc '
  set -euo pipefail
  rm -rf /tmp/libcurl-safe-final-check
  bash safe/scripts/export-tracked-tree.sh --safe-only --dest /tmp/libcurl-safe-final-check
  test -f /tmp/libcurl-safe-final-check/Cargo.lock
  test -f /tmp/libcurl-safe-final-check/.cargo/config.toml
  test -d /tmp/libcurl-safe-final-check/vendor/cargo
  rg -n "replace-with *= *\"vendored-sources\"|directory *= *\"vendor/cargo\"" /tmp/libcurl-safe-final-check/.cargo/config.toml >/dev/null
  rg -n "cargo:native" /tmp/libcurl-safe-final-check/debian/control >/dev/null
  rg -n "rustc:native" /tmp/libcurl-safe-final-check/debian/control >/dev/null
  rg -n "CARGO_NET_OFFLINE=true|--offline" /tmp/libcurl-safe-final-check/debian/rules >/dev/null
  rg -n -- "--locked" /tmp/libcurl-safe-final-check/debian/rules >/dev/null
  test "$(cat /tmp/libcurl-safe-final-check/debian/source/format)" = "3.0 (quilt)"
  test -f /tmp/libcurl-safe-final-check/debian/patches/series
  while IFS= read -r patch; do
    case "$patch" in
      ''|'#'*) continue ;;
    esac
    test -f "/tmp/libcurl-safe-final-check/debian/patches/$patch"
  done </tmp/libcurl-safe-final-check/debian/patches/series
  cd /tmp/libcurl-safe-final-check
  mk-build-deps -ir -t 'apt-get -y --no-install-recommends' debian/control
  dpkg-buildpackage -us -uc -b
  '
  python3 safe/scripts/verify-manifests.py --abi safe/metadata/abi-manifest.json --tests safe/metadata/test-manifest.json --cves safe/metadata/cve-manifest.json
  bash safe/scripts/verify-public-headers.sh --expected original/include/curl --actual safe/include/curl
  bash safe/scripts/verify-export-names.sh --expected original/libcurl.def --flavor openssl
  bash safe/scripts/verify-export-names.sh --expected original/libcurl.def --flavor gnutls
  bash safe/scripts/verify-symbol-versions.sh --expected original/debian/libcurl4t64.symbols --flavor openssl
  bash safe/scripts/verify-symbol-versions.sh --expected original/debian/libcurl3t64-gnutls.symbols --flavor gnutls
  bash safe/scripts/build-compat-consumers.sh --flavor openssl --all
  bash safe/scripts/build-compat-consumers.sh --flavor gnutls --all
  cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test public_abi
  cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test public_abi
  cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test abi_layout
  cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test abi_layout
  bash safe/scripts/run-public-abi-smoke.sh --flavor openssl
  bash safe/scripts/run-public-abi-smoke.sh --flavor gnutls
  bash safe/scripts/run-link-compat.sh --flavor openssl --all-objects
  bash safe/scripts/run-link-compat.sh --flavor gnutls --all-objects
  bash safe/scripts/run-upstream-tests.sh --flavor openssl --require-all-runtests --no-exclusion-keywords
  bash safe/scripts/run-upstream-tests.sh --flavor gnutls --require-all-runtests --no-exclusion-keywords
  bash safe/scripts/run-curl-tool-smoke.sh --implementation compat --flavor openssl
  bash safe/scripts/run-curl-tool-smoke.sh --implementation compat --flavor gnutls
  bash /tmp/libcurl-safe-final-check/scripts/run-curl-tool-smoke.sh --implementation packaged --flavor openssl --package-root /tmp/libcurl-safe-final-check
  bash safe/scripts/run-http-client-tests.sh --flavor openssl --all
  bash safe/scripts/run-http-client-tests.sh --flavor gnutls --all
  cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test unit_port
  cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test unit_port
  python3 safe/scripts/verify-cve-coverage.py --manifest safe/metadata/cve-manifest.json --mapping safe/metadata/cve-to-test.json --cases-dir safe/tests/cve_cases
  cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test cve_regressions
  cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test cve_regressions
  rm -rf safe/.bench-output/final
  mkdir -p safe/.bench-output/final
  bash safe/scripts/benchmark-local.sh --implementation original --flavor openssl --matrix core --output-dir safe/.bench-output/final/original/openssl
  bash safe/scripts/benchmark-local.sh --implementation safe --flavor openssl --matrix core --output-dir safe/.bench-output/final/safe/openssl
  python3 safe/scripts/compare-benchmarks.py --baseline safe/.bench-output/final/original/openssl --candidate safe/.bench-output/final/safe/openssl --thresholds safe/benchmarks/thresholds.json
  bash safe/scripts/benchmark-local.sh --implementation original --flavor gnutls --matrix core --output-dir safe/.bench-output/final/original/gnutls
  bash safe/scripts/benchmark-local.sh --implementation safe --flavor gnutls --matrix core --output-dir safe/.bench-output/final/safe/gnutls
  python3 safe/scripts/compare-benchmarks.py --baseline safe/.bench-output/final/original/gnutls --candidate safe/.bench-output/final/safe/gnutls --thresholds safe/benchmarks/thresholds.json
  bash /tmp/libcurl-safe-final-check/scripts/run-ldap-devpkg-test.sh --flavor openssl --package-root /tmp/libcurl-safe-final-check
  bash /tmp/libcurl-safe-final-check/scripts/run-ldap-devpkg-test.sh --flavor gnutls --package-root /tmp/libcurl-safe-final-check
  bash /tmp/libcurl-safe-final-check/scripts/verify-devpkg-tooling-contract.sh --package-root /tmp/libcurl-safe-final-check
  python3 /tmp/libcurl-safe-final-check/scripts/verify-package-control-contract.py --expected-control original/debian/control --actual-control /tmp/libcurl-safe-final-check/debian/control --package-root /tmp/libcurl-safe-final-check --require-source-build-deps cargo:native rustc:native
  bash /tmp/libcurl-safe-final-check/scripts/verify-package-install-layout.sh --package-root /tmp/libcurl-safe-final-check
  bash /tmp/libcurl-safe-final-check/scripts/verify-autopkgtest-contract.sh --expected-control original/debian/tests/control --actual-control /tmp/libcurl-safe-final-check/debian/tests/control
  bash /tmp/libcurl-safe-final-check/scripts/run-packaged-autopkgtests.sh --package-root /tmp/libcurl-safe-final-check --test upstream-tests-openssl
  bash /tmp/libcurl-safe-final-check/scripts/run-packaged-autopkgtests.sh --package-root /tmp/libcurl-safe-final-check --test upstream-tests-gnutls
  bash /tmp/libcurl-safe-final-check/scripts/run-packaged-autopkgtests.sh --package-root /tmp/libcurl-safe-final-check --test curl-ldapi-test
  bash ./test-original.sh --implementation safe
  bash /tmp/libcurl-safe-final-check/scripts/audit-final-build-independence.sh --package-root /tmp/libcurl-safe-final-check
  cargo clippy --manifest-path safe/Cargo.toml --all-targets --no-default-features --features openssl-flavor -- -D warnings
  cargo clippy --manifest-path safe/Cargo.toml --all-targets --no-default-features --features gnutls-flavor -- -D warnings
  ```

## Critical Files

- `safe/Cargo.toml` — defines the Rust crate, flavor split for `openssl-flavor` and `gnutls-flavor`, library output type, and shared test/build dependencies.
- `safe/Cargo.lock`, `safe/.cargo/config.toml`, and `safe/vendor/cargo/*` — checked-in Cargo resolution and vendored crate sources that keep detached `dpkg-buildpackage` runs and `test-original.sh --implementation safe` builds offline and independent of crates.io or user-global Cargo state.
- `safe/build.rs` — generates version scripts from Debian symbol files, compiles the required C shims, and injects per-flavor linker metadata.
- `safe/include/curl/*.h` — canonical installed public headers copied from `original/include/curl/*.h` and kept source-compatible.
- `safe/src/lib.rs` — top-level module wiring for the Rust libcurl port.
- `safe/src/abi/generated.rs` — checked-in header-derived `#[repr(C)]` scaffolding with explicit target-conditioned branches so builds do not depend on live binding generation while preserving per-architecture ABI details from `system.h`.
- `safe/src/abi/public_types.rs` — `#[repr(C)]` representations and layout checks for public structs and enums.
- `safe/src/abi/easy.rs`, `safe/src/abi/share.rs`, `safe/src/abi/url.rs`, and `safe/src/abi/multi.rs` — exported C ABI wrappers around the Rust core.
- `safe/c_shim/forwarders.c` — temporary whole-surface fallback bridge used only during incremental migration and removed in the final phase.
- `safe/c_shim/variadic.c` — permanent C ABI shim for variadic entrypoints such as `curl_easy_setopt`, `curl_easy_getinfo`, `curl_multi_setopt`, `curl_share_setopt`, and `curl_formadd`.
- `safe/c_shim/mprintf.c` — permanent ABI shim for the `curl_mprintf*` family.
- `safe/src/alloc.rs` — runtime-switchable allocator facade required by `curl_global_init_mem`.
- `safe/src/global.rs` — process-global initialization, cleanup, allocator registration, trace hooks, and backend selection state.
- `safe/src/version.rs` — `curl_version`, `curl_version_info`, feature bits, protocol lists, backend reporting, and patchstamp handling.
- `safe/src/easy/*` — easy-handle lifecycle, option dispatch, callbacks, MIME/form integration, and `curl_easy_perform`.
- `safe/src/share.rs` — share-handle state, callbacks, and shared-resource coordination.
- `safe/src/urlapi.rs`, `safe/src/mime.rs`, `safe/src/form.rs`, and `safe/src/slist.rs` — URL parsing, MIME, legacy form, and slist behavior.
- `safe/src/multi/*` — multi-handle state machine, timer tree, poll/wakeup integration, socket-action plumbing, and message queue handling.
- `safe/src/conn/*` — connection cache, connection filters, connection keys, reuse policy, and connect-only state.
- `safe/src/dns/*` and `safe/src/doh.rs` — resolver and DNS-over-HTTPS support.
- `safe/src/transfer/*` — transfer loop, speed/timeout logic, read/write state, pause handling, and callback dispatch.
- `safe/src/http/*` — HTTP request/response processing, proxy logic, redirect policy, headers API, cookies, HSTS, alt-svc, and authentication.
- `safe/src/ws.rs` and `safe/src/rand.rs` — WebSocket framing/state and strong randomness for masks/nonces.
- `safe/src/tls/*` — backend-neutral TLS policy plus OpenSSL and GnuTLS adapters.
- `safe/src/vquic/*` and `safe/src/ssh/*` — optional QUIC/HTTP3 boundary code when enabled and SSH backend integration.
- `safe/src/protocols/*` — protocol-specific state machines for FILE, FTP, IMAP, POP3, SMTP, LDAP, SMB, TELNET, TFTP, DICT, GOPHER, RTSP, and MQTT.
- `safe/metadata/abi-manifest.json` — authoritative ABI contract: exported symbols, symbol versions, sonames, shared-library filenames, header hashes, public struct layouts, public enum values, ABI-relevant macro aliases, version strings, and option metadata.
- `safe/metadata/test-manifest.json` — authoritative test, consumer, and vendored-compatibility-source manifest: data tests, the tracked `tests/data/DISABLED` semantics, libtests, unit ids, HTTP clients, server helpers, tool sources, autopkgtests, downstream dependent names, the exact tracked upstream asset inventory copied into `safe/vendor/upstream/`, and the exact common/target-specific compile and link metadata plus per-target libcurl-consumer versus helper roles needed to rebuild compatibility consumers without rediscovery.
- `safe/metadata/cve-manifest.json` — authoritative security manifest built from `relevant_cves.json` and the existing Debian CVE patch files.
- `safe/metadata/cve-to-test.json` — required mapping from every curated CVE to a specific regression case.
- `safe/scripts/verify-cve-coverage.py` — checker that fails if any curated CVE is unmapped, mapped to a missing regression case, or routed to a shared case without justification.
- `safe/abi/libcurl-openssl.map` and `safe/abi/libcurl-gnutls.map` — version scripts that preserve `CURL_OPENSSL_4` and `CURL_GNUTLS_3`.
- `safe/scripts/build-reference-curl.sh` — transitional helper that rebuilds tracked original libcurl trees for temporary forwarding without consuming dirty outputs.
- `safe/scripts/vendor-compat-assets.sh` — copies the manifest-backed upstream compatibility-source inventory into `safe/vendor/upstream/` and records the copied files in `safe/vendor/upstream/manifest.json`.
- `safe/vendor/upstream/manifest.json`, `safe/vendor/upstream/src/*`, `safe/vendor/upstream/tests/*`, `safe/vendor/upstream/lib/*`, `safe/vendor/upstream/.pc/90_gnutls.patch/*`, and `safe/vendor/upstream/debian/tests/LDAP-bindata.c` — vendored tracked upstream compatibility assets kept inside `safe/` so package builds and autopkgtests remain self-contained.
- `safe/scripts/export-tracked-tree.sh` — shared tracked-source export helper reused by compatibility, package, and dependent harnesses, with fixed `--safe-only` and `--with-root-harness` layouts.
- `safe/compat/CMakeLists.txt` and `safe/compat/generated-sources.cmake` — compatibility-consumer build that compiles the vendored upstream `curl` tool, server helpers, libtests, HTTP client programs, and LDAP dev-package test using the manifest-recorded upstream link contracts, linking only the actual libcurl consumers against the safe library.
- `safe/compat/link-manifest.json` — defines curated and full runnable object-file relink matrices over the stable consumer target ids recorded in `safe/metadata/test-manifest.json`, including per-target runtime adapters.
- `safe/scripts/build-compat-consumers.sh` — driver for the compatibility-consumer build rooted at `safe/vendor/upstream/`, which also emits per-flavor build-state describing the actual object and executable outputs for relink reuse.
- `safe/scripts/run-curated-libtests.sh` — stable wrapper for phase-specific libtest subsets.
- `safe/scripts/run-link-compat.sh` — relink harness that reuses the object files and resolved command metadata emitted by `safe/scripts/build-compat-consumers.sh`, relinks those `.o` files against the safe shared libraries without recompiling, and then executes the relinked binaries through declared runtime adapters.
- `safe/scripts/run-upstream-tests.sh` — wrapper around the vendored upstream `runtests.pl` assets that enforces manifest-backed full-suite coverage when requested, preserves duplicate token ordering, honors the tracked `tests/data/DISABLED` semantics without forcing `-f`, and forbids extra exclusion filters in full-suite mode.
- `safe/scripts/run-public-abi-smoke.sh` — flavor-isolated C smoke runner for `safe/tests/smoke/public_api_smoke.c`.
- `safe/scripts/run-curl-tool-smoke.sh` — runtime smoke harness for the compatibility-built `curl` tool in both flavors and the packaged OpenSSL `curl` binary.
- `safe/scripts/run-http-client-tests.sh` — runner for the 7 tracked `tests/http/clients` programs.
- `safe/scripts/run-ldap-devpkg-test.sh` — package/dev-header compile test using the vendored/copied `LDAP-bindata.c` source kept inside `safe/`.
- `safe/scripts/verify-autopkgtest-contract.sh` — autopkgtest control-file checker that fails on drift in test names, `Depends`, or `Restrictions`.
- `safe/scripts/verify-package-control-contract.py` — package-control checker that fails on drift in the six Ubuntu binary package stanzas and on mismatches between `safe/debian/control` and the built `.deb` metadata.
- `safe/scripts/verify-package-install-layout.sh` — package-payload checker that extracts the built `.deb`s and fails if required installed files or symlinks are missing from the runtime, dev, tool, or doc packages.
- `safe/scripts/verify-devpkg-tooling-contract.sh` — developer-tooling checker that validates the packaged `curl-config`, `libcurl.pc`, and `libcurl.m4` contract for both dev packages.
- `safe/scripts/run-packaged-autopkgtests.sh` — driver that installs the just-built packages from a detached `safe/` export, resolves the selected autopkgtest `Depends` (including `@builddeps@` expansion against `safe/debian/control`), and executes the actual entrypoints from `safe/debian/tests/`.
- `safe/scripts/http-fixtures.sh` and `safe/scripts/http-fixture.py` — shared loopback fixture/server helpers reused by compatibility tests, benchmarks, and the package-dependent harness.
- `safe/tests/public_abi.rs` — integration tests for the public easy/share/url/mime/form/global ABI surface.
- `safe/tests/abi_layout.rs` — layout and constant checks against the ABI manifest.
- `safe/tests/unit_port.rs`, `safe/tests/unit_port_cases/`, and `safe/tests/port-map.json` — Rust integration tests ported from the 46 original unit ids plus their source-to-port mapping.
- `safe/tests/cve_regressions.rs` and `safe/tests/cve_cases/` — CVE regressions and shared security behavior tests.
- `safe/benchmarks/scenarios.json` — canonical benchmark scenario definitions.
- `safe/benchmarks/thresholds.json` — explicit acceptable performance-regression budgets.
- `safe/benchmarks/harness/easy_loop.c` and `safe/benchmarks/harness/multi_parallel.c` — public-API benchmark drivers compiled against either the original or safe library.
- `safe/scripts/benchmark-local.sh` — benchmark runner for original-vs-safe comparison on loopback fixtures.
- `safe/scripts/compare-benchmarks.py` — regression-budget checker for benchmark JSON output.
- `safe/docs/performance.md` — benchmark methodology, scenario definitions, and performance expectations.
- `safe/debian/control`, `safe/debian/changelog`, `safe/debian/copyright`, `safe/debian/source/format`, `safe/debian/patches/series`, `safe/debian/patches/*.patch`, `safe/debian/*.install`, `safe/debian/*.links`, `safe/debian/*.docs`, `safe/debian/*.examples`, `safe/debian/*.lintian-overrides`, `safe/debian/*.manpages`, `safe/debian/*.symbols`, and `safe/debian/tests/*` — Debian packaging, explicit quilt-series state, and autopkgtest definitions for the safe port.
- `safe/libcurl.pc`, `safe/curl-config`, and `safe/docs/libcurl/libcurl.m4` — installed developer tooling and metadata preserved for package compatibility.
- `safe/scripts/audit-final-build-independence.sh` — final proof script that the temporary forwarder bridge and reference-build dependency are gone.
- `safe/docs/unsafe-audit.md` — final record of unavoidable `unsafe` boundaries.
- `test-original.sh` — root-level Docker harness updated to build/install packages from a tracked-file export containing `safe/` plus root `dependents.json`, and to run the existing 12 dependent smoke tests while preserving the original baseline mode.

Files under `original/` should remain the read-only reference snapshot and generally should not be modified by the implementation workflow. They are consumed as inputs for manifests, compatibility builds, tests, docs, packaging behavior, and benchmark scenario design.

## Final Verification

After all phases complete, the final checker should verify the safe port with the complete matrix below. This is the required end state, not an optional stretch goal. The pass criteria are:

- all 1677 ordered `TESTCASES` tokens from `original/tests/data/Makefile.inc` are accounted for: every token not disabled by the tracked `tests/data/DISABLED` rules for the selected flavor runs through full `runtests.pl` execution in order with the duplicate `test1190` preserved, every token disabled by that tracked file is reported as an intentional upstream skip, and the disabled former-unit ids `1300`, `1309`, `1323`, `1602`, `1603`, `1604`, `1661`, and `2601` are additionally exercised through the Rust unit-port suite
- every manifest-recorded compatibility target builds with its upstream link contract preserved; the libcurl-consuming targets among them link the safe library, while auxiliary helper targets such as `chkhostname` and the tracked server helpers keep their original non-libcurl link lines
- every selected original object-file consumer in the final relink manifest reuses the object outputs built from the manifest-recorded upstream compile metadata, relinks without recompilation, and then runs successfully under the required fixtures in both flavors
- all 46 original unit source ids execute through the Rust unit-port suite
- all 7 tracked HTTP client programs execute in both flavors
- the flavor-isolated `public_api_smoke.c` C ABI check passes once against the OpenSSL build and once against the GnuTLS build
- both Debian symbol/version contracts are preserved
- every curated CVE in `safe/metadata/cve-manifest.json` is mapped in `safe/metadata/cve-to-test.json`, every mapped case exists, and the CVE regression suite still passes in both flavors
- a detached export of `safe/` alone builds with `dpkg-buildpackage` in offline Cargo mode using checked-in `Cargo.lock`, `.cargo/config.toml`, vendored `vendor/cargo/`, and an explicit `safe/debian/patches/series` under `3.0 (quilt)`, and the resulting packages/tests run without reading `original/` or another sibling tree
- the six Ubuntu binary package stanzas remain compatible in both `safe/debian/control` and the built `.deb` metadata
- the built `.deb` payloads preserve the Ubuntu install layout, including the runtime-library soname/symlink files, packaged `curl` binary/manpage, public headers under `/usr/include/$DEB_HOST_MULTIARCH/curl`, both dev-package metadata paths, and the `libcurl4-doc` docs/examples/manpages/symlink set
- both development packages preserve the packaged `curl-config`, `libcurl.pc`, and `usr/share/aclocal/libcurl.m4` tooling contract
- `safe/debian/tests/control` preserves the original autopkgtest names, `Depends`, and `Restrictions`, and the actual `upstream-tests-openssl`, `upstream-tests-gnutls`, and `curl-ldapi-test` entrypoints pass
- the compatibility-built `curl` tool runs correctly in both flavors and the packaged OpenSSL `curl` binary runs correctly
- the full 12-dependent Docker matrix passes with safe packages installed
- the final independence audit confirms that `safe/c_shim/forwarders.c` is absent and that neither final flavor library nor packaged binary depends on `safe/.reference/` or `libcurl-reference-*`
- the benchmark thresholds in `safe/benchmarks/thresholds.json` are satisfied
- both feature-flavored `cargo clippy --all-targets -D warnings` runs pass on an executor with `rust-clippy` explicitly installed

Required command matrix:

Run the package-build shell block below in the prepared Ubuntu 24.04 executor described in the generated-workflow contract and phase-10 implementation details.

```bash
bash -lc '
set -euo pipefail
rm -rf /tmp/libcurl-safe-final-check
bash safe/scripts/export-tracked-tree.sh --safe-only --dest /tmp/libcurl-safe-final-check
test -f /tmp/libcurl-safe-final-check/Cargo.lock
test -f /tmp/libcurl-safe-final-check/.cargo/config.toml
test -d /tmp/libcurl-safe-final-check/vendor/cargo
rg -n "replace-with *= *\"vendored-sources\"|directory *= *\"vendor/cargo\"" /tmp/libcurl-safe-final-check/.cargo/config.toml >/dev/null
rg -n "cargo:native" /tmp/libcurl-safe-final-check/debian/control >/dev/null
rg -n "rustc:native" /tmp/libcurl-safe-final-check/debian/control >/dev/null
rg -n "CARGO_NET_OFFLINE=true|--offline" /tmp/libcurl-safe-final-check/debian/rules >/dev/null
rg -n -- "--locked" /tmp/libcurl-safe-final-check/debian/rules >/dev/null
test "$(cat /tmp/libcurl-safe-final-check/debian/source/format)" = "3.0 (quilt)"
test -f /tmp/libcurl-safe-final-check/debian/patches/series
while IFS= read -r patch; do
  case "$patch" in
    ''|'#'*) continue ;;
  esac
  test -f "/tmp/libcurl-safe-final-check/debian/patches/$patch"
done </tmp/libcurl-safe-final-check/debian/patches/series
cd /tmp/libcurl-safe-final-check
mk-build-deps -ir -t 'apt-get -y --no-install-recommends' debian/control
dpkg-buildpackage -us -uc -b
'
python3 safe/scripts/verify-manifests.py --abi safe/metadata/abi-manifest.json --tests safe/metadata/test-manifest.json --cves safe/metadata/cve-manifest.json
bash safe/scripts/verify-public-headers.sh --expected original/include/curl --actual safe/include/curl
bash safe/scripts/verify-export-names.sh --expected original/libcurl.def --flavor openssl
bash safe/scripts/verify-export-names.sh --expected original/libcurl.def --flavor gnutls
bash safe/scripts/verify-symbol-versions.sh --expected original/debian/libcurl4t64.symbols --flavor openssl
bash safe/scripts/verify-symbol-versions.sh --expected original/debian/libcurl3t64-gnutls.symbols --flavor gnutls
bash safe/scripts/build-compat-consumers.sh --flavor openssl --all
bash safe/scripts/build-compat-consumers.sh --flavor gnutls --all
cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test public_abi
cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test public_abi
cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test abi_layout
cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test abi_layout
bash safe/scripts/run-public-abi-smoke.sh --flavor openssl
bash safe/scripts/run-public-abi-smoke.sh --flavor gnutls
bash safe/scripts/run-link-compat.sh --flavor openssl --all-objects
bash safe/scripts/run-link-compat.sh --flavor gnutls --all-objects
bash safe/scripts/run-upstream-tests.sh --flavor openssl --require-all-runtests --no-exclusion-keywords
bash safe/scripts/run-upstream-tests.sh --flavor gnutls --require-all-runtests --no-exclusion-keywords
bash safe/scripts/run-curl-tool-smoke.sh --implementation compat --flavor openssl
bash safe/scripts/run-curl-tool-smoke.sh --implementation compat --flavor gnutls
bash /tmp/libcurl-safe-final-check/scripts/run-curl-tool-smoke.sh --implementation packaged --flavor openssl --package-root /tmp/libcurl-safe-final-check
bash safe/scripts/run-http-client-tests.sh --flavor openssl --all
bash safe/scripts/run-http-client-tests.sh --flavor gnutls --all
cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test unit_port
cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test unit_port
python3 safe/scripts/verify-cve-coverage.py --manifest safe/metadata/cve-manifest.json --mapping safe/metadata/cve-to-test.json --cases-dir safe/tests/cve_cases
cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test cve_regressions
cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test cve_regressions
rm -rf safe/.bench-output/final
mkdir -p safe/.bench-output/final
bash safe/scripts/benchmark-local.sh --implementation original --flavor openssl --matrix core --output-dir safe/.bench-output/final/original/openssl
bash safe/scripts/benchmark-local.sh --implementation safe --flavor openssl --matrix core --output-dir safe/.bench-output/final/safe/openssl
python3 safe/scripts/compare-benchmarks.py --baseline safe/.bench-output/final/original/openssl --candidate safe/.bench-output/final/safe/openssl --thresholds safe/benchmarks/thresholds.json
bash safe/scripts/benchmark-local.sh --implementation original --flavor gnutls --matrix core --output-dir safe/.bench-output/final/original/gnutls
bash safe/scripts/benchmark-local.sh --implementation safe --flavor gnutls --matrix core --output-dir safe/.bench-output/final/safe/gnutls
python3 safe/scripts/compare-benchmarks.py --baseline safe/.bench-output/final/original/gnutls --candidate safe/.bench-output/final/safe/gnutls --thresholds safe/benchmarks/thresholds.json
bash /tmp/libcurl-safe-final-check/scripts/run-ldap-devpkg-test.sh --flavor openssl --package-root /tmp/libcurl-safe-final-check
bash /tmp/libcurl-safe-final-check/scripts/run-ldap-devpkg-test.sh --flavor gnutls --package-root /tmp/libcurl-safe-final-check
bash /tmp/libcurl-safe-final-check/scripts/verify-devpkg-tooling-contract.sh --package-root /tmp/libcurl-safe-final-check
python3 /tmp/libcurl-safe-final-check/scripts/verify-package-control-contract.py --expected-control original/debian/control --actual-control /tmp/libcurl-safe-final-check/debian/control --package-root /tmp/libcurl-safe-final-check --require-source-build-deps cargo:native rustc:native
bash /tmp/libcurl-safe-final-check/scripts/verify-package-install-layout.sh --package-root /tmp/libcurl-safe-final-check
bash /tmp/libcurl-safe-final-check/scripts/verify-autopkgtest-contract.sh --expected-control original/debian/tests/control --actual-control /tmp/libcurl-safe-final-check/debian/tests/control
bash /tmp/libcurl-safe-final-check/scripts/run-packaged-autopkgtests.sh --package-root /tmp/libcurl-safe-final-check --test upstream-tests-openssl
bash /tmp/libcurl-safe-final-check/scripts/run-packaged-autopkgtests.sh --package-root /tmp/libcurl-safe-final-check --test upstream-tests-gnutls
bash /tmp/libcurl-safe-final-check/scripts/run-packaged-autopkgtests.sh --package-root /tmp/libcurl-safe-final-check --test curl-ldapi-test
bash ./test-original.sh --implementation safe
bash /tmp/libcurl-safe-final-check/scripts/audit-final-build-independence.sh --package-root /tmp/libcurl-safe-final-check
cargo clippy --manifest-path safe/Cargo.toml --all-targets --no-default-features --features openssl-flavor -- -D warnings
cargo clippy --manifest-path safe/Cargo.toml --all-targets --no-default-features --features gnutls-flavor -- -D warnings
```
