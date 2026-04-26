use crate::abi::{
    curl_lock_access, curl_lock_data, CURLSHcode, CURLSHoption, CURL, CURLSH, CURLSHE_BAD_OPTION,
    CURLSHE_OK, CURLSHOPT_LOCKFUNC, CURLSHOPT_SHARE, CURLSHOPT_UNLOCKFUNC, CURLSHOPT_UNSHARE,
    CURLSHOPT_USERDATA, CURL_LOCK_ACCESS_SINGLE, CURL_LOCK_DATA_CONNECT, CURL_LOCK_DATA_COOKIE,
    CURL_LOCK_DATA_DNS, CURL_LOCK_DATA_HSTS, CURL_LOCK_DATA_PSL, CURL_LOCK_DATA_SSL_SESSION,
};
use core::ffi::{c_char, c_int, c_void};
use std::collections::HashMap;

type CurlShareLockFn =
    Option<unsafe extern "C" fn(*mut CURL, curl_lock_data, curl_lock_access, *mut c_void)>;
type CurlShareUnlockFn = Option<unsafe extern "C" fn(*mut CURL, curl_lock_data, *mut c_void)>;

const CURLSHE_INVALID: CURLSHcode = 3;
const CURLSHE_NOT_BUILT_IN: CURLSHcode = 5;

#[repr(C)]
struct ShareHandle {
    state: ShareState,
    cookies: crate::http::cookies::CookieStore,
    hsts: crate::http::hsts::HstsStore,
    ssl_sessions: HashMap<String, Vec<u8>>,
}

#[derive(Clone, Copy, Default)]
struct ShareState {
    shared_mask: u32,
    lock_cb: CurlShareLockFn,
    unlock_cb: CurlShareUnlockFn,
    user_data: usize,
}

impl ShareState {
    fn is_shared(&self, data: curl_lock_data) -> bool {
        data < u32::BITS && (self.shared_mask & (1u32 << data)) != 0
    }

    fn share(&mut self, data: curl_lock_data) {
        if data < u32::BITS {
            self.shared_mask |= 1u32 << data;
        }
    }

    fn unshare(&mut self, data: curl_lock_data) {
        if data < u32::BITS {
            self.shared_mask &= !(1u32 << data);
        }
    }
}

unsafe fn handle_mut(handle: *mut CURLSH) -> Option<&'static mut ShareHandle> {
    if handle.is_null() {
        None
    } else {
        Some(unsafe { &mut *handle.cast::<ShareHandle>() })
    }
}

fn share_strerror_message(code: CURLSHcode) -> &'static [u8] {
    match code {
        CURLSHE_OK => b"No error\0",
        CURLSHE_BAD_OPTION => b"Unknown share option\0",
        2 => b"Share currently in use\0",
        CURLSHE_INVALID => b"Invalid share handle\0",
        4 => b"Out of memory\0",
        CURLSHE_NOT_BUILT_IN => b"Feature not built in\0",
        _ => b"Unknown share error\0",
    }
}

fn is_supported_shared_data(data: curl_lock_data) -> bool {
    matches!(
        data,
        CURL_LOCK_DATA_COOKIE
            | CURL_LOCK_DATA_DNS
            | CURL_LOCK_DATA_SSL_SESSION
            | CURL_LOCK_DATA_CONNECT
            | CURL_LOCK_DATA_PSL
            | CURL_LOCK_DATA_HSTS
    )
}

pub(crate) unsafe fn share_init() -> *mut CURLSH {
    Box::into_raw(Box::new(ShareHandle {
        state: ShareState::default(),
        cookies: crate::http::cookies::CookieStore::default(),
        hsts: crate::http::hsts::HstsStore::default(),
        ssl_sessions: HashMap::new(),
    }))
    .cast()
}

pub(crate) unsafe fn share_cleanup(handle: *mut CURLSH) -> CURLSHcode {
    let Some(handle) = (unsafe { handle_mut(handle) }) else {
        return CURLSHE_INVALID;
    };
    let state = handle.state;
    touch_state(
        &state,
        core::ptr::null_mut(),
        CURL_LOCK_DATA_CONNECT,
        CURL_LOCK_ACCESS_SINGLE,
        5,
    );
    drop(unsafe { Box::from_raw(handle as *mut ShareHandle) });
    CURLSHE_OK
}

pub(crate) unsafe fn share_strerror(code: CURLSHcode) -> *const c_char {
    share_strerror_message(code).as_ptr().cast()
}

pub(crate) fn share_setopt_int(
    handle: *mut CURLSH,
    option: CURLSHoption,
    value: c_int,
) -> CURLSHcode {
    let Some(handle) = (unsafe { handle_mut(handle) }) else {
        return CURLSHE_INVALID;
    };
    let data = value as curl_lock_data;
    match option {
        CURLSHOPT_SHARE => {
            if !is_supported_shared_data(data) {
                return CURLSHE_BAD_OPTION;
            }
            handle.state.share(data);
            CURLSHE_OK
        }
        CURLSHOPT_UNSHARE => {
            if !is_supported_shared_data(data) {
                return CURLSHE_BAD_OPTION;
            }
            handle.state.unshare(data);
            match data {
                CURL_LOCK_DATA_COOKIE => {
                    handle.cookies = crate::http::cookies::CookieStore::default()
                }
                CURL_LOCK_DATA_HSTS => handle.hsts = crate::http::hsts::HstsStore::default(),
                CURL_LOCK_DATA_SSL_SESSION => handle.ssl_sessions.clear(),
                _ => {}
            }
            CURLSHE_OK
        }
        _ => CURLSHE_BAD_OPTION,
    }
}

pub(crate) fn share_setopt_function(
    handle: *mut CURLSH,
    option: CURLSHoption,
    value: Option<unsafe extern "C" fn()>,
) -> CURLSHcode {
    let Some(handle) = (unsafe { handle_mut(handle) }) else {
        return CURLSHE_INVALID;
    };
    match option {
        CURLSHOPT_LOCKFUNC => {
            handle.state.lock_cb = unsafe { core::mem::transmute(value) };
            CURLSHE_OK
        }
        CURLSHOPT_UNLOCKFUNC => {
            handle.state.unlock_cb = unsafe { core::mem::transmute(value) };
            CURLSHE_OK
        }
        _ => CURLSHE_BAD_OPTION,
    }
}

pub(crate) fn share_setopt_ptr(
    handle: *mut CURLSH,
    option: CURLSHoption,
    value: *mut c_void,
) -> CURLSHcode {
    let Some(handle) = (unsafe { handle_mut(handle) }) else {
        return CURLSHE_INVALID;
    };
    match option {
        CURLSHOPT_USERDATA => {
            handle.state.user_data = value as usize;
            CURLSHE_OK
        }
        _ => CURLSHE_BAD_OPTION,
    }
}

pub(crate) fn touch_connect_callbacks(
    handle: *mut CURL,
    share_handle: Option<usize>,
    times: usize,
) {
    let Some(share_handle) = share_handle else {
        return;
    };
    let Some(share) = (unsafe { handle_mut(share_handle as *mut CURLSH) }) else {
        return;
    };
    if !share.state.is_shared(CURL_LOCK_DATA_CONNECT) {
        return;
    }
    let state = share.state;
    touch_state(
        &state,
        handle,
        CURL_LOCK_DATA_CONNECT,
        CURL_LOCK_ACCESS_SINGLE,
        times + 2,
    );
}

pub(crate) fn with_shared_cookies_mut<R>(
    share_handle: Option<usize>,
    f: impl FnOnce(&mut crate::http::cookies::CookieStore) -> R,
) -> Option<R> {
    let share = unsafe { handle_mut(share_handle? as *mut CURLSH) }?;
    if !share.state.is_shared(CURL_LOCK_DATA_COOKIE) {
        return None;
    }
    Some(f(&mut share.cookies))
}

pub(crate) fn with_shared_hsts_mut<R>(
    share_handle: Option<usize>,
    f: impl FnOnce(&mut crate::http::hsts::HstsStore) -> R,
) -> Option<R> {
    let share = unsafe { handle_mut(share_handle? as *mut CURLSH) }?;
    if !share.state.is_shared(CURL_LOCK_DATA_HSTS) {
        return None;
    }
    Some(f(&mut share.hsts))
}

pub(crate) fn with_shared_ssl_sessions_mut<R>(
    share_handle: Option<usize>,
    f: impl FnOnce(&mut HashMap<String, Vec<u8>>) -> R,
) -> Option<R> {
    let share = unsafe { handle_mut(share_handle? as *mut CURLSH) }?;
    if !share.state.is_shared(CURL_LOCK_DATA_SSL_SESSION) {
        return None;
    }
    Some(f(&mut share.ssl_sessions))
}

fn touch_state(
    state: &ShareState,
    handle: *mut CURL,
    data: curl_lock_data,
    access: curl_lock_access,
    times: usize,
) {
    if !state.is_shared(data) {
        return;
    }

    for _ in 0..times {
        if let Some(lock_cb) = state.lock_cb {
            unsafe { lock_cb(handle, data, access, state.user_data as *mut c_void) };
        }
        if let Some(unlock_cb) = state.unlock_cb {
            unsafe { unlock_cb(handle, data, state.user_data as *mut c_void) };
        }
    }
}
