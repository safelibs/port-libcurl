use crate::abi::CURL;
use crate::global;
use core::ffi::{c_char, c_int};
use core::{ptr, slice};
use std::ffi::CStr;

const EASY_HANDLE_MAGIC: usize = 0x4355_524c_4541_5359;

#[repr(C)]
struct EasyHandle {
    magic: usize,
}

fn wrapper_from_ptr(handle: *mut CURL) -> Option<&'static mut EasyHandle> {
    if handle.is_null() {
        return None;
    }
    let wrapper = unsafe { &mut *(handle as *mut EasyHandle) };
    (wrapper.magic == EASY_HANDLE_MAGIC).then_some(wrapper)
}

pub(crate) fn is_public_handle(handle: *mut CURL) -> bool {
    wrapper_from_ptr(handle).is_some()
}

pub(crate) unsafe fn alloc_public_handle() -> *mut CURL {
    Box::into_raw(Box::new(EasyHandle {
        magic: EASY_HANDLE_MAGIC,
    }))
    .cast()
}

pub(crate) unsafe fn free_public_handle(handle: *mut CURL) {
    let Some(wrapper) = wrapper_from_ptr(handle) else {
        return;
    };
    wrapper.magic = 0;
    unsafe {
        drop(Box::from_raw(handle as *mut EasyHandle));
    }
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

pub(crate) unsafe fn easy_init() -> *mut CURL {
    if global::ensure_global_init_for_easy().is_err() {
        return ptr::null_mut();
    }
    let public = unsafe { alloc_public_handle() };
    crate::easy::perform::register_handle(public);
    public
}

pub(crate) unsafe fn easy_cleanup(handle: *mut CURL) {
    if !is_public_handle(handle) {
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
    unsafe { crate::easy::reference::release_handle(handle) };
    unsafe { free_public_handle(handle) };
}

pub(crate) unsafe fn easy_duphandle(handle: *mut CURL) -> *mut CURL {
    if !is_public_handle(handle) {
        return ptr::null_mut();
    }
    let public = unsafe { alloc_public_handle() };
    crate::easy::perform::register_duplicate(handle, public);
    public
}

pub(crate) unsafe fn easy_reset(handle: *mut CURL) {
    if !is_public_handle(handle) {
        return;
    }
    crate::transfer::release_handle_state(handle);
    crate::easy::perform::reset_handle(handle);
    unsafe { crate::easy::reference::release_handle(handle) };
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
