pub(crate) mod poll;
pub(crate) mod state;

use crate::abi::{
    curl_off_t, curl_pushheaders, curl_socket_t, CURLMcode, CURLMoption, CURLMsg, CURLcode, CURL,
    CURLM, CURLMSG,
};
use crate::conn::cache::ConnectionCache;
use crate::dns::ResolverOwner;
use crate::{alloc, easy, transfer};
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
const CURLOPT_URL: c_long = 10002;
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

#[derive(Clone, Debug)]
pub(crate) struct SyntheticPushRequest {
    pub url: String,
    pub headers: Vec<String>,
}

enum TransferEvent {
    Completed { easy_key: usize, result: CURLcode },
    PushesOffered {
        parent_easy_key: usize,
        pushes: Vec<SyntheticPushRequest>,
    },
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

fn remember_connection_id(
    inner: &mut MultiInner,
    plan: &transfer::TransferPlan,
    metadata: &easy::perform::EasyMetadata,
) -> usize {
    let maxconnects = inner
        .maxconnects
        .max(metadata.maxconnects.unwrap_or(0).max(0) as usize);
    let next_connection_id = inner.next_connection_id;
    let (connection_id, reused) =
        inner
            .conncache
            .remember(plan.cache_key.clone(), maxconnects, next_connection_id);
    if !reused {
        inner.next_connection_id += 1;
    }
    connection_id
}

fn spawn_transfer_worker(
    wrapper: &MultiHandle,
    multi_ptr: *mut CURLM,
    easy_handle: *mut CURL,
    plan: transfer::TransferPlan,
) -> Result<(UnixStream, std::thread::JoinHandle<()>), CURLMcode> {
    for next_state in state_path_for(&plan) {
        easy::perform::on_transfer_progress(easy_handle, next_state);
    }
    debug_assert!(
        !plan.reference_backend,
        "public multi transfers must stay on the native path"
    );

    let Ok((poll_reader, mut poll_writer)) = UnixStream::pair() else {
        return Err(CURLM_INTERNAL_ERROR);
    };
    let socket_fd = poll_reader.as_raw_fd() as curl_socket_t;
    let socket_rc = invoke_socket_callback(wrapper, multi_ptr, easy_handle, socket_fd, CURL_POLL_IN);
    if socket_rc != crate::abi::CURLM_OK {
        return Err(socket_rc);
    }
    let events = Arc::clone(&wrapper.events);
    let easy_key = easy_handle as usize;
    let join = transfer::spawn_transfer(easy_key, plan, move |result| {
        let _ = poll_writer.write_all(&[1]);
        let _ = poll_writer.flush();
        events.push(TransferEvent::Completed { easy_key, result });
    });
    Ok((poll_reader, join))
}

fn cleanup_synthetic_handle(easy_handle: *mut CURL) {
    if easy_handle.is_null() {
        return;
    }
    unsafe { crate::easy::handle::easy_cleanup(easy_handle) };
}

pub(crate) fn schedule_http2_pushes(parent_easy: *mut CURL, pushes: Vec<SyntheticPushRequest>) {
    if pushes.is_empty() || parent_easy.is_null() {
        return;
    }

    let Some(multi_ptr) = easy::perform::attached_multi_for(parent_easy).map(|value| value as *mut CURLM)
    else {
        return;
    };
    let Some(wrapper) = wrapper_from_ptr(multi_ptr) else {
        return;
    };
    let (push_cb, push_userp) = {
        let guard = wrapper.inner.lock().expect("multi mutex poisoned");
        (guard.callbacks.push_cb, guard.callbacks.push_userp)
    };
    let Some(push_cb) = push_cb else {
        return;
    };

    for push in pushes {
        let pushed_easy = unsafe { easy::handle::alloc_public_handle() };
        easy::perform::register_duplicate(parent_easy, pushed_easy);
        easy::perform::configure_push_handle(pushed_easy, push.url.clone());

        let headers = crate::protocols::create_push_headers(&push.headers);
        {
            let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
            if guard.callbacks.in_callback {
                crate::protocols::release_push_headers(headers);
                cleanup_synthetic_handle(pushed_easy);
                return;
            }
            guard.callbacks.in_callback = true;
        }
        let decision = unsafe {
            push_cb(
                parent_easy,
                pushed_easy,
                push.headers.len(),
                headers,
                push_userp,
            )
        };
        crate::protocols::release_push_headers(headers);
        {
            let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
            guard.callbacks.in_callback = false;
        }
        if decision != CURL_PUSH_OK {
            cleanup_synthetic_handle(pushed_easy);
            continue;
        }

        let metadata = easy::perform::snapshot_metadata(pushed_easy);
        let plan = transfer::build_plan(&metadata, ResolverOwner::Multi);
        let connection_id = {
            let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
            if guard.records.contains_key(&(pushed_easy as usize)) {
                cleanup_synthetic_handle(pushed_easy);
                continue;
            }
            let connection_id = remember_connection_id(&mut guard, &plan, &metadata);
            if !guard.easies.contains(&pushed_easy) {
                guard.easies.push(pushed_easy);
            }
            guard.records.insert(
                pushed_easy as usize,
                TransferRecord {
                    easy: pushed_easy,
                    state: state::MultiState::Pending,
                    plan: plan.clone(),
                    connection_id,
                    poll_reader: None,
                    worker: None,
                    started: true,
                    completed: false,
                    message_enqueued: false,
                },
            );
            connection_id
        };
        let _ = connection_id;
        easy::perform::on_attached(pushed_easy, multi_ptr as usize, state::MultiState::Pending);
        match spawn_transfer_worker(wrapper, multi_ptr, pushed_easy, plan) {
            Ok((poll_reader, worker)) => {
                if let Some(record) = wrapper
                    .inner
                    .lock()
                    .expect("multi mutex poisoned")
                    .records
                    .get_mut(&(pushed_easy as usize))
                {
                    record.poll_reader = Some(poll_reader);
                    record.worker = Some(worker);
                }
                let _ = update_timer(wrapper, multi_ptr);
            }
            Err(_) => {
                {
                    let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
                    guard.easies.retain(|candidate| *candidate != pushed_easy);
                    guard.records.remove(&(pushed_easy as usize));
                }
                cleanup_synthetic_handle(pushed_easy);
            }
        }
    }
}

pub(crate) fn offer_http2_pushes(parent_easy: *mut CURL, pushes: Vec<SyntheticPushRequest>) {
    if pushes.is_empty() || !has_push_callback(parent_easy) {
        return;
    }

    let Some(multi_ptr) = easy::perform::attached_multi_for(parent_easy).map(|value| value as *mut CURLM)
    else {
        return;
    };
    let Some(wrapper) = wrapper_from_ptr(multi_ptr) else {
        return;
    };
    wrapper.events.push(TransferEvent::PushesOffered {
        parent_easy_key: parent_easy as usize,
        pushes,
    });
}

pub(crate) fn has_push_callback(parent_easy: *mut CURL) -> bool {
    if parent_easy.is_null() {
        return false;
    }

    let Some(multi_ptr) = easy::perform::attached_multi_for(parent_easy).map(|value| value as *mut CURLM)
    else {
        return false;
    };
    let Some(wrapper) = wrapper_from_ptr(multi_ptr) else {
        return false;
    };
    wrapper
        .inner
        .lock()
        .expect("multi mutex poisoned")
        .callbacks
        .push_cb
        .is_some()
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
    let (joins, easies) = {
        let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
        let joins = guard
            .records
            .values_mut()
            .filter_map(|record| record.worker.take())
            .collect::<Vec<_>>();
        let easies = guard.easies.clone();
        guard.records.clear();
        guard.easies.clear();
        guard.messages.clear();
        guard.current_msg = None;
        guard.assignments.clear();
        (joins, easies)
    };

    for join in joins {
        let _ = join.join();
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
    let plan = easy::perform::plan_for_multi(easy_handle, &metadata);
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
    let join = {
        let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
        let Some(mut record) = guard.records.remove(&(easy_handle as usize)) else {
            return CURLM_BAD_EASY_HANDLE;
        };
        let _ = record.connection_id;
        guard.easies.retain(|candidate| *candidate != easy_handle);
        guard.messages.retain(|msg| msg.easy_handle != easy_handle);
        record.worker.take()
    };

    if let Some(join) = join {
        let _ = join.join();
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
        let join = {
            let mut guard = wrapper.inner.lock().expect("multi mutex poisoned");
            guard.easies.retain(|candidate| *candidate != easy_handle);
            guard.messages.retain(|msg| msg.easy_handle != easy_handle);
            let Some(mut record) = guard.records.remove(&(easy_handle as usize)) else {
                return;
            };
            record.worker.take()
        };
        if let Some(join) = join {
            let _ = join.join();
        }
    }
}

pub(crate) unsafe fn fdset_handle(
    multi: *mut CURLM,
    read_fd_set: *mut libc_fd_set,
    _write_fd_set: *mut libc_fd_set,
    _exc_fd_set: *mut libc_fd_set,
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
    unsafe { *milliseconds = native_timeout_ms(wrapper) };
    crate::abi::CURLM_OK
}

pub(crate) unsafe fn wakeup_handle(multi: *mut CURLM) -> CURLMcode {
    let Some(wrapper) = wrapper_from_ptr(multi) else {
        return CURLM_BAD_HANDLE;
    };
    wrapper.events.push(TransferEvent::Wakeup);
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
    _socket: curl_socket_t,
    _ev_bitmask: c_int,
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
    drop(guard);
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

fn native_timeout_from_inner(inner: &MultiInner) -> c_long {
    if inner.dead {
        0
    } else if !inner.messages.is_empty()
        || inner
            .records
            .values()
            .any(|record| !record.started && !record.completed)
    {
        0
    } else if inner.records.values().any(|record| !record.completed) {
        transfer::EASY_PERFORM_WAIT_TIMEOUT_MS as c_long
    } else {
        -1
    }
}

fn native_timeout_ms(wrapper: &MultiHandle) -> c_long {
    let guard = wrapper.inner.lock().expect("multi mutex poisoned");
    native_timeout_from_inner(&guard)
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
        let _ = states;
        let (poll_reader, join) =
            match spawn_transfer_worker(wrapper, multi_ptr, easy_handle, plan) {
                Ok(values) => values,
                Err(code) => return code,
            };
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
    let timeout_ms = poll::clamp_timeout(timeout_ms, native_timeout_ms(wrapper));
    zero_extra_fds(extra_fds, extra_nfds);

    let mut activity = drain_events(wrapper);
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
            guard.records.values().any(|record| !record.completed)
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
        TransferEvent::PushesOffered {
            parent_easy_key,
            pushes,
        } => {
            schedule_http2_pushes(parent_easy_key as *mut CURL, pushes);
            1
        }
        TransferEvent::Wakeup => 1,
    }
}

fn update_timer(wrapper: &MultiHandle, multi_ptr: *mut CURLM) -> CURLMcode {
    let (callback, userp, timeout_ms) = {
        let guard = wrapper.inner.lock().expect("multi mutex poisoned");
        let timeout_ms = native_timeout_from_inner(&guard);
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
