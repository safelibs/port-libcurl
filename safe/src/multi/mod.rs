pub(crate) mod poll;
pub(crate) mod state;

use crate::abi::{
    curl_off_t, curl_socket_t, CURLMcode, CURLMoption, CURLMsg, CURLcode, CURL, CURLM, CURLMSG,
};
use crate::{alloc, global};
use core::ffi::{c_char, c_int, c_long, c_uint, c_void};
use core::{mem, ptr};
use std::collections::VecDeque;
use std::mem::size_of;
use std::sync::{Mutex, OnceLock};

pub(crate) const CURLM_BAD_HANDLE: CURLMcode = 1;
pub(crate) const CURLM_BAD_EASY_HANDLE: CURLMcode = 2;
pub(crate) const CURLM_OUT_OF_MEMORY: CURLMcode = 3;
pub(crate) const CURLM_INTERNAL_ERROR: CURLMcode = 4;
pub(crate) const CURLM_BAD_SOCKET: CURLMcode = 5;
pub(crate) const CURLM_UNKNOWN_OPTION: CURLMcode = 6;
pub(crate) const CURLM_ADDED_ALREADY: CURLMcode = 7;
pub(crate) const CURLM_RECURSIVE_API_CALL: CURLMcode = 8;
pub(crate) const CURLM_WAKEUP_FAILURE: CURLMcode = 9;
pub(crate) const CURLM_BAD_FUNCTION_ARGUMENT: CURLMcode = 10;
pub(crate) const CURLM_ABORTED_BY_CALLBACK: CURLMcode = 11;
pub(crate) const CURLM_UNRECOVERABLE_POLL: CURLMcode = 12;

pub(crate) const CURLMSG_DONE: CURLMSG = 1;

const MULTI_MAGIC: usize = 0x4352_4d52;

const CURLMOPT_SOCKETFUNCTION: CURLMoption = 20001;
const CURLMOPT_SOCKETDATA: CURLMoption = 10002;
const CURLMOPT_TIMERFUNCTION: CURLMoption = 20004;
const CURLMOPT_TIMERDATA: CURLMoption = 10005;
pub(crate) const CURLMOPT_MAXCONNECTS: CURLMoption = 6;

type CurlSocketCallback = Option<
    unsafe extern "C" fn(*mut CURL, curl_socket_t, c_int, *mut c_void, *mut c_void) -> c_int,
>;
type CurlMultiTimerCallback =
    Option<unsafe extern "C" fn(*mut CURLM, c_long, *mut c_void) -> c_int>;

type RefMultiInitFn = unsafe extern "C" fn() -> *mut CURLM;
type RefMultiCleanupFn = unsafe extern "C" fn(*mut CURLM) -> CURLMcode;
type RefMultiAddHandleFn = unsafe extern "C" fn(*mut CURLM, *mut CURL) -> CURLMcode;
type RefMultiRemoveHandleFn = unsafe extern "C" fn(*mut CURLM, *mut CURL) -> CURLMcode;
type RefMultiFdsetFn = unsafe extern "C" fn(
    *mut CURLM,
    *mut libc_fd_set,
    *mut libc_fd_set,
    *mut libc_fd_set,
    *mut c_int,
) -> CURLMcode;
type RefMultiPerformFn = unsafe extern "C" fn(*mut CURLM, *mut c_int) -> CURLMcode;
type RefMultiWaitFn = unsafe extern "C" fn(
    *mut CURLM,
    *mut crate::abi::curl_waitfd,
    c_uint,
    c_int,
    *mut c_int,
) -> CURLMcode;
type RefMultiPollFn = RefMultiWaitFn;
type RefMultiTimeoutFn = unsafe extern "C" fn(*mut CURLM, *mut c_long) -> CURLMcode;
type RefMultiWakeupFn = unsafe extern "C" fn(*mut CURLM) -> CURLMcode;
type RefMultiInfoReadFn = unsafe extern "C" fn(*mut CURLM, *mut c_int) -> *mut CURLMsg;
type RefMultiSocketFn = unsafe extern "C" fn(*mut CURLM, curl_socket_t, *mut c_int) -> CURLMcode;
type RefMultiSocketAllFn = unsafe extern "C" fn(*mut CURLM, *mut c_int) -> CURLMcode;
type RefMultiSocketActionFn =
    unsafe extern "C" fn(*mut CURLM, curl_socket_t, c_int, *mut c_int) -> CURLMcode;
type RefMultiAssignFn = unsafe extern "C" fn(*mut CURLM, curl_socket_t, *mut c_void) -> CURLMcode;
type RefMultiStrErrorFn = unsafe extern "C" fn(CURLMcode) -> *const c_char;
type RefMultiSetoptFn = unsafe extern "C" fn(*mut CURLM, CURLMoption, ...) -> CURLMcode;
type RefMultiGetHandlesFn = unsafe extern "C" fn(*mut CURLM) -> *mut *mut CURL;

#[repr(C)]
pub(crate) struct libc_fd_set {
    fds_bits: [c_long; 16],
}

#[derive(Clone, Copy)]
struct QueuedMessage {
    easy_handle: *mut CURL,
    result: CURLcode,
}

#[derive(Clone, Copy, Default)]
struct CallbackState {
    socket_cb: CurlSocketCallback,
    socket_userp: *mut c_void,
    timer_cb: CurlMultiTimerCallback,
    timer_userp: *mut c_void,
}

#[derive(Default)]
struct MultiInner {
    easies: Vec<*mut CURL>,
    messages: VecDeque<QueuedMessage>,
    current_msg: Option<Box<CURLMsg>>,
    callbacks: CallbackState,
}

pub(crate) struct MultiHandle {
    magic: usize,
    ref_multi: *mut CURLM,
    inner: Mutex<MultiInner>,
}

unsafe impl Send for MultiHandle {}
unsafe impl Sync for MultiHandle {}

fn ref_multi_init() -> RefMultiInitFn {
    static FN: OnceLock<RefMultiInitFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_init\0") })
}

fn ref_multi_cleanup() -> RefMultiCleanupFn {
    static FN: OnceLock<RefMultiCleanupFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_cleanup\0") })
}

fn ref_multi_add_handle() -> RefMultiAddHandleFn {
    static FN: OnceLock<RefMultiAddHandleFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_add_handle\0") })
}

fn ref_multi_remove_handle() -> RefMultiRemoveHandleFn {
    static FN: OnceLock<RefMultiRemoveHandleFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_remove_handle\0") })
}

fn ref_multi_fdset() -> RefMultiFdsetFn {
    static FN: OnceLock<RefMultiFdsetFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_fdset\0") })
}

fn ref_multi_perform() -> RefMultiPerformFn {
    static FN: OnceLock<RefMultiPerformFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_perform\0") })
}

fn ref_multi_wait() -> RefMultiWaitFn {
    static FN: OnceLock<RefMultiWaitFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_wait\0") })
}

fn ref_multi_poll() -> RefMultiPollFn {
    static FN: OnceLock<RefMultiPollFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_poll\0") })
}

fn ref_multi_timeout() -> RefMultiTimeoutFn {
    static FN: OnceLock<RefMultiTimeoutFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_timeout\0") })
}

fn ref_multi_wakeup() -> RefMultiWakeupFn {
    static FN: OnceLock<RefMultiWakeupFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_wakeup\0") })
}

fn ref_multi_info_read() -> RefMultiInfoReadFn {
    static FN: OnceLock<RefMultiInfoReadFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_info_read\0") })
}

fn ref_multi_socket() -> RefMultiSocketFn {
    static FN: OnceLock<RefMultiSocketFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_socket\0") })
}

fn ref_multi_socket_action() -> RefMultiSocketActionFn {
    static FN: OnceLock<RefMultiSocketActionFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_socket_action\0") })
}

fn ref_multi_socket_all() -> RefMultiSocketAllFn {
    static FN: OnceLock<RefMultiSocketAllFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_socket_all\0") })
}

fn ref_multi_assign() -> RefMultiAssignFn {
    static FN: OnceLock<RefMultiAssignFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_assign\0") })
}

fn ref_multi_strerror() -> RefMultiStrErrorFn {
    static FN: OnceLock<RefMultiStrErrorFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_strerror\0") })
}

fn ref_multi_setopt() -> RefMultiSetoptFn {
    static FN: OnceLock<RefMultiSetoptFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_setopt\0") })
}

fn ref_multi_get_handles() -> RefMultiGetHandlesFn {
    static FN: OnceLock<RefMultiGetHandlesFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_get_handles\0") })
}

fn wrapper_from_ptr(multi: *mut CURLM) -> Option<&'static MultiHandle> {
    if multi.is_null() {
        return None;
    }
    let wrapper = unsafe { &*(multi as *mut MultiHandle) };
    if wrapper.magic != MULTI_MAGIC {
        return None;
    }
    Some(wrapper)
}

fn update_progress_state(wrapper: &MultiHandle, next: state::MultiState) {
    let easies = wrapper
        .inner
        .lock()
        .expect("multi mutex poisoned")
        .easies
        .clone();
    for easy in easies {
        crate::easy::perform::on_transfer_progress(easy, next);
    }
}

fn drain_messages(wrapper: &MultiHandle) {
    let mut drained = Vec::new();
    loop {
        let msg = unsafe { ref_multi_info_read()(wrapper.ref_multi, ptr::null_mut()) };
        if msg.is_null() {
            break;
        }
        let easy_handle = unsafe { (*msg).easy_handle };
        let result = unsafe { (*msg).data.result };
        drained.push(QueuedMessage {
            easy_handle,
            result,
        });
    }

    if drained.is_empty() {
        return;
    }

    let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
    for item in drained {
        crate::easy::perform::on_transfer_progress(item.easy_handle, state::MultiState::Completed);
        guard.messages.push_back(item);
    }
}

unsafe extern "C" fn timer_trampoline(
    _multi: *mut CURLM,
    timeout_ms: c_long,
    userp: *mut c_void,
) -> c_int {
    let Some(wrapper) = wrapper_from_ptr(userp.cast()) else {
        return 0;
    };
    let (callback, callback_userp) = {
        let guard = wrapper.inner.lock().expect("multi mutex poisoned");
        (guard.callbacks.timer_cb, guard.callbacks.timer_userp)
    };
    if let Some(callback) = callback {
        unsafe { callback(userp.cast(), timeout_ms, callback_userp) }
    } else {
        0
    }
}

unsafe extern "C" fn socket_trampoline(
    easy: *mut CURL,
    socket: curl_socket_t,
    what: c_int,
    userp: *mut c_void,
    socketp: *mut c_void,
) -> c_int {
    let Some(wrapper) = wrapper_from_ptr(userp.cast()) else {
        return 0;
    };
    let (callback, callback_userp) = {
        let guard = wrapper.inner.lock().expect("multi mutex poisoned");
        (guard.callbacks.socket_cb, guard.callbacks.socket_userp)
    };
    if let Some(callback) = callback {
        unsafe { callback(easy, socket, what, callback_userp, socketp) }
    } else {
        0
    }
}

pub(crate) unsafe fn init_handle() -> *mut CURLM {
    let ref_multi = unsafe { ref_multi_init()() };
    if ref_multi.is_null() {
        return ptr::null_mut();
    }

    let wrapper = Box::new(MultiHandle {
        magic: MULTI_MAGIC,
        ref_multi,
        inner: Mutex::new(MultiInner::default()),
    });
    let raw = Box::into_raw(wrapper);
    let self_ptr = raw.cast::<CURLM>();
    let self_data = raw.cast::<c_void>();

    let socket_data_rc = unsafe { ref_multi_setopt()(ref_multi, CURLMOPT_SOCKETDATA, self_data) };
    let timer_data_rc = unsafe { ref_multi_setopt()(ref_multi, CURLMOPT_TIMERDATA, self_data) };
    if socket_data_rc != crate::abi::CURLM_OK || timer_data_rc != crate::abi::CURLM_OK {
        let _ = unsafe { ref_multi_cleanup()(ref_multi) };
        unsafe { drop(Box::from_raw(raw)) };
        return ptr::null_mut();
    }

    self_ptr
}

pub(crate) unsafe fn cleanup_owned_multi(multi: *mut CURLM) {
    if multi.is_null() {
        return;
    }
    let _ = unsafe { cleanup_handle(multi) };
}

pub(crate) unsafe fn cleanup_handle(multi: *mut CURLM) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return unsafe { ref_multi_cleanup()(multi) };
    };

    let easies = wrapper
        .inner
        .lock()
        .expect("multi mutex poisoned")
        .easies
        .clone();
    for easy in easies {
        crate::easy::perform::on_detached(easy, multi as usize, state::MultiState::Init);
    }

    let raw = multi as *mut MultiHandle;
    let code = unsafe { ref_multi_cleanup()(wrapper.ref_multi) };
    if code == crate::abi::CURLM_OK {
        unsafe {
            (*raw).magic = 0;
            drop(Box::from_raw(raw));
        }
    }
    code
}

pub(crate) unsafe fn add_handle(multi: *mut CURLM, easy: *mut CURL) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return unsafe { ref_multi_add_handle()(multi, easy) };
    };
    if easy.is_null() {
        return CURLM_BAD_EASY_HANDLE;
    }
    if crate::easy::perform::attached_multi_for(easy).is_some() {
        return CURLM_ADDED_ALREADY;
    }

    let code = unsafe { ref_multi_add_handle()(wrapper.ref_multi, easy) };
    if code == crate::abi::CURLM_OK {
        let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
        if !guard.easies.contains(&easy) {
            guard.easies.push(easy);
        }
        drop(guard);
        crate::easy::perform::on_attached(easy, multi as usize, state::MultiState::Pending);
    }
    code
}

pub(crate) unsafe fn remove_handle(multi: *mut CURLM, easy: *mut CURL) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return unsafe { ref_multi_remove_handle()(multi, easy) };
    };

    let code = unsafe { ref_multi_remove_handle()(wrapper.ref_multi, easy) };
    if code == crate::abi::CURLM_OK {
        drop_easy_reference(multi, easy);
        crate::easy::perform::on_detached(easy, multi as usize, state::MultiState::Done);
    }
    code
}

pub(crate) unsafe fn drop_easy_reference(multi: *mut CURLM, easy: *mut CURL) {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return;
    };

    let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
    guard.easies.retain(|candidate| *candidate != easy);
    guard.messages.retain(|msg| msg.easy_handle != easy);
    if guard
        .current_msg
        .as_ref()
        .is_some_and(|msg| msg.easy_handle == easy)
    {
        guard.current_msg = None;
    }
}

pub(crate) unsafe fn fdset_handle(
    multi: *mut CURLM,
    read_fd_set: *mut libc_fd_set,
    write_fd_set: *mut libc_fd_set,
    exc_fd_set: *mut libc_fd_set,
    max_fd: *mut c_int,
) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return unsafe { ref_multi_fdset()(multi, read_fd_set, write_fd_set, exc_fd_set, max_fd) };
    };
    unsafe {
        ref_multi_fdset()(
            wrapper.ref_multi,
            read_fd_set,
            write_fd_set,
            exc_fd_set,
            max_fd,
        )
    }
}

pub(crate) unsafe fn perform_handle(multi: *mut CURLM, running_handles: *mut c_int) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return unsafe { ref_multi_perform()(multi, running_handles) };
    };

    let code = unsafe { ref_multi_perform()(wrapper.ref_multi, running_handles) };
    if code == crate::abi::CURLM_OK {
        update_progress_state(wrapper, state::MultiState::Performing);
        drain_messages(wrapper);
    }
    code
}

pub(crate) unsafe fn wait_handle(
    multi: *mut CURLM,
    extra_fds: *mut crate::abi::curl_waitfd,
    extra_nfds: c_uint,
    timeout_ms: c_int,
    ret: *mut c_int,
) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return unsafe { ref_multi_wait()(multi, extra_fds, extra_nfds, timeout_ms, ret) };
    };
    unsafe { ref_multi_wait()(wrapper.ref_multi, extra_fds, extra_nfds, timeout_ms, ret) }
}

pub(crate) unsafe fn poll_handle(
    multi: *mut CURLM,
    extra_fds: *mut crate::abi::curl_waitfd,
    extra_nfds: c_uint,
    timeout_ms: c_int,
    ret: *mut c_int,
) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return unsafe { ref_multi_poll()(multi, extra_fds, extra_nfds, timeout_ms, ret) };
    };
    unsafe { ref_multi_poll()(wrapper.ref_multi, extra_fds, extra_nfds, timeout_ms, ret) }
}

pub(crate) unsafe fn timeout_handle(multi: *mut CURLM, milliseconds: *mut c_long) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return unsafe { ref_multi_timeout()(multi, milliseconds) };
    };
    unsafe { ref_multi_timeout()(wrapper.ref_multi, milliseconds) }
}

pub(crate) unsafe fn wakeup_handle(multi: *mut CURLM) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return unsafe { ref_multi_wakeup()(multi) };
    };
    unsafe { ref_multi_wakeup()(wrapper.ref_multi) }
}

pub(crate) unsafe fn info_read_handle(
    multi: *mut CURLM,
    msgs_in_queue: *mut c_int,
) -> *mut CURLMsg {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return unsafe { ref_multi_info_read()(multi, msgs_in_queue) };
    };

    drain_messages(wrapper);
    let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
    if let Some(entry) = guard.messages.pop_front() {
        guard.current_msg = Some(Box::new(CURLMsg {
            msg: CURLMSG_DONE,
            easy_handle: entry.easy_handle,
            data: crate::abi::CURLMsgData {
                result: entry.result,
            },
        }));
        crate::easy::perform::mark_message_sent(entry.easy_handle);
        if !msgs_in_queue.is_null() {
            unsafe { *msgs_in_queue = guard.messages.len() as c_int };
        }
        guard
            .current_msg
            .as_mut()
            .map(|msg| &mut **msg as *mut CURLMsg)
            .unwrap_or(ptr::null_mut())
    } else {
        if !msgs_in_queue.is_null() {
            unsafe { *msgs_in_queue = 0 };
        }
        guard.current_msg = None;
        ptr::null_mut()
    }
}

pub(crate) unsafe fn socket_handle(
    multi: *mut CURLM,
    socket: curl_socket_t,
    running_handles: *mut c_int,
) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return unsafe { ref_multi_socket()(multi, socket, running_handles) };
    };

    let code = unsafe { ref_multi_socket()(wrapper.ref_multi, socket, running_handles) };
    if code == crate::abi::CURLM_OK {
        update_progress_state(wrapper, state::MultiState::Performing);
        drain_messages(wrapper);
    }
    code
}

pub(crate) unsafe fn socket_all_handle(
    multi: *mut CURLM,
    running_handles: *mut c_int,
) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return unsafe { ref_multi_socket_all()(multi, running_handles) };
    };

    let code = unsafe { ref_multi_socket_all()(wrapper.ref_multi, running_handles) };
    if code == crate::abi::CURLM_OK {
        update_progress_state(wrapper, state::MultiState::Performing);
        drain_messages(wrapper);
    }
    code
}

pub(crate) unsafe fn socket_action_handle(
    multi: *mut CURLM,
    socket: curl_socket_t,
    ev_bitmask: c_int,
    running_handles: *mut c_int,
) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return unsafe { ref_multi_socket_action()(multi, socket, ev_bitmask, running_handles) };
    };

    let code = unsafe {
        ref_multi_socket_action()(wrapper.ref_multi, socket, ev_bitmask, running_handles)
    };
    if code == crate::abi::CURLM_OK {
        update_progress_state(wrapper, state::MultiState::Performing);
        drain_messages(wrapper);
    }
    code
}

pub(crate) unsafe fn assign_handle(
    multi: *mut CURLM,
    socket: curl_socket_t,
    socketp: *mut c_void,
) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return unsafe { ref_multi_assign()(multi, socket, socketp) };
    };
    unsafe { ref_multi_assign()(wrapper.ref_multi, socket, socketp) }
}

pub(crate) unsafe fn multi_strerror(code: CURLMcode) -> *const c_char {
    unsafe { ref_multi_strerror()(code) }
}

pub(crate) unsafe fn get_handles_copy(multi: *mut CURLM) -> *mut *mut CURL {
    if let Some(wrapper) = wrapper_from_ptr(multi) {
        let easies = wrapper
            .inner
            .lock()
            .expect("multi mutex poisoned")
            .easies
            .clone();
        let handles = unsafe { alloc::calloc_bytes(easies.len() + 1, size_of::<*mut CURL>()) }
            as *mut *mut CURL;
        if handles.is_null() {
            return ptr::null_mut();
        }
        unsafe {
            ptr::copy_nonoverlapping(easies.as_ptr(), handles, easies.len());
        }
        return handles;
    }

    let reference_handles = unsafe { ref_multi_get_handles()(multi) };
    if reference_handles.is_null() {
        return ptr::null_mut();
    }

    let mut count = 0usize;
    while unsafe { !(*reference_handles.add(count)).is_null() } {
        count += 1;
    }

    let handles =
        unsafe { alloc::calloc_bytes(count + 1, size_of::<*mut CURL>()) } as *mut *mut CURL;
    if handles.is_null() {
        unsafe { global::free_reference_allocation(reference_handles.cast()) };
        return ptr::null_mut();
    }
    unsafe {
        ptr::copy_nonoverlapping(reference_handles, handles, count);
        global::free_reference_allocation(reference_handles.cast());
    }
    handles
}

pub(crate) unsafe fn dispatch_setopt_long(
    multi: *mut CURLM,
    option: CURLMoption,
    value: c_long,
) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return unsafe { ref_multi_setopt()(multi, option, value) };
    };
    unsafe { ref_multi_setopt()(wrapper.ref_multi, option, value) }
}

pub(crate) unsafe fn dispatch_setopt_ptr(
    multi: *mut CURLM,
    option: CURLMoption,
    value: *mut c_void,
) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return unsafe { ref_multi_setopt()(multi, option, value) };
    };

    match option {
        CURLMOPT_SOCKETDATA => {
            wrapper
                .inner
                .lock()
                .expect("multi mutex poisoned")
                .callbacks
                .socket_userp = value;
            crate::abi::CURLM_OK
        }
        CURLMOPT_TIMERDATA => {
            wrapper
                .inner
                .lock()
                .expect("multi mutex poisoned")
                .callbacks
                .timer_userp = value;
            crate::abi::CURLM_OK
        }
        _ => unsafe { ref_multi_setopt()(wrapper.ref_multi, option, value) },
    }
}

pub(crate) unsafe fn dispatch_setopt_function(
    multi: *mut CURLM,
    option: CURLMoption,
    value: Option<unsafe extern "C" fn()>,
) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return unsafe { ref_multi_setopt()(multi, option, value) };
    };

    match option {
        CURLMOPT_SOCKETFUNCTION => {
            let callback: CurlSocketCallback = unsafe { mem::transmute(value) };
            wrapper
                .inner
                .lock()
                .expect("multi mutex poisoned")
                .callbacks
                .socket_cb = callback;
            let trampoline: CurlSocketCallback = if callback.is_some() {
                Some(socket_trampoline)
            } else {
                None
            };
            unsafe { ref_multi_setopt()(wrapper.ref_multi, option, trampoline) }
        }
        CURLMOPT_TIMERFUNCTION => {
            let callback: CurlMultiTimerCallback = unsafe { mem::transmute(value) };
            wrapper
                .inner
                .lock()
                .expect("multi mutex poisoned")
                .callbacks
                .timer_cb = callback;
            let trampoline: CurlMultiTimerCallback = if callback.is_some() {
                Some(timer_trampoline)
            } else {
                None
            };
            unsafe { ref_multi_setopt()(wrapper.ref_multi, option, trampoline) }
        }
        _ => unsafe { ref_multi_setopt()(wrapper.ref_multi, option, value) },
    }
}

pub(crate) unsafe fn dispatch_setopt_off_t(
    multi: *mut CURLM,
    option: CURLMoption,
    value: curl_off_t,
) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return unsafe { ref_multi_setopt()(multi, option, value) };
    };
    unsafe { ref_multi_setopt()(wrapper.ref_multi, option, value) }
}
