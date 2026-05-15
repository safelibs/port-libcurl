# libcurl Safe Rust Port Implementation Plan

## Context

This repository is the SafeLibs port workspace for `libcurl` on Ubuntu 24.04. The goal is to complete a drop-in Rust implementation under `safe/` that preserves the original libcurl C API, ELF ABI, runtime behavior, Debian package layout, and downstream compatibility.

The workspace already contains the canonical upstream and packaging inputs under `original/`, plus a substantial partial Rust port under `safe/`. The current `safe/` tree includes:

- A Rust crate in `safe/Cargo.toml` with `openssl-flavor` and `gnutls-flavor` features.
- Rust modules for ABI scaffolding, allocator/global state, easy/multi handles, URL API, MIME/form, HTTP, cookies, HSTS, ALTSVC, TLS, SSH, WebSockets, protocols, and transfer logic under `safe/src/`.
- Permanent and transitional C ABI shims under `safe/c_shim/`, including variadic dispatch, mprintf, public export thunks, HTTP/2 transport, TLS/SSH helpers, and a reference sidecar resolver.
- Copied public headers under `safe/include/curl/`.
- ABI, test, CVE, link, and benchmark manifests under `safe/metadata/`, `safe/compat/`, and `safe/benchmarks/`.
- Compatibility harness scripts under `safe/scripts/`.
- Debian package metadata under `safe/debian/`.
- Existing `.plan/` artifacts, including older phase files and workflow structure, are historical artifacts only. The generated workflow must be driven by this file and inline prompts, not by those files as indirection.

The original libcurl snapshot is the compatibility source of truth. Important public ABI files are:

- `original/include/curl/curl.h`, with public structs such as `struct curl_httppost` at line 176 and `struct curl_version_info_data` at line 3052.
- `original/include/curl/easy.h`, with core easy API declarations at lines 41-119.
- `original/include/curl/multi.h`, with `struct curl_waitfd` at line 118 and multi API declarations at lines 131-439.
- `original/include/curl/header.h`, with `struct curl_header` at line 31 and headers API declarations at lines 58-65.
- `original/include/curl/options.h`, with `struct curl_easyoption` at line 51.
- `original/include/curl/websockets.h`, with `struct curl_ws_frame` at line 31 and WebSocket API declarations at lines 55-78.
- `original/include/curl/urlapi.h`, with URL API declarations at lines 108-144.
- `original/debian/libcurl4t64.symbols` and `original/debian/libcurl3t64-gnutls.symbols`, each listing 93 exported `curl_*` symbols plus the flavor namespace.
- `original/libcurl.def`, listing the Windows-style exported symbol inventory.
- `original/lib/libcurl.vers.in`, the upstream version-script input.

The C implementation is large and state-machine heavy. `original/lib/Makefile.inc` enumerates 164 library C files. Core state and behavior are concentrated in:

- `original/lib/urldata.h`, with `struct UrlState` at line 1346, `struct UserDefined` at line 1667, and `struct Curl_easy` at line 1972.
- `original/lib/multihandle.h`, with the multi state enum at line 44 and `struct Curl_multi` at line 87.
- `original/lib/easy.c`, with global initialization at line 144 and `curl_easy_perform` at line 778.
- `original/lib/setopt.c`, with `curl_easy_setopt` at line 3153.
- `original/lib/getinfo.c`, with typed getinfo dispatch helpers starting at line 94.
- `original/lib/multi.c`, with `curl_multi_perform` at line 2672 and `curl_multi_socket_action` at line 3328.
- `original/lib/http.c`, with HTTP request, authentication, redirect, cookie, size, and header processing across lines 641, 990, 2848, 3516, and 3999.
- `original/lib/http2.c`, with HTTP/2 upgrade and connection-filter logic across lines 1609, 2337, 2684, and 2741.
- `original/lib/cookie.c`, `original/lib/hsts.c`, `original/lib/altsvc.c`, `original/lib/headers.c`, `original/lib/ws.c`, `original/lib/mime.c`, `original/lib/formdata.c`, and `original/lib/urlapi.c`.

The test and dependent artifacts already exist and must be consumed in place:

- `original/tests/data/Makefile.inc` defines 1677 ordered testcase tokens and 1676 unique test ids because `test1190` appears twice.
- `original/tests/data/DISABLED` is the tracked upstream disabled-test contract.
- `original/tests/libtest/Makefile.inc` defines 256 libtest programs.
- `original/tests/unit/Makefile.inc` defines 46 unit source ids and 3 enabled upstream `UNITPROGS`.
- `original/tests/http/clients/Makefile.inc` defines 7 tracked HTTP/WebSocket client programs: 5 HTTP/TLS clients that must run successfully, plus `ws-data` and `ws-pingpong`, which must compile under Ubuntu's disabled-WebSocket configuration and report the original "websockets not enabled in libcurl" behavior instead of running as functional WebSocket clients.
- `original/tests/server/Makefile.inc` defines 10 server-helper programs.
- `original/debian/tests/control` defines the OpenSSL upstream autopkgtest, GnuTLS upstream autopkgtest, and LDAP dev-package autopkgtest.
- `dependents.json` lists 12 curated downstream dependents: Git, CMake, PHP cURL extension, PycURL, R curl package, GDAL, OSTree, librepo, HTSlib, pacman/libalpm, HTTPDirFS, and fwupd.
- `relevant_cves.json` lists 107 non-memory-corruption CVEs that still need explicit mitigation and regression coverage after a Rust rewrite.
- `original/debian/patches/CVE-*.patch` contains 21 Debian CVE patch files that must be used as regression design inputs.

The current partial Rust port must be hardened rather than restarted. In particular:

- `safe/build.rs` already builds flavor-specific version scripts from Debian `.symbols` files, compiles C shims, and builds reference sidecars through `safe/scripts/build-reference-curl.sh`.
- `safe/c_shim/forwarders.c` currently `dlopen`s `libcurl-reference-<flavor>.so.4` and forwards selected behavior. This is transitional. Phase 9 owns deleting this file and removing every non-benchmark source-level sidecar marker, package-build hook, public smoke dependency, compatibility-staging dependency, upstream safe-test dependency, validator dependency, and dependent safe-mode dependency that can build, install, link, load, stage, or record a reference sidecar or resolver marker. After Phase 9, only the explicitly benchmark-only original-baseline helpers `safe/scripts/build-reference-curl.sh` and `safe/scripts/benchmark-local.sh` may retain reference-helper behavior. Verifier and audit scripts named `safe/scripts/verify-*` or `scripts/verify-*` may contain marker strings only as inert forbidden-pattern detector constants; they must not build, load, stage, copy, or record a reference sidecar.
- `safe/src/easy/reference.rs` maintains active reference-handle state. This is transitional. Phase 9 owns deleting this source file, removing `mod reference`, removing `load_reference`, `ReferenceRegistry`, `ReferenceHandle`, `reference_backend`, resolver declarations in C shims, and replacing all remaining reference-backend CVE placeholders or documentation markers outside the two benchmark-only helper scripts. Phase 10 may only verify this stayed removed while completing the unsafe audit and final end-to-end fixes; unavoidable third-party C boundaries must be named after the actual third-party library or OS interface, not a libcurl reference sidecar.
- Ubuntu 24.04's source package enables protocol and feature surfaces beyond the current partial safe implementation. `original/lib/version.c`, `original/lib/url.c`, `original/lib/curl_rtmp.c`, `original/debian/rules`, and `original/debian/control` require the final safe library to advertise and route `gophers`, `ldap`, `ldaps`, and the six RTMP schemes (`rtmp`, `rtmpe`, `rtmps`, `rtmpt`, `rtmpte`, `rtmpts`) in addition to the other existing safe routes. The current `safe/src/version.rs` and `safe/src/protocols/mod.rs` must be corrected to match the original-derived contract.
- `safe/debian/rules` currently still resembles the upstream autotools packaging rules and does not yet drive Cargo as the source of the packaged libraries. Phase 9 must rewrite it for the Rust package.
- `safe/debian/changelog` currently starts at the Ubuntu version `8.5.0-2ubuntu10.8` without a SafeLibs suffix. Phase 9 must prepend the tracked `8.5.0-2ubuntu10.8+safelibs0` baseline so detached direct `dpkg-buildpackage` builds are SafeLibs-versioned without relying on the root build hook.
- `scripts/lib/build-deb-common.sh` currently removes `debian/patches` while synthesizing the `3.0 (quilt)` orig tarball. The safe packaging phase must switch the root CI hook to a binary-only `.deb` build (`dpkg-buildpackage -us -uc -b`) that copies only built `.deb` files into `dist/`, and it must keep `safe/debian/patches/series` as an explicit tracked safe-local patch stack, even if empty.
- The root `test-original.sh` currently tests the original runtime only. It must gain a safe-package mode that installs `dist/*.deb` and runs the same dependent matrix against libcurl-safe.
- Generated and dirty build outputs under `safe/.compat/`, `safe/.reference/`, `safe/target/`, `target/`, and dirty `original/` build products are disposable and must not be treated as canonical inputs.
- `safe/scripts/compat_harness.py` currently copies configured upstream build metadata from `safe/.reference/<flavor>/source/upstream/lib/curl_config.h`, `tests/config`, and `curl-config`. Those files do not exist as tracked configured artifacts under `safe/vendor/upstream/`, which only carries templates such as `lib/curl_config.h.in` and `tests/config.in`. Phase 3 must replace this flow with committed per-flavor compatibility config artifacts under `safe/compat/config/{openssl,gnutls}/`, and Phase 10 must prove the harness consumes those artifacts without reading or staging `safe/.reference/`.

The final result must satisfy three compatibility properties:

- Source compatibility: C programs compiling against Ubuntu 24.04 libcurl headers must compile against `safe/include/curl` and the installed safe development packages.
- Link compatibility: objects compiled against the original headers and link lines must relink against the safe shared libraries with the same exported symbol names, SONAMEs, and symbol namespaces.
- Runtime compatibility: original libcurl consumers and the 12 dependent programs in `dependents.json` must behave the same when the safe packages replace Ubuntu's original libcurl packages.

## Generated Workflow Contract

- The generated workflow must be strictly linear. Do not use `parallel_groups`.
- The generated workflow YAML must be self-contained and inline-only. Do not use top-level `include`, and do not use phase-level `prompt_file`, `workflow_file`, `workflow_dir`, `checks`, or any other YAML-source indirection.
- Existing files under `.plan/phases/` and `.plan/workflow-structure.yaml` are not workflow source inputs. Inline all prompts and check instructions derived from this `plan.md`.
- Use one fixed `bounce_target` per check phase. Do not use `bounce_targets` lists or verifier-guided routing.
- Every verifier must be an explicit top-level `check` phase.
- Every verifier must stay in the implement block it verifies and must bounce only to that implement phase.
- If a verifier needs to run tests, lint, build, package, Docker, benchmark, ABI, or review commands, those commands must be written into the checker's instructions. Do not model them as non-agentic workflow phases.
- Checker commands must be runnable exactly as written from the repository root unless the command block itself performs `cd`.
- If the goal or workspace already provides artifacts, list them as existing inputs and consume or update them in place. Do not refetch, recollect, rediscover, or regenerate them from scratch unless the phase explicitly updates a derived safe artifact from a listed input.
- Prepared artifacts such as source snapshots, CVE data, dependent inventories, test manifests, and harnesses must be preserved as consume-existing-artifacts inputs. The workflow must not replace them with freshly downloaded upstream material.
- Every implement prompt in the final generated workflow must instruct the agent to commit its work to git before yielding.
- Each implementation phase must own only its listed `New Outputs` and `File Changes`. Later checks must not invoke scripts or generated sources that are first produced by a later phase.
- Detached package verifiers must export a tracked safe-only source tree first, then build inside that detached tree. This forces accidental sibling `original/` dependencies to fail.
- Package-build verifiers must run in an Ubuntu 24.04 package-build environment with at least `build-essential`, `ca-certificates`, `devscripts`, `equivs`, `dpkg-dev`, `fakeroot`, `pkgconf`, `python3`, `ripgrep`, `jq`, `git`, `curl`, `file`, `rsync`, and `xz-utils`. The verifier commands must run `mk-build-deps -ir -t 'apt-get -y --no-install-recommends' debian/control` before `dpkg-buildpackage`.
- Package-build verifiers must set `DEBIAN_FRONTEND=noninteractive`, `CARGO_NET_OFFLINE=true`, and `CARGO_HOME` to a fresh empty directory under the verifier's temporary directory before invoking `dpkg-buildpackage` or `scripts/build-debs.sh`. The verifier must assert that `CARGO_HOME` is not `$HOME/.cargo`, is empty before the build starts, and that `safe/debian/rules` invokes Cargo with `--locked` plus offline behavior so a warm host registry cache or lockfile rewrite cannot satisfy missing crate inputs.
- SafeLibs package version stamping is explicit for both build paths. `safe/debian/changelog` must contain a committed first stanza for version `8.5.0-2ubuntu10.8+safelibs0`; detached safe-only `dpkg-buildpackage` verifiers consume that tracked stanza directly and must assert the changelog version and every built `.deb` version contain `+safelibs`. The root `scripts/build-debs.sh` hook may still call `stamp_safelibs_changelog` to prepend a commit-epoch version such as `8.5.0-2ubuntu10.8+safelibs<epoch>`, but it must do so only in a temporary package build tree or restore the live file before returning. The helper must strip any existing `+safelibs[0-9]+` suffix from the committed baseline before stamping, must replace any previous generated SafeLibs build stanza in the build tree rather than accumulating duplicate generated stanzas, and detached verifiers must not rely on the root hook dirtying `safe/debian/changelog`.
- The package-build crate-source policy is vendored-only. Every crates.io package in `safe/Cargo.lock` must have a checked-in source copy under `safe/vendor/cargo`, and `safe/.cargo/config.toml` must replace `crates-io` with that repository-relative vendor directory. Debian packages may provide the Rust compiler and Cargo binaries, but packaged builds must not rely on Debian's Cargo registry cache or a host Cargo cache for crate source. `safe/scripts/verify-cargo-source-policy.py` must enforce this policy for both the live `safe/` tree and detached safe-only exports before any package build.
- The safe package must not fetch crates, upstream sources, tests, or Debian metadata during build. Prepared upstream compatibility assets remain under `safe/vendor/upstream`; Rust crate sources remain under `safe/vendor/cargo`. Package-build verifiers must run a no-refetch scan before each `dpkg-buildpackage` or `scripts/build-debs.sh` invocation. The scan must cover `safe/debian/rules`, package build helper scripts reachable from `debian/rules` or `build.rs`, `scripts/build-debs.sh`, and `scripts/lib/build-deb-common.sh`, and must reject network-fetch paths such as `curl`, `wget`, `fetch`, `git clone`, `git fetch`, `git pull`, `git submodule`, `cargo fetch`, `cargo vendor`, `cargo update`, registry/index updates, `pip install`, `npm install`, `go get`, and remote `rsync`/URL downloads. Installing Debian build dependencies with `apt-get` through `mk-build-deps` is the only planned network use in package-build verifiers; the no-refetch verifier must recognize that prerequisite separately from forbidden source, crate, test, or metadata downloads.
- Compatibility consumer builds must use tracked configured metadata under `safe/compat/config/<flavor>/`, not `.reference` build trees. Phase 3 owns generating or refreshing those committed derived files from tracked Ubuntu configure inputs; every later phase must consume them as existing artifacts and must not run `configure`, read `safe/.reference/`, or recover configured metadata from the benchmark-only original-baseline helper.
- Debian package-control metadata is part of the compatibility contract. Phase 9 and final package verifiers must compare the six safe binary package stanzas and built `.deb` control fields against a tracked contract derived from `original/debian/control`, preserving package names plus key fields such as `Architecture`, `Section`, `Provides`, `Conflicts`, `Replaces`, `Breaks`, `Multi-Arch`, `Pre-Depends`, `Depends`, `Recommends`, `Suggests`, and `X-Time64-Compat`. The only allowed differences are documented Rust source `Build-Depends`, SafeLibs version stamping, and explicitly justified dependency substitutions that do not weaken Ubuntu's virtual-package, conflict, replacement, or time64 compatibility behavior.
- Debian development-package tooling is also part of the source-compatibility contract. Phase 9 and final package verifiers must install `libcurl4-openssl-dev` and `libcurl4-gnutls-dev` separately from the safe `.deb`s under test, then run a tracked dev-tooling contract check. The dev packages must preserve Ubuntu's multiarch header layout under `/usr/include/$DEB_HOST_MULTIARCH/curl/` only; package payload checks must fail if any safe `.deb` ships `./usr/include/curl/`, and installed checks must fail if `/usr/include/curl` exists after either safe dev package is installed. They must also preserve static development archives and generic linker names: `libcurl4-openssl-dev` must ship `/usr/lib/$DEB_HOST_MULTIARCH/libcurl.a` and `/usr/lib/$DEB_HOST_MULTIARCH/libcurl.so`; `libcurl4-gnutls-dev` must ship `/usr/lib/$DEB_HOST_MULTIARCH/libcurl-gnutls.a` and `/usr/lib/$DEB_HOST_MULTIARCH/libcurl-gnutls.so` plus Ubuntu's generic development links `/usr/lib/$DEB_HOST_MULTIARCH/libcurl.a -> libcurl-gnutls.a` and `/usr/lib/$DEB_HOST_MULTIARCH/libcurl.so -> libcurl-gnutls.so` from `libcurl4-gnutls-dev.links`. The dev-tooling check must compile and run a C smoke program through both `curl-config --cflags --libs` and `pkg-config --cflags --libs libcurl`, verify the linked shared-library SONAME for the selected flavor, compile and link a static C smoke object through both `curl-config --static-libs` and `pkg-config --static --libs libcurl`, fail if the GnuTLS dev package succeeds only through `-lcurl-gnutls` while the original generic `-lcurl` link-name contract is absent, compare every supported `curl-config` option (`--built-shared`, `--ca`, `--cc`, `--cflags`, `--checkfor`, `--configure`, `--features`, `--help`, `--libs`, `--prefix`, `--protocols`, `--ssl-backends`, `--static-libs`, `--version`, and `--vernum`) plus no-argument and unknown-option failure behavior with `safe/metadata/dev-tooling-contract.json` and `curl_version_info`, inspect `libcurl.pc` fields, `includedir`, `Cflags`, `Libs`, and `Libs.private` for that flavor, and prove `/usr/share/aclocal/libcurl.m4` can configure and link a small autoconf project using the multiarch include path without embedding sibling `original/` or local absolute paths.
- The Ubuntu-advertised protocol and feature contract is explicit. `curl_version_info_data.protocols` must expose exactly `dict`, `file`, `ftp`, `ftps`, `gopher`, `gophers`, `http`, `https`, `imap`, `imaps`, `ldap`, `ldaps`, `mqtt`, `pop3`, `pop3s`, `rtmp`, `rtmpe`, `rtmps`, `rtmpt`, `rtmpte`, `rtmpts`, `rtsp`, `scp`, `sftp`, `smb`, `smbs`, `smtp`, `smtps`, `telnet`, and `tftp` for both OpenSSL and GnuTLS flavors. `curl-config --protocols` and `libcurl.pc:supported_protocols` must expose the original configure-style uppercase list `DICT FILE FTP FTPS GOPHER GOPHERS HTTP HTTPS IMAP IMAPS LDAP LDAPS MQTT POP3 POP3S RTMP RTSP SCP SFTP SMB SMBS SMTP SMTPS TELNET TFTP`; RTMP appears once in those tooling surfaces even though `curl_version_info_data.protocols` lists all six RTMP URL schemes. `curl_version_info_data.feature_names`, `curl-config --features`, and `libcurl.pc:supported_features` must preserve the original-derived Ubuntu feature set: `alt-svc`, `AsynchDNS`, `brotli`, `GSS-API`, `HSTS`, `HTTP2`, `HTTPS-proxy`, `IDN`, `IPv6`, `Kerberos`, `Largefile`, `libz`, `NTLM`, `PSL`, `SPNEGO`, `SSL`, `threadsafe`, `TLS-SRP`, `UnixSockets`, and `zstd`, with each surface using its native ordering. Ubuntu's `original/debian/rules` does not pass `--enable-websockets`; therefore `USE_WEBSOCKETS` must remain undefined in safe compatibility config artifacts, `ws` and `wss` must not be routed or advertised, `curl_ws_recv` and `curl_ws_send` must return `CURLE_NOT_BUILT_IN`, and `curl_ws_meta` must return `NULL`. WebSocket ABI symbols remain exported and tested as disabled-build ABI symbols only.
- Advertised RTMP-family and LDAPS support must be functionally verified, not only registered. Phase 6 and the final verifier must run tracked local fixtures for both OpenSSL and GnuTLS. The RTMP fixture must perform at least one successful download/read transfer and at least one successful upload/write or publish-command path through the RTMP implementation, and it must distinguish the plain TCP (`rtmp`), encrypted RTMP (`rtmpe`), TLS (`rtmps`), HTTP-tunneled (`rtmpt`), encrypted HTTP-tunneled (`rtmpte`), and TLS-over-HTTP-tunneled (`rtmpts`) URL scheme families. The LDAPS fixture must start a local OpenLDAP TLS listener or equivalent deterministic LDAP-over-TLS server, perform a successful `ldaps://` query with a trusted CA, and prove certificate verification fails with no CA or a hostname/CA mismatch unless the corresponding libcurl verification option is explicitly disabled. Closed-port, DNS-failure, timeout, unsupported-protocol rejection-only, or registration-only probes do not satisfy functional RTMP or LDAPS coverage.
- Phase 9 source `Build-Depends` must preserve every original feature-enabling dependency unless the plan and verifier contract name a safe substitute that preserves the same observable protocol, feature, authentication, content-decoding, IDN, and tooling behavior. At minimum the safe source package must retain `libbrotli-dev`, `libgnutls28-dev`, `libidn2-dev`, `libkrb5-dev`, `libldap2-dev`, `libnghttp2-dev`, `libpsl-dev`, `librtmp-dev`, `libssh-dev`, `libssh2-1-dev`, `libssl-dev`, `libzstd-dev`, and `zlib1g-dev` in the source dependency contract, with Rust build dependencies added on top rather than replacing those feature dependencies. On Ubuntu, SSH transfer behavior must follow the original rules' selected `libssh` backend (`--with-libssh --without-libssh2`) unless a verifier-backed substitute preserves `curl_version_info`, `curl-config`, `libcurl.pc`, static link flags, and SCP/SFTP behavior.
- Verifiers that run Clippy must run in an executor with `rust-clippy` or an equivalent rustup Clippy component already available. This is a verifier prerequisite, not a Debian `Build-Depends`, unless `safe/debian/rules` itself invokes Clippy.
- Verifiers that run HTTP/2 client fixtures or benchmarks must run in an executor with `nghttpx` available, such as Ubuntu's `nghttp2-proxy` package. The checker commands must assert this before invoking those harnesses.
- Verifiers that run RTMP functional fixtures must have `python3`, `cc`, and TLS certificate tooling available; if the implementation uses `librtmp` FFI, they must also have `librtmp-dev` available. Verifiers that run LDAPS functional fixtures must have `slapd`, `ldap-utils`, `openssl`, `cc`, `pkgconf`, `libldap2-dev`, and `libc-dev` available. The scripts must assert these prerequisites before starting the fixture and must fail hard instead of silently downgrading to compile-only or routing-only coverage.
- Dependent safe-mode verifiers run the Docker/FUSE downstream matrix and therefore require an Ubuntu 24.04 executor with `docker`, a reachable Docker daemon, `git`, `jq`, `/dev/fuse` as a character device, and Docker privileges sufficient to run containers with `--device /dev/fuse`, `--cap-add SYS_ADMIN`, and `--security-opt apparmor:unconfined`. `check-dependent-safe-mode` and `check-final-hardening-full` must assert those prerequisites in their command blocks before building packages or invoking `test-original.sh --implementation safe`; they must fail hard if any assertion fails and must not downgrade HTTPDirFS or any other dependent to a partial matrix.
- The safe source package must preserve `safe/debian/source/format` as `3.0 (quilt)` and must keep `safe/debian/patches/series` tracked. If no safe-local patches are needed, the file must remain empty or comment-only.
- The original Debian patch stack under `original/debian/patches/` remains a reference input for behavior and tests only. Detached safe package builds must never use it as the active patch stack.
- Generated safe metadata and vendor manifests must use repository-relative provenance paths. Do not embed local absolute paths such as `/home/<user>/...` in `safe/metadata/*` or `safe/vendor/upstream/manifest.json`.
- Final package and installed-runtime verifiers must inspect the built `.deb` payloads and installed ELF artifacts, not only source files. They must fail if any package payload contains `libcurl-reference`, `.reference`, `reference_library_path`, `port_safe_resolve_reference_symbol`, `bridge_resolve_symbol`, or related transitional sidecar resolver names. Package payload audits must also inspect `dpkg-deb --contents` output and extracted symlinks, reject package listing paths or symlink targets containing local checkout or staging path patterns, and then run `readelf -Wd` on packaged and installed `curl`, `libcurl.so.4*`, `libcurl-gnutls.so.4*`, `libcurl-gnutls.so.3`, and development linker symlink targets. They must fail on `NEEDED`, `RPATH`, or `RUNPATH` entries, package paths, or symlink targets containing `libcurl-reference`, `.reference`, `/home/`, `../original`, `/original`, `safe/target`, `target/`, `.compat`, `debian/build`, `/tmp/`, `/var/tmp/`, or any other local checkout, reference-build, or staging path. Phase 9 must provide one tracked package-payload verifier, `safe/scripts/verify-package-payload-contract.py`, and `check-package-build`, `check-packaged-autopkgtests`, `check-dependent-safe-mode`, and `check-final-hardening-full` must all call that same script for every freshly built `.deb` directory before installing or consuming those packages.
- Phase 9 verifiers must also enforce sidecar-free package-consuming paths, not only sidecar-free package payloads. `check-package-build`, `check-packaged-autopkgtests`, and `check-dependent-safe-mode` must scan the live package-consuming scripts, live Debian autopkgtest files under `safe/debian/tests/`, and their detached safe-only or root-harness exports for `libcurl-reference`, `.reference`, `reference_library_path`, `reference_root`, `reference_config`, `reference_curl_config`, `reference_tests_config`, `port_safe_resolve_reference_symbol`, `bridge_resolve_symbol`, `bridge_open_reference`, `REFERENCE_LIBRARY`, `run_reference_build`, `build-reference-curl`, `forwarders.c`, `reference_backend`, `reference backend`, and `libcurl reference`. These source scans must exclude only explicitly benchmark-only original-baseline helpers such as `safe/scripts/build-reference-curl.sh` and `safe/scripts/benchmark-local.sh`, plus verifier/audit scripts named `safe/scripts/verify-*` or `scripts/verify-*` when the matched strings are detector constants. The verifier exception is text-only: it does not authorize any verify script or other path to build, load, link, stage, copy, record, or otherwise consume a reference sidecar. It also does not apply to generated outputs, package payloads, installed files, detached runtime artifacts, or harness logs. Those same Phase 9 checks must assert after public ABI smoke, compatibility staging, upstream safe tests, port tests, validator execution, packaged autopkgtests, packaged runtime smoke, and dependent safe-mode execution that no detached safe-only export, detached root-harness export, `safe/.compat/*/build-state.json`, runtime staging directory, `dist/`, or `.work/validation` output contains a `.reference` directory, a `libcurl-reference-*` file, reference-backend placeholder, or sidecar resolver marker text. Because Phase 9 scans whole detached source exports with this marker set, Phase 9 owns all non-benchmark source cleanup for those markers; Phase 10 must not be assigned primary deletion of files or call sites that Phase 9 scanners would reject.
- Broad object-link coverage is derived from `safe/metadata/test-manifest.json`, not from a hand-maintained count. The final `safe/compat/link-manifest.json` must include every `compatibility_build.targets[]` entry whose `role` is `libcurl-consumer`, plus the `src:curl` tool target, and must exclude every `helper` target. This explicitly includes `http-client:ws-data` and `http-client:ws-pingpong` as disabled-WebSocket object relink/runtime entries, and it explicitly includes `src:curl`. Deriving the target set from the current test manifest produces 263 final link entries: 262 libcurl consumers plus `src:curl`; the currently checked-in `safe/compat/link-manifest.json` has 260 entries and must be expanded. `src:curl` is already present and must remain present; the missing `libtest:libauthretry`, `libtest:libntlmconnect`, and `libtest:libprereq` coverage must be added to `all-objects` with working relink/runtime adapters, and they must not be silently omitted.
- The workflow must treat these as canonical existing inputs:
  - `original/include/curl/*.h`
  - `original/libcurl.def`
  - `original/lib/libcurl.vers.in`
  - `original/lib/Makefile.inc`
  - `original/lib/curl_config.h.in`
  - `original/configure`
  - `original/configure.ac`
  - `original/src/Makefile.am`
  - `original/src/Makefile.inc`
  - tracked files under `original/src/`
  - `original/curl-config.in`
  - `original/libcurl.pc.in`
  - `original/tests/config.in`
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
  - tracked files under `original/.pc/90_gnutls.patch/`
  - `original/tests/runtests.pl`
  - `original/tests/data/Makefile.inc`
  - `original/tests/data/DISABLED`
  - `original/tests/data/test*`
  - tracked files under `original/tests/libtest/`
  - tracked files under `original/tests/server/`
  - tracked files under `original/tests/unit/`
  - tracked files under `original/tests/http/`
  - `dependents.json`
  - `relevant_cves.json`
  - `all_cves.json`
  - `test-original.sh`
  - existing tracked artifacts under `safe/`, including manifests, scripts, tests, Rust modules, copied headers, and Debian metadata
- The generated workflow order must be:
  1. `impl-foundation-refresh`
  2. `check-foundation-manifests`
  3. `check-foundation-build`
  4. `impl-public-abi`
  5. `check-public-abi`
  6. `check-public-layout`
  7. `impl-harness-foundation`
  8. `check-harness-vendor-export`
  9. `check-harness-consumer-build`
  10. `impl-transfer-core`
  11. `check-transfer-core`
  12. `check-transfer-link-smoke`
  13. `impl-http-security`
  14. `check-http-security-cves`
  15. `check-http-security-upstream`
  16. `impl-backends-protocols`
  17. `check-backends-http-clients`
  18. `check-backends-protocol-matrix`
  19. `impl-unit-port-link`
  20. `check-unit-port`
  21. `check-link-compat-broad`
  22. `impl-performance`
  23. `check-performance`
  24. `impl-packaging`
  25. `check-package-build`
  26. `check-packaged-autopkgtests`
  27. `check-dependent-safe-mode`
  28. `impl-final-hardening`
  29. `check-final-hardening-full`

## Implementation Phases

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

- All canonical inputs listed in the workflow contract.
- Existing `safe/Cargo.toml`, `safe/build.rs`, `safe/src/lib.rs`, `safe/src/abi/generated.rs`, `safe/src/abi/public_types.rs`, `safe/include/curl/*.h`, `safe/abi/*.map`, `safe/metadata/*.json`, `safe/scripts/generate-*.py`, `safe/scripts/verify-*.sh`, `safe/scripts/verify-cve-coverage.py`, `safe/c_shim/*.c`, `safe/debian/*`, and `safe/vendor/upstream/manifest.json`.

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

### 2. Public ABI, Allocator Contract, Opaque Handles, Varargs, URL, MIME/Form, and Version APIs

**Phase Name:** Public ABI, Allocator Contract, Opaque Handles, Varargs, URL, MIME/Form, and Version APIs

**Implement Phase ID:** `impl-public-abi`

**Verification Phases:**

- `check-public-abi`
  - Type: `check`
  - Fixed `bounce_target`: `impl-public-abi`
  - Purpose: confirm public APIs compile and execute through the safe shared library for both flavors.
  - Commands:
    ```bash
    CARGO_TARGET_DIR=safe/target/check-public-abi-openssl \
      cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test public_abi
    CARGO_TARGET_DIR=safe/target/check-public-abi-gnutls \
      cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test public_abi
    bash safe/scripts/run-public-abi-smoke.sh --flavor openssl
    bash safe/scripts/run-public-abi-smoke.sh --flavor gnutls
    ```

- `check-public-layout`
  - Type: `check`
  - Fixed `bounce_target`: `impl-public-abi`
  - Purpose: verify public struct layout, option table values, and symbol surfaces match original headers and Debian symbols.
  - Commands:
    ```bash
    CARGO_TARGET_DIR=safe/target/check-public-layout-openssl \
      cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test abi_layout
    CARGO_TARGET_DIR=safe/target/check-public-layout-openssl \
      cargo build --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor
    CARGO_TARGET_DIR=safe/target/check-public-layout-gnutls \
      cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test abi_layout
    CARGO_TARGET_DIR=safe/target/check-public-layout-gnutls \
      cargo build --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor
    bash safe/scripts/verify-abi-manifest.sh safe/metadata/abi-manifest.json
    bash safe/scripts/verify-public-headers.sh --expected original/include/curl --actual safe/include/curl
    bash safe/scripts/verify-export-names.sh --expected original/libcurl.def --flavor openssl \
      --artifact safe/target/check-public-layout-openssl/debug/libport_libcurl_safe.so
    bash safe/scripts/verify-export-names.sh --expected original/libcurl.def --flavor gnutls \
      --artifact safe/target/check-public-layout-gnutls/debug/libport_libcurl_safe.so
    ```

**Preexisting Inputs:**

- Phase 1 outputs.
- `original/lib/easy.c`, `original/lib/version.c`, `original/lib/setopt.c`, `original/lib/getinfo.c`, `original/lib/easyoptions.c`, `original/lib/easygetopt.c`, `original/lib/urlapi.c`, `original/lib/share.c`, `original/lib/mime.c`, `original/lib/formdata.c`, `original/lib/strerror.c`, `original/lib/mprintf.c`.
- Existing `safe/src/alloc.rs`, `safe/src/global.rs`, `safe/src/version.rs`, `safe/src/slist.rs`, `safe/src/share.rs`, `safe/src/urlapi.rs`, `safe/src/mime.rs`, `safe/src/form.rs`, `safe/src/easy/*`, `safe/c_shim/variadic.c`, `safe/c_shim/mprintf.c`, `safe/tests/public_abi.rs`, `safe/tests/abi_layout.rs`, and `safe/tests/smoke/public_api_smoke.c`.

**New Outputs:**

- Hardened Rust implementations for all non-transport public API families.
- Updated typed varargs dispatch helpers in Rust and C.
- Updated tests proving allocator, handle, MIME/form, URL, share, version, strerror, getdate/getenv, and option-table behavior.

**File Changes:**

- `safe/src/alloc.rs`: ensure every public allocation returned to C flows through the runtime-switchable allocator facade and `curl_free`.
- `safe/src/global.rs`: preserve libcurl global init depth, `curl_global_init_mem` one-time allocator semantics, global cleanup, and `curl_global_sslset` timing rules.
- `safe/src/version.rs`: align `curl_version` and `curl_version_info` fields with the selected flavor and packaged feature set.
- `safe/src/easy/handle.rs`, `safe/src/easy/options.rs`, `safe/src/easy/perform.rs`: complete easy lifecycle, `setopt`, `getinfo`, reset, duplicate, escape/unescape, and option iteration.
- `safe/src/share.rs`: complete share-handle locks, user data, share/unshare semantics, and cleanup.
- `safe/src/urlapi.rs`: complete URL parser/render behavior for all `CURLUPart` values and flags, including default scheme/port, append query, percent encode/decode, punycode, IPv6 zones, and error codes.
- `safe/src/mime.rs` and `safe/src/form.rs`: preserve MIME and legacy form ownership, callback, nested part, header ownership, `curl_formget`, and `curl_formfree` behavior.
- `safe/c_shim/variadic.c`: keep only the permanent varargs boundary and route to typed Rust functions.
- `safe/c_shim/mprintf.c`: either keep a small permanent C implementation that uses the safe allocator or replace it with a proven Rust-compatible varargs boundary.
- `safe/tests/public_abi.rs`, `safe/tests/abi_layout.rs`, `safe/tests/smoke/public_api_smoke.c`: add missing cases for public families and custom allocator ownership.

**Implementation Details:**

- Public handles remain opaque to C. Rust can use internal wrapper structs, but it must never expose layout through the ABI.
- `curl_easy_setopt`, `curl_easy_getinfo`, `curl_multi_setopt`, `curl_share_setopt`, and `curl_formadd` must remain C variadic entrypoints because Rust cannot directly implement C varargs in stable Rust.
- `curl_global_init_mem` must reject missing callbacks, ignore later allocator changes after initialization begins, and ensure the reference bridge, if still present in this phase, receives the same allocator snapshot.
- `curl_multi_get_handles` must allocate the returned `CURL **` array with the safe allocator.
- `curl_easyoption` values must be generated from headers and manifest data so alias and type flags remain stable.
- All exported `curl_*` symbols must be versioned in `CURL_OPENSSL_4` or `CURL_GNUTLS_3`.
- Implementer must commit this phase's work before yielding.

**Verification:**

- The two public ABI check phases are required before later harness phases consume the safe library.

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

- Phase 2 outputs.
- `safe/scripts/compat_harness.py`, `safe/scripts/vendor-compat-assets.sh`, `safe/scripts/export-tracked-tree.sh`, `safe/scripts/build-compat-consumers.sh`, `safe/scripts/run-upstream-tests.sh`, `safe/scripts/run-curated-libtests.sh`, `safe/scripts/run-link-compat.sh`, `safe/scripts/run-curl-tool-smoke.sh`, `safe/scripts/run-http-client-tests.sh`, `safe/scripts/run-ldap-devpkg-test.sh`, `safe/scripts/http-fixture.py`, `safe/scripts/http-fixtures.sh`.
- `safe/vendor/upstream/manifest.json` and all listed vendored files.
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

### 4. Easy/Multi Transfer Core, DNS, Connection Cache, Connect-Only, Callbacks, and Basic Protocol Routing

**Phase Name:** Easy/Multi Transfer Core, DNS, Connection Cache, Connect-Only, Callbacks, and Basic Protocol Routing

**Implement Phase ID:** `impl-transfer-core`

**Verification Phases:**

- `check-transfer-core`
  - Type: `check`
  - Fixed `bounce_target`: `impl-transfer-core`
  - Purpose: verify the safe easy and multi transfer loop for basic HTTP/1.1, file, callbacks, timeout, connect-only, and selected upstream libtests.
  - Commands:
    ```bash
    CARGO_TARGET_DIR=safe/target/check-transfer-openssl \
      cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test public_abi
    CARGO_TARGET_DIR=safe/target/check-transfer-gnutls \
      cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test public_abi
    bash safe/scripts/run-curated-libtests.sh --flavor openssl --tests 500 501 502 504 509 510 511 512 518 519 520 521 523 524
    bash safe/scripts/run-curated-libtests.sh --flavor gnutls --tests 500 501 502 504 509 510 511 512 518 519 520 521 523 524
    ```

- `check-transfer-link-smoke`
  - Type: `check`
  - Fixed `bounce_target`: `impl-transfer-core`
  - Purpose: prove objects compiled against the original libcurl link lines relink and run against safe for representative easy/multi transfer tests.
  - Commands:
    ```bash
    bash safe/scripts/run-link-compat.sh --flavor openssl --target lib500 --target lib526 --target lib547 --target lib670 --target curl
    bash safe/scripts/run-link-compat.sh --flavor gnutls --target lib500 --target lib526 --target lib547 --target lib670 --target curl
    ```

**Preexisting Inputs:**

- Phase 3 outputs.
- `original/lib/easy.c`, `original/lib/multi.c`, `original/lib/transfer.c`, `original/lib/url.c`, `original/lib/connect.c`, `original/lib/conncache.c`, `original/lib/cfilters.h`, `original/lib/hostip*.c`, `original/lib/select.c`, `original/lib/progress.c`, `original/lib/speedcheck.c`, `original/lib/file.c`.
- Existing `safe/src/easy/perform.rs`, `safe/src/multi/mod.rs`, `safe/src/multi/state.rs`, `safe/src/multi/poll.rs`, `safe/src/transfer/mod.rs`, `safe/src/dns/mod.rs`, `safe/src/conn/cache.rs`, `safe/src/conn/filter.rs`, `safe/src/protocols/mod.rs`, `safe/src/protocols/file.rs`, and `safe/src/abi/connect_only.rs`.

**New Outputs:**

- Completed native transfer planning and execution for easy and multi HTTP/1.1 plus file/local protocol paths required by early libtests.
- Reduced reference-backed fallback use for easy/multi transfer paths.
- Tests for callbacks, progress, low-speed, timeout, pause, connect-only send/recv, socket callbacks, timers, wakeup, and multi info messages.

**File Changes:**

- `safe/src/easy/perform.rs`: track option state, callbacks, error buffers, read/write/header callbacks, request metadata, and transfer info.
- `safe/src/transfer/mod.rs`: implement URL parsing, DNS resolution, connection setup, request execution, response parsing, callbacks, body upload/download, ranges, redirects where not HTTP-security-specific, low-speed timeouts, connect-only sessions, and `CURLINFO_*`.
- `safe/src/multi/mod.rs`: implement add/remove, scheduling, worker lifecycle, fdset/wait/poll/wakeup, socket/timer callbacks, info queue, cleanup, and connection cache handoff.
- `safe/src/dns/mod.rs`: implement `CURLOPT_RESOLVE`, `CURLOPT_CONNECT_TO`, IPv4/IPv6, DNS ownership, and error mapping.
- `safe/src/conn/cache.rs`: implement cache keys that isolate scheme, host, port, proxy, TLS/backend, credentials, and share state.
- `safe/src/protocols/file.rs`: complete local file transfer compatibility for upload/download cases.

**Implementation Details:**

- Native transfer code must not expose Rust panics across C ABI. Convert internal errors to libcurl `CURLcode` or `CURLMcode` and write error buffer text where libcurl would.
- Multi callbacks must prevent recursive API misuse and preserve message ordering.
- Connection reuse must be conservative: if a reuse key is uncertain, create a fresh connection rather than leaking credentials or state.
- Connect-only mode must honor pause/unpause and `CURLE_AGAIN`.
- Implementer must commit this phase's work before yielding.

**Verification:**

- Run both transfer checks. Any failure must be fixed in this phase, not deferred to HTTP/security phases unless the failing test is explicitly HTTP-policy-specific.

### 5. HTTP Semantics, Security Policies, CVE Regression Matrix, Headers API, Cookies, HSTS, ALTSVC, Auth, Proxies, and WebSocket Disabled ABI

**Phase Name:** HTTP Semantics, Security Policies, CVE Regression Matrix, Headers API, Cookies, HSTS, ALTSVC, Auth, Proxies, and WebSocket Disabled ABI

**Implement Phase ID:** `impl-http-security`

**Verification Phases:**

- `check-http-security-cves`
  - Type: `check`
  - Fixed `bounce_target`: `impl-http-security`
  - Purpose: verify all curated CVEs have case mappings and run the native Rust CVE regression suite for both flavors.
  - Commands:
    ```bash
    python3 safe/scripts/verify-cve-coverage.py
    CARGO_TARGET_DIR=safe/target/check-http-security-openssl \
      cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test cve_regressions
    CARGO_TARGET_DIR=safe/target/check-http-security-gnutls \
      cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test cve_regressions
    ```

- `check-http-security-upstream`
  - Type: `check`
  - Fixed `bounce_target`: `impl-http-security`
  - Purpose: run upstream libtests that exercise HTTP redirects, proxy auth, cookies, HSTS, headers, credentials, content decoding, and WebSocket disabled-build ABI behavior.
  - Commands:
    ```bash
    bash safe/scripts/run-curated-libtests.sh --flavor openssl --tests 555 560 567 571 574 578 586 589 597 1500 1501 1502 1520 1521 1527 1530 1531 1533 1550 1553 1556 1557 1559 1565 1567 1591 1593 1596 1600 1601 1602 1603 1604 1605 1606 1607 1608 1609 1610 1611 1612 1614 1620 1621 1650 1651 1652 1653 1654 1655 1656 1660 1661
    bash safe/scripts/run-curated-libtests.sh --flavor gnutls --tests 555 560 567 571 574 578 586 589 597 1500 1501 1502 1520 1521 1527 1530 1531 1533 1550 1553 1556 1557 1559 1565 1567 1591 1593 1596 1600 1601 1602 1603 1604 1605 1606 1607 1608 1609 1610 1611 1612 1620 1621 1650 1651 1652 1653 1654 1655 1656 1660 1661
    ```

**Preexisting Inputs:**

- Phase 4 outputs.
- `relevant_cves.json`, `all_cves.json`, `original/debian/patches/CVE-*.patch`, `safe/metadata/cve-manifest.json`, `safe/metadata/cve-to-test.json`, and `safe/tests/cve_cases/*.json`.
- `original/lib/http.c`, `original/lib/http1.c`, `original/lib/headers.c`, `original/lib/cookie.c`, `original/lib/hsts.c`, `original/lib/altsvc.c`, `original/lib/http_proxy.c`, `original/lib/http_digest.c`, `original/lib/http_ntlm.c`, `original/lib/http_aws_sigv4.c`, `original/lib/content_encoding.c`, `original/lib/ws.c`.
- Existing `safe/src/http/*.rs`, `safe/src/ws.rs`, `safe/src/rand.rs`, `safe/tests/cve_regressions.rs`.

**New Outputs:**

- Completed native HTTP/1.1 policy implementation.
- Expanded CVE case files and runtime tests where current shared reference-backend mappings are too broad.
- Ubuntu-compatible disabled WebSocket ABI behavior: `curl_ws_recv` and `curl_ws_send` return `CURLE_NOT_BUILT_IN`, `curl_ws_meta` returns `NULL`, and `ws://`/`wss://` are not routed or advertised.

**File Changes:**

- `safe/src/http/request.rs`: redirects, referer stripping, credential isolation, method rewriting, body/retry rules, proxy tunnel request targets.
- `safe/src/http/response.rs`: response/header limits, 1xx/CONNECT/trailer origins, content-length/range/chunked parsing.
- `safe/src/http/auth.rs`: Basic, Digest, NTLM/Negotiate boundaries, Bearer, AWS SigV4, and proxy auth reuse isolation.
- `safe/src/http/proxy.rs`: HTTP proxy tunneling, proxy headers, proxy credential separation, no-proxy matching.
- `safe/src/http/cookies.rs`: Netscape cookie jar loading/saving, PSL rejection, host-only/domain/path/secure matching, session clearing, and Set-Cookie parsing.
- `safe/src/http/hsts.rs`: HSTS file/callback handling, includeSubDomains, IP exclusion, expiry, persistence, and lookup.
- `safe/src/http/altsvc.rs`: ALTSVC persistence, host isolation, HSTS interaction, and origin constraints.
- `safe/src/http/headers_api.rs`: `curl_easy_header` and `curl_easy_nextheader` semantics.
- `safe/src/ws.rs`: preserve exported `curl_ws_meta`, `curl_ws_recv`, and `curl_ws_send` ABI symbols while matching Ubuntu's disabled-WebSocket build: recv/send return `CURLE_NOT_BUILT_IN` for all inputs and meta returns `NULL`.
- `safe/src/rand.rs`: cryptographic randomness for HTTP auth, TLS-adjacent nonce material, and any enabled third-party boundary that needs randomness, with deterministic test injection only behind test-only code. WebSocket mask-generation paths must not be reachable while `USE_WEBSOCKETS` is disabled.
- `safe/tests/cve_regressions.rs` and `safe/tests/cve_cases/*.json`: map every curated CVE to an executable or justified shared case.

**Implementation Details:**

- Rust memory safety is not enough for this phase. Each CVE class in `relevant_cves.json` must be mapped to explicit behavior: certificate/transport validation, credential leakage, redirects, cookies, HSTS, connection reuse, parser canonicalization, randomness, platform loading, resource limits, and API contracts.
- Any case currently labeled as `reference_backend_*` must either move to a native test when the native code owns the behavior or retain a written temporary justification that the behavior is still delegated in this phase. Phase 9 must remove any remaining reference-backend labels and replace unavoidable third-party coverage with boundary-specific non-reference case names, because Phase 9 scans whole detached source exports for these markers.
- WebSocket CVE cases must be rewritten around the disabled Ubuntu contract unless the package contract is intentionally changed to enable WebSockets. The disabled-contract tests must prove the ABI symbols remain exported, `curl_ws_recv`/`curl_ws_send` return `CURLE_NOT_BUILT_IN`, `curl_ws_meta` returns `NULL`, `ws://`/`wss://` are not in protocol/tooling lists, and no WebSocket handshake or mask-generation path is reachable.
- Response header memory must be bounded. Exceeding configured limits must produce libcurl-compatible errors rather than allocation growth.
- Auth and cookies must never leak across origins, redirects, proxies, or reused connections unless the corresponding libcurl option explicitly allows it.
- Implementer must commit this phase's work before yielding.

**Verification:**

- Both check phases must pass for both flavors.

### 6. TLS Backends, HTTP/2, SSH, Remaining Protocols, and Tracked HTTP Client Programs

**Phase Name:** TLS Backends, HTTP/2, SSH, Remaining Protocols, and Tracked HTTP Client Programs

**Implement Phase ID:** `impl-backends-protocols`

**Verification Phases:**

- `check-backends-http-clients`
  - Type: `check`
  - Fixed `bounce_target`: `impl-backends-protocols`
  - Purpose: run the 5 tracked non-WebSocket HTTP/TLS client programs against safe libraries and local fixtures, then prove the 2 tracked WebSocket client programs compile and take the original disabled-WebSocket path.
  - Commands:
    ```bash
    command -v nghttpx >/dev/null
    bash safe/scripts/run-http-client-tests.sh --flavor openssl --clients h2-download h2-pausing h2-serverpush h2-upgrade-extreme tls-session-reuse
    bash safe/scripts/run-http-client-tests.sh --flavor gnutls --clients h2-download h2-pausing h2-serverpush h2-upgrade-extreme tls-session-reuse
    bash safe/scripts/run-websocket-disabled-smoke.sh --flavor openssl --clients ws-data ws-pingpong
    bash safe/scripts/run-websocket-disabled-smoke.sh --flavor gnutls --clients ws-data ws-pingpong
    ```

- `check-backends-protocol-matrix`
  - Type: `check`
  - Fixed `bounce_target`: `impl-backends-protocols`
  - Purpose: exercise TLS, HTTP/2, SSH, RTMP, `gophers`, LDAPS, and all other Ubuntu-advertised non-HTTP protocols through contract probes, functional RTMP/LDAPS fixtures, upstream libtests, and CVE regressions.
  - Commands:
    ```bash
    CARGO_TARGET_DIR=safe/target/check-backends-openssl \
      cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test cve_regressions
    CARGO_TARGET_DIR=safe/target/check-backends-openssl \
      cargo build --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor
    CARGO_TARGET_DIR=safe/target/check-backends-gnutls \
      cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test cve_regressions
    CARGO_TARGET_DIR=safe/target/check-backends-gnutls \
      cargo build --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor
    python3 safe/scripts/verify-protocol-feature-contract.py \
      --flavor openssl \
      --artifact safe/target/check-backends-openssl/debug/libport_libcurl_safe.so \
      --assert-routing
    python3 safe/scripts/verify-protocol-feature-contract.py \
      --flavor gnutls \
      --artifact safe/target/check-backends-gnutls/debug/libport_libcurl_safe.so \
      --assert-routing
    command -v python3 >/dev/null
    command -v cc >/dev/null
    command -v openssl >/dev/null
    command -v pkgconf >/dev/null
    command -v ldapsearch >/dev/null
    test -x /usr/sbin/slapd || command -v slapd >/dev/null
    pkgconf --exists ldap
    bash safe/scripts/run-rtmp-functional-tests.sh \
      --flavor openssl \
      --artifact safe/target/check-backends-openssl/debug/libport_libcurl_safe.so \
      --schemes rtmp,rtmpe,rtmps,rtmpt,rtmpte,rtmpts \
      --require-download \
      --require-upload
    bash safe/scripts/run-rtmp-functional-tests.sh \
      --flavor gnutls \
      --artifact safe/target/check-backends-gnutls/debug/libport_libcurl_safe.so \
      --schemes rtmp,rtmpe,rtmps,rtmpt,rtmpte,rtmpts \
      --require-download \
      --require-upload
    bash safe/scripts/run-ldaps-functional-test.sh \
      --flavor openssl \
      --artifact safe/target/check-backends-openssl/debug/libport_libcurl_safe.so
    bash safe/scripts/run-ldaps-functional-test.sh \
      --flavor gnutls \
      --artifact safe/target/check-backends-gnutls/debug/libport_libcurl_safe.so
    bash safe/scripts/run-curated-libtests.sh --flavor openssl --tests 3010 3026 3100 3101 3102 3200 1900 1903 1906 1907 1911 1915 1916 1919 1934 1935 1936 1937 1938 1947 1948 1955 1956 1957 1958 1964 1970 1971 1972 1974 1975 2302 2304 2404 2502 2600 2601 2602 2603
    bash safe/scripts/run-curated-libtests.sh --flavor gnutls --tests 3010 3026 3100 3101 3102 3200 1900 1903 1906 1907 1911 1915 1916 1919 1934 1935 1936 1937 1938 1947 1948 1955 1956 1957 1958 1964 1970 1971 1972 1974 1975 2302 2304 2404 2502 2600 2601 2602 2603
    ```

**Preexisting Inputs:**

- Phase 5 outputs.
- `original/lib/version.c`, `original/lib/url.c`, `original/lib/urldata.h`, `original/debian/rules`, and `original/debian/control` for the exact Ubuntu protocol, feature, backend, and dependency contract.
- `original/lib/vtls/*.c`, `original/lib/vssh/*.c`, `original/lib/vquic/*.c`, `original/lib/http2.c`, `original/lib/doh.c`, `original/lib/idn.c`, `original/lib/content_encoding.c`, and protocol implementations for file, FTP, IMAP, POP3, SMTP, LDAP, SMB, TELNET, TFTP, DICT, GOPHER, RTSP, MQTT, and RTMP.
- `original/lib/gopher.c`, `original/lib/gopher.h`, `original/lib/curl_rtmp.c`, and `original/lib/curl_rtmp.h`.
- Existing `safe/src/version.rs`, `safe/src/tls/*.rs`, `safe/src/ssh/mod.rs`, `safe/src/vquic/mod.rs`, `safe/src/doh.rs`, `safe/src/idn.rs`, `safe/src/protocols/*.rs`, `safe/c_shim/http2_transport.c`, `safe/c_shim/tls_backend.c`, `safe/c_shim/ssh_backend.c`.

**New Outputs:**

- Completed flavor-correct TLS support for OpenSSL and GnuTLS.
- HTTP/2 behavior sufficient for tracked non-WebSocket clients, plus disabled-WebSocket compile/runtime smoke coverage for `ws-data` and `ws-pingpong`.
- Native or explicitly justified protocol engines for the full Ubuntu-advertised protocol list, including `gophers`, `ldap`, `ldaps`, and all six RTMP URL schemes.
- `safe/src/protocols/rtmp.rs` or an equivalent module implementing RTMP-family routing through a justified `librtmp` FFI boundary.
- `safe/scripts/verify-protocol-feature-contract.py`, which validates `curl_version_info`, feature bits/names, advertised protocol names, default routing, and registration/routing probes for both flavors without using sibling `original/`; it is complemented by the functional RTMP and LDAPS fixtures for advertised protocols that cannot be proven by routing alone.
- `safe/scripts/rtmp-fixture.py` and `safe/scripts/run-rtmp-functional-tests.sh`, which provide deterministic local functional coverage for every advertised RTMP URL scheme without external network access.
- `safe/scripts/run-ldaps-functional-test.sh`, which starts a local TLS-enabled LDAP fixture and verifies successful `ldaps://` query behavior plus certificate/CA failure modes for artifact and installed-package modes.
- Reduced transport sidecar dependency.

**File Changes:**

- `safe/src/tls/openssl.rs` and `safe/src/tls/gnutls.rs`: certificate verification, CA path/bundle/blob, hostname, pinned public keys, ALPN, certinfo, session reuse, and error mapping.
- `safe/src/tls/certinfo.rs`: public certinfo list allocation and lifetime.
- `safe/src/version.rs`: expose the exact Ubuntu-derived protocol and feature contract for the active flavor. `curl_version_info_data.protocols` must list `dict`, `file`, `ftp`, `ftps`, `gopher`, `gophers`, `http`, `https`, `imap`, `imaps`, `ldap`, `ldaps`, `mqtt`, `pop3`, `pop3s`, `rtmp`, `rtmpe`, `rtmps`, `rtmpt`, `rtmpte`, `rtmpts`, `rtsp`, `scp`, `sftp`, `smb`, `smbs`, `smtp`, `smtps`, `telnet`, and `tftp`. `feature_names` and the `features` bitmask must include `alt-svc`, `AsynchDNS`, `brotli`, `GSS-API`, `HSTS`, `HTTP2`, `HTTPS-proxy`, `IDN`, `IPv6`, `Kerberos`, `Largefile`, `libz`, `NTLM`, `PSL`, `SPNEGO`, `SSL`, `threadsafe`, `TLS-SRP`, `UnixSockets`, and `zstd`, with version fields populated for zlib, brotli, zstd, libidn2, nghttp2, libpsl, the selected TLS backend, and SSH.
- `safe/src/ssh/mod.rs`: SCP/SFTP auth and transport semantics through Ubuntu's selected `libssh` backend, matching `original/debian/rules`, unless a documented verifier-backed substitute preserves the same public `curl_version_info`, `curl-config`, `libcurl.pc`, static link, and runtime behavior.
- `safe/src/vquic/mod.rs`: QUIC option surface and clear not-built-in behavior unless native support is implemented.
- `safe/src/doh.rs`: DNS-over-HTTPS option behavior and safe fallback.
- `safe/src/idn.rs`: IDNA conversion consistent with libcurl 8.5.0 behavior.
- `safe/src/http/response.rs` or a dedicated content-decoding module: implement gzip/deflate, brotli, and zstd decoding consistently with the advertised `libz`, `brotli`, and `zstd` features.
- `safe/src/http/cookies.rs` and `safe/src/idn.rs`: use or wrap libpsl and libidn2 behavior so the advertised `PSL` and `IDN` features are true at runtime.
- `safe/src/protocols/mod.rs`: route every advertised scheme to a concrete handler. `gophers` must route to the Gopher handler with TLS enabled, not to the unknown/unsupported path. `ldap` and `ldaps` must remain advertised and routed. `rtmp`, `rtmpe`, `rtmps`, `rtmpt`, `rtmpte`, and `rtmpts` must route to the RTMP handler with correct default ports and TLS/HTTP-tunnel semantics.
- `safe/src/protocols/mod.rs`: under the Ubuntu-compatible default configuration, do not route `ws` or `wss`; those schemes must behave like the original build without `--enable-websockets` and must not appear in `curl_version_info`, `curl-config`, or `libcurl.pc`.
- `safe/src/protocols/gopher.rs`: accept both `gopher` and `gophers`, use default port 70 for both, and use the transfer TLS path for `gophers`.
- `safe/src/protocols/rtmp.rs`: implement the RTMP family through `librtmp` or a native Rust implementation. The initial acceptable boundary is `librtmp` FFI that validates URLs, pointers, buffer lengths, callbacks, and ownership; maps `RTMP_SetupURL`, `RTMP_Connect1`, `RTMP_ConnectStream`, `RTMP_Read`, `RTMP_Write`, and close/free failures to libcurl-compatible errors; and supports uploads/downloads sufficiently for `rtmp://`, `rtmpe://`, `rtmps://`, `rtmpt://`, `rtmpte://`, and `rtmpts://`.
- `safe/src/protocols/*.rs`: only protocols omitted by the Ubuntu package, such as HTTP/3/QUIC unless implemented and advertised, may return `CURLE_UNSUPPORTED_PROTOCOL` or `CURLE_NOT_BUILT_IN`. No scheme in the explicit Ubuntu-advertised list may fail with either unsupported/not-built-in from the initial routing/protocol-selection layer.
- `safe/tests/public_abi.rs` and any version/protocol smoke tests: update expected protocol and feature lists to the exact Ubuntu-derived contract rather than the current partial safe inventory.
- `safe/scripts/run-websocket-disabled-smoke.sh`: build the tracked `ws-data` and `ws-pingpong` clients from vendored upstream sources using `safe/compat/config/<flavor>/`, run them with no arguments or dummy arguments as needed, require a non-zero exit, and require stderr to contain the original `websockets not enabled in libcurl` diagnostic. The script must fail if the clients enter their functional WebSocket branch, if `USE_WEBSOCKETS` is defined, or if `curl-config --protocols` advertises `WS` or `WSS`.
- `safe/scripts/rtmp-fixture.py`: implement a local deterministic RTMP/RTMPT server sufficient for the advertised libcurl contract. It must support the RTMP handshake, AMF command sequence needed for connect/createStream/play and publish or an equivalent write-command path, deterministic sentinel media bytes for download assertions, upload/write capture for byte-for-byte assertions, TLS wrapping for `rtmps`/`rtmpts`, HTTP tunnel endpoints for `rtmpt`/`rtmpte`/`rtmpts`, and encrypted RTMP-family coverage for `rtmpe`/`rtmpte` without delegating success to an unsupported-protocol or connection-failure path.
- `safe/scripts/run-rtmp-functional-tests.sh`: compile a small C probe against either `--artifact <safe shared library>` or installed `libcurl`, run it against `safe/scripts/rtmp-fixture.py`, require success for all six schemes, assert that at least one download/read and at least one upload/write or publish-command path are observed by the fixture, verify the TLS and HTTP-tunnel fixture logs for their respective schemes, and fail on `CURLE_UNSUPPORTED_PROTOCOL`, `CURLE_NOT_BUILT_IN`, early connection failure, timeout-only success, or missing fixture observations.
- `safe/scripts/run-ldaps-functional-test.sh`: compile a small C probe against either `--artifact <safe shared library>` or installed `libcurl`, start an isolated local OpenLDAP `slapd` instance on high ports with temporary database, known entries, and a tracked or generated local CA/server certificate, then run `ldaps://localhost:<port>/...` queries. It must require success when `CURLOPT_CAINFO` trusts the fixture CA, require certificate verification failure when the CA is absent or the hostname/CA is wrong, and record that the query returned the expected LDAP attributes through libcurl's write callback.
- `safe/c_shim/http2_transport.c`, `safe/c_shim/tls_backend.c`, `safe/c_shim/ssh_backend.c`: shrink to unavoidable C-library FFI boundaries or replace with Rust FFI wrappers.
- `safe/scripts/verify-protocol-feature-contract.py`: build or load a small C probe against the selected safe artifact, compare `curl_version_info_data.protocols`, `feature_names`, feature bitmask, version fields, and routing behavior against the hard-coded original-derived Ubuntu contract. With `--assert-routing`, it must call `curl_easy_perform` on every advertised scheme at least far enough to prove the scheme is registered: closed-port network probes for ordinary TCP protocols may accept connection, timeout, DNS, authentication, or protocol-level failures, but must fail the check on `CURLE_UNSUPPORTED_PROTOCOL` or `CURLE_NOT_BUILT_IN`; `file://` must use a temporary local file; `gophers://`, `ldaps://`, and every RTMP variant must be included in this probe set. Passing this routing probe is not sufficient for RTMP-family or LDAPS compatibility; `run-rtmp-functional-tests.sh` and `run-ldaps-functional-test.sh` are mandatory verifier commands.

**Implementation Details:**

- `curl_version_info`, `curl-config --protocols`, and `libcurl.pc:supported_protocols` must agree with actually implemented/advertised protocol support, accounting for the original libcurl distinction that `curl_version_info_data.protocols` lists all six RTMP URL schemes while configure-style tooling surfaces list the aggregate `RTMP` protocol once.
- The advertised protocol set for both flavors is fixed in this plan. It includes `gophers`, `ldap`, `ldaps`, `rtmp`, `rtmpe`, `rtmps`, `rtmpt`, `rtmpte`, and `rtmpts`; omitting any of them is a compatibility failure even if upstream tests do not exercise that route.
- WebSocket ABI functions and tracked HTTP/WebSocket clients remain in scope only as disabled-build compatibility checks. `ws` and `wss` are not part of the Ubuntu-advertised protocol list, must not be routed, and must not be added unless the package configuration is deliberately changed to match an original build with `--enable-websockets` and all protocol, tooling, CVE, client, package, and dev-tooling contracts are updated in the same phase.
- RTMP support must be present because `original/debian/control` Build-Depends on `librtmp-dev`, Ubuntu package descriptions advertise RTMP, `original/lib/version.c` advertises the six RTMP schemes under `USE_LIBRTMP`, and `original/lib/url.c` routes them. Phase 6 must provide functional local RTMP-family coverage; a closed-port RTMP smoke, DNS failure, timeout, or registration-only probe is not sufficient for any advertised RTMP scheme.
- LDAPS support must be present because `original/lib/version.c` advertises `ldaps` when OpenLDAP and TLS are enabled and `original/lib/url.c` routes it. The existing `curl-ldapi-test` Unix-socket LDAP autopkgtest remains required, but it is not LDAPS coverage; Phase 6 must add the local TLS LDAP fixture and prove successful `ldaps://` transfer plus certificate/CA enforcement for both flavors.
- For protocols still delegated to third-party C libraries, unsafe boundaries must validate pointers, lengths, callbacks, and ownership.
- Do not implement HTTP/3/QUIC unless the Ubuntu 24.04 package advertises it; unsupported protocol behavior must match libcurl's configured build.
- Implementer must commit this phase's work before yielding.

**Verification:**

- Both backend checks must pass before broad object relinking.

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

- Phase 6 outputs.
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

### 8. Performance Baseline, Benchmark Harness, and Regression Tuning

**Phase Name:** Performance Baseline, Benchmark Harness, and Regression Tuning

**Implement Phase ID:** `impl-performance`

**Verification Phases:**

- `check-performance`
  - Type: `check`
  - Fixed `bounce_target`: `impl-performance`
  - Purpose: compare safe versus original performance for core scenarios and enforce threshold policy without weakening compatibility or security.
  - Commands:
    ```bash
    command -v nghttpx >/dev/null
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
    ```

**Preexisting Inputs:**

- Phase 7 outputs.
- `safe/benchmarks/scenarios.json`, `safe/benchmarks/thresholds.json`, `safe/benchmarks/harness/easy_loop.c`, `safe/benchmarks/harness/multi_parallel.c`, `safe/scripts/benchmark-local.sh`, `safe/scripts/compare-benchmarks.py`.

**New Outputs:**

- Updated benchmark harness or thresholds only if the current harness is wrong or unstable.
- `safe/docs/performance.md` documenting baseline scenarios, current results, and justified optimizations.
- Performance fixes in transfer, HTTP, TLS, or multi modules.

**File Changes:**

- `safe/benchmarks/scenarios.json`: maintain the core scenarios `easy-http1-reuse`, `multi-http1-parallel`, `h2-download-multiplex`, and `tls-session-reuse`.
- `safe/benchmarks/thresholds.json`: keep threshold values explicit and conservative.
- `safe/scripts/benchmark-local.sh`: ensure original and safe builds use the same local fixtures and link behavior.
- `safe/scripts/compare-benchmarks.py`: produce deterministic pass/fail JSON.
- `safe/docs/performance.md`: record methodology and tradeoffs.

**Implementation Details:**

- Performance work must not remove security checks or broaden connection reuse keys.
- Optimize allocation churn, header parsing, callback buffering, connection pooling, and HTTP/2 scheduling only after compatibility tests are green.
- Implementer must commit this phase's work before yielding.

**Verification:**

- `check-performance` must pass. If a threshold fails, document the exact technical reason and fix it in the same phase before yielding.

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

- Phase 8 outputs.
- `safe/debian/*`, `scripts/build-debs.sh`, `scripts/lib/build-deb-common.sh`, `scripts/install-build-deps.sh`, `scripts/run-upstream-tests.sh`, `scripts/run-port-tests.sh`, `scripts/run-validation-tests.sh`, `packaging/package.env`, `.github/workflows/ci-release.yml`, `test-original.sh`, `dependents.json`.
- Original Debian files listed in the workflow contract as reference inputs.

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

- Phase 9 outputs.
- All safe Rust modules, C shims, harness scripts, metadata, tests, packaging, root CI scripts, `dependents.json`, `relevant_cves.json`, and original reference inputs.

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

## Critical Files

- `safe/Cargo.toml`: crate metadata, flavor features, dependencies, library crate types. It must remain a standard Rust package root.
- `safe/Cargo.lock`: lock dependency versions for reproducible builds if the package policy keeps vendored or locked crates.
- `safe/build.rs`: flavor detection, version-script generation, easy-option generation, permanent C shim compilation, linker args, and Phase 9 removal of reference-sidecar build logic.
- `safe/.cargo/config.toml` and `safe/.cargo/cc-linker-wrapper.sh`: relative linker wrapper for rustc/version-script compatibility plus repository-relative Cargo source replacement for `safe/vendor/cargo`.
- `safe/vendor/cargo/`: checked-in Cargo vendor tree for every crates.io source in `safe/Cargo.lock`.
- `safe/include/curl/*.h`: copied public headers; must stay byte-for-byte aligned with `original/include/curl/*.h`.
- `safe/abi/libcurl-openssl.map` and `safe/abi/libcurl-gnutls.map`: checked-in flavor version scripts generated from Debian symbols.
- `safe/metadata/abi-manifest.json`: ABI source of truth for headers, symbols, structs, constants, options, and flavor library metadata.
- `safe/metadata/test-manifest.json`: upstream test and harness inventory source of truth.
- `safe/metadata/cve-manifest.json` and `safe/metadata/cve-to-test.json`: CVE source of truth and regression mapping.
- `safe/metadata/debian-control-contract.json`: package-control compatibility contract derived from `original/debian/control`.
- `safe/metadata/dev-tooling-contract.json`: source-compatibility contract for installed `curl-config`, `libcurl.pc`, and `libcurl.m4` behavior.
- `safe/src/lib.rs`: module graph, feature gating, and public ABI shim retention.
- `safe/src/abi/generated.rs` and `safe/src/abi/public_types.rs`: target-conditioned public ABI types and constants.
- `safe/src/abi/easy.rs`, `safe/src/abi/multi.rs`, `safe/src/abi/share.rs`, `safe/src/abi/url.rs`, `safe/src/abi/connect_only.rs`: exported ABI entrypoint wrappers.
- `safe/src/alloc.rs`: libcurl allocator contract and `curl_free`.
- `safe/src/global.rs`: global init/cleanup, SSL backend selection, and `curl_multi_get_handles`.
- `safe/src/version.rs`: `curl_version`, `curl_version_info`, feature/protocol reporting.
- `safe/src/easy/handle.rs`, `safe/src/easy/options.rs`, `safe/src/easy/perform.rs`: easy lifecycle, options, callbacks, transfer metadata, and getinfo/setopt.
- `safe/src/easy/reference.rs`: transitional reference-handle registry that Phase 9 deletes; it must remain absent in final hardening.
- `safe/src/multi/mod.rs`, `safe/src/multi/poll.rs`, `safe/src/multi/state.rs`: multi-handle state machine, socket/timer callbacks, wakeup, messages, connection cache integration.
- `safe/src/transfer/mod.rs`: core transfer engine and HTTP/file execution.
- `safe/src/dns/mod.rs`: resolver overrides and connection overrides.
- `safe/src/conn/cache.rs`, `safe/src/conn/filter.rs`: connection identity, reuse, and filter-chain policy.
- `safe/src/http/*.rs`: HTTP request/response/auth/proxy/cookie/HSTS/ALTSVC/header semantics and CVE-sensitive policy.
- `safe/src/ws.rs`: exported WebSocket ABI symbols with Ubuntu-compatible disabled behavior: recv/send return `CURLE_NOT_BUILT_IN`, meta returns `NULL`, and no handshake or mask path is reachable unless the entire package contract is deliberately changed to enable WebSockets.
- `safe/src/rand.rs`: randomness for HTTP auth, TLS-adjacent nonce material, and enabled third-party boundaries; WebSocket mask-generation paths must remain unreachable under the Ubuntu disabled-WebSocket contract.
- `safe/src/tls/*.rs`: OpenSSL/GnuTLS/certinfo backend behavior.
- `safe/src/ssh/mod.rs`, `safe/src/vquic/mod.rs`, `safe/src/doh.rs`, `safe/src/idn.rs`, `safe/src/protocols/*.rs`: remaining backend and protocol surfaces, including `gophers`, `ldap`, `ldaps`, and the six RTMP schemes required by Ubuntu's advertised contract.
- `safe/src/protocols/rtmp.rs`: RTMP-family implementation or `librtmp` FFI boundary for `rtmp`, `rtmpe`, `rtmps`, `rtmpt`, `rtmpte`, and `rtmpts`.
- `safe/src/slist.rs`, `safe/src/mime.rs`, `safe/src/form.rs`, `safe/src/share.rs`, `safe/src/urlapi.rs`: non-transport public API families.
- `safe/c_shim/variadic.c`: permanent C varargs dispatch boundary.
- `safe/c_shim/mprintf.c`: varargs printf family boundary unless replaced with a proven safe alternative.
- `safe/c_shim/forwarders.c`: transitional reference bridge that Phase 9 deletes; it must remain absent in final hardening.
- `safe/c_shim/http2_transport.c`, `safe/c_shim/tls_backend.c`, `safe/c_shim/ssh_backend.c`: transport FFI helpers to minimize or justify.
- `safe/scripts/generate-manifests.py`, `safe/scripts/generate-bindings.py`, `safe/scripts/verify-manifests.py`, `safe/scripts/verify-*.sh`, `safe/scripts/verify-cve-coverage.py`, `safe/scripts/verify-protocol-feature-contract.py`, `safe/scripts/verify-cargo-source-policy.py`, `safe/scripts/verify-debian-control-contract.py`, `safe/scripts/verify-dev-tooling-contract.sh`, `safe/scripts/verify-package-no-refetch.py`, `safe/scripts/verify-package-payload-contract.py`: deterministic generation, validation, protocol/feature, package-control, dev-tooling, package-build source isolation, and built-package payload checks.
- `safe/scripts/compat_harness.py`, `safe/scripts/build-compat-consumers.sh`, `safe/scripts/run-link-compat.sh`, `safe/scripts/run-upstream-tests.sh`, `safe/scripts/run-curated-libtests.sh`, `safe/scripts/run-http-client-tests.sh`, `safe/scripts/run-websocket-disabled-smoke.sh`, `safe/scripts/run-curl-tool-smoke.sh`, `safe/scripts/run-ldap-devpkg-test.sh`, `safe/scripts/run-ldaps-functional-test.sh`, `safe/scripts/run-rtmp-functional-tests.sh`, `safe/scripts/rtmp-fixture.py`, `safe/scripts/http-fixture.py`, `safe/scripts/http-fixtures.sh`: compatibility harness and fixtures.
- `safe/scripts/vendor-compat-assets.sh` and `safe/vendor/upstream/manifest.json`: tracked upstream compatibility asset vendoring.
- `safe/scripts/export-tracked-tree.sh`: detached source export for package and harness checks.
- `safe/compat/config/{openssl,gnutls}/lib/curl_config.h`, `safe/compat/config/{openssl,gnutls}/tests/config`, and `safe/compat/config/{openssl,gnutls}/curl-config`: committed per-flavor configured upstream metadata used only to compile compatibility consumers from the vendored upstream source tree, replacing the transitional `.reference` metadata source.
- `safe/compat/link-manifest.json`: link/object compatibility matrix.
- `safe/tests/public_abi.rs`, `safe/tests/abi_layout.rs`, `safe/tests/unit_port.rs`, `safe/tests/cve_regressions.rs`, `safe/tests/smoke/public_api_smoke.c`, `safe/tests/unit_port_cases/*.json`, `safe/tests/cve_cases/*.json`, `safe/tests/port-map.json`: Rust and C test coverage.
- `safe/benchmarks/*` and `safe/docs/performance.md`: performance scenarios, thresholds, harnesses, and results.
- `safe/docs/unsafe-audit.md`: final unsafe-boundary documentation.
- `safe/debian/control`, `safe/debian/rules`, `safe/debian/changelog`, `safe/debian/copyright`, `safe/debian/source/format`, `safe/debian/patches/series`, `safe/debian/*.install`, `safe/debian/*.links`, `safe/debian/*.symbols`, `safe/debian/tests/*`: Ubuntu 24.04 package source and autopkgtest contract. `safe/debian/changelog` must carry the committed `8.5.0-2ubuntu10.8+safelibs0` baseline used by direct detached package builds.
- `scripts/build-debs.sh`, `scripts/lib/build-deb-common.sh`, `scripts/install-build-deps.sh`, `scripts/run-upstream-tests.sh`, `scripts/run-port-tests.sh`, `scripts/run-validation-tests.sh`, `packaging/package.env`, `.github/workflows/ci-release.yml`: repository CI hooks and SafeLibs validator integration. The root build hook must build binary packages only and leave only `.deb` artifacts in `dist/`.
- `test-original.sh`: original and safe dependent harness. Must preserve original mode and add safe package mode.
- `dependents.json`, `relevant_cves.json`, `all_cves.json`: prepared input artifacts to consume, never regenerate during implementation.

## Final Verification

Final verification is `check-final-hardening-full` in Phase 10. The generated workflow must place it as the final explicit top-level `check` phase with `bounce_target: impl-final-hardening`.

At completion, that checker must prove:

- Layout and generated manifests are current.
- Public headers match original headers.
- All curated CVEs have regression mappings and both flavor CVE suites pass.
- No transitional libcurl reference sidecar or `dlopen` fallback remains in the safe runtime, package, build script, public ABI smoke tests, compatibility config artifact flow, compatibility staging, upstream safe harnesses, link harnesses, Debian autopkgtest files, root CI hooks, validator inputs, detached root-harness exports, or dependent safe-package tests. The final checker scans the live and detached package-consuming scripts, live and detached `safe/debian/tests/*` autopkgtest files, and generated output roots such as `safe/.compat`, `safe/target/public-abi`, `dist`, `.work/validation`, detached package/autopkgtest runtime directories, and `SAFE_MODE_ARTIFACT_ROOT` for `.reference`, `libcurl-reference-*`, `reference_backend`, and resolver marker text. Benchmark-only original-baseline building is the sole allowed reference/original helper use; verifier/audit scripts may contain those strings only as inert detector constants and are explicitly excluded from broad source marker scans.
- Built package payloads, package listing paths, package symlink targets, installed symlink targets, and installed `curl`/`libcurl` ELF artifacts contain no `libcurl-reference`, `.reference`, transitional resolver strings, sidecar `DT_NEEDED` entries, sidecar `RPATH`/`RUNPATH` entries, or dynamic search paths that point at local checkout or staging locations such as `/home/`, `../original`, `/original`, `safe/target`, `target/`, `.compat`, `debian/build`, `/tmp/`, or `/var/tmp/`. Every package-producing checker uses the same `safe/scripts/verify-package-payload-contract.py` payload audit before those packages are installed, copied to `dist/`, validated, or used by the dependent matrix.
- The committed `safe/compat/config/{openssl,gnutls}/` artifacts provide `lib/curl_config.h`, `tests/config`, and executable `curl-config` files without `.reference`, `libcurl-reference`, sibling `original/`, or local absolute paths; compatibility harness scripts consume those artifacts directly and never recover configured metadata from `.reference`.
- `safe/docs/unsafe-audit.md` covers every remaining Rust unsafe/extern/export boundary and every final C shim with machine-checked boundary ids, purpose, invariants, and necessity, and contains no stale entries.
- Both flavor cargo tests and `cargo clippy --all-targets -D warnings` pass.
- Both flavor protocol/feature contract probes pass, including `curl_version_info_data.protocols`, feature names/bits, version fields, and registered-route probes for every advertised scheme. The probes must specifically include `gophers`, `ldap`, `ldaps`, `rtmp`, `rtmpe`, `rtmps`, `rtmpt`, `rtmpte`, and `rtmpts` and must fail if any advertised scheme returns `CURLE_UNSUPPORTED_PROTOCOL` or `CURLE_NOT_BUILT_IN` from protocol selection. In addition, both flavors must pass functional RTMP-family tests for all six RTMP schemes and functional LDAPS tests with successful CA-trusted transfer plus certificate/CA failure modes; routing-only checks are not enough for those advertised protocols.
- Public ABI smoke, symbol names, SONAMEs, and symbol namespaces match original packages.
- Compatibility consumers build from vendored tracked sources.
- Broad object relinking and runtime checks pass for the `all-objects` link manifest set, with the checker deriving the required set from `safe/metadata/test-manifest.json` and asserting that it covers all 263 required final entries: every `libcurl-consumer` target plus `src:curl`, including the disabled-WebSocket clients and the three currently omitted libtests.
- Full upstream `runtests.pl` runs for both flavors, honoring only tracked `DISABLED` entries.
- The 5 tracked non-WebSocket HTTP/TLS clients pass for both flavors, and the 2 tracked WebSocket clients compile and take the original disabled-WebSocket path for both flavors.
- Debian packages build from both `scripts/build-debs.sh` and a detached safe-only source export with `CARGO_NET_OFFLINE=true`, Cargo `--locked`, a fresh empty `CARGO_HOME`, the checked-in `safe/vendor/cargo` source replacement policy enforced, and the package no-refetch verifier run before each package build; both outputs contain exactly `curl`, `libcurl4t64`, `libcurl3t64-gnutls`, `libcurl4-openssl-dev`, `libcurl4-gnutls-dev`, and `libcurl4-doc`, and every built `.deb` version contains `+safelibs`.
- Safe binary package control metadata and built `.deb` control fields match `safe/metadata/debian-control-contract.json`, preserving Ubuntu's virtual package, conflict, replacement, dependency, time64, and multiarch semantics.
- Safe source package metadata preserves the original feature-enabling `Build-Depends` for brotli, zstd, IDN, Kerberos/GSS, LDAP, HTTP/2, PSL, RTMP, SSH, TLS, and zlib, with only explicitly allowed Rust build-dependency additions or verifier-backed substitutes.
- The final package payloads provide the required runtime library SONAME links, GnuTLS `.so.3` compatibility link, `/usr/bin/curl`, dev headers only under `/usr/include/<multiarch>/curl/`, static dev archives `/usr/lib/<multiarch>/libcurl.a` and `/usr/lib/<multiarch>/libcurl-gnutls.a`, flavor-named dev linker symlinks, GnuTLS generic `/usr/lib/<multiarch>/libcurl.so -> libcurl-gnutls.so` and `/usr/lib/<multiarch>/libcurl.a -> libcurl-gnutls.a` development links, `curl-config`, multiarch `libcurl.pc`, `/usr/share/aclocal/libcurl.m4`, and libcurl manual pages. The installed OpenSSL and GnuTLS dev packages must pass the dev-tooling contract through dynamic and static `curl-config` linkage, dynamic and static `pkg-config` linkage, and an autoconf `libcurl.m4` smoke project that uses the multiarch include path. The GnuTLS installed-dev verifier must fail if linkage only works through explicit `-lcurl-gnutls` and the generic `-lcurl` contract or generic links are absent. The dev-tooling contract must prove every original `curl-config` option and failure mode, exact original-derived protocol and feature lists, aggregate `RTMP` in configure-style tooling, all six RTMP schemes in `curl_version_info_data.protocols`, `gophers`, and the brotli/zstd/IDN/Kerberos/GSS/PSL feature names.
- `safe/debian/tests/control` preserves the original `upstream-tests-openssl`, `upstream-tests-gnutls`, and `curl-ldapi-test` names, `Depends`, and `Restrictions`; the three autopkgtest scripts plus `LDAP-bindata.c` are scanned as package-consuming files before every packaged autopkgtest run.
- The final checker first verifies the live `scripts/build-debs.sh` hook output for package shape, `+safelibs` versions, control metadata, and the root-hook contract that `dist/` contains only `.deb` artifacts, and asserts that the hook did not leave `safe/debian/changelog` dirty. It then overwrites root `dist/*.deb` with packages built inside a detached safe-only export where `../original` is absent. The detached build must consume the committed `safe/debian/changelog` SafeLibs baseline directly and must not depend on the live hook's changelog mutation. The final packaged autopkgtests, packaged curl smoke, SafeLibs validator input, and dependent safe-mode matrix all use those detached-built packages, not the earlier live-hook-built packages.
- The final detached-built `dist/*.deb` safe packages install in an Ubuntu 24.04 environment without installing conflicting dev packages together; `/usr/bin/curl` is owned by the safe `curl` package, links to the safe OpenSSL runtime package, the installed dev tooling is flavor-correct, the packaged RTMP-family and LDAPS functional fixture scripts pass for each installed dev flavor, and the packaged `upstream-tests-openssl`, `upstream-tests-gnutls`, and `curl-ldapi-test` autopkgtest entrypoints pass with explicit `AUTOPKGTEST_TMP` directories while run from the same detached safe-only export where `../original` is absent.
- CI hooks build packages and run upstream, port, and validator tests.
- The final checker asserts the dependent safe-mode executor prerequisites before package/dependent work: `docker`, a reachable Docker daemon, `git`, `jq`, `/dev/fuse` as a character device, and Docker run support for `--device /dev/fuse --cap-add SYS_ADMIN --security-opt apparmor:unconfined`.
- The 12-dependent safe-package matrix in `test-original.sh` passes from a detached root-harness export where `original/` is absent, using an absolute `--safe-deb-dir` that points at the detached-built safe `.deb`s under test, with HTTPDirFS exercised through the required Docker/FUSE privileges rather than skipped or downgraded.
- Core benchmarks stay within `safe/benchmarks/thresholds.json` or are fixed before the phase yields.
