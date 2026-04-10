use crate::abi::curl_slist;
use crate::alloc;
use core::ffi::{c_char, c_void};
use core::mem;
use core::ptr;

#[no_mangle]
pub unsafe extern "C" fn curl_slist_append(
    list: *mut curl_slist,
    data: *const c_char,
) -> *mut curl_slist {
    if data.is_null() {
        return ptr::null_mut();
    }

    let duplicate = unsafe { alloc::strdup_bytes(data) };
    if duplicate.is_null() {
        return ptr::null_mut();
    }

    let node = unsafe { alloc::malloc_bytes(mem::size_of::<curl_slist>()) as *mut curl_slist };
    if node.is_null() {
        unsafe { alloc::free_ptr(duplicate.cast::<c_void>()) };
        return ptr::null_mut();
    }

    unsafe {
        (*node).data = duplicate;
        (*node).next = ptr::null_mut();
    }

    if list.is_null() {
        return node;
    }

    let mut tail = list;
    unsafe {
        while !(*tail).next.is_null() {
            tail = (*tail).next;
        }
        (*tail).next = node;
    }
    list
}

#[no_mangle]
pub unsafe extern "C" fn curl_slist_free_all(mut list: *mut curl_slist) {
    while !list.is_null() {
        let next = unsafe { (*list).next };
        unsafe {
            alloc::free_ptr((*list).data.cast::<c_void>());
            alloc::free_ptr(list.cast::<c_void>());
        }
        list = next;
    }
}
