use crate::abi::{curl_version_info_data, size_t, time_t, CURLversion};
use crate::{alloc, global};
use core::ffi::{c_char, c_void};
use core::ptr;
use std::ffi::CStr;
use std::sync::{Mutex, OnceLock};

unsafe extern "C" {
    fn getenv(name: *const c_char) -> *mut c_char;
}

type CurlGetDateFn = unsafe extern "C" fn(*const c_char, *const time_t) -> time_t;
type CurlVersionFn = unsafe extern "C" fn() -> *mut c_char;
type CurlVersionInfoFn = unsafe extern "C" fn(CURLversion) -> *mut curl_version_info_data;

static VERSION_CACHE: Mutex<Option<usize>> = Mutex::new(None);

fn ref_getdate() -> CurlGetDateFn {
    static FN: OnceLock<CurlGetDateFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_getdate\0") })
}

fn ref_version() -> CurlVersionFn {
    static FN: OnceLock<CurlVersionFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_version\0") })
}

fn ref_version_info() -> CurlVersionInfoFn {
    static FN: OnceLock<CurlVersionInfoFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_version_info\0") })
}

pub(crate) fn clear_cached_version() {
    let cached = VERSION_CACHE
        .lock()
        .expect("version cache mutex poisoned")
        .take();
    if let Some(ptr) = cached {
        unsafe { alloc::free_ptr((ptr as *mut c_char).cast::<c_void>()) };
    }
}

fn ascii_lower(byte: u8) -> u8 {
    if byte.is_ascii_uppercase() {
        byte + 32
    } else {
        byte
    }
}

#[no_mangle]
pub unsafe extern "C" fn curl_getenv(variable: *const c_char) -> *mut c_char {
    if variable.is_null() {
        return ptr::null_mut();
    }

    let value = unsafe { getenv(variable) };
    if value.is_null() {
        return ptr::null_mut();
    }

    unsafe { alloc::strdup_bytes(value) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_getdate(input: *const c_char, unused: *const time_t) -> time_t {
    unsafe { ref_getdate()(input, unused) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_strequal(s1: *const c_char, s2: *const c_char) -> i32 {
    if s1.is_null() || s2.is_null() {
        return 0;
    }

    let mut idx = 0usize;
    loop {
        let a = unsafe { *s1.add(idx) } as u8;
        let b = unsafe { *s2.add(idx) } as u8;
        if ascii_lower(a) != ascii_lower(b) {
            return 0;
        }
        if a == 0 {
            return 1;
        }
        idx += 1;
    }
}

#[no_mangle]
pub unsafe extern "C" fn curl_strnequal(s1: *const c_char, s2: *const c_char, n: size_t) -> i32 {
    if n == 0 {
        return 1;
    }
    if s1.is_null() || s2.is_null() {
        return 0;
    }

    let mut idx = 0usize;
    while idx < n {
        let a = unsafe { *s1.add(idx) } as u8;
        let b = unsafe { *s2.add(idx) } as u8;
        if ascii_lower(a) != ascii_lower(b) {
            return 0;
        }
        if a == 0 {
            return 1;
        }
        idx += 1;
    }
    1
}

#[no_mangle]
pub unsafe extern "C" fn curl_version() -> *mut c_char {
    if let Some(ptr) = *VERSION_CACHE.lock().expect("version cache mutex poisoned") {
        return ptr as *mut c_char;
    }

    let source = unsafe { ref_version()() };
    if source.is_null() {
        return ptr::null_mut();
    }

    let source = unsafe { CStr::from_ptr(source) };
    let copy = unsafe { alloc::alloc_and_copy(source.to_bytes()) };
    if copy.is_null() {
        return ptr::null_mut();
    }

    *VERSION_CACHE.lock().expect("version cache mutex poisoned") = Some(copy as usize);
    copy
}

#[no_mangle]
pub unsafe extern "C" fn curl_version_info(stamp: CURLversion) -> *mut curl_version_info_data {
    unsafe { ref_version_info()(stamp) }
}
