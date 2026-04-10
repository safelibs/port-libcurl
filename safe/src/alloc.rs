use crate::abi::{
    curl_calloc_callback, curl_free_callback, curl_malloc_callback, curl_realloc_callback,
    curl_strdup_callback, size_t,
};
use core::ffi::{c_char, c_void};
use core::ptr;
use std::sync::Mutex;

unsafe extern "C" {
    fn malloc(size: size_t) -> *mut c_void;
    fn free(ptr: *mut c_void);
    fn realloc(ptr: *mut c_void, size: size_t) -> *mut c_void;
    fn calloc(nmemb: size_t, size: size_t) -> *mut c_void;
    fn strdup(s: *const c_char) -> *mut c_char;
}

#[derive(Clone, Copy)]
pub(crate) struct AllocatorFns {
    pub(crate) malloc: curl_malloc_callback,
    pub(crate) free: curl_free_callback,
    pub(crate) realloc: curl_realloc_callback,
    pub(crate) strdup: curl_strdup_callback,
    pub(crate) calloc: curl_calloc_callback,
}

unsafe extern "C" fn libc_malloc(size: size_t) -> *mut c_void {
    unsafe { malloc(size) }
}

unsafe extern "C" fn libc_free(ptr: *mut c_void) {
    unsafe { free(ptr) }
}

unsafe extern "C" fn libc_realloc(ptr: *mut c_void, size: size_t) -> *mut c_void {
    unsafe { realloc(ptr, size) }
}

unsafe extern "C" fn libc_calloc(nmemb: size_t, size: size_t) -> *mut c_void {
    unsafe { calloc(nmemb, size) }
}

unsafe extern "C" fn libc_strdup(s: *const c_char) -> *mut c_char {
    unsafe { strdup(s) }
}

const DEFAULT_ALLOCATOR: AllocatorFns = AllocatorFns {
    malloc: Some(libc_malloc),
    free: Some(libc_free),
    realloc: Some(libc_realloc),
    strdup: Some(libc_strdup),
    calloc: Some(libc_calloc),
};

static ALLOCATOR: Mutex<AllocatorFns> = Mutex::new(DEFAULT_ALLOCATOR);

pub(crate) fn snapshot() -> AllocatorFns {
    *ALLOCATOR.lock().expect("allocator mutex poisoned")
}

pub(crate) fn reset_to_default() {
    *ALLOCATOR.lock().expect("allocator mutex poisoned") = DEFAULT_ALLOCATOR;
}

pub(crate) fn set_custom(
    malloc: curl_malloc_callback,
    free: curl_free_callback,
    realloc: curl_realloc_callback,
    strdup: curl_strdup_callback,
    calloc: curl_calloc_callback,
) {
    *ALLOCATOR.lock().expect("allocator mutex poisoned") = AllocatorFns {
        malloc,
        free,
        realloc,
        strdup,
        calloc,
    };
}

pub(crate) unsafe fn malloc_bytes(size: usize) -> *mut c_void {
    match snapshot().malloc {
        Some(callback) => unsafe { callback(size) },
        None => ptr::null_mut(),
    }
}

pub(crate) unsafe fn calloc_bytes(nmemb: usize, size: usize) -> *mut c_void {
    match snapshot().calloc {
        Some(callback) => unsafe { callback(nmemb, size) },
        None => ptr::null_mut(),
    }
}

pub(crate) unsafe fn realloc_bytes(ptr: *mut c_void, size: usize) -> *mut c_void {
    match snapshot().realloc {
        Some(callback) => unsafe { callback(ptr, size) },
        None => ptr::null_mut(),
    }
}

pub(crate) unsafe fn strdup_bytes(ptr: *const c_char) -> *mut c_char {
    match snapshot().strdup {
        Some(callback) => unsafe { callback(ptr) },
        None => ptr::null_mut(),
    }
}

pub(crate) unsafe fn free_ptr(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    if let Some(callback) = snapshot().free {
        unsafe { callback(ptr) };
    }
}

pub(crate) unsafe fn alloc_and_copy(bytes: &[u8]) -> *mut c_char {
    let allocation = unsafe { malloc_bytes(bytes.len() + 1) } as *mut u8;
    if allocation.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), allocation, bytes.len());
        *allocation.add(bytes.len()) = 0;
    }
    allocation.cast()
}

#[no_mangle]
pub unsafe extern "C" fn curl_free(ptr: *mut c_void) {
    unsafe { free_ptr(ptr) };
}

#[no_mangle]
pub unsafe extern "C" fn curl_safe_malloc(size: size_t) -> *mut c_void {
    unsafe { malloc_bytes(size) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_safe_calloc(nmemb: size_t, size: size_t) -> *mut c_void {
    unsafe { calloc_bytes(nmemb, size) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_safe_realloc(ptr: *mut c_void, size: size_t) -> *mut c_void {
    unsafe { realloc_bytes(ptr, size) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_safe_strdup(ptr: *const c_char) -> *mut c_char {
    unsafe { strdup_bytes(ptr) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_safe_free(ptr: *mut c_void) {
    unsafe { free_ptr(ptr) };
}
