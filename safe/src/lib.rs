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
mod global;
mod version;
mod slist;
mod mime;
mod form;
mod urlapi;
mod share;
mod easy;

#[path = "abi/easy.rs"]
mod abi_easy;
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
    fn retain_variadic_c_shims();
    #[link_name = "curl_maprintf"]
    fn retain_mprintf_c_shims();
}

// Keep the standalone C shim objects linked into the cdylib even when Rust
// does not call them directly. The smoke harness links against these public
// ABI entry points.
#[used]
static RETAIN_PUBLIC_C_SHIMS: [unsafe extern "C" fn(); 2] =
    [retain_variadic_c_shims, retain_mprintf_c_shims];
