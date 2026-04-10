use crate::abi::{curl_easyoption, CURL};
use core::ffi::{c_char, c_int};

#[no_mangle]
pub unsafe extern "C" fn curl_easy_init() -> *mut CURL {
    unsafe { crate::easy::handle::easy_init() }
}

#[no_mangle]
pub unsafe extern "C" fn curl_easy_cleanup(handle: *mut CURL) {
    unsafe { crate::easy::handle::easy_cleanup(handle) };
}

#[no_mangle]
pub unsafe extern "C" fn curl_easy_duphandle(handle: *mut CURL) -> *mut CURL {
    unsafe { crate::easy::handle::easy_duphandle(handle) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_easy_reset(handle: *mut CURL) {
    unsafe { crate::easy::handle::easy_reset(handle) };
}

#[no_mangle]
pub unsafe extern "C" fn curl_easy_escape(
    handle: *mut CURL,
    input: *const c_char,
    len: c_int,
) -> *mut c_char {
    unsafe { crate::easy::handle::easy_escape(handle, input, len) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_escape(input: *const c_char, len: c_int) -> *mut c_char {
    unsafe { crate::easy::handle::easy_escape(core::ptr::null_mut(), input, len) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_easy_unescape(
    handle: *mut CURL,
    input: *const c_char,
    len: c_int,
    out_len: *mut c_int,
) -> *mut c_char {
    unsafe { crate::easy::handle::easy_unescape(handle, input, len, out_len) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_unescape(input: *const c_char, len: c_int) -> *mut c_char {
    unsafe {
        crate::easy::handle::easy_unescape(core::ptr::null_mut(), input, len, core::ptr::null_mut())
    }
}

#[no_mangle]
pub unsafe extern "C" fn curl_easy_option_by_name(name: *const c_char) -> *const curl_easyoption {
    unsafe { crate::easy::options::option_by_name(name) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_easy_option_by_id(id: u32) -> *const curl_easyoption {
    unsafe { crate::easy::options::option_by_id(id) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_easy_option_next(
    prev: *const curl_easyoption,
) -> *const curl_easyoption {
    unsafe { crate::easy::options::option_next(prev) }
}
