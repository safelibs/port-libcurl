# Phase Name
Public ABI, Global State, Allocator Contract, Varargs, MIME/Form, URL API, and Opaque Handles

## Implement Phase ID
`impl-public-abi`

## Preexisting Inputs
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

## New Outputs
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

## File Changes
- Replace temporary forwarders for non-I/O public functions with Rust implementations.
- Add a runtime-switchable allocator facade driven by `curl_global_init_mem`.
- Add typed Rust dispatchers for varargs APIs behind thin C shims.
- Add MIME, legacy form, URL API, slist, share-handle, and version-reporting support with ABI-compatible public structs and allocation behavior.

## Implementation Details
- Implement `curl_global_init`, `curl_global_init_mem`, `curl_global_cleanup`, `curl_global_trace`, `curl_global_sslset`, `curl_free`, `curl_getenv`, `curl_getdate`, `curl_strequal`, `curl_strnequal`, `curl_version`, and `curl_version_info` in Rust, preserving the process-global semantics from `original/lib/easy.c` and `original/lib/version.c`.
- The allocation facade in `safe/src/alloc.rs` must default to libc allocators and switch to user-provided callbacks exactly once `curl_global_init_mem` succeeds. Any memory returned through the public ABI, including strings from `curl_easy_escape`, `curl_url_get`, `curl_version`, `curl_maprintf`, and `curl_mvaprintf`, plus arrays from `curl_multi_get_handles`, must use this facade.
- Model `CURL`, `CURLSH`, and `CURLU` as Rust-owned opaque state with C-visible pointers only at the ABI boundary.
- Generate the `curl_easyoption` table from `original/lib/easyoptions.c` and preserve aliases and type tags from `original/include/curl/options.h`.
- Keep permanent C varargs shims for `curl_easy_setopt`, `curl_easy_getinfo`, `curl_multi_setopt`, `curl_share_setopt`, and `curl_formadd`. The shim should inspect option metadata and route to type-specific Rust setters and getters.
- Keep permanent C implementations for the `curl_mprintf*` family because preserving `va_list` semantics there is simpler and safer than re-implementing them directly in Rust. The allocating variants in that family must route through the safe allocator facade rather than raw `malloc`.
- Implement the object-model and non-transport portions of `curl_easy_init`, `curl_easy_cleanup`, `curl_easy_reset`, `curl_easy_duphandle`, `curl_share_init`, `curl_share_cleanup`, `curl_share_strerror`, `curl_url`, `curl_url_cleanup`, `curl_url_dup`, `curl_url_get`, `curl_url_set`, `curl_url_strerror`, `curl_easy_option_by_name`, `curl_easy_option_by_id`, `curl_easy_option_next`, `curl_mime_*`, `curl_formget`, and `curl_formfree`.
- Public layout verification must include at minimum `curl_httppost`, `curl_blob`, `curl_waitfd`, `curl_header`, `curl_ws_frame`, `curl_ssl_backend`, `curl_tlssessioninfo`, `curl_version_info_data`, and every other public struct recorded in `safe/metadata/abi-manifest.json`.
- `safe/scripts/run-public-abi-smoke.sh` must build exactly one flavor per invocation using an isolated `CARGO_TARGET_DIR` such as `safe/target/public-abi/<flavor>`, compile `safe/tests/smoke/public_api_smoke.c` against that flavor’s headers and library directory, and run it with `LD_LIBRARY_PATH` restricted to that same flavor output so the OpenSSL and GnuTLS smoke checks cannot accidentally share artifacts.

## Verification Phases
### `check-public-abi-smoke`
- Type: `check`
- Bounce Target: `impl-public-abi`
- Purpose: confirm that non-transport public APIs compile and execute against the Rust implementation using only installed headers and the safe shared library.
- Commands it should run:
```bash
cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test public_abi
cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test public_abi
bash safe/scripts/run-public-abi-smoke.sh --flavor openssl
bash safe/scripts/run-public-abi-smoke.sh --flavor gnutls
```

### `check-public-abi-layout`
- Type: `check`
- Bounce Target: `impl-public-abi`
- Purpose: verify public layout and option-table compatibility against the phase-1 ABI manifest.
- Commands it should run:
```bash
cargo test --manifest-path safe/Cargo.toml --no-default-features --features openssl-flavor --test abi_layout
cargo test --manifest-path safe/Cargo.toml --no-default-features --features gnutls-flavor --test abi_layout
bash safe/scripts/verify-abi-manifest.sh safe/metadata/abi-manifest.json
```

## Success Criteria
- Every listed `Preexisting Input` is consumed as an existing artifact rather than rediscovered, regenerated, or refetched.
- Every listed `New Output` for this implement phase exists and is ready for downstream phases in the linear workflow.
- The verifier phase(s) `check-public-abi-smoke`, `check-public-abi-layout` pass exactly as written for `impl-public-abi`.

## Git Commit Requirement
The implementer must commit this phase's work to git before yielding. Ignored-only or untracked-only outputs are not acceptable.
