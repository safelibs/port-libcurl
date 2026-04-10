use crate::abi::{CURLSH, CURLSHcode};
use core::ffi::c_char;

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
