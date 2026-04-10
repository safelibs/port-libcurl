use crate::abi::{curl_off_t, curl_socket_t, CURLcode, CURLoption, CURL, CURLINFO};
use core::ffi::{c_char, c_int, c_long, c_void};

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
    option: CURLoption,
    value: *mut c_void,
) {
    crate::easy::perform::observe_easy_setopt_ptr(handle, option, value);
}

#[no_mangle]
pub unsafe extern "C" fn curl_safe_easy_setopt_observe_function(
    handle: *mut CURL,
    option: CURLoption,
    value: Option<unsafe extern "C" fn()>,
) {
    crate::easy::perform::observe_easy_setopt_function(handle, option, value);
}

#[no_mangle]
pub unsafe extern "C" fn curl_safe_easy_setopt_observe_off_t(
    handle: *mut CURL,
    option: CURLoption,
    value: curl_off_t,
) {
    crate::easy::perform::observe_easy_setopt_off_t(handle, option, value);
}

#[no_mangle]
pub unsafe extern "C" fn curl_safe_easy_getinfo_long(
    handle: *mut CURL,
    info: CURLINFO,
    value: *mut c_long,
    result: *mut CURLcode,
) -> c_int {
    let Some(code) = crate::easy::perform::easy_getinfo_long(handle, info, value) else {
        return 0;
    };
    if !result.is_null() {
        unsafe { *result = code };
    }
    1
}

#[no_mangle]
pub unsafe extern "C" fn curl_safe_easy_getinfo_string(
    handle: *mut CURL,
    info: CURLINFO,
    value: *mut *mut c_char,
    result: *mut CURLcode,
) -> c_int {
    let Some(code) = crate::easy::perform::easy_getinfo_string(handle, info, value) else {
        return 0;
    };
    if !result.is_null() {
        unsafe { *result = code };
    }
    1
}

#[no_mangle]
pub unsafe extern "C" fn curl_safe_easy_getinfo_off_t(
    handle: *mut CURL,
    info: CURLINFO,
    value: *mut curl_off_t,
    result: *mut CURLcode,
) -> c_int {
    let Some(code) = crate::easy::perform::easy_getinfo_off_t(handle, info, value) else {
        return 0;
    };
    if !result.is_null() {
        unsafe { *result = code };
    }
    1
}

#[no_mangle]
pub unsafe extern "C" fn curl_safe_easy_getinfo_socket(
    handle: *mut CURL,
    info: CURLINFO,
    value: *mut curl_socket_t,
    result: *mut CURLcode,
) -> c_int {
    let Some(code) = crate::easy::perform::easy_getinfo_socket(handle, info, value) else {
        return 0;
    };
    if !result.is_null() {
        unsafe { *result = code };
    }
    1
}
