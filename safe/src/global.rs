use crate::abi::{
    curl_calloc_callback, curl_free_callback, curl_malloc_callback, curl_realloc_callback,
    curl_ssl_backend, curl_sslbackend, curl_strdup_callback, CURLcode, CURLsslset, CURL,
    CURLE_FAILED_INIT, CURLE_OK, CURLM, CURL_GLOBAL_DEFAULT,
};
use crate::{alloc, version};
use core::ffi::{c_char, c_long, c_void};
use std::mem;
use std::process;
use std::sync::{Mutex, OnceLock};

unsafe extern "C" {
    fn curl_safe_resolve_reference_symbol(name: *const c_char) -> *mut c_void;
}

#[derive(Clone, Copy)]
struct GlobalState {
    init_depth: usize,
}

static GLOBAL_STATE: Mutex<GlobalState> = Mutex::new(GlobalState { init_depth: 0 });

type CurlGlobalInitFn = unsafe extern "C" fn(c_long) -> CURLcode;
type CurlGlobalInitMemFn = unsafe extern "C" fn(
    c_long,
    curl_malloc_callback,
    curl_free_callback,
    curl_realloc_callback,
    curl_strdup_callback,
    curl_calloc_callback,
) -> CURLcode;
type CurlGlobalCleanupFn = unsafe extern "C" fn();
type CurlGlobalTraceFn = unsafe extern "C" fn(*const c_char) -> CURLcode;
type CurlGlobalSslSetFn = unsafe extern "C" fn(
    curl_sslbackend,
    *const c_char,
    *mut *const *const curl_ssl_backend,
) -> CURLsslset;
type CurlFreeFn = unsafe extern "C" fn(*mut c_void);
pub(crate) unsafe fn load_reference<T: Copy>(symbol: &'static [u8]) -> T {
    let ptr = unsafe { curl_safe_resolve_reference_symbol(symbol.as_ptr().cast()) };
    if ptr.is_null() {
        process::abort();
    }
    unsafe { mem::transmute_copy(&ptr) }
}

fn ref_global_init() -> CurlGlobalInitFn {
    static FN: OnceLock<CurlGlobalInitFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { load_reference(b"curl_global_init\0") })
}

fn ref_global_init_mem() -> CurlGlobalInitMemFn {
    static FN: OnceLock<CurlGlobalInitMemFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { load_reference(b"curl_global_init_mem\0") })
}

fn ref_global_cleanup() -> CurlGlobalCleanupFn {
    static FN: OnceLock<CurlGlobalCleanupFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { load_reference(b"curl_global_cleanup\0") })
}

fn ref_global_trace() -> CurlGlobalTraceFn {
    static FN: OnceLock<CurlGlobalTraceFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { load_reference(b"curl_global_trace\0") })
}

fn ref_global_sslset() -> CurlGlobalSslSetFn {
    static FN: OnceLock<CurlGlobalSslSetFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { load_reference(b"curl_global_sslset\0") })
}

fn ref_curl_free() -> CurlFreeFn {
    static FN: OnceLock<CurlFreeFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { load_reference(b"curl_free\0") })
}

pub(crate) fn ensure_global_init_for_easy() -> Result<(), CURLcode> {
    let should_init = GLOBAL_STATE
        .lock()
        .expect("global mutex poisoned")
        .init_depth
        == 0;
    if !should_init {
        return Ok(());
    }

    let code = unsafe { curl_global_init(CURL_GLOBAL_DEFAULT) };
    if code == CURLE_OK {
        Ok(())
    } else {
        Err(code)
    }
}

pub(crate) unsafe fn free_reference_allocation(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    unsafe { ref_curl_free()(ptr) };
}

#[no_mangle]
pub unsafe extern "C" fn curl_global_init(flags: c_long) -> CURLcode {
    let first_init = GLOBAL_STATE
        .lock()
        .expect("global mutex poisoned")
        .init_depth
        == 0;
    if first_init {
        version::clear_cached_version();
        alloc::reset_to_default();
    }

    let code = unsafe { ref_global_init()(flags) };
    if code == CURLE_OK {
        let mut state = GLOBAL_STATE.lock().expect("global mutex poisoned");
        state.init_depth += 1;
    }
    code
}

#[no_mangle]
pub unsafe extern "C" fn curl_global_init_mem(
    flags: c_long,
    malloc: curl_malloc_callback,
    free: curl_free_callback,
    realloc: curl_realloc_callback,
    strdup: curl_strdup_callback,
    calloc: curl_calloc_callback,
) -> CURLcode {
    if malloc.is_none()
        || free.is_none()
        || realloc.is_none()
        || strdup.is_none()
        || calloc.is_none()
    {
        return CURLE_FAILED_INIT;
    }

    let first_init = GLOBAL_STATE
        .lock()
        .expect("global mutex poisoned")
        .init_depth
        == 0;
    if first_init {
        version::clear_cached_version();
    }

    let code = unsafe { ref_global_init_mem()(flags, malloc, free, realloc, strdup, calloc) };
    if code == CURLE_OK {
        if first_init {
            alloc::set_custom(malloc, free, realloc, strdup, calloc);
        }
        let mut state = GLOBAL_STATE.lock().expect("global mutex poisoned");
        state.init_depth += 1;
    }
    code
}

#[no_mangle]
pub unsafe extern "C" fn curl_global_cleanup() {
    unsafe { ref_global_cleanup()() };

    let should_clear = {
        let mut state = GLOBAL_STATE.lock().expect("global mutex poisoned");
        if state.init_depth > 0 {
            state.init_depth -= 1;
        }
        state.init_depth == 0
    };

    if should_clear {
        version::clear_cached_version();
        crate::easy::perform::clear_registry();
    }
}

#[no_mangle]
pub unsafe extern "C" fn curl_global_trace(config: *const c_char) -> CURLcode {
    unsafe { ref_global_trace()(config) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_global_sslset(
    id: curl_sslbackend,
    name: *const c_char,
    avail: *mut *const *const curl_ssl_backend,
) -> CURLsslset {
    unsafe { ref_global_sslset()(id, name, avail) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_multi_get_handles(multi_handle: *mut CURLM) -> *mut *mut CURL {
    unsafe { crate::multi::get_handles_copy(multi_handle) }
}
