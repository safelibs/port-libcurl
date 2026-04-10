use crate::abi::{CURLUPart, CURLUcode, CURLU, CURLUE_OK, CURLUE_OUT_OF_MEMORY};
use crate::{alloc, global};
use core::ffi::c_char;
use core::ptr;
use std::ffi::CStr;
use std::sync::OnceLock;

type CurlUrlFn = unsafe extern "C" fn() -> *mut CURLU;
type CurlUrlCleanupFn = unsafe extern "C" fn(*mut CURLU);
type CurlUrlDupFn = unsafe extern "C" fn(*const CURLU) -> *mut CURLU;
type CurlUrlGetFn =
    unsafe extern "C" fn(*const CURLU, CURLUPart, *mut *mut c_char, u32) -> CURLUcode;
type CurlUrlSetFn = unsafe extern "C" fn(*mut CURLU, CURLUPart, *const c_char, u32) -> CURLUcode;
type CurlUrlStrErrorFn = unsafe extern "C" fn(CURLUcode) -> *const c_char;

fn ref_url() -> CurlUrlFn {
    static FN: OnceLock<CurlUrlFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_url\0") })
}

fn ref_url_cleanup() -> CurlUrlCleanupFn {
    static FN: OnceLock<CurlUrlCleanupFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_url_cleanup\0") })
}

fn ref_url_dup() -> CurlUrlDupFn {
    static FN: OnceLock<CurlUrlDupFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_url_dup\0") })
}

fn ref_url_get() -> CurlUrlGetFn {
    static FN: OnceLock<CurlUrlGetFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_url_get\0") })
}

fn ref_url_set() -> CurlUrlSetFn {
    static FN: OnceLock<CurlUrlSetFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_url_set\0") })
}

fn ref_url_strerror() -> CurlUrlStrErrorFn {
    static FN: OnceLock<CurlUrlStrErrorFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_url_strerror\0") })
}

pub(crate) unsafe fn url() -> *mut CURLU {
    unsafe { ref_url()() }
}

pub(crate) unsafe fn url_cleanup(handle: *mut CURLU) {
    unsafe { ref_url_cleanup()(handle) };
}

pub(crate) unsafe fn url_dup(handle: *const CURLU) -> *mut CURLU {
    unsafe { ref_url_dup()(handle) }
}

pub(crate) unsafe fn url_get(
    handle: *const CURLU,
    what: CURLUPart,
    part: *mut *mut c_char,
    flags: u32,
) -> CURLUcode {
    if part.is_null() {
        return unsafe { ref_url_get()(handle, what, part, flags) };
    }

    let mut reference_part = ptr::null_mut();
    let code = unsafe { ref_url_get()(handle, what, &mut reference_part, flags) };
    if code != CURLUE_OK || reference_part.is_null() {
        unsafe { *part = ptr::null_mut() };
        return code;
    }

    let copy = unsafe { alloc::alloc_and_copy(CStr::from_ptr(reference_part).to_bytes()) };
    unsafe { global::free_reference_allocation(reference_part.cast()) };
    if copy.is_null() {
        unsafe { *part = ptr::null_mut() };
        return CURLUE_OUT_OF_MEMORY;
    }

    unsafe { *part = copy };
    CURLUE_OK
}

pub(crate) unsafe fn url_set(
    handle: *mut CURLU,
    what: CURLUPart,
    part: *const c_char,
    flags: u32,
) -> CURLUcode {
    unsafe { ref_url_set()(handle, what, part, flags) }
}

pub(crate) unsafe fn url_strerror(code: CURLUcode) -> *const c_char {
    unsafe { ref_url_strerror()(code) }
}
