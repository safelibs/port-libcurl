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

pub const BUILD_FLAVOR: &str = if cfg!(feature = "openssl-flavor") {
    "openssl"
} else {
    "gnutls"
};

