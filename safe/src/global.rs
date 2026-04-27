use crate::abi::{
    curl_calloc_callback, curl_free_callback, curl_malloc_callback, curl_realloc_callback,
    curl_ssl_backend, curl_sslbackend, curl_strdup_callback, CURLcode, CURLsslset, CURL,
    CURLE_FAILED_INIT, CURLE_OK, CURLM, CURLSSLBACKEND_GNUTLS, CURLSSLBACKEND_OPENSSL,
    CURLSSLSET_OK, CURLSSLSET_TOO_LATE, CURLSSLSET_UNKNOWN_BACKEND, CURL_GLOBAL_DEFAULT,
};
use crate::{alloc, version, BUILD_FLAVOR};
use core::ffi::{c_char, c_long, c_void};
use std::mem;
use std::sync::Mutex;

unsafe extern "C" {
    fn port_safe_resolve_reference_symbol(name: *const c_char) -> *mut c_void;
}

#[derive(Clone, Copy)]
struct GlobalState {
    init_depth: usize,
    ssl_backend_locked: bool,
}

static GLOBAL_STATE: Mutex<GlobalState> = Mutex::new(GlobalState {
    init_depth: 0,
    ssl_backend_locked: false,
});

struct SyncBackends([curl_ssl_backend; 1]);
unsafe impl Sync for SyncBackends {}

struct SyncBackendList([*const curl_ssl_backend; 2]);
unsafe impl Sync for SyncBackendList {}

pub(crate) unsafe fn load_reference<T: Copy>(symbol: &'static [u8]) -> T {
    let ptr = unsafe { port_safe_resolve_reference_symbol(symbol.as_ptr().cast()) };
    if ptr.is_null() {
        std::process::abort();
    }
    unsafe { mem::transmute_copy(&ptr) }
}

fn compiled_ssl_backend_id() -> curl_sslbackend {
    if BUILD_FLAVOR == "openssl" {
        CURLSSLBACKEND_OPENSSL
    } else {
        CURLSSLBACKEND_GNUTLS
    }
}

fn compiled_ssl_backend_name() -> *const c_char {
    if BUILD_FLAVOR == "openssl" {
        c"openssl".as_ptr()
    } else {
        c"gnutls".as_ptr()
    }
}

static SSL_BACKENDS: SyncBackends = SyncBackends([curl_ssl_backend {
    id: if cfg!(feature = "openssl-flavor") {
        CURLSSLBACKEND_OPENSSL
    } else {
        CURLSSLBACKEND_GNUTLS
    },
    name: if cfg!(feature = "openssl-flavor") {
        c"openssl".as_ptr()
    } else {
        c"gnutls".as_ptr()
    },
}]);

static SSL_BACKEND_LIST: SyncBackendList =
    SyncBackendList([SSL_BACKENDS.0.as_ptr(), core::ptr::null()]);

fn set_backend_list(avail: *mut *const *const curl_ssl_backend) {
    if !avail.is_null() {
        unsafe {
            *avail = SSL_BACKEND_LIST.0.as_ptr();
        }
    }
}

fn matches_backend_name(name: *const c_char) -> bool {
    if name.is_null() {
        return false;
    }
    let expected = unsafe { std::ffi::CStr::from_ptr(compiled_ssl_backend_name()) };
    let actual = unsafe { std::ffi::CStr::from_ptr(name) };
    expected.to_bytes().eq_ignore_ascii_case(actual.to_bytes())
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

#[no_mangle]
pub unsafe extern "C" fn curl_global_init(flags: c_long) -> CURLcode {
    let _ = flags;
    let mut state = GLOBAL_STATE.lock().expect("global mutex poisoned");
    if state.init_depth == 0 {
        version::clear_cached_version();
        alloc::reset_to_default();
    }
    state.init_depth += 1;
    state.ssl_backend_locked = true;
    CURLE_OK
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
    let _ = flags;
    if malloc.is_none()
        || free.is_none()
        || realloc.is_none()
        || strdup.is_none()
        || calloc.is_none()
    {
        return CURLE_FAILED_INIT;
    }

    let mut state = GLOBAL_STATE.lock().expect("global mutex poisoned");
    if state.init_depth == 0 {
        version::clear_cached_version();
        alloc::set_custom(malloc, free, realloc, strdup, calloc);
    }
    state.init_depth += 1;
    state.ssl_backend_locked = true;
    CURLE_OK
}

#[no_mangle]
pub unsafe extern "C" fn curl_global_cleanup() {
    let should_clear = {
        let mut state = GLOBAL_STATE.lock().expect("global mutex poisoned");
        if state.init_depth > 0 {
            state.init_depth -= 1;
        }
        if state.init_depth == 0 {
            state.ssl_backend_locked = false;
            true
        } else {
            false
        }
    };

    if should_clear {
        unsafe { crate::easy::reference::clear_all() };
        version::clear_cached_version();
        alloc::reset_to_default();
        crate::easy::perform::clear_registry();
    }
}

#[no_mangle]
pub unsafe extern "C" fn curl_global_trace(_config: *const c_char) -> CURLcode {
    CURLE_OK
}

#[no_mangle]
pub unsafe extern "C" fn curl_global_sslset(
    id: curl_sslbackend,
    name: *const c_char,
    avail: *mut *const *const curl_ssl_backend,
) -> CURLsslset {
    set_backend_list(avail);

    let mut state = GLOBAL_STATE.lock().expect("global mutex poisoned");
    if state.init_depth != 0 || state.ssl_backend_locked {
        return CURLSSLSET_TOO_LATE;
    }

    let id_matches = id == compiled_ssl_backend_id();
    let name_matches = id == u32::MAX && matches_backend_name(name);
    if id_matches || name_matches {
        state.ssl_backend_locked = true;
        CURLSSLSET_OK
    } else {
        CURLSSLSET_UNKNOWN_BACKEND
    }
}

#[no_mangle]
pub unsafe extern "C" fn curl_multi_get_handles(multi_handle: *mut CURLM) -> *mut *mut CURL {
    unsafe { crate::multi::get_handles_copy(multi_handle) }
}
