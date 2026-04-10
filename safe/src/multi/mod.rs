pub(crate) mod poll;
pub(crate) mod state;

use crate::abi::{
    curl_off_t, curl_pushheaders, curl_socket_t, CURLMcode, CURLMoption, CURLMsg, CURLcode, CURL,
    CURLM, CURLMSG,
};
use crate::conn::cache::ConnectionCache;
use crate::dns::ResolverOwner;
use crate::{alloc, easy, global, transfer};
use core::ffi::{c_char, c_int, c_long, c_uint, c_void};
use core::{mem, ptr};
use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::mem::size_of;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

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
const CURL_SOCKET_BAD: curl_socket_t = -1;
const CURL_POLL_IN: c_int = 1;
const CURL_POLL_INOUT: c_int = 3;
const CURL_POLL_REMOVE: c_int = 4;
const CURLM_CALL_MULTI_PERFORM: CURLMcode = -1;
const CURL_PUSH_OK: c_int = 0;
const CURL_PUSH_DENY: c_int = 1;
const CURL_PUSH_ERROROUT: c_int = 2;

const CURLMOPT_PIPELINING: CURLMoption = 3;
const CURLOPT_VERBOSE: c_long = 41;
const CURLMOPT_SOCKETFUNCTION: CURLMoption = 20001;
const CURLMOPT_SOCKETDATA: CURLMoption = 10002;
const CURLMOPT_TIMERFUNCTION: CURLMoption = 20004;
const CURLMOPT_TIMERDATA: CURLMoption = 10005;
pub(crate) const CURLMOPT_MAXCONNECTS: CURLMoption = 6;
const CURLMOPT_MAX_HOST_CONNECTIONS: CURLMoption = 7;
const CURLMOPT_MAX_PIPELINE_LENGTH: CURLMoption = 8;
const CURLMOPT_CONTENT_LENGTH_PENALTY_SIZE: CURLMoption = 30009;
const CURLMOPT_CHUNK_LENGTH_PENALTY_SIZE: CURLMoption = 30010;
const CURLMOPT_PIPELINING_SITE_BL: CURLMoption = 10011;
const CURLMOPT_PIPELINING_SERVER_BL: CURLMoption = 10012;
const CURLMOPT_MAX_TOTAL_CONNECTIONS: CURLMoption = 13;
const CURLMOPT_PUSHFUNCTION: CURLMoption = 20014;
const CURLMOPT_PUSHDATA: CURLMoption = 10015;
const CURLMOPT_MAX_CONCURRENT_STREAMS: CURLMoption = 16;

type CurlSocketCallback = Option<
    unsafe extern "C" fn(*mut CURL, curl_socket_t, c_int, *mut c_void, *mut c_void) -> c_int,
>;
type CurlMultiTimerCallback =
    Option<unsafe extern "C" fn(*mut CURLM, c_long, *mut c_void) -> c_int>;
type CurlPushCallback = Option<
    unsafe extern "C" fn(*mut CURL, *mut CURL, usize, *mut curl_pushheaders, *mut c_void) -> c_int,
>;

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
type RefMultiWaitFn = unsafe extern "C" fn(
    *mut CURLM,
    *mut crate::abi::curl_waitfd,
    c_uint,
    c_int,
    *mut c_int,
) -> CURLMcode;
type RefMultiPollFn = unsafe extern "C" fn(
    *mut CURLM,
    *mut crate::abi::curl_waitfd,
    c_uint,
    c_int,
    *mut c_int,
) -> CURLMcode;
type RefMultiPerformFn = unsafe extern "C" fn(*mut CURLM, *mut c_int) -> CURLMcode;
type RefMultiTimeoutFn = unsafe extern "C" fn(*mut CURLM, *mut c_long) -> CURLMcode;
type RefMultiInfoReadFn = unsafe extern "C" fn(*mut CURLM, *mut c_int) -> *mut CURLMsg;
type RefMultiSocketActionFn =
    unsafe extern "C" fn(*mut CURLM, curl_socket_t, c_int, *mut c_int) -> CURLMcode;
type RefMultiAssignFn = unsafe extern "C" fn(*mut CURLM, curl_socket_t, *mut c_void) -> CURLMcode;
type RefMultiWakeupFn = unsafe extern "C" fn(*mut CURLM) -> CURLMcode;

unsafe extern "C" {
    fn curl_safe_reference_easy_setopt_long(
        handle: *mut CURL,
        option: c_long,
        value: c_long,
    ) -> CURLcode;
    fn curl_safe_reference_multi_setopt_long(
        multi_handle: *mut CURLM,
        option: CURLMoption,
        value: c_long,
    ) -> CURLMcode;
    fn curl_safe_reference_multi_setopt_ptr(
        multi_handle: *mut CURLM,
        option: CURLMoption,
        value: *mut c_void,
    ) -> CURLMcode;
    fn curl_safe_reference_multi_setopt_function(
        multi_handle: *mut CURLM,
        option: CURLMoption,
        value: Option<unsafe extern "C" fn()>,
    ) -> CURLMcode;
    fn curl_safe_reference_multi_setopt_off_t(
        multi_handle: *mut CURLM,
        option: CURLMoption,
        value: curl_off_t,
    ) -> CURLMcode;
}

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
    push_cb: CurlPushCallback,
    push_userp: *mut c_void,
    in_callback: bool,
}

enum TransferEvent {
    Completed { easy_key: usize, result: CURLcode },
    Wakeup,
}

struct EventQueue {
    queue: Mutex<VecDeque<TransferEvent>>,
    ready: Condvar,
}

impl EventQueue {
    fn push(&self, event: TransferEvent) {
        let mut guard = self.queue.lock().expect("multi event queue mutex poisoned");
        guard.push_back(event);
        self.ready.notify_all();
    }

    fn pop_timeout(&self, timeout: Duration) -> Option<TransferEvent> {
        let guard = self.queue.lock().expect("multi event queue mutex poisoned");
        let mut guard = if guard.is_empty() {
            let (guard, _) = self
                .ready
                .wait_timeout_while(guard, timeout, |queue| queue.is_empty())
                .expect("multi event queue mutex poisoned");
            guard
        } else {
            guard
        };
        guard.pop_front()
    }

    fn drain(&self) -> Vec<TransferEvent> {
        let mut guard = self.queue.lock().expect("multi event queue mutex poisoned");
        guard.drain(..).collect()
    }
}

struct TransferRecord {
    easy: *mut CURL,
    state: state::MultiState,
    plan: transfer::TransferPlan,
    connection_id: usize,
    reference_verbose_suppressed: bool,
    poll_reader: Option<UnixStream>,
    worker: Option<std::thread::JoinHandle<()>>,
    started: bool,
    completed: bool,
    message_enqueued: bool,
}

struct MultiInner {
    easies: Vec<*mut CURL>,
    records: HashMap<usize, TransferRecord>,
    messages: VecDeque<QueuedMessage>,
    current_msg: Option<Box<CURLMsg>>,
    callbacks: CallbackState,
    assignments: HashMap<curl_socket_t, *mut c_void>,
    conncache: ConnectionCache,
    next_connection_id: usize,
    maxconnects: usize,
    max_host_connections: c_long,
    max_total_connections: c_long,
    max_concurrent_streams: c_long,
    multiplexing: bool,
    reference_multi: *mut CURLM,
    reference_timeout_ms: c_long,
    dead: bool,
}

impl Default for MultiInner {
    fn default() -> Self {
        Self {
            easies: Vec::new(),
            records: HashMap::new(),
            messages: VecDeque::new(),
            current_msg: None,
            callbacks: CallbackState::default(),
            assignments: HashMap::new(),
            conncache: ConnectionCache::default(),
            next_connection_id: 1,
            maxconnects: 0,
            max_host_connections: 0,
            max_total_connections: 0,
            max_concurrent_streams: 100,
            multiplexing: false,
            reference_multi: ptr::null_mut(),
            reference_timeout_ms: -1,
            dead: false,
        }
    }
}

pub(crate) struct MultiHandle {
    magic: usize,
    events: Arc<EventQueue>,
    inner: Mutex<MultiInner>,
}

unsafe impl Send for MultiHandle {}
unsafe impl Sync for MultiHandle {}

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

fn ref_multi_init() -> RefMultiInitFn {
    static FN: std::sync::OnceLock<RefMultiInitFn> = std::sync::OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_init\0") })
}

fn ref_multi_cleanup() -> RefMultiCleanupFn {
    static FN: std::sync::OnceLock<RefMultiCleanupFn> = std::sync::OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_cleanup\0") })
}

fn ref_multi_add_handle() -> RefMultiAddHandleFn {
    static FN: std::sync::OnceLock<RefMultiAddHandleFn> = std::sync::OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_add_handle\0") })
}

fn ref_multi_remove_handle() -> RefMultiRemoveHandleFn {
    static FN: std::sync::OnceLock<RefMultiRemoveHandleFn> = std::sync::OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_remove_handle\0") })
}

fn ref_multi_fdset() -> RefMultiFdsetFn {
    static FN: std::sync::OnceLock<RefMultiFdsetFn> = std::sync::OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_fdset\0") })
}

fn ref_multi_wait() -> RefMultiWaitFn {
    static FN: std::sync::OnceLock<RefMultiWaitFn> = std::sync::OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_wait\0") })
}

fn ref_multi_poll() -> RefMultiPollFn {
    static FN: std::sync::OnceLock<RefMultiPollFn> = std::sync::OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_poll\0") })
}

fn ref_multi_perform() -> RefMultiPerformFn {
    static FN: std::sync::OnceLock<RefMultiPerformFn> = std::sync::OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_perform\0") })
}

fn ref_multi_timeout() -> RefMultiTimeoutFn {
    static FN: std::sync::OnceLock<RefMultiTimeoutFn> = std::sync::OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_timeout\0") })
}

fn ref_multi_info_read() -> RefMultiInfoReadFn {
    static FN: std::sync::OnceLock<RefMultiInfoReadFn> = std::sync::OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_info_read\0") })
}

fn ref_multi_socket_action() -> RefMultiSocketActionFn {
    static FN: std::sync::OnceLock<RefMultiSocketActionFn> = std::sync::OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_socket_action\0") })
}

fn ref_multi_assign() -> RefMultiAssignFn {
    static FN: std::sync::OnceLock<RefMultiAssignFn> = std::sync::OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_assign\0") })
}

fn ref_multi_wakeup() -> RefMultiWakeupFn {
    static FN: std::sync::OnceLock<RefMultiWakeupFn> = std::sync::OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_multi_wakeup\0") })
}

unsafe extern "C" fn reference_socket_callback(
    easy_handle: *mut CURL,
    socket: curl_socket_t,
    what: c_int,
    userp: *mut c_void,
    _socketp: *mut c_void,
) -> c_int {
    let Some(wrapper) = wrapper_from_ptr(userp.cast()) else {
        return -1;
    };
    let multi_ptr: *mut CURLM = userp.cast();
    match invoke_socket_callback(wrapper, multi_ptr, easy_handle, socket, what) {
        crate::abi::CURLM_OK => 0,
        _ => -1,
    }
}

unsafe extern "C" fn reference_timer_callback(
    _reference_multi: *mut CURLM,
    timeout_ms: c_long,
    userp: *mut c_void,
) -> c_int {
    let Some(wrapper) = wrapper_from_ptr(userp.cast()) else {
        return -1;
    };
    {
        let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
        guard.reference_timeout_ms = timeout_ms;
    }
    match update_timer(wrapper, userp.cast()) {
        crate::abi::CURLM_OK => 0,
        _ => -1,
    }
}

unsafe extern "C" fn reference_push_callback(
    parent: *mut CURL,
    easy_handle: *mut CURL,
    num_headers: usize,
    headers: *mut curl_pushheaders,
    userp: *mut c_void,
) -> c_int {
    let Some(wrapper) = wrapper_from_ptr(userp.cast()) else {
        return CURL_PUSH_ERROROUT;
    };
    let multi_ptr: *mut CURLM = userp.cast();
    let (push_cb, push_userp, parent_plan, parent_connection_id) = {
        let guard = wrapper.inner.lock().expect("multi mutex poisoned");
        let parent_record = guard.records.get(&(parent as usize));
        let plan = parent_record
            .map(|record| record.plan.clone())
            .unwrap_or_else(|| {
                transfer::build_plan(
                    &easy::perform::snapshot_metadata(parent),
                    ResolverOwner::Multi,
                )
            });
        let connection_id = parent_record
            .map(|record| record.connection_id)
            .unwrap_or(0);
        (
            guard.callbacks.push_cb,
            guard.callbacks.push_userp,
            plan,
            connection_id,
        )
    };

    let decision = if let Some(push_cb) = push_cb {
        unsafe { push_cb(parent, easy_handle, num_headers, headers, push_userp) }
    } else {
        CURL_PUSH_DENY
    };
    if decision != CURL_PUSH_OK {
        return decision;
    }
    let suppressed_verbose = suppress_reference_verbose(easy_handle);

    {
        let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
        if !guard.easies.contains(&easy_handle) {
            guard.easies.push(easy_handle);
        }
        guard.records.insert(
            easy_handle as usize,
            TransferRecord {
                easy: easy_handle,
                state: state::MultiState::Performing,
                plan: parent_plan,
                connection_id: parent_connection_id,
                reference_verbose_suppressed: suppressed_verbose,
                poll_reader: None,
                worker: None,
                started: true,
                completed: false,
                message_enqueued: false,
            },
        );
    }
    easy::perform::on_attached(
        easy_handle,
        multi_ptr as usize,
        state::MultiState::Performing,
    );
    CURL_PUSH_OK
}

pub(crate) unsafe fn init_handle() -> *mut CURLM {
    let wrapper = Box::new(MultiHandle {
        magic: MULTI_MAGIC,
        events: Arc::new(EventQueue {
            queue: Mutex::new(VecDeque::new()),
            ready: Condvar::new(),
        }),
        inner: Mutex::new(MultiInner::default()),
    });
    Box::into_raw(wrapper).cast()
}

pub(crate) unsafe fn cleanup_owned_multi(multi: *mut CURLM) {
    if multi.is_null() {
        return;
    }
    let _ = unsafe { cleanup_handle(multi) };
}

pub(crate) unsafe fn cleanup_handle(multi: *mut CURLM) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return CURLM_BAD_HANDLE;
    };

    drain_events(wrapper);
    let (joins, easies, reference_multi, reference_easies) = {
        let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
        let joins = guard
            .records
            .values_mut()
            .filter(|record| !record.plan.reference_backend)
            .filter_map(|record| record.worker.take())
            .collect::<Vec<_>>();
        let reference_easies = guard
            .records
            .values()
            .filter(|record| record.plan.reference_backend)
            .map(|record| (record.easy, record.reference_verbose_suppressed))
            .collect::<Vec<_>>();
        let easies = guard.easies.clone();
        let reference_multi = guard.reference_multi;
        guard.records.clear();
        guard.easies.clear();
        guard.messages.clear();
        guard.current_msg = None;
        guard.assignments.clear();
        guard.reference_multi = ptr::null_mut();
        guard.reference_timeout_ms = -1;
        (joins, easies, reference_multi, reference_easies)
    };

    for join in joins {
        let _ = join.join();
    }
    if !reference_multi.is_null() {
        for (easy, suppressed_verbose) in reference_easies {
            restore_reference_verbose(easy, suppressed_verbose);
            let _ = unsafe { ref_multi_remove_handle()(reference_multi, easy) };
        }
        let _ = unsafe { ref_multi_cleanup()(reference_multi) };
    }
    for easy in easies {
        easy::perform::on_detached(easy, multi as usize, state::MultiState::Init);
    }

    let raw = multi as *mut MultiHandle;
    unsafe {
        (*raw).magic = 0;
        drop(Box::from_raw(raw));
    }
    crate::abi::CURLM_OK
}

pub(crate) unsafe fn add_handle(multi: *mut CURLM, easy_handle: *mut CURL) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return CURLM_BAD_HANDLE;
    };
    if easy_handle.is_null() {
        return CURLM_BAD_EASY_HANDLE;
    }
    if easy::perform::attached_multi_for(easy_handle).is_some() {
        return CURLM_ADDED_ALREADY;
    }

    drain_events(wrapper);
    let metadata = easy::perform::snapshot_metadata(easy_handle);
    let plan = transfer::build_plan(&metadata, ResolverOwner::Multi);
    let connection_id = {
        let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
        if guard.records.contains_key(&(easy_handle as usize)) {
            return CURLM_ADDED_ALREADY;
        }
        let maxconnects = guard
            .maxconnects
            .max(metadata.maxconnects.unwrap_or(0).max(0) as usize);
        let next_connection_id = guard.next_connection_id;
        let (connection_id, reused) =
            guard
                .conncache
                .remember(plan.cache_key.clone(), maxconnects, next_connection_id);
        if !reused {
            guard.next_connection_id += 1;
        }
        if !guard.easies.contains(&easy_handle) {
            guard.easies.push(easy_handle);
        }
        guard.records.insert(
            easy_handle as usize,
            TransferRecord {
                easy: easy_handle,
                state: state::MultiState::Pending,
                plan,
                connection_id,
                reference_verbose_suppressed: false,
                poll_reader: None,
                worker: None,
                started: false,
                completed: false,
                message_enqueued: false,
            },
        );
        connection_id
    };

    let _ = connection_id;
    easy::perform::on_attached(easy_handle, multi as usize, state::MultiState::Pending);
    let timer_rc = update_timer(wrapper, multi);
    if timer_rc != crate::abi::CURLM_OK {
        return timer_rc;
    }
    crate::abi::CURLM_OK
}

pub(crate) unsafe fn remove_handle(multi: *mut CURLM, easy_handle: *mut CURL) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return CURLM_BAD_HANDLE;
    };
    if easy_handle.is_null() {
        return CURLM_BAD_EASY_HANDLE;
    }

    drain_events(wrapper);
    let (join, reference_multi, reference_backend, cleanup_reference_multi, suppressed_verbose) = {
        let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
        let Some(mut record) = guard.records.remove(&(easy_handle as usize)) else {
            return CURLM_BAD_EASY_HANDLE;
        };
        let _ = record.connection_id;
        guard.easies.retain(|candidate| *candidate != easy_handle);
        guard.messages.retain(|msg| msg.easy_handle != easy_handle);
        if guard
            .current_msg
            .as_ref()
            .is_some_and(|msg| msg.easy_handle == easy_handle)
        {
            guard.current_msg = None;
        }
        let cleanup_reference_multi = record.plan.reference_backend
            && !guard
                .records
                .values()
                .any(|candidate| candidate.plan.reference_backend);
        let reference_multi = guard.reference_multi;
        if cleanup_reference_multi {
            guard.reference_multi = ptr::null_mut();
            guard.reference_timeout_ms = -1;
        }
        (
            record.worker.take(),
            reference_multi,
            record.plan.reference_backend,
            cleanup_reference_multi,
            record.reference_verbose_suppressed,
        )
    };

    if let Some(join) = join {
        let _ = join.join();
    }
    if reference_backend && !reference_multi.is_null() {
        restore_reference_verbose(easy_handle, suppressed_verbose);
        let _ = unsafe { ref_multi_remove_handle()(reference_multi, easy_handle) };
        if cleanup_reference_multi {
            let _ = unsafe { ref_multi_cleanup()(reference_multi) };
        }
    }
    easy::perform::on_detached(easy_handle, multi as usize, state::MultiState::Done);
    let timer_rc = update_timer(wrapper, multi);
    if timer_rc != crate::abi::CURLM_OK {
        return timer_rc;
    }
    crate::abi::CURLM_OK
}

pub(crate) unsafe fn drop_easy_reference(multi: *mut CURLM, easy_handle: *mut CURL) {
    if let Some(wrapper) = wrapper_from_ptr(multi) {
        let (join, reference_multi, reference_backend, cleanup_reference_multi, suppressed_verbose) = {
            let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
            guard.easies.retain(|candidate| *candidate != easy_handle);
            guard.messages.retain(|msg| msg.easy_handle != easy_handle);
            if guard
                .current_msg
                .as_ref()
                .is_some_and(|msg| msg.easy_handle == easy_handle)
            {
                guard.current_msg = None;
            }
            let Some(mut record) = guard.records.remove(&(easy_handle as usize)) else {
                return;
            };
            let cleanup_reference_multi = record.plan.reference_backend
                && !guard
                    .records
                    .values()
                    .any(|candidate| candidate.plan.reference_backend);
            let reference_multi = guard.reference_multi;
            if cleanup_reference_multi {
                guard.reference_multi = ptr::null_mut();
                guard.reference_timeout_ms = -1;
            }
            (
                record.worker.take(),
                reference_multi,
                record.plan.reference_backend,
                cleanup_reference_multi,
                record.reference_verbose_suppressed,
            )
        };
        if let Some(join) = join {
            let _ = join.join();
        }
        if reference_backend && !reference_multi.is_null() {
            restore_reference_verbose(easy_handle, suppressed_verbose);
            let _ = unsafe { ref_multi_remove_handle()(reference_multi, easy_handle) };
            if cleanup_reference_multi {
                let _ = unsafe { ref_multi_cleanup()(reference_multi) };
            }
        }
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
        return CURLM_BAD_HANDLE;
    };
    if is_in_callback(wrapper) {
        return CURLM_RECURSIVE_API_CALL;
    }
    drain_events(wrapper);
    let mut highest = -1;
    {
        let guard = wrapper.inner.lock().expect("multi mutex poisoned");
        for record in guard.records.values() {
            let Some(reader) = record.poll_reader.as_ref() else {
                continue;
            };
            let socket = reader.as_raw_fd() as curl_socket_t;
            fd_set_insert(read_fd_set, socket);
            highest = highest.max(socket as c_int);
        }
    }
    let reference_multi = wrapper
        .inner
        .lock()
        .expect("multi mutex poisoned")
        .reference_multi;
    if !reference_multi.is_null() {
        let mut reference_max = -1;
        let rc = unsafe {
            ref_multi_fdset()(
                reference_multi,
                read_fd_set,
                write_fd_set,
                exc_fd_set,
                &mut reference_max,
            )
        };
        if rc != crate::abi::CURLM_OK {
            return rc;
        }
        highest = highest.max(reference_max);
    }
    if !max_fd.is_null() {
        unsafe { *max_fd = highest };
    }
    crate::abi::CURLM_OK
}

pub(crate) unsafe fn perform_handle(multi: *mut CURLM, running_handles: *mut c_int) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return CURLM_BAD_HANDLE;
    };
    if is_in_callback(wrapper) {
        return CURLM_RECURSIVE_API_CALL;
    }
    if is_dead(wrapper) {
        return CURLM_ABORTED_BY_CALLBACK;
    }

    drain_events(wrapper);
    let callback_rc = start_pending_transfers(wrapper, multi);
    if callback_rc != crate::abi::CURLM_OK {
        return callback_rc;
    }
    let reference_rc = perform_reference_transfers(wrapper);
    if reference_rc != crate::abi::CURLM_OK {
        return reference_rc;
    }
    harvest_reference_messages(wrapper);

    let running = {
        let guard = wrapper.inner.lock().expect("multi mutex poisoned");
        guard
            .records
            .values()
            .filter(|record| !record.completed)
            .count() as c_int
    };
    if !running_handles.is_null() {
        unsafe { *running_handles = running };
    }
    update_timer(wrapper, multi)
}

pub(crate) unsafe fn wait_handle(
    multi: *mut CURLM,
    extra_fds: *mut crate::abi::curl_waitfd,
    extra_nfds: c_uint,
    timeout_ms: c_int,
    ret: *mut c_int,
) -> CURLMcode {
    wait_common(multi, extra_fds, extra_nfds, timeout_ms, ret, false)
}

pub(crate) unsafe fn poll_handle(
    multi: *mut CURLM,
    extra_fds: *mut crate::abi::curl_waitfd,
    extra_nfds: c_uint,
    timeout_ms: c_int,
    ret: *mut c_int,
) -> CURLMcode {
    wait_common(multi, extra_fds, extra_nfds, timeout_ms, ret, true)
}

pub(crate) unsafe fn timeout_handle(multi: *mut CURLM, milliseconds: *mut c_long) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return CURLM_BAD_HANDLE;
    };
    if is_in_callback(wrapper) {
        return CURLM_RECURSIVE_API_CALL;
    }
    if milliseconds.is_null() {
        return CURLM_BAD_FUNCTION_ARGUMENT;
    }

    drain_events(wrapper);
    let native_timeout = {
        let guard = wrapper.inner.lock().expect("multi mutex poisoned");
        if guard.dead {
            0
        } else if !guard.messages.is_empty()
            || guard
                .records
                .values()
                .any(|record| !record.started && !record.completed)
        {
            0
        } else if guard.records.values().any(|record| !record.completed) {
            transfer::EASY_PERFORM_WAIT_TIMEOUT_MS as c_long
        } else {
            -1
        }
    };
    unsafe { *milliseconds = combine_timeouts(native_timeout, reference_timeout_ms(wrapper)) };
    crate::abi::CURLM_OK
}

pub(crate) unsafe fn wakeup_handle(multi: *mut CURLM) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return CURLM_BAD_HANDLE;
    };
    wrapper.events.push(TransferEvent::Wakeup);
    let reference_multi = wrapper
        .inner
        .lock()
        .expect("multi mutex poisoned")
        .reference_multi;
    if !reference_multi.is_null() {
        let rc = unsafe { ref_multi_wakeup()(reference_multi) };
        if rc != crate::abi::CURLM_OK {
            return rc;
        }
    }
    crate::abi::CURLM_OK
}

pub(crate) unsafe fn info_read_handle(
    multi: *mut CURLM,
    msgs_in_queue: *mut c_int,
) -> *mut CURLMsg {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return ptr::null_mut();
    };
    if is_in_callback(wrapper) {
        if !msgs_in_queue.is_null() {
            unsafe { *msgs_in_queue = 0 };
        }
        return ptr::null_mut();
    }

    drain_events(wrapper);
    harvest_reference_messages(wrapper);
    let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
    if let Some(entry) = guard.messages.pop_front() {
        guard.current_msg = Some(Box::new(CURLMsg {
            msg: CURLMSG_DONE,
            easy_handle: entry.easy_handle,
            data: crate::abi::CURLMsgData {
                result: entry.result,
            },
        }));
        easy::perform::mark_message_sent(entry.easy_handle);
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
    unsafe { socket_action_handle(multi, socket, 0, running_handles) }
}

pub(crate) unsafe fn socket_all_handle(
    multi: *mut CURLM,
    running_handles: *mut c_int,
) -> CURLMcode {
    unsafe { perform_handle(multi, running_handles) }
}

pub(crate) unsafe fn socket_action_handle(
    multi: *mut CURLM,
    socket: curl_socket_t,
    ev_bitmask: c_int,
    running_handles: *mut c_int,
) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return CURLM_BAD_HANDLE;
    };
    if is_in_callback(wrapper) {
        return CURLM_RECURSIVE_API_CALL;
    }
    if is_dead(wrapper) {
        return CURLM_ABORTED_BY_CALLBACK;
    }

    drain_events(wrapper);
    let callback_rc = start_pending_transfers(wrapper, multi);
    if callback_rc != crate::abi::CURLM_OK {
        return callback_rc;
    }

    let reference_multi = wrapper
        .inner
        .lock()
        .expect("multi mutex poisoned")
        .reference_multi;
    if !reference_multi.is_null() {
        let mut running = 0;
        let rc =
            unsafe { ref_multi_socket_action()(reference_multi, socket, ev_bitmask, &mut running) };
        if rc != crate::abi::CURLM_OK {
            return rc;
        }
    }
    harvest_reference_messages(wrapper);

    let running = {
        let guard = wrapper.inner.lock().expect("multi mutex poisoned");
        guard
            .records
            .values()
            .filter(|record| !record.completed)
            .count() as c_int
    };
    if !running_handles.is_null() {
        unsafe { *running_handles = running };
    }
    update_timer(wrapper, multi)
}

pub(crate) unsafe fn assign_handle(
    multi: *mut CURLM,
    socket: curl_socket_t,
    socketp: *mut c_void,
) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return CURLM_BAD_HANDLE;
    };
    if is_in_callback(wrapper) {
        return CURLM_RECURSIVE_API_CALL;
    }

    wrapper
        .inner
        .lock()
        .expect("multi mutex poisoned")
        .assignments
        .insert(socket, socketp);
    let reference_multi = wrapper
        .inner
        .lock()
        .expect("multi mutex poisoned")
        .reference_multi;
    if !reference_multi.is_null() {
        let rc = unsafe { ref_multi_assign()(reference_multi, socket, socketp) };
        if rc != crate::abi::CURLM_OK {
            return rc;
        }
    }
    crate::abi::CURLM_OK
}

pub(crate) unsafe fn multi_strerror(code: CURLMcode) -> *const c_char {
    match code {
        CURLM_CALL_MULTI_PERFORM => c"Please call curl_multi_perform() soon".as_ptr(),
        0 => c"No error".as_ptr(),
        CURLM_BAD_HANDLE => c"Invalid multi handle".as_ptr(),
        CURLM_BAD_EASY_HANDLE => c"Invalid easy handle".as_ptr(),
        CURLM_OUT_OF_MEMORY => c"Out of memory".as_ptr(),
        CURLM_INTERNAL_ERROR => c"Internal error".as_ptr(),
        CURLM_BAD_SOCKET => c"Invalid socket argument".as_ptr(),
        CURLM_UNKNOWN_OPTION => c"Unknown option".as_ptr(),
        CURLM_ADDED_ALREADY => c"The easy handle is already added to a multi handle".as_ptr(),
        CURLM_RECURSIVE_API_CALL => c"API function called from within callback".as_ptr(),
        CURLM_WAKEUP_FAILURE => c"Wakeup is unavailable or failed".as_ptr(),
        CURLM_BAD_FUNCTION_ARGUMENT => c"A libcurl function was given a bad argument".as_ptr(),
        CURLM_ABORTED_BY_CALLBACK => c"Operation was aborted by an application callback".as_ptr(),
        CURLM_UNRECOVERABLE_POLL => c"Unrecoverable error in select/poll".as_ptr(),
        _ => c"Unknown error".as_ptr(),
    }
}

pub(crate) unsafe fn get_handles_copy(multi: *mut CURLM) -> *mut *mut CURL {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return ptr::null_mut();
    };
    let easies = wrapper
        .inner
        .lock()
        .expect("multi mutex poisoned")
        .easies
        .clone();
    let handles =
        unsafe { alloc::calloc_bytes(easies.len() + 1, size_of::<*mut CURL>()) } as *mut *mut CURL;
    if handles.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        ptr::copy_nonoverlapping(easies.as_ptr(), handles, easies.len());
    }
    handles
}

pub(crate) unsafe fn dispatch_setopt_long(
    multi: *mut CURLM,
    option: CURLMoption,
    value: c_long,
) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return CURLM_BAD_HANDLE;
    };
    if is_in_callback(wrapper) {
        return CURLM_RECURSIVE_API_CALL;
    }

    let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
    match option {
        CURLMOPT_PIPELINING => guard.multiplexing = value != 0,
        CURLMOPT_MAXCONNECTS => guard.maxconnects = value.max(0) as usize,
        CURLMOPT_MAX_HOST_CONNECTIONS => guard.max_host_connections = value,
        CURLMOPT_MAX_PIPELINE_LENGTH => {}
        CURLMOPT_MAX_TOTAL_CONNECTIONS => guard.max_total_connections = value,
        CURLMOPT_MAX_CONCURRENT_STREAMS => {
            guard.max_concurrent_streams = if value < 1 { 100 } else { value }
        }
        _ => return CURLM_UNKNOWN_OPTION,
    }
    let reference_multi = guard.reference_multi;
    drop(guard);
    if !reference_multi.is_null() {
        let rc = unsafe { curl_safe_reference_multi_setopt_long(reference_multi, option, value) };
        if rc != crate::abi::CURLM_OK {
            return rc;
        }
    }
    update_timer(wrapper, multi)
}

pub(crate) unsafe fn dispatch_setopt_ptr(
    multi: *mut CURLM,
    option: CURLMoption,
    value: *mut c_void,
) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return CURLM_BAD_HANDLE;
    };
    if is_in_callback(wrapper) {
        return CURLM_RECURSIVE_API_CALL;
    }

    let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
    match option {
        CURLMOPT_SOCKETDATA => guard.callbacks.socket_userp = value,
        CURLMOPT_TIMERDATA => guard.callbacks.timer_userp = value,
        CURLMOPT_PUSHDATA => guard.callbacks.push_userp = value,
        CURLMOPT_PIPELINING_SITE_BL | CURLMOPT_PIPELINING_SERVER_BL => {}
        _ => return CURLM_UNKNOWN_OPTION,
    }
    crate::abi::CURLM_OK
}

pub(crate) unsafe fn dispatch_setopt_function(
    multi: *mut CURLM,
    option: CURLMoption,
    value: Option<unsafe extern "C" fn()>,
) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return CURLM_BAD_HANDLE;
    };
    if is_in_callback(wrapper) {
        return CURLM_RECURSIVE_API_CALL;
    }

    let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
    match option {
        CURLMOPT_SOCKETFUNCTION => {
            guard.callbacks.socket_cb = unsafe { mem::transmute(value) };
        }
        CURLMOPT_TIMERFUNCTION => {
            guard.callbacks.timer_cb = unsafe { mem::transmute(value) };
        }
        CURLMOPT_PUSHFUNCTION => {
            guard.callbacks.push_cb = unsafe { mem::transmute(value) };
        }
        _ => return CURLM_UNKNOWN_OPTION,
    }
    crate::abi::CURLM_OK
}

pub(crate) unsafe fn dispatch_setopt_off_t(
    multi: *mut CURLM,
    option: CURLMoption,
    _value: curl_off_t,
) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return CURLM_BAD_HANDLE;
    };
    if is_in_callback(wrapper) {
        return CURLM_RECURSIVE_API_CALL;
    }

    match option {
        CURLMOPT_CONTENT_LENGTH_PENALTY_SIZE | CURLMOPT_CHUNK_LENGTH_PENALTY_SIZE => {
            crate::abi::CURLM_OK
        }
        _ => CURLM_UNKNOWN_OPTION,
    }
}

fn is_in_callback(wrapper: &MultiHandle) -> bool {
    wrapper
        .inner
        .lock()
        .expect("multi mutex poisoned")
        .callbacks
        .in_callback
}

fn is_dead(wrapper: &MultiHandle) -> bool {
    wrapper.inner.lock().expect("multi mutex poisoned").dead
}

fn has_reference_handles(wrapper: &MultiHandle) -> bool {
    wrapper
        .inner
        .lock()
        .expect("multi mutex poisoned")
        .records
        .values()
        .any(|record| record.plan.reference_backend && !record.completed)
}

fn reference_timeout_ms(wrapper: &MultiHandle) -> c_long {
    let reference_multi = wrapper
        .inner
        .lock()
        .expect("multi mutex poisoned")
        .reference_multi;
    if reference_multi.is_null() {
        return -1;
    }

    let mut timeout_ms = -1;
    let rc = unsafe { ref_multi_timeout()(reference_multi, &mut timeout_ms) };
    if rc != crate::abi::CURLM_OK {
        return -1;
    }
    timeout_ms
}

fn combine_timeouts(native_timeout: c_long, reference_timeout: c_long) -> c_long {
    match (native_timeout, reference_timeout) {
        (-1, other) => other,
        (other, -1) => other,
        (native, reference) => native.min(reference),
    }
}

fn suppress_reference_verbose(easy_handle: *mut CURL) -> bool {
    let verbose = easy::perform::snapshot_metadata(easy_handle).verbose;
    if !verbose {
        return false;
    }
    (unsafe { curl_safe_reference_easy_setopt_long(easy_handle, CURLOPT_VERBOSE, 0) })
        == crate::abi::CURLE_OK
}

fn restore_reference_verbose(easy_handle: *mut CURL, suppressed: bool) {
    if !suppressed {
        return;
    }
    let _ = unsafe { curl_safe_reference_easy_setopt_long(easy_handle, CURLOPT_VERBOSE, 1) };
}

fn ensure_reference_multi(
    wrapper: &MultiHandle,
    multi_ptr: *mut CURLM,
) -> Result<*mut CURLM, CURLMcode> {
    let existing = wrapper
        .inner
        .lock()
        .expect("multi mutex poisoned")
        .reference_multi;
    if !existing.is_null() {
        return Ok(existing);
    }

    let reference_multi = unsafe { ref_multi_init()() };
    if reference_multi.is_null() {
        return Err(CURLM_OUT_OF_MEMORY);
    }

    let (
        multiplexing,
        maxconnects,
        max_host_connections,
        max_total_connections,
        max_concurrent_streams,
    ) = {
        let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
        guard.reference_multi = reference_multi;
        guard.reference_timeout_ms = -1;
        (
            guard.multiplexing,
            guard.maxconnects as c_long,
            guard.max_host_connections,
            guard.max_total_connections,
            guard.max_concurrent_streams,
        )
    };

    let configure = |rc: CURLMcode| {
        if rc == crate::abi::CURLM_OK {
            Ok(())
        } else {
            Err(rc)
        }
    };
    let reference_socket = Some(unsafe {
        mem::transmute::<
            unsafe extern "C" fn(
                *mut CURL,
                curl_socket_t,
                c_int,
                *mut c_void,
                *mut c_void,
            ) -> c_int,
            unsafe extern "C" fn(),
        >(reference_socket_callback)
    });
    let reference_timer = Some(unsafe {
        mem::transmute::<
            unsafe extern "C" fn(*mut CURLM, c_long, *mut c_void) -> c_int,
            unsafe extern "C" fn(),
        >(reference_timer_callback)
    });
    let reference_push = Some(unsafe {
        mem::transmute::<
            unsafe extern "C" fn(
                *mut CURL,
                *mut CURL,
                usize,
                *mut curl_pushheaders,
                *mut c_void,
            ) -> c_int,
            unsafe extern "C" fn(),
        >(reference_push_callback)
    });

    if configure(unsafe {
        curl_safe_reference_multi_setopt_function(
            reference_multi,
            CURLMOPT_SOCKETFUNCTION,
            reference_socket,
        )
    })
    .and_then(|_| {
        configure(unsafe {
            curl_safe_reference_multi_setopt_ptr(
                reference_multi,
                CURLMOPT_SOCKETDATA,
                multi_ptr.cast(),
            )
        })
    })
    .and_then(|_| {
        configure(unsafe {
            curl_safe_reference_multi_setopt_function(
                reference_multi,
                CURLMOPT_TIMERFUNCTION,
                reference_timer,
            )
        })
    })
    .and_then(|_| {
        configure(unsafe {
            curl_safe_reference_multi_setopt_ptr(
                reference_multi,
                CURLMOPT_TIMERDATA,
                multi_ptr.cast(),
            )
        })
    })
    .and_then(|_| {
        configure(unsafe {
            curl_safe_reference_multi_setopt_function(
                reference_multi,
                CURLMOPT_PUSHFUNCTION,
                reference_push,
            )
        })
    })
    .and_then(|_| {
        configure(unsafe {
            curl_safe_reference_multi_setopt_ptr(
                reference_multi,
                CURLMOPT_PUSHDATA,
                multi_ptr.cast(),
            )
        })
    })
    .and_then(|_| {
        configure(unsafe {
            curl_safe_reference_multi_setopt_long(
                reference_multi,
                CURLMOPT_PIPELINING,
                multiplexing as c_long,
            )
        })
    })
    .and_then(|_| {
        configure(unsafe {
            curl_safe_reference_multi_setopt_long(
                reference_multi,
                CURLMOPT_MAXCONNECTS,
                maxconnects,
            )
        })
    })
    .and_then(|_| {
        configure(unsafe {
            curl_safe_reference_multi_setopt_long(
                reference_multi,
                CURLMOPT_MAX_HOST_CONNECTIONS,
                max_host_connections,
            )
        })
    })
    .and_then(|_| {
        configure(unsafe {
            curl_safe_reference_multi_setopt_long(
                reference_multi,
                CURLMOPT_MAX_TOTAL_CONNECTIONS,
                max_total_connections,
            )
        })
    })
    .and_then(|_| {
        configure(unsafe {
            curl_safe_reference_multi_setopt_long(
                reference_multi,
                CURLMOPT_MAX_CONCURRENT_STREAMS,
                max_concurrent_streams,
            )
        })
    })
    .is_err()
    {
        let _ = unsafe { ref_multi_cleanup()(reference_multi) };
        let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
        if guard.reference_multi == reference_multi {
            guard.reference_multi = ptr::null_mut();
            guard.reference_timeout_ms = -1;
        }
        return Err(CURLM_INTERNAL_ERROR);
    }

    Ok(reference_multi)
}

fn perform_reference_transfers(wrapper: &MultiHandle) -> CURLMcode {
    let reference_multi = wrapper
        .inner
        .lock()
        .expect("multi mutex poisoned")
        .reference_multi;
    if reference_multi.is_null() {
        return crate::abi::CURLM_OK;
    }

    let mut running = 0;
    let rc = unsafe { ref_multi_perform()(reference_multi, &mut running) };
    if rc != crate::abi::CURLM_OK {
        return rc;
    }
    crate::abi::CURLM_OK
}

fn harvest_reference_messages(wrapper: &MultiHandle) {
    let reference_multi = wrapper
        .inner
        .lock()
        .expect("multi mutex poisoned")
        .reference_multi;
    if reference_multi.is_null() {
        return;
    }

    loop {
        let mut queued = 0;
        let msg = unsafe { ref_multi_info_read()(reference_multi, &mut queued) };
        if msg.is_null() {
            break;
        }
        if unsafe { (*msg).msg } != CURLMSG_DONE {
            continue;
        }

        let easy_handle = unsafe { (*msg).easy_handle };
        let result = unsafe { (*msg).data.result };
        let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
        let Some(record) = guard.records.get_mut(&(easy_handle as usize)) else {
            continue;
        };
        if record.message_enqueued {
            continue;
        }
        record.completed = true;
        record.message_enqueued = true;
        record.state = state::MultiState::Completed;
        let connection_id = record.connection_id;
        let host = record.plan.cache_key.host.clone();
        let suppressed_verbose = record.reference_verbose_suppressed;
        record.reference_verbose_suppressed = false;
        guard.messages.push_back(QueuedMessage {
            easy_handle,
            result,
        });
        drop(guard);
        restore_reference_verbose(easy_handle, suppressed_verbose);
        if result == crate::abi::CURLE_OK && easy::perform::snapshot_metadata(easy_handle).verbose {
            eprintln!(
                "* Connection #{} to host {} left intact",
                connection_id.saturating_sub(1),
                host
            );
        }
    }
}

fn start_pending_transfers(wrapper: &MultiHandle, multi_ptr: *mut CURLM) -> CURLMcode {
    let starts = {
        let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
        let mut starts = Vec::new();
        for record in guard.records.values_mut() {
            if record.started || record.completed {
                continue;
            }
            record.started = true;
            let states = state_path_for(&record.plan);
            record.state = *states.last().unwrap_or(&state::MultiState::Performing);
            starts.push((record.easy, record.plan.clone(), states));
        }
        starts
    };

    for (easy_handle, plan, states) in starts {
        for next_state in states {
            easy::perform::on_transfer_progress(easy_handle, next_state);
        }
        if plan.reference_backend {
            let reference_multi = match ensure_reference_multi(wrapper, multi_ptr) {
                Ok(reference_multi) => reference_multi,
                Err(code) => return code,
            };
            let suppressed_verbose = suppress_reference_verbose(easy_handle);
            let rc = unsafe { ref_multi_add_handle()(reference_multi, easy_handle) };
            if rc != crate::abi::CURLM_OK {
                restore_reference_verbose(easy_handle, suppressed_verbose);
                return rc;
            }
            if let Some(record) = wrapper
                .inner
                .lock()
                .expect("multi mutex poisoned")
                .records
                .get_mut(&(easy_handle as usize))
            {
                record.reference_verbose_suppressed = suppressed_verbose;
            }
        } else {
            let Ok((poll_reader, mut poll_writer)) = UnixStream::pair() else {
                return CURLM_INTERNAL_ERROR;
            };
            let socket_fd = poll_reader.as_raw_fd() as curl_socket_t;
            let socket_rc =
                invoke_socket_callback(wrapper, multi_ptr, easy_handle, socket_fd, CURL_POLL_IN);
            if socket_rc != crate::abi::CURLM_OK {
                return socket_rc;
            }
            let events = Arc::clone(&wrapper.events);
            let easy_key = easy_handle as usize;
            let join = transfer::spawn_transfer(easy_key, plan, move |result| {
                let _ = poll_writer.write_all(&[1]);
                let _ = poll_writer.flush();
                events.push(TransferEvent::Completed { easy_key, result });
            });
            if let Some(record) = wrapper
                .inner
                .lock()
                .expect("multi mutex poisoned")
                .records
                .get_mut(&(easy_handle as usize))
            {
                record.poll_reader = Some(poll_reader);
                record.worker = Some(join);
            }
        }
    }

    update_timer(wrapper, multi_ptr)
}

fn wait_common(
    multi: *mut CURLM,
    extra_fds: *mut crate::abi::curl_waitfd,
    extra_nfds: c_uint,
    timeout_ms: c_int,
    ret: *mut c_int,
    allow_idle_wait: bool,
) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return CURLM_BAD_HANDLE;
    };
    if is_in_callback(wrapper) {
        return CURLM_RECURSIVE_API_CALL;
    }
    if is_dead(wrapper) {
        return CURLM_ABORTED_BY_CALLBACK;
    }
    let timeout_ms = match poll::validate_timeout(timeout_ms, CURLM_BAD_FUNCTION_ARGUMENT) {
        Ok(timeout_ms) => timeout_ms,
        Err(code) => return code,
    };
    zero_extra_fds(extra_fds, extra_nfds);

    let mut activity = drain_events(wrapper);
    if has_reference_handles(wrapper) {
        let mut reference_activity = 0;
        let reference_multi = wrapper
            .inner
            .lock()
            .expect("multi mutex poisoned")
            .reference_multi;
        if !reference_multi.is_null() {
            let rc = unsafe {
                if allow_idle_wait {
                    ref_multi_poll()(
                        reference_multi,
                        extra_fds,
                        extra_nfds,
                        timeout_ms,
                        &mut reference_activity,
                    )
                } else {
                    ref_multi_wait()(
                        reference_multi,
                        extra_fds,
                        extra_nfds,
                        timeout_ms,
                        &mut reference_activity,
                    )
                }
            };
            if rc != crate::abi::CURLM_OK {
                return rc;
            }
            activity += reference_activity;
            harvest_reference_messages(wrapper);
        }
    }
    let has_unstarted = {
        let guard = wrapper.inner.lock().expect("multi mutex poisoned");
        guard
            .records
            .values()
            .any(|record| !record.started && !record.completed)
    };
    if has_unstarted {
        if !ret.is_null() {
            unsafe { *ret = activity };
        }
        return crate::abi::CURLM_OK;
    }

    if activity == 0 && timeout_ms > 0 {
        let has_running = {
            let guard = wrapper.inner.lock().expect("multi mutex poisoned");
            guard
                .records
                .values()
                .any(|record| !record.completed && !record.plan.reference_backend)
        };
        if has_running || allow_idle_wait {
            match wait_for_event(wrapper, timeout_ms) {
                Ok(count) => activity += count,
                Err(code) => return code,
            }
        }
    }

    if !ret.is_null() {
        unsafe { *ret = activity };
    }
    crate::abi::CURLM_OK
}

fn wait_for_event(wrapper: &MultiHandle, timeout_ms: c_int) -> Result<c_int, CURLMcode> {
    let event = wrapper
        .events
        .pop_timeout(Duration::from_millis(timeout_ms as u64));
    let mut activity = 0;
    if let Some(event) = event {
        activity += process_event(wrapper, event);
        activity += drain_events(wrapper);
    }
    Ok(activity)
}

fn drain_events(wrapper: &MultiHandle) -> c_int {
    let events = wrapper.events.drain();
    let mut activity = 0;
    for event in events {
        activity += process_event(wrapper, event);
    }
    activity
}

fn process_event(wrapper: &MultiHandle, event: TransferEvent) -> c_int {
    match event {
        TransferEvent::Completed { easy_key, result } => {
            let (easy_handle, join, multi_ptr, socket_fd, connection_id, host, reference_backend) = {
                let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
                let Some(record) = guard.records.get_mut(&easy_key) else {
                    return 0;
                };
                if record.message_enqueued {
                    return 0;
                }
                record.completed = true;
                record.message_enqueued = true;
                record.state = state::MultiState::Completed;
                let easy_handle = record.easy;
                let join = record.worker.take();
                let socket_fd = record
                    .poll_reader
                    .as_ref()
                    .map(|reader| reader.as_raw_fd() as curl_socket_t)
                    .unwrap_or(CURL_SOCKET_BAD);
                let _ = record.poll_reader.take();
                let connection_id = record.connection_id;
                let host = record.plan.cache_key.host.clone();
                let reference_backend = record.plan.reference_backend;
                let multi_ptr = wrapper as *const MultiHandle as *mut CURLM;
                let _ = record;
                guard.messages.push_back(QueuedMessage {
                    easy_handle,
                    result,
                });
                (
                    easy_handle,
                    join,
                    multi_ptr,
                    socket_fd,
                    connection_id,
                    host,
                    reference_backend,
                )
            };

            if let Some(join) = join {
                let _ = join.join();
            }
            if !reference_backend
                && result == crate::abi::CURLE_OK
                && easy::perform::snapshot_metadata(easy_handle).verbose
            {
                eprintln!(
                    "* Connection #{} to host {} left intact",
                    connection_id.saturating_sub(1),
                    host
                );
            }
            easy::perform::on_transfer_progress(easy_handle, state::MultiState::Done);
            easy::perform::on_transfer_progress(easy_handle, state::MultiState::Completed);
            let _ = invoke_socket_callback(
                wrapper,
                multi_ptr,
                easy_handle,
                socket_fd,
                CURL_POLL_REMOVE,
            );
            let _ = update_timer(wrapper, multi_ptr);
            1
        }
        TransferEvent::Wakeup => 1,
    }
}

fn update_timer(wrapper: &MultiHandle, multi_ptr: *mut CURLM) -> CURLMcode {
    let (callback, userp, timeout_ms) = {
        let guard = wrapper.inner.lock().expect("multi mutex poisoned");
        let native_timeout = if guard.dead {
            0
        } else if !guard.messages.is_empty()
            || guard
                .records
                .values()
                .any(|record| !record.started && !record.completed)
        {
            0
        } else if guard.records.values().any(|record| !record.completed) {
            transfer::EASY_PERFORM_WAIT_TIMEOUT_MS as c_long
        } else {
            -1
        };
        let timeout_ms = combine_timeouts(native_timeout, guard.reference_timeout_ms);
        (
            guard.callbacks.timer_cb,
            guard.callbacks.timer_userp,
            timeout_ms,
        )
    };

    let Some(callback) = callback else {
        return crate::abi::CURLM_OK;
    };

    {
        let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
        if guard.callbacks.in_callback {
            return CURLM_RECURSIVE_API_CALL;
        }
        guard.callbacks.in_callback = true;
    }
    let rc = unsafe { callback(multi_ptr, timeout_ms, userp) };
    {
        let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
        guard.callbacks.in_callback = false;
        if rc == -1 {
            guard.dead = true;
        }
    }

    if rc == -1 {
        CURLM_ABORTED_BY_CALLBACK
    } else {
        crate::abi::CURLM_OK
    }
}

fn invoke_socket_callback(
    wrapper: &MultiHandle,
    multi_ptr: *mut CURLM,
    easy_handle: *mut CURL,
    socket: curl_socket_t,
    what: c_int,
) -> CURLMcode {
    let (callback, userp, socketp) = {
        let guard = wrapper.inner.lock().expect("multi mutex poisoned");
        let socketp = guard
            .assignments
            .get(&socket)
            .copied()
            .unwrap_or(ptr::null_mut());
        (
            guard.callbacks.socket_cb,
            guard.callbacks.socket_userp,
            socketp,
        )
    };

    let Some(callback) = callback else {
        return crate::abi::CURLM_OK;
    };

    {
        let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
        if guard.callbacks.in_callback {
            return CURLM_RECURSIVE_API_CALL;
        }
        guard.callbacks.in_callback = true;
    }
    let rc = unsafe { callback(easy_handle, socket, what, userp, socketp) };
    {
        let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
        guard.callbacks.in_callback = false;
        if rc == -1 {
            guard.dead = true;
        }
    }
    let _ = multi_ptr;

    if rc == -1 {
        CURLM_ABORTED_BY_CALLBACK
    } else {
        crate::abi::CURLM_OK
    }
}

fn state_path_for(plan: &transfer::TransferPlan) -> Vec<state::MultiState> {
    let mut states = vec![state::MultiState::Connect];
    if !plan.resolve_overrides.is_empty() {
        states.push(state::MultiState::Resolving);
    }
    states.push(state::MultiState::Connecting);
    if plan.cache_key.proxy_host.is_some() && plan.cache_key.tunnel_proxy {
        states.push(state::MultiState::Tunneling);
    }
    states.push(state::MultiState::ProtoConnect);
    states.push(state::MultiState::Do);
    if plan.low_speed.enabled() {
        states.push(state::MultiState::RateLimiting);
    }
    states.push(state::MultiState::Performing);
    states
}

fn zero_extra_fds(extra_fds: *mut crate::abi::curl_waitfd, extra_nfds: c_uint) {
    for idx in 0..extra_nfds as usize {
        let waitfd = unsafe { extra_fds.add(idx) };
        if !waitfd.is_null() {
            unsafe { (*waitfd).revents = 0 };
        }
    }
}

fn fd_set_insert(set: *mut libc_fd_set, socket: curl_socket_t) {
    if set.is_null() || socket < 0 {
        return;
    }

    let socket = socket as usize;
    let bits_per_word = 8 * size_of::<c_long>();
    let word = socket / bits_per_word;
    if word >= unsafe { (*set).fds_bits.len() } {
        return;
    }
    let bit = socket % bits_per_word;
    unsafe {
        (*set).fds_bits[word] |= 1 << bit;
    }
}
