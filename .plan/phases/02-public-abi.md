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

- `safe/metadata/abi-manifest.json`, `safe/metadata/test-manifest.json`, `safe/metadata/cve-manifest.json`, `safe/src/abi/generated.rs`, `safe/src/abi/public_types.rs`, `safe/abi/libcurl-openssl.map`, `safe/abi/libcurl-gnutls.map`, `safe/vendor/upstream/manifest.json`, `safe/debian/source/format`, and `safe/debian/patches/series`.
- `safe/include/curl/curl.h`, `safe/include/curl/curlver.h`, `safe/include/curl/easy.h`, `safe/include/curl/header.h`, `safe/include/curl/mprintf.h`, `safe/include/curl/multi.h`, `safe/include/curl/options.h`, `safe/include/curl/stdcheaders.h`, `safe/include/curl/system.h`, `safe/include/curl/typecheck-gcc.h`, `safe/include/curl/urlapi.h`, and `safe/include/curl/websockets.h`.
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

**Success Criteria:**

- Every item listed under `New Outputs` is present, updated, or explicitly left unchanged because it already satisfied the plan.
- Every required `File Changes` and `Implementation Details` invariant for this phase is satisfied.
- Every verifier listed under `Verification Phases` passes exactly as written and bounces only to this phase on failure.
- All listed `Preexisting Inputs` are consumed in place; existing artifacts are not rediscovered, refetched, or regenerated from untracked sources unless this phase explicitly updates a derived safe artifact from them.

**Git Commit Requirement:**

- The implementer must commit this phase's work to git before yielding.
