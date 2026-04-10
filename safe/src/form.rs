use crate::abi::{curl_formget_callback, curl_httppost};
use crate::global;
use core::ffi::{c_int, c_void};
use std::sync::OnceLock;

type CurlFormGetFn =
    unsafe extern "C" fn(*mut curl_httppost, *mut c_void, curl_formget_callback) -> c_int;
type CurlFormFreeFn = unsafe extern "C" fn(*mut curl_httppost);

fn ref_formget() -> CurlFormGetFn {
    static FN: OnceLock<CurlFormGetFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_formget\0") })
}

fn ref_formfree() -> CurlFormFreeFn {
    static FN: OnceLock<CurlFormFreeFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_formfree\0") })
}

#[no_mangle]
pub unsafe extern "C" fn curl_formget(
    form: *mut curl_httppost,
    arg: *mut c_void,
    append: curl_formget_callback,
) -> c_int {
    unsafe { ref_formget()(form, arg, append) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_formfree(form: *mut curl_httppost) {
    unsafe { ref_formfree()(form) };
}
