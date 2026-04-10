use crate::abi::{CURLSH, CURLSHcode};
use crate::global;
use core::ffi::c_char;
use std::sync::OnceLock;

type CurlShareInitFn = unsafe extern "C" fn() -> *mut CURLSH;
type CurlShareCleanupFn = unsafe extern "C" fn(*mut CURLSH) -> CURLSHcode;
type CurlShareStrErrorFn = unsafe extern "C" fn(CURLSHcode) -> *const c_char;

fn ref_share_init() -> CurlShareInitFn {
    static FN: OnceLock<CurlShareInitFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_share_init\0") })
}

fn ref_share_cleanup() -> CurlShareCleanupFn {
    static FN: OnceLock<CurlShareCleanupFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_share_cleanup\0") })
}

fn ref_share_strerror() -> CurlShareStrErrorFn {
    static FN: OnceLock<CurlShareStrErrorFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_share_strerror\0") })
}

pub(crate) unsafe fn share_init() -> *mut CURLSH {
    unsafe { ref_share_init()() }
}

pub(crate) unsafe fn share_cleanup(handle: *mut CURLSH) -> CURLSHcode {
    unsafe { ref_share_cleanup()(handle) }
}

pub(crate) unsafe fn share_strerror(code: CURLSHcode) -> *const c_char {
    unsafe { ref_share_strerror()(code) }
}
