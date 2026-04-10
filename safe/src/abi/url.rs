use crate::abi::{CURLU, CURLUPart, CURLUcode};
use core::ffi::c_char;

#[no_mangle]
pub unsafe extern "C" fn curl_url() -> *mut CURLU {
    unsafe { crate::urlapi::url() }
}

#[no_mangle]
pub unsafe extern "C" fn curl_url_cleanup(handle: *mut CURLU) {
    unsafe { crate::urlapi::url_cleanup(handle) };
}

#[no_mangle]
pub unsafe extern "C" fn curl_url_dup(handle: *const CURLU) -> *mut CURLU {
    unsafe { crate::urlapi::url_dup(handle) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_url_get(
    handle: *const CURLU,
    what: CURLUPart,
    part: *mut *mut c_char,
    flags: u32,
) -> CURLUcode {
    unsafe { crate::urlapi::url_get(handle, what, part, flags) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_url_set(
    handle: *mut CURLU,
    what: CURLUPart,
    part: *const c_char,
    flags: u32,
) -> CURLUcode {
    unsafe { crate::urlapi::url_set(handle, what, part, flags) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_url_strerror(code: CURLUcode) -> *const c_char {
    unsafe { crate::urlapi::url_strerror(code) }
}
