use crate::abi::CURL;
use crate::global;
use core::ffi::{c_char, c_int};
use core::{ptr, slice};
use std::collections::HashMap;
use std::ffi::CStr;
use std::sync::{Mutex, OnceLock};

const EASY_HANDLE_MAGIC: usize = 0x4355_524c_4541_5359;

type CurlEasyInitFn = unsafe extern "C" fn() -> *mut CURL;
type CurlEasyCleanupFn = unsafe extern "C" fn(*mut CURL);
type CurlEasyDupHandleFn = unsafe extern "C" fn(*mut CURL) -> *mut CURL;

#[repr(C)]
struct EasyHandle {
    magic: usize,
    reference: *mut CURL,
}

fn reference_map() -> &'static Mutex<HashMap<usize, usize>> {
    static MAP: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

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

fn wrapper_from_ptr(handle: *mut CURL) -> Option<&'static mut EasyHandle> {
    if handle.is_null() {
        return None;
    }
    let wrapper = unsafe { &mut *(handle as *mut EasyHandle) };
    (wrapper.magic == EASY_HANDLE_MAGIC).then_some(wrapper)
}

fn register_reference(public_handle: *mut CURL, reference_handle: *mut CURL) {
    if public_handle.is_null() || reference_handle.is_null() {
        return;
    }
    reference_map()
        .lock()
        .expect("easy reference map mutex poisoned")
        .insert(reference_handle as usize, public_handle as usize);
}

fn unregister_reference(public_handle: *mut CURL, reference_handle: *mut CURL) {
    if public_handle.is_null() || reference_handle.is_null() {
        return;
    }
    let mut guard = reference_map()
        .lock()
        .expect("easy reference map mutex poisoned");
    if guard.get(&(reference_handle as usize)).copied() == Some(public_handle as usize) {
        guard.remove(&(reference_handle as usize));
    }
}

fn alloc_public_handle(reference: *mut CURL) -> *mut CURL {
    let wrapper = Box::new(EasyHandle {
        magic: EASY_HANDLE_MAGIC,
        reference,
    });
    Box::into_raw(wrapper).cast()
}

fn effective_input_bytes<'a>(input: *const c_char, len: c_int) -> Option<&'a [u8]> {
    if input.is_null() {
        return None;
    }
    if len <= 0 {
        Some(unsafe { CStr::from_ptr(input) }.to_bytes())
    } else {
        Some(unsafe { slice::from_raw_parts(input.cast::<u8>(), len as usize) })
    }
}

fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

fn hex(byte: u8) -> u8 {
    match byte {
        0..=9 => b'0' + byte,
        _ => b'A' + (byte - 10),
    }
}

fn decode_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub(crate) fn reference_handle(handle: *mut CURL) -> *mut CURL {
    wrapper_from_ptr(handle)
        .map(|wrapper| wrapper.reference)
        .unwrap_or(handle)
}

pub(crate) fn public_from_reference(reference_handle: *mut CURL) -> *mut CURL {
    reference_map()
        .lock()
        .expect("easy reference map mutex poisoned")
        .get(&(reference_handle as usize))
        .copied()
        .map(|value| value as *mut CURL)
        .unwrap_or(reference_handle)
}

pub(crate) unsafe fn easy_init() -> *mut CURL {
    if global::ensure_global_init_for_easy().is_err() {
        return ptr::null_mut();
    }
    let reference = unsafe { ref_easy_init()() };
    if reference.is_null() {
        return ptr::null_mut();
    }
    let public = alloc_public_handle(reference);
    register_reference(public, reference);
    crate::easy::perform::register_handle(public);
    public
}

pub(crate) unsafe fn easy_cleanup(handle: *mut CURL) {
    if handle.is_null() {
        return;
    }
    if let Some(attached_multi) = crate::easy::perform::attached_multi_for(handle) {
        let _ = unsafe {
            crate::multi::remove_handle(attached_multi as *mut crate::abi::CURLM, handle)
        };
    }
    crate::transfer::release_handle_state(handle);
    let private_multi = crate::easy::perform::unregister_handle(handle);
    if let Some(multi) = private_multi {
        unsafe { crate::multi::cleanup_owned_multi(multi as *mut crate::abi::CURLM) };
    }

    if let Some(wrapper) = wrapper_from_ptr(handle) {
        let reference = wrapper.reference;
        unregister_reference(handle, reference);
        if !reference.is_null() {
            unsafe { ref_easy_cleanup()(reference) };
        }
        wrapper.magic = 0;
        unsafe {
            drop(Box::from_raw(handle as *mut EasyHandle));
        }
    } else {
        unsafe { ref_easy_cleanup()(handle) };
    }
}

pub(crate) unsafe fn easy_duphandle(handle: *mut CURL) -> *mut CURL {
    let reference = reference_handle(handle);
    if reference.is_null() {
        return ptr::null_mut();
    }

    let duplicate_reference = unsafe { ref_easy_duphandle()(reference) };
    if duplicate_reference.is_null() {
        return ptr::null_mut();
    }

    let public = alloc_public_handle(duplicate_reference);
    register_reference(public, duplicate_reference);
    crate::easy::perform::register_duplicate(handle, public);
    public
}

pub(crate) unsafe fn easy_reset(handle: *mut CURL) {
    if handle.is_null() {
        return;
    }
    crate::transfer::release_handle_state(handle);
    crate::easy::perform::reset_handle(handle);

    if let Some(wrapper) = wrapper_from_ptr(handle) {
        let old_reference = wrapper.reference;
        let new_reference = unsafe { ref_easy_init()() };
        unregister_reference(handle, old_reference);
        if !old_reference.is_null() {
            unsafe { ref_easy_cleanup()(old_reference) };
        }
        wrapper.reference = new_reference;
        register_reference(handle, new_reference);
    }
}

pub(crate) unsafe fn easy_escape(
    _handle: *mut CURL,
    input: *const c_char,
    len: c_int,
) -> *mut c_char {
    let Some(bytes) = effective_input_bytes(input, len) else {
        return ptr::null_mut();
    };
    let mut encoded = Vec::with_capacity(bytes.len() * 3);
    for &byte in bytes {
        if is_unreserved(byte) {
            encoded.push(byte);
        } else {
            encoded.push(b'%');
            encoded.push(hex(byte >> 4));
            encoded.push(hex(byte & 0x0f));
        }
    }
    unsafe { crate::alloc::alloc_and_copy(&encoded) }
}

pub(crate) unsafe fn easy_unescape(
    _handle: *mut CURL,
    input: *const c_char,
    len: c_int,
    out_len: *mut c_int,
) -> *mut c_char {
    let Some(bytes) = effective_input_bytes(input, len) else {
        return ptr::null_mut();
    };
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut idx = 0usize;
    while idx < bytes.len() {
        if bytes[idx] == b'%' && idx + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (decode_hex(bytes[idx + 1]), decode_hex(bytes[idx + 2])) {
                decoded.push((hi << 4) | lo);
                idx += 3;
                continue;
            }
        }
        decoded.push(bytes[idx]);
        idx += 1;
    }

    if !out_len.is_null() {
        unsafe {
            *out_len = decoded.len().min(c_int::MAX as usize) as c_int;
        }
    }
    unsafe { crate::alloc::alloc_and_copy(&decoded) }
}
