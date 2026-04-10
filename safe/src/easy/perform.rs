use crate::abi::{
    CURLcode, CURLoption, CURL, CURLE_BAD_FUNCTION_ARGUMENT, CURLE_FAILED_INIT, CURLM,
};
use crate::multi::state::MultiState;
use crate::transfer::{map_multi_code, EASY_PERFORM_WAIT_TIMEOUT_MS};
use core::ffi::{c_int, c_long, c_void};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

type CurlEasyPauseFn = unsafe extern "C" fn(*mut CURL, c_int) -> CURLcode;
type CurlEasyRecvFn = unsafe extern "C" fn(*mut CURL, *mut c_void, usize, *mut usize) -> CURLcode;
type CurlEasySendFn = unsafe extern "C" fn(*mut CURL, *const c_void, usize, *mut usize) -> CURLcode;
type CurlEasyUpkeepFn = unsafe extern "C" fn(*mut CURL) -> CURLcode;

const CURLOPT_MAXCONNECTS: CURLoption = 71;

#[derive(Clone, Copy, Debug)]
struct EasyShadow {
    private_multi: Option<usize>,
    attached_multi: Option<usize>,
    explicit_maxconnects: Option<c_long>,
    state: MultiState,
}

impl Default for EasyShadow {
    fn default() -> Self {
        Self {
            private_multi: None,
            attached_multi: None,
            explicit_maxconnects: None,
            state: MultiState::Init,
        }
    }
}

fn registry() -> &'static Mutex<HashMap<usize, EasyShadow>> {
    static REGISTRY: OnceLock<Mutex<HashMap<usize, EasyShadow>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ref_easy_pause() -> CurlEasyPauseFn {
    static FN: OnceLock<CurlEasyPauseFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { crate::global::load_reference(b"curl_easy_pause\0") })
}

fn ref_easy_recv() -> CurlEasyRecvFn {
    static FN: OnceLock<CurlEasyRecvFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { crate::global::load_reference(b"curl_easy_recv\0") })
}

fn ref_easy_send() -> CurlEasySendFn {
    static FN: OnceLock<CurlEasySendFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { crate::global::load_reference(b"curl_easy_send\0") })
}

fn ref_easy_upkeep() -> CurlEasyUpkeepFn {
    static FN: OnceLock<CurlEasyUpkeepFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { crate::global::load_reference(b"curl_easy_upkeep\0") })
}

pub(crate) fn register_handle(handle: *mut CURL) {
    if handle.is_null() {
        return;
    }
    registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .entry(handle as usize)
        .or_default();
}

pub(crate) fn register_duplicate(source: *mut CURL, duplicate: *mut CURL) {
    if duplicate.is_null() {
        return;
    }

    let explicit_maxconnects = registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get(&(source as usize))
        .and_then(|shadow| shadow.explicit_maxconnects);

    registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .insert(
            duplicate as usize,
            EasyShadow {
                explicit_maxconnects,
                ..EasyShadow::default()
            },
        );
}

pub(crate) fn reset_handle(handle: *mut CURL) {
    if handle.is_null() {
        return;
    }
    if let Some(shadow) = registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get_mut(&(handle as usize))
    {
        shadow.explicit_maxconnects = None;
        shadow.state = MultiState::Init;
    }
}

pub(crate) fn unregister_handle(handle: *mut CURL) -> Option<usize> {
    if handle.is_null() {
        return None;
    }

    let shadow = registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .remove(&(handle as usize));

    let Some(shadow) = shadow else {
        return None;
    };

    if let Some(attached_multi) = shadow.attached_multi {
        unsafe { crate::multi::drop_easy_reference(attached_multi as *mut CURLM, handle) };
    }

    shadow.private_multi
}

pub(crate) fn observe_easy_setopt_long(handle: *mut CURL, option: CURLoption, value: c_long) {
    if handle.is_null() || option != CURLOPT_MAXCONNECTS {
        return;
    }

    registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .entry(handle as usize)
        .or_default()
        .explicit_maxconnects = Some(value);
}

pub(crate) fn on_attached(handle: *mut CURL, multi: usize, next_state: MultiState) {
    if handle.is_null() {
        return;
    }

    let mut guard = registry().lock().expect("easy registry mutex poisoned");
    let shadow = guard.entry(handle as usize).or_default();
    shadow.attached_multi = Some(multi);
    shadow.state = MultiState::transition(shadow.state, next_state);
}

pub(crate) fn on_detached(handle: *mut CURL, multi: usize, next_state: MultiState) {
    if handle.is_null() {
        return;
    }

    let mut guard = registry().lock().expect("easy registry mutex poisoned");
    if let Some(shadow) = guard.get_mut(&(handle as usize)) {
        if shadow.attached_multi == Some(multi) {
            shadow.attached_multi = None;
        }
        shadow.state = MultiState::transition(shadow.state, next_state);
    }
}

pub(crate) fn on_transfer_progress(handle: *mut CURL, next_state: MultiState) {
    if handle.is_null() {
        return;
    }

    let mut guard = registry().lock().expect("easy registry mutex poisoned");
    if let Some(shadow) = guard.get_mut(&(handle as usize)) {
        shadow.state = MultiState::transition(shadow.state, next_state);
    }
}

pub(crate) fn mark_message_sent(handle: *mut CURL) {
    on_transfer_progress(handle, MultiState::MsgSent);
}

fn private_multi_for(handle: *mut CURL) -> Option<usize> {
    registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get(&(handle as usize))
        .and_then(|shadow| shadow.private_multi)
}

fn explicit_maxconnects_for(handle: *mut CURL) -> Option<c_long> {
    registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get(&(handle as usize))
        .and_then(|shadow| shadow.explicit_maxconnects)
}

pub(crate) fn attached_multi_for(handle: *mut CURL) -> Option<usize> {
    registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get(&(handle as usize))
        .and_then(|shadow| shadow.attached_multi)
}

fn set_private_multi(handle: *mut CURL, multi: Option<usize>) {
    registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .entry(handle as usize)
        .or_default()
        .private_multi = multi;
}

pub(crate) unsafe fn easy_perform(handle: *mut CURL) -> CURLcode {
    if handle.is_null() {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    }
    register_handle(handle);

    if attached_multi_for(handle).is_some() {
        return CURLE_FAILED_INIT;
    }

    let mut created_multi = false;
    let multi = if let Some(existing) = private_multi_for(handle) {
        existing as *mut CURLM
    } else {
        let new_multi = unsafe { crate::multi::init_handle() };
        if new_multi.is_null() {
            return crate::abi::CURLE_OUT_OF_MEMORY;
        }
        set_private_multi(handle, Some(new_multi as usize));
        created_multi = true;
        new_multi
    };

    if let Some(maxconnects) = explicit_maxconnects_for(handle) {
        let _ = unsafe {
            crate::multi::dispatch_setopt_long(
                multi,
                crate::multi::CURLMOPT_MAXCONNECTS,
                maxconnects,
            )
        };
    }

    let add_code = unsafe { crate::multi::add_handle(multi, handle) };
    if add_code != crate::abi::CURLM_OK {
        if created_multi {
            let _ = unsafe { crate::multi::cleanup_handle(multi) };
            set_private_multi(handle, None);
        }
        return if add_code == crate::multi::CURLM_OUT_OF_MEMORY {
            crate::abi::CURLE_OUT_OF_MEMORY
        } else {
            CURLE_FAILED_INIT
        };
    }

    let mut result = crate::abi::CURLE_OK;
    loop {
        let poll_code = unsafe {
            crate::multi::poll_handle(
                multi,
                core::ptr::null_mut(),
                0,
                EASY_PERFORM_WAIT_TIMEOUT_MS,
                core::ptr::null_mut(),
            )
        };
        if poll_code != crate::abi::CURLM_OK {
            result = map_multi_code(poll_code);
            break;
        }

        let mut still_running = 0;
        let perform_code = unsafe { crate::multi::perform_handle(multi, &mut still_running) };
        if perform_code != crate::abi::CURLM_OK {
            result = map_multi_code(perform_code);
            break;
        }

        if still_running == 0 {
            let mut queued = 0;
            let msg = unsafe { crate::multi::info_read_handle(multi, &mut queued) };
            if !msg.is_null() && unsafe { (*msg).msg == crate::multi::CURLMSG_DONE } {
                result = unsafe { (*msg).data.result };
            }
            break;
        }
    }

    let _ = unsafe { crate::multi::remove_handle(multi, handle) };
    result
}

pub(crate) unsafe fn easy_pause(handle: *mut CURL, bitmask: c_int) -> CURLcode {
    unsafe { ref_easy_pause()(handle, bitmask) }
}

pub(crate) unsafe fn easy_recv(
    handle: *mut CURL,
    buffer: *mut c_void,
    buflen: usize,
    nread: *mut usize,
) -> CURLcode {
    unsafe { ref_easy_recv()(handle, buffer, buflen, nread) }
}

pub(crate) unsafe fn easy_send(
    handle: *mut CURL,
    buffer: *const c_void,
    buflen: usize,
    nwritten: *mut usize,
) -> CURLcode {
    unsafe { ref_easy_send()(handle, buffer, buflen, nwritten) }
}

pub(crate) unsafe fn easy_upkeep(handle: *mut CURL) -> CURLcode {
    unsafe { ref_easy_upkeep()(handle) }
}
