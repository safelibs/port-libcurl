use crate::abi::CURL;
use crate::{alloc, global};
use core::ffi::{c_char, c_int};
use core::{ptr, slice};
use std::ffi::CStr;
use std::sync::OnceLock;

type CurlEasyInitFn = unsafe extern "C" fn() -> *mut CURL;
type CurlEasyCleanupFn = unsafe extern "C" fn(*mut CURL);
type CurlEasyDupHandleFn = unsafe extern "C" fn(*mut CURL) -> *mut CURL;
type CurlEasyResetFn = unsafe extern "C" fn(*mut CURL);
type CurlEasyEscapeFn = unsafe extern "C" fn(*mut CURL, *const c_char, c_int) -> *mut c_char;
type CurlEasyUnescapeFn =
    unsafe extern "C" fn(*mut CURL, *const c_char, c_int, *mut c_int) -> *mut c_char;

fn ref_easy_init() -> CurlEasyInitFn {
    static FN: OnceLock<CurlEasyInitFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_easy_init\0") })
}

fn ref_easy_cleanup() -> CurlEasyCleanupFn {
    static FN: OnceLock<CurlEasyCleanupFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_easy_cleanup\0") })
}

fn ref_easy_duphandle() -> CurlEasyDupHandleFn {
    static FN: OnceLock<CurlEasyDupHandleFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_easy_duphandle\0") })
}

fn ref_easy_reset() -> CurlEasyResetFn {
    static FN: OnceLock<CurlEasyResetFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_easy_reset\0") })
}

fn ref_easy_escape() -> CurlEasyEscapeFn {
    static FN: OnceLock<CurlEasyEscapeFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_easy_escape\0") })
}

fn ref_easy_unescape() -> CurlEasyUnescapeFn {
    static FN: OnceLock<CurlEasyUnescapeFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_easy_unescape\0") })
}

pub(crate) unsafe fn easy_init() -> *mut CURL {
    if global::ensure_global_init_for_easy().is_err() {
        return core::ptr::null_mut();
    }
    let handle = unsafe { ref_easy_init()() };
    crate::easy::perform::register_handle(handle);
    handle
}

pub(crate) unsafe fn easy_cleanup(handle: *mut CURL) {
    if let Some(attached_multi) = crate::easy::perform::attached_multi_for(handle) {
        let _ = unsafe {
            crate::multi::remove_handle(attached_multi as *mut crate::abi::CURLM, handle)
        };
    }
    let private_multi = crate::easy::perform::unregister_handle(handle);
    if let Some(multi) = private_multi {
        unsafe { crate::multi::cleanup_owned_multi(multi as *mut crate::abi::CURLM) };
    }
    unsafe { ref_easy_cleanup()(handle) };
}

pub(crate) unsafe fn easy_duphandle(handle: *mut CURL) -> *mut CURL {
    let duplicate = unsafe { ref_easy_duphandle()(handle) };
    crate::easy::perform::register_duplicate(handle, duplicate);
    duplicate
}

pub(crate) unsafe fn easy_reset(handle: *mut CURL) {
    crate::easy::perform::reset_handle(handle);
    unsafe { ref_easy_reset()(handle) };
}

pub(crate) unsafe fn easy_escape(
    handle: *mut CURL,
    input: *const c_char,
    len: c_int,
) -> *mut c_char {
    let escaped = unsafe { ref_easy_escape()(handle, input, len) };
    if escaped.is_null() {
        return ptr::null_mut();
    }

    let copy = unsafe { alloc::alloc_and_copy(CStr::from_ptr(escaped).to_bytes()) };
    unsafe { global::free_reference_allocation(escaped.cast()) };
    copy
}

pub(crate) unsafe fn easy_unescape(
    handle: *mut CURL,
    input: *const c_char,
    len: c_int,
    out_len: *mut c_int,
) -> *mut c_char {
    let mut local_len = 0;
    let effective_out_len = if out_len.is_null() {
        &mut local_len
    } else {
        out_len
    };
    let unescaped = unsafe { ref_easy_unescape()(handle, input, len, effective_out_len) };
    if unescaped.is_null() {
        return ptr::null_mut();
    }

    let copy = unsafe {
        alloc::alloc_and_copy(slice::from_raw_parts(
            unescaped.cast::<u8>(),
            (*effective_out_len).max(0) as usize,
        ))
    };
    unsafe { global::free_reference_allocation(unescaped.cast()) };
    copy
}
