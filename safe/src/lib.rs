#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]

#[cfg(all(feature = "openssl-flavor", feature = "gnutls-flavor"))]
compile_error!("enable exactly one of `openssl-flavor` or `gnutls-flavor`");

#[cfg(not(any(feature = "openssl-flavor", feature = "gnutls-flavor")))]
compile_error!("enable one of `openssl-flavor` or `gnutls-flavor`");

pub mod abi {
    include!("abi/generated.rs");
}

mod alloc;
mod conn;
mod dns;
mod doh;
mod easy;
mod form;
mod global;
mod http;
mod idn;
mod mime;
mod multi;
mod protocols;
mod rand;
mod share;
mod slist;
mod ssh;
mod tls;
mod transfer;
mod urlapi;
mod version;
mod vquic;
mod ws;

#[path = "abi/connect_only.rs"]
mod abi_connect_only;
#[path = "abi/easy.rs"]
mod abi_easy;
#[path = "abi/multi.rs"]
mod abi_multi;
#[path = "abi/share.rs"]
mod abi_share;
#[path = "abi/url.rs"]
mod abi_url;

pub const BUILD_FLAVOR: &str = if cfg!(feature = "openssl-flavor") {
    "openssl"
} else {
    "gnutls"
};

unsafe extern "C" {
    #[link_name = "curl_easy_setopt"]
    fn retain_variadic_c_shims(
        handle: *mut crate::abi::CURL,
        option: crate::abi::CURLoption,
        ...
    ) -> crate::abi::CURLcode;
    #[link_name = "curl_maprintf"]
    fn retain_mprintf_c_shims(format: *const core::ffi::c_char, ...) -> *mut core::ffi::c_char;
}

// Keep the standalone public ABI shim objects linked into the cdylib even when
// Rust does not call the exported entry points directly. The reduced
// foundation-bridge still relies on the variadic surface and the permanent
// mprintf interop boundary for smoke and ABI tests.
#[used]
static RETAIN_VARIADIC_C_SHIM: unsafe extern "C" fn(
    *mut crate::abi::CURL,
    crate::abi::CURLoption,
    ...
) -> crate::abi::CURLcode = retain_variadic_c_shims;

#[used]
static RETAIN_MPRINTF_C_SHIM: unsafe extern "C" fn(
    *const core::ffi::c_char,
    ...
) -> *mut core::ffi::c_char = retain_mprintf_c_shims;
