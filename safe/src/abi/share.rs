use crate::abi::{CURLSHcode, CURLSHoption, CURLSH};
use core::ffi::{c_char, c_int, c_void};

#[no_mangle]
pub unsafe extern "C" fn curl_share_init() -> *mut CURLSH {
    unsafe { crate::share::share_init() }
}

#[no_mangle]
pub unsafe extern "C" fn curl_share_cleanup(handle: *mut CURLSH) -> CURLSHcode {
    unsafe { crate::share::share_cleanup(handle) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_share_strerror(code: CURLSHcode) -> *const c_char {
    unsafe { crate::share::share_strerror(code) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_safe_share_setopt_int(
    handle: *mut CURLSH,
    option: CURLSHoption,
    value: c_int,
) -> CURLSHcode {
    crate::share::share_setopt_int(handle, option, value)
}

#[no_mangle]
pub unsafe extern "C" fn curl_safe_share_setopt_function(
    handle: *mut CURLSH,
    option: CURLSHoption,
    value: Option<unsafe extern "C" fn()>,
) -> CURLSHcode {
    crate::share::share_setopt_function(handle, option, value)
}

#[no_mangle]
pub unsafe extern "C" fn curl_safe_share_setopt_ptr(
    handle: *mut CURLSH,
    option: CURLSHoption,
    value: *mut c_void,
) -> CURLSHcode {
    crate::share::share_setopt_ptr(handle, option, value)
}
