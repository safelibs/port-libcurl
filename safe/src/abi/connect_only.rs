use crate::abi::{CURLcode, CURL};
use core::ffi::{c_int, c_void};

#[no_mangle]
pub unsafe extern "C" fn curl_easy_perform(curl: *mut CURL) -> CURLcode {
    unsafe { crate::easy::perform::easy_perform(curl) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_easy_pause(handle: *mut CURL, bitmask: c_int) -> CURLcode {
    unsafe { crate::easy::perform::easy_pause(handle, bitmask) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_easy_recv(
    curl: *mut CURL,
    buffer: *mut c_void,
    buflen: usize,
    n: *mut usize,
) -> CURLcode {
    unsafe { crate::easy::perform::easy_recv(curl, buffer, buflen, n) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_easy_send(
    curl: *mut CURL,
    buffer: *const c_void,
    buflen: usize,
    n: *mut usize,
) -> CURLcode {
    unsafe { crate::easy::perform::easy_send(curl, buffer, buflen, n) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_easy_upkeep(curl: *mut CURL) -> CURLcode {
    unsafe { crate::easy::perform::easy_upkeep(curl) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_safe_easy_setopt_observe_long(
    handle: *mut CURL,
    option: crate::abi::CURLoption,
    value: core::ffi::c_long,
) {
    crate::easy::perform::observe_easy_setopt_long(handle, option, value);
}

#[no_mangle]
pub unsafe extern "C" fn curl_safe_easy_setopt_observe_ptr(
    handle: *mut CURL,
    option: crate::abi::CURLoption,
    value: *mut c_void,
) {
    crate::easy::perform::observe_easy_setopt_ptr(handle, option, value);
}
