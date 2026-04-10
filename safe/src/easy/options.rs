use crate::abi::{curl_easyoption, CURLoption, CURLOT_FLAG_ALIAS};
use core::ffi::c_char;

include!(concat!(env!("OUT_DIR"), "/easy_options.rs"));

fn ascii_lower(byte: u8) -> u8 {
    if byte.is_ascii_uppercase() {
        byte + 32
    } else {
        byte
    }
}

fn eq_ignore_case(lhs: *const c_char, rhs: *const c_char) -> bool {
    if lhs.is_null() || rhs.is_null() {
        return false;
    }

    let mut idx = 0usize;
    loop {
        let left = unsafe { *lhs.add(idx) } as u8;
        let right = unsafe { *rhs.add(idx) } as u8;
        if ascii_lower(left) != ascii_lower(right) {
            return false;
        }
        if left == 0 {
            return true;
        }
        idx += 1;
    }
}

pub(crate) fn option_count() -> usize {
    EASY_OPTION_COUNT
}

pub(crate) unsafe fn option_by_name(name: *const c_char) -> *const curl_easyoption {
    if name.is_null() {
        return core::ptr::null();
    }

    for option in &EASY_OPTIONS[..EASY_OPTION_COUNT] {
        if eq_ignore_case(option.name, name) {
            return option;
        }
    }
    core::ptr::null()
}

pub(crate) unsafe fn option_by_id(id: CURLoption) -> *const curl_easyoption {
    for option in &EASY_OPTIONS[..EASY_OPTION_COUNT] {
        if option.id == id && (option.flags & CURLOT_FLAG_ALIAS) == 0 {
            return option;
        }
    }
    core::ptr::null()
}

pub(crate) unsafe fn option_next(prev: *const curl_easyoption) -> *const curl_easyoption {
    if prev.is_null() {
        return &EASY_OPTIONS[0];
    }

    if unsafe { (*prev).name }.is_null() {
        return core::ptr::null();
    }

    let next = unsafe { prev.add(1) };
    if unsafe { (*next).name }.is_null() {
        core::ptr::null()
    } else {
        next
    }
}
