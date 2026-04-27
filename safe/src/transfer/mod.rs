use crate::abi::{
    curl_hstsentry, curl_index, curl_off_t, curl_slist, curl_socket_t, CURLMcode, CURLSTScode,
    CURLcode, CURL, CURLE_BAD_FUNCTION_ARGUMENT, CURLINFO,
};
use crate::conn::cache::{parse_proxy_authority, parse_url_authority, ConnectionCacheKey};
use crate::conn::filter::{ConnectionFilterChain, ConnectionFilterStep};
use crate::dns::{ConnectOverride, ResolveOverride, ResolverLease, ResolverOwner};
use crate::easy::perform::{self, EasyCallbacks, EasyMetadata, RecordedTransferInfo};
use crate::http::auth;
use crate::http::cookies;
use crate::http::hsts;
use crate::http::request::{self, Origin};
use crate::http::response::{self, HEADER_ORIGIN_1XX, HEADER_ORIGIN_HEADER};
use core::ffi::{c_int, c_long, c_void};
use std::collections::HashMap;
use std::fs::File;
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::net::{IpAddr, Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(crate) const EASY_PERFORM_WAIT_TIMEOUT_MS: c_int = 1000;
pub(crate) const CURLINFO_ACTIVESOCKET: CURLINFO = 0x500000 + 44;

const CURLM_OUT_OF_MEMORY: CURLMcode = 3;
const CURLE_LOGIN_DENIED: CURLcode = 67;

const CURLE_UNSUPPORTED_PROTOCOL: CURLcode = 1;
const CURLE_URL_MALFORMAT: CURLcode = 3;
const CURLE_COULDNT_RESOLVE_HOST: CURLcode = 6;
const CURLE_COULDNT_CONNECT: CURLcode = 7;
const CURLE_HTTP_RETURNED_ERROR: CURLcode = 22;
const CURLE_WRITE_ERROR: CURLcode = 23;
const CURLE_READ_ERROR: CURLcode = 26;
const CURLE_OPERATION_TIMEDOUT: CURLcode = 28;
const CURLE_RANGE_ERROR: CURLcode = 33;
const CURLE_FILE_COULDNT_READ_FILE: CURLcode = 37;
const CURLE_ABORTED_BY_CALLBACK: CURLcode = 42;
const CURLE_SEND_ERROR: CURLcode = 55;
const CURLE_RECV_ERROR: CURLcode = 56;
const CURLE_AGAIN: CURLcode = 81;
const CURLSTS_OK: CURLSTScode = 0;
const CURLSTS_DONE: CURLSTScode = 1;
const CURLSTS_FAIL: CURLSTScode = 2;
const CURLHEADER_SEPARATE: c_long = 1 << 0;

const CURLPAUSE_RECV: c_int = 1 << 0;
const CURLPAUSE_SEND: c_int = 1 << 2;
const CURLPAUSE_ALL: c_int = CURLPAUSE_RECV | CURLPAUSE_SEND;
const CURL_WRITEFUNC_PAUSE: usize = 0x10000001;
const CURL_READFUNC_PAUSE: usize = 0x10000001;
const CURLSOCKTYPE_IPCXN: c_int = 0;
const CURL_SOCKET_BAD: curl_socket_t = -1;
const AF_INET: c_int = 2;
const AF_INET6: c_int = 10;
const SOCK_STREAM: c_int = 1;
const IPPROTO_TCP: c_int = 6;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const IO_POLL_INTERVAL: Duration = Duration::from_millis(200);
const HEADER_WAIT_TIMEOUT: Duration = Duration::from_secs(30);
const REDIRECT_LIMIT: usize = 8;
const CURL_HTTP_VERSION_1_0: c_long = 1;
const CURL_HTTP_VERSION_1_1: c_long = 2;
const CURL_HTTP_VERSION_2_0: c_long = 3;
const CURL_HTTP_VERSION_2TLS: c_long = 4;
const CURL_HTTP_VERSION_2_PRIOR_KNOWLEDGE: c_long = 5;

unsafe extern "C" {
    static mut stdin: *mut c_void;
    static mut stdout: *mut c_void;
    fn fread(ptr: *mut c_void, size: usize, nmemb: usize, stream: *mut c_void) -> usize;
    fn fwrite(ptr: *const c_void, size: usize, nmemb: usize, stream: *mut c_void) -> usize;
    fn connect(fd: c_int, addr: *const crate::abi::sockaddr, len: u32) -> c_int;
    fn close(fd: c_int) -> c_int;
}

#[repr(C)]
struct in_addr {
    s_addr: u32,
}

#[repr(C)]
struct sockaddr_in {
    sin_family: u16,
    sin_port: u16,
    sin_addr: in_addr,
    sin_zero: [u8; 8],
}

#[repr(C)]
struct in6_addr {
    s6_addr: [u8; 16],
}

#[repr(C)]
struct sockaddr_in6 {
    sin6_family: u16,
    sin6_port: u16,
    sin6_flowinfo: u32,
    sin6_addr: in6_addr,
    sin6_scope_id: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct LowSpeedWindow {
    pub limit_bytes_per_second: c_long,
    pub time_window_secs: c_long,
}

impl LowSpeedWindow {
    pub(crate) const fn enabled(self) -> bool {
        self.limit_bytes_per_second > 0 && self.time_window_secs > 0
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TransferPlan {
    pub cache_key: ConnectionCacheKey,
    pub resolver: ResolverLease,
    pub resolve_overrides: Vec<ResolveOverride>,
    pub connect_override: Option<ConnectOverride>,
    pub route: crate::protocols::TransferRoute,
    pub tls: Option<crate::tls::TlsPolicy>,
    pub filters: ConnectionFilterChain,
    pub low_speed: LowSpeedWindow,
    pub connect_only: bool,
    pub reference_backend: bool,
}

pub(crate) struct ConnectOnlySession {
    pub(crate) stream: TcpStream,
    pub(crate) paused: c_int,
    pub(crate) websocket: Option<crate::ws::WebSocketSession>,
}

pub(crate) struct ConnectedStream {
    stream: TcpStream,
    info: RecordedTransferInfo,
}

pub(crate) enum TransportStream {
    Plain(TcpStream),
    Tls(crate::tls::TlsConnection),
}

impl TransportStream {
    pub(crate) fn set_read_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        match self {
            Self::Plain(stream) => stream.set_read_timeout(timeout),
            Self::Tls(stream) => stream.set_read_timeout(timeout),
        }
    }

    pub(crate) fn set_write_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        match self {
            Self::Plain(stream) => stream.set_write_timeout(timeout),
            Self::Tls(stream) => stream.set_write_timeout(timeout),
        }
    }
}

impl Read for TransportStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.read(buf),
            Self::Tls(stream) => stream.read(buf),
        }
    }
}

impl Write for TransportStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.write(buf),
            Self::Tls(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Plain(stream) => stream.flush(),
            Self::Tls(stream) => stream.flush(),
        }
    }
}

struct ParsedUrl {
    scheme: String,
    host: String,
    port: u16,
    host_header: String,
    path_and_query: String,
}

struct RequestContext {
    url: String,
    scheme: String,
    host_header: String,
    target_host: String,
    target_port: u16,
    proxy: Option<(String, u16)>,
    request_target: String,
    method: String,
    request_headers: Vec<String>,
    proxy_headers: Vec<String>,
    tunnel_proxy: bool,
    websocket_style: bool,
    use_chunked_upload: bool,
    range_header: Option<String>,
    body_length: Option<usize>,
}

struct ResponseMeta {
    status_code: u16,
    content_length: Option<usize>,
    has_content_range: bool,
    retry_after: Option<curl_off_t>,
    location: Option<String>,
    body_prefix: Vec<u8>,
}

struct TransferOutcome {
    result: CURLcode,
    response_code: u16,
    retry_after: Option<curl_off_t>,
    location: Option<String>,
    info: RecordedTransferInfo,
}

pub(crate) struct LowSpeedGuard {
    window: LowSpeedWindow,
    window_start: Instant,
    window_bytes: usize,
}

impl LowSpeedGuard {
    pub(crate) fn new(window: LowSpeedWindow) -> Self {
        Self {
            window,
            window_start: Instant::now(),
            window_bytes: 0,
        }
    }

    pub(crate) fn observe_idle(&mut self) -> Result<(), CURLcode> {
        self.check(Instant::now())
    }

    pub(crate) fn observe_progress(&mut self, count: usize) -> Result<(), CURLcode> {
        self.window_bytes = self.window_bytes.saturating_add(count);
        let now = Instant::now();
        self.check(now)?;
        if self.window.enabled()
            && now.duration_since(self.window_start).as_secs()
                >= self.window.time_window_secs.max(0) as u64
        {
            self.window_start = now;
            self.window_bytes = 0;
        }
        Ok(())
    }

    fn check(&self, now: Instant) -> Result<(), CURLcode> {
        if !self.window.enabled() {
            return Ok(());
        }

        let elapsed_secs = now.duration_since(self.window_start).as_secs();
        if elapsed_secs >= self.window.time_window_secs.max(0) as u64 {
            let required = (self.window.limit_bytes_per_second.max(0) as usize)
                .saturating_mul(elapsed_secs as usize);
            if self.window_bytes < required {
                return Err(CURLE_OPERATION_TIMEDOUT);
            }
        }
        Ok(())
    }
}

fn connect_only_registry() -> &'static Mutex<HashMap<usize, ConnectOnlySession>> {
    static REGISTRY: OnceLock<Mutex<HashMap<usize, ConnectOnlySession>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

struct PauseRegistry {
    bits: Mutex<HashMap<usize, c_int>>,
    ready: std::sync::Condvar,
}

fn pause_registry() -> &'static PauseRegistry {
    static REGISTRY: OnceLock<PauseRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| PauseRegistry {
        bits: Mutex::new(HashMap::new()),
        ready: std::sync::Condvar::new(),
    })
}

fn set_pause_mask(handle: *mut CURL, bitmask: c_int) {
    if handle.is_null() {
        return;
    }
    let registry = pause_registry();
    let mut guard = registry.bits.lock().expect("pause registry mutex poisoned");
    if bitmask == 0 {
        guard.remove(&(handle as usize));
    } else {
        guard.insert(handle as usize, bitmask);
    }
    registry.ready.notify_all();
}

fn add_pause_mask(handle: *mut CURL, mask: c_int) {
    if handle.is_null() {
        return;
    }
    let registry = pause_registry();
    let mut guard = registry.bits.lock().expect("pause registry mutex poisoned");
    let entry = guard.entry(handle as usize).or_insert(0);
    *entry |= mask;
    registry.ready.notify_all();
}

fn clear_pause_state(handle: *mut CURL) {
    if handle.is_null() {
        return;
    }
    let registry = pause_registry();
    let mut guard = registry.bits.lock().expect("pause registry mutex poisoned");
    guard.remove(&(handle as usize));
    registry.ready.notify_all();
}

fn wait_for_pause_clear(handle: *mut CURL, mask: c_int) {
    if handle.is_null() {
        return;
    }
    let registry = pause_registry();
    let guard = registry.bits.lock().expect("pause registry mutex poisoned");
    let _guard = registry
        .ready
        .wait_while(guard, |bits| {
            bits.get(&(handle as usize)).copied().unwrap_or(0) & mask != 0
        })
        .expect("pause registry mutex poisoned");
}

pub(crate) fn with_connect_only_session_mut<R>(
    handle: *mut CURL,
    f: impl FnOnce(&mut ConnectOnlySession) -> R,
) -> Option<R> {
    if handle.is_null() {
        return None;
    }
    let mut guard = connect_only_registry()
        .lock()
        .expect("connect-only registry mutex poisoned");
    let session = guard.get_mut(&(handle as usize))?;
    Some(f(session))
}

pub(crate) const fn map_multi_code(code: CURLMcode) -> CURLcode {
    if code == CURLM_OUT_OF_MEMORY {
        crate::abi::CURLE_OUT_OF_MEMORY
    } else {
        crate::abi::CURLE_BAD_FUNCTION_ARGUMENT
    }
}

pub(crate) fn build_plan(metadata: &EasyMetadata, resolver_owner: ResolverOwner) -> TransferPlan {
    let authority = metadata
        .url
        .as_deref()
        .and_then(parse_url_authority)
        .unwrap_or_else(|| crate::conn::cache::UrlAuthority {
            scheme: "http".to_string(),
            host: String::new(),
            port: 0,
        });
    let route = crate::protocols::route_scheme(
        &authority.scheme,
        metadata.connect_mode,
        metadata.http_version,
    );
    let tls = crate::tls::policy_for_route(route, metadata);
    let connect_override = metadata
        .connect_overrides
        .iter()
        .find(|candidate| candidate.matches(&authority.host, authority.port))
        .cloned();
    let proxy = metadata
        .proxy
        .as_deref()
        .and_then(|proxy| parse_proxy_authority(proxy, &authority.scheme));
    let pre_proxy = metadata
        .pre_proxy
        .as_deref()
        .and_then(|proxy| parse_proxy_authority(proxy, &authority.scheme));
    let resolver = ResolverLease::for_share(metadata.share_handle, resolver_owner);
    let share_scope = resolver.share_scope.clone();
    let mut filters = ConnectionFilterChain::default();

    if !metadata.resolve_overrides.is_empty() {
        filters.push(ConnectionFilterStep::ResolveOverrides {
            count: metadata.resolve_overrides.len(),
        });
    }
    if let Some(override_target) = connect_override.as_ref() {
        let target = match (&override_target.target_host, override_target.target_port) {
            (Some(host), Some(port)) => format!("{host}:{port}"),
            (Some(host), None) => host.clone(),
            (None, Some(port)) => format!(":{port}"),
            (None, None) => String::new(),
        };
        if !target.is_empty() {
            filters.push(ConnectionFilterStep::ConnectTo { target });
        }
    }
    if let Some((proxy_host, proxy_port)) = pre_proxy.as_ref() {
        filters.push(ConnectionFilterStep::PreProxy {
            authority: format!("{proxy_host}:{proxy_port}"),
        });
    }
    if let Some((proxy_host, proxy_port)) = proxy.as_ref() {
        filters.push(ConnectionFilterStep::Proxy {
            authority: format!("{proxy_host}:{proxy_port}"),
            tunnel: metadata.tunnel_proxy,
        });
    }
    if let Some(scope) = share_scope.as_ref() {
        filters.push(ConnectionFilterStep::ShareLock {
            scope: scope.clone(),
        });
    }
    if metadata.low_speed.enabled() {
        filters.push(ConnectionFilterStep::LowSpeedGuard {
            limit_bytes_per_second: metadata.low_speed.limit_bytes_per_second,
            time_window_secs: metadata.low_speed.time_window_secs,
        });
    }
    if metadata.connect_only {
        filters.push(ConnectionFilterStep::ConnectOnly);
    }
    if metadata.follow_location {
        filters.push(ConnectionFilterStep::FollowRedirects);
    }
    filters.push(ConnectionFilterStep::TransferLoop);

    TransferPlan {
        cache_key: ConnectionCacheKey {
            scheme: authority.scheme,
            host: authority.host,
            port: authority.port,
            proxy_host: proxy.as_ref().map(|(host, _)| host.clone()),
            proxy_port: proxy.as_ref().map(|(_, port)| *port),
            pre_proxy_host: pre_proxy.as_ref().map(|(host, _)| host.clone()),
            pre_proxy_port: pre_proxy.as_ref().map(|(_, port)| *port),
            tunnel_proxy: metadata.tunnel_proxy,
            conn_to_host: connect_override
                .as_ref()
                .and_then(|candidate| candidate.target_host.clone()),
            conn_to_port: connect_override
                .as_ref()
                .and_then(|candidate| candidate.target_port),
            tls_peer_identity: metadata.tls_peer_identity(),
            auth_context: metadata.auth_context(),
            share_scope,
        },
        resolver,
        resolve_overrides: metadata.resolve_overrides.clone(),
        connect_override,
        route,
        tls,
        filters,
        low_speed: metadata.low_speed,
        connect_only: metadata.connect_only,
        reference_backend: requires_reference_backend(metadata, route),
    }
}

pub(crate) fn spawn_transfer<F>(
    handle_key: usize,
    plan: TransferPlan,
    on_complete: F,
) -> std::thread::JoinHandle<()>
where
    F: FnOnce(CURLcode) + Send + 'static,
{
    std::thread::spawn(move || {
        let result = perform_transfer(handle_key, plan);
        on_complete(result);
    })
}

pub(crate) fn perform_transfer_sync(handle: *mut CURL, plan: TransferPlan) -> CURLcode {
    if handle.is_null() {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    }
    let metadata = perform::snapshot_metadata(handle);
    let callbacks = perform::snapshot_callbacks(handle);
    perform_transfer_impl(handle, plan, metadata, callbacks)
}

pub(crate) fn perform_transfer_sync_with(
    handle: *mut CURL,
    plan: TransferPlan,
    metadata: EasyMetadata,
    callbacks: EasyCallbacks,
) -> CURLcode {
    if handle.is_null() {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    }
    perform_transfer_impl(handle, plan, metadata, callbacks)
}

pub(crate) fn release_handle_state(handle: *mut CURL) {
    if handle.is_null() {
        return;
    }

    clear_pause_state(handle);
    let mut guard = connect_only_registry()
        .lock()
        .expect("connect-only registry mutex poisoned");
    if let Some(session) = guard.remove(&(handle as usize)) {
        let _ = session.stream.shutdown(Shutdown::Both);
    }
    if guard.is_empty() {
        guard.shrink_to_fit();
    }
}

pub(crate) fn active_socket(handle: *mut CURL) -> Option<curl_socket_t> {
    if handle.is_null() {
        return None;
    }

    connect_only_registry()
        .lock()
        .expect("connect-only registry mutex poisoned")
        .get(&(handle as usize))
        .map(|session| session.stream.as_raw_fd() as curl_socket_t)
}

pub(crate) fn has_connect_only_session(handle: *mut CURL) -> bool {
    if handle.is_null() {
        return false;
    }

    connect_only_registry()
        .lock()
        .expect("connect-only registry mutex poisoned")
        .contains_key(&(handle as usize))
}

pub(crate) unsafe fn pause_handle(handle: *mut CURL, bitmask: c_int) -> CURLcode {
    if handle.is_null() {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    }

    let pause_bits = bitmask & CURLPAUSE_ALL;
    if let Some(session) = connect_only_registry()
        .lock()
        .expect("connect-only registry mutex poisoned")
        .get_mut(&(handle as usize))
    {
        session.paused = pause_bits;
    }
    set_pause_mask(handle, pause_bits);
    crate::abi::CURLE_OK
}

pub(crate) unsafe fn recv_handle(
    handle: *mut CURL,
    buffer: *mut c_void,
    buflen: usize,
    nread: *mut usize,
) -> CURLcode {
    if handle.is_null() || buffer.is_null() || nread.is_null() {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    }
    unsafe { *nread = 0 };

    let mut guard = connect_only_registry()
        .lock()
        .expect("connect-only registry mutex poisoned");
    let Some(session) = guard.get_mut(&(handle as usize)) else {
        return CURLE_AGAIN;
    };
    if session.paused & CURLPAUSE_RECV != 0 {
        return CURLE_AGAIN;
    }

    match session
        .stream
        .read(unsafe { std::slice::from_raw_parts_mut(buffer.cast::<u8>(), buflen) })
    {
        Ok(read) => {
            unsafe { *nread = read };
            crate::abi::CURLE_OK
        }
        Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
            CURLE_AGAIN
        }
        Err(_) => CURLE_RECV_ERROR,
    }
}

pub(crate) unsafe fn send_handle(
    handle: *mut CURL,
    buffer: *const c_void,
    buflen: usize,
    nwritten: *mut usize,
) -> CURLcode {
    if handle.is_null() || buffer.is_null() || nwritten.is_null() {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    }
    unsafe { *nwritten = 0 };

    let mut guard = connect_only_registry()
        .lock()
        .expect("connect-only registry mutex poisoned");
    let Some(session) = guard.get_mut(&(handle as usize)) else {
        return CURLE_AGAIN;
    };
    if session.paused & CURLPAUSE_SEND != 0 {
        return CURLE_AGAIN;
    }

    match session
        .stream
        .write(unsafe { std::slice::from_raw_parts(buffer.cast::<u8>(), buflen) })
    {
        Ok(written) => {
            unsafe { *nwritten = written };
            crate::abi::CURLE_OK
        }
        Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
            CURLE_AGAIN
        }
        Err(_) => CURLE_SEND_ERROR,
    }
}

pub(crate) unsafe fn upkeep_handle(handle: *mut CURL) -> CURLcode {
    if handle.is_null() {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    }

    let mut guard = connect_only_registry()
        .lock()
        .expect("connect-only registry mutex poisoned");
    let Some(session) = guard.get_mut(&(handle as usize)) else {
        return crate::abi::CURLE_OK;
    };

    match session.stream.write(&[]) {
        Ok(_) => crate::abi::CURLE_OK,
        Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
            crate::abi::CURLE_OK
        }
        Err(_) => CURLE_SEND_ERROR,
    }
}

fn perform_transfer_impl(
    handle: *mut CURL,
    plan: TransferPlan,
    metadata: EasyMetadata,
    callbacks: EasyCallbacks,
) -> CURLcode {
    match plan.route.handler {
        crate::protocols::SchemeHandler::File => {
            return perform_file_transfer(handle, &metadata, callbacks);
        }
        crate::protocols::SchemeHandler::Http | crate::protocols::SchemeHandler::WebSocket => {}
        _ => {
            return crate::protocols::perform_transfer(handle, &plan, &metadata, callbacks);
        }
    }
    let Some(initial_url) = metadata.url.clone() else {
        perform::set_error_buffer(handle, "No URL set");
        return CURLE_URL_MALFORMAT;
    };

    let _ = perform::with_http_state_mut(handle, |state| state.clear_transient());
    if let Err(code) = preload_hsts(handle, &metadata, callbacks, metadata.hsts_ctrl != 0) {
        return code;
    }
    let mut current_url = initial_url;
    let initial_origin = Origin::from_url(&current_url);
    let mut referer_value = metadata.referer.clone();
    let mut allow_cross_origin_auth = false;
    crate::share::touch_connect_callbacks(handle, metadata.share_handle, 6);
    let redirect_limit = if metadata.follow_location {
        metadata
            .max_redirs
            .unwrap_or(REDIRECT_LIMIT as c_long)
            .max(0) as usize
    } else {
        0
    };

    for redirect_count in 0..=redirect_limit {
        let mut request = match RequestContext::new(&current_url, &metadata) {
            Ok(request) => request,
            Err(code) => return code,
        };
        if redirect_count == 0 && should_use_reference_http2_transport(handle, &metadata, &request)
        {
            let result = crate::multi::run_reference_http2_session(handle);
            return finalize_hsts(
                handle,
                &metadata,
                callbacks,
                metadata.hsts_ctrl != 0,
                result,
            );
        }
        prepare_request_headers(
            handle,
            &metadata,
            &current_url,
            initial_origin.as_ref(),
            allow_cross_origin_auth,
            referer_value.as_deref(),
            &mut request,
        );

        let outcome = if plan.connect_only {
            connect_only_transfer(handle, &request, &metadata, callbacks)
        } else {
            execute_http_transfer(
                handle,
                &request,
                &plan,
                &metadata,
                callbacks,
                redirect_count,
            )
        };

        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(code) => {
                return finalize_hsts(handle, &metadata, callbacks, metadata.hsts_ctrl != 0, code)
            }
        };

        let mut recorded_info = outcome.info.clone();
        recorded_info.response_code = outcome.response_code as c_long;
        recorded_info.retry_after = outcome.retry_after;
        perform::record_transfer_info(handle, recorded_info);

        let decision = request::decide_redirect(
            &current_url,
            outcome.response_code,
            outcome.location.as_deref(),
            redirect_count,
            request::RedirectPolicy {
                enabled: metadata.follow_location,
                max_redirs: redirect_limit,
                unrestricted_auth: metadata.unrestricted_auth,
                auto_referer: metadata.auto_referer,
            },
            initial_origin.as_ref(),
        );
        if let Some(decision) = decision {
            current_url = decision.next_url;
            allow_cross_origin_auth = decision.allow_cross_origin_auth;
            if let Some(referer) = decision.referer {
                referer_value = Some(referer);
            }
            continue;
        }

        return finalize_hsts(
            handle,
            &metadata,
            callbacks,
            metadata.hsts_ctrl != 0,
            outcome.result,
        );
    }

    perform::set_error_buffer(handle, "Maximum redirects followed");
    finalize_hsts(
        handle,
        &metadata,
        callbacks,
        metadata.hsts_ctrl != 0,
        CURLE_BAD_FUNCTION_ARGUMENT,
    )
}

fn perform_transfer(handle_key: usize, plan: TransferPlan) -> CURLcode {
    let handle = handle_key as *mut CURL;
    release_handle_state(handle);
    let metadata = perform::snapshot_metadata(handle);
    let callbacks = perform::snapshot_callbacks(handle);
    perform_transfer_impl(handle, plan, metadata, callbacks)
}

fn preload_hsts(
    handle: *mut CURL,
    metadata: &EasyMetadata,
    callbacks: EasyCallbacks,
    enabled: bool,
) -> Result<(), CURLcode> {
    if !enabled {
        return Ok(());
    }
    let Some(callback) = callbacks.hsts_read_function else {
        return Ok(());
    };

    loop {
        let mut name = [0i8; 256];
        let mut entry = curl_hstsentry {
            name: name.as_mut_ptr(),
            namelen: name.len(),
            includeSubDomains: 0,
            expire: [0; 18],
        };
        let rc = unsafe { callback(handle, &mut entry, callbacks.hsts_read_data as *mut c_void) };
        match rc {
            CURLSTS_OK => {
                let host = unsafe { std::ffi::CStr::from_ptr(entry.name) }
                    .to_string_lossy()
                    .into_owned();
                let expire = hsts_expire_from_abi(&entry);
                let _ = with_hsts_store_mut(handle, metadata, |store| {
                    store.remember_callback_entry(
                        &host,
                        entry.includeSubDomains != 0,
                        expire.as_deref().unwrap_or_default(),
                    );
                });
            }
            CURLSTS_DONE => return Ok(()),
            CURLSTS_FAIL => return Err(CURLE_ABORTED_BY_CALLBACK),
            _ => return Err(CURLE_ABORTED_BY_CALLBACK),
        }
    }
}

fn finalize_hsts(
    handle: *mut CURL,
    metadata: &EasyMetadata,
    callbacks: EasyCallbacks,
    enabled: bool,
    result: CURLcode,
) -> CURLcode {
    if !enabled {
        return result;
    }
    let Some(callback) = callbacks.hsts_write_function else {
        return result;
    };

    let Some(entries) = with_hsts_store_mut(handle, metadata, |store| store.entries().to_vec())
    else {
        return result;
    };
    let total = entries.len();
    for (index, stored) in entries.iter().enumerate() {
        let name = std::ffi::CString::new(stored.host.clone())
            .expect("stored HSTS host contains no interior NUL");
        let mut entry = curl_hstsentry {
            name: name.as_ptr().cast_mut(),
            namelen: stored.host.len() + 1,
            includeSubDomains: stored.include_subdomains as u8,
            expire: [0; 18],
        };
        fill_hsts_expire(&mut entry, stored.expire_text.as_deref());
        let mut position = curl_index { index, total };
        let rc = unsafe {
            callback(
                handle,
                &mut entry,
                &mut position,
                callbacks.hsts_write_data as *mut c_void,
            )
        };
        if rc == CURLSTS_FAIL {
            return CURLE_ABORTED_BY_CALLBACK;
        }
    }
    result
}

fn perform_file_transfer(
    handle: *mut CURL,
    metadata: &EasyMetadata,
    callbacks: EasyCallbacks,
) -> CURLcode {
    let Some(url) = metadata.url.as_deref() else {
        perform::set_error_buffer(handle, "No URL set");
        return CURLE_URL_MALFORMAT;
    };
    let path = match crate::protocols::file::decode_url_path(url) {
        Ok(path) => path,
        Err(code) => {
            perform::set_error_buffer(handle, "Malformed file:// URL");
            return code;
        }
    };
    let started = Instant::now();
    let mut info = RecordedTransferInfo::default();

    if metadata.upload {
        let mut file = match File::create(&path) {
            Ok(file) => file,
            Err(_) => {
                perform::set_error_buffer(handle, "Failed to open local file for upload");
                return CURLE_READ_ERROR;
            }
        };
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            let read = match read_request_body_chunk(handle, callbacks, &mut buf) {
                Ok(read) => read,
                Err(code) => return code,
            };
            if read == 0 {
                break;
            }
            if file.write_all(&buf[..read]).is_err() {
                perform::set_error_buffer(handle, "Failed writing local file data");
                return CURLE_WRITE_ERROR;
            }
        }
    } else {
        let mut file = match File::open(&path) {
            Ok(file) => file,
            Err(_) => {
                perform::set_error_buffer(handle, "Failed to open local file");
                return CURLE_FILE_COULDNT_READ_FILE;
            }
        };
        let file_len = match file.metadata() {
            Ok(metadata) => metadata.len() as usize,
            Err(_) => 0,
        };
        if metadata.resume_from > 0
            && file
                .seek(SeekFrom::Start(metadata.resume_from.max(0) as u64))
                .is_err()
        {
            perform::set_error_buffer(handle, "Failed seeking local file");
            return CURLE_RANGE_ERROR;
        }
        if !metadata.nobody {
            let mut low_speed = LowSpeedGuard::new(metadata.low_speed);
            let announced = file_len.saturating_sub(metadata.resume_from.max(0) as usize);
            if let Err(code) = invoke_progress_callback(callbacks, 0, Some(announced)) {
                return code;
            }
            let mut buf = vec![0u8; 16 * 1024];
            loop {
                match file.read(&mut buf) {
                    Ok(0) => break,
                    Ok(read) => {
                        let mut chunk = buf[..read].to_vec();
                        if let Err(code) = deliver_write(handle, callbacks, &mut chunk) {
                            return code;
                        }
                        if let Err(code) = low_speed.observe_progress(read) {
                            return code;
                        }
                    }
                    Err(_) => {
                        perform::set_error_buffer(handle, "Failed reading local file");
                        return CURLE_FILE_COULDNT_READ_FILE;
                    }
                }
            }
        }
    }

    info.total_time_us = elapsed_us(started.elapsed());
    perform::record_transfer_info(handle, info);
    crate::abi::CURLE_OK
}

fn connect_only_transfer(
    handle: *mut CURL,
    request: &RequestContext,
    metadata: &EasyMetadata,
    callbacks: EasyCallbacks,
) -> Result<TransferOutcome, CURLcode> {
    let ConnectedStream {
        mut stream,
        mut info,
    } = connect_stream(request, metadata, &[], callbacks)?;
    let websocket = if request.websocket_style {
        Some(crate::ws::WebSocketSession::handshake(
            &mut stream,
            &request.host_header,
            &request.request_target,
            &request.request_headers,
            crate::ws::raw_mode_enabled(perform::snapshot_metadata(handle).ws_options),
        )?)
    } else {
        None
    };
    stream
        .set_nonblocking(true)
        .map_err(|_| CURLE_COULDNT_CONNECT)?;
    info.pretransfer_time_us = info.connect_time_us;
    info.starttransfer_time_us = info.connect_time_us;
    info.total_time_us = info.connect_time_us;
    connect_only_registry()
        .lock()
        .expect("connect-only registry mutex poisoned")
        .insert(
            handle as usize,
            ConnectOnlySession {
                stream,
                paused: 0,
                websocket,
            },
        );

    Ok(TransferOutcome {
        result: crate::abi::CURLE_OK,
        response_code: 0,
        retry_after: None,
        location: None,
        info,
    })
}

fn execute_http_transfer(
    handle: *mut CURL,
    request: &RequestContext,
    plan: &TransferPlan,
    metadata: &EasyMetadata,
    callbacks: EasyCallbacks,
    request_index: usize,
) -> Result<TransferOutcome, CURLcode> {
    let request_started = Instant::now();
    let ConnectedStream {
        mut stream,
        mut info,
    } = connect_stream(request, metadata, &plan.resolve_overrides, callbacks)?;
    stream
        .set_read_timeout(Some(IO_POLL_INTERVAL))
        .map_err(|_| CURLE_COULDNT_CONNECT)?;
    stream
        .set_write_timeout(Some(CONNECT_TIMEOUT))
        .map_err(|_| CURLE_COULDNT_CONNECT)?;
    let response_prefix = if request.tunnel_proxy {
        establish_proxy_tunnel(&mut stream, handle, callbacks, metadata, request)?
    } else {
        Vec::new()
    };
    let mut stream = if let Some(policy) = plan.tls.as_ref() {
        let tls = crate::tls::connect(
            stream,
            &request.target_host,
            request.target_port,
            metadata,
            policy,
        )?;
        record_certinfo(handle, policy.certinfo, &tls);
        TransportStream::Tls(tls)
    } else {
        TransportStream::Plain(stream)
    };
    write_request(&mut stream, request)?;
    if metadata.upload {
        write_request_body(&mut stream, handle, callbacks, request)?;
    }

    let response = read_response_meta_with_prefix(
        &mut stream,
        handle,
        callbacks,
        metadata,
        request,
        request_index,
        response_prefix,
    )?;
    flush_cookie_jar(handle, metadata);
    info.pretransfer_time_us = info.connect_time_us;
    info.starttransfer_time_us = elapsed_us(request_started.elapsed());
    let mut outcome = TransferOutcome {
        result: crate::abi::CURLE_OK,
        response_code: response.status_code,
        retry_after: response.retry_after,
        location: response.location,
        info,
    };

    let resume_requested = metadata.resume_from > 0;
    let ignore_body = if resume_requested && response.status_code == 416 {
        true
    } else if resume_requested
        && (200..300).contains(&response.status_code)
        && !response.has_content_range
    {
        outcome.result = CURLE_RANGE_ERROR;
        true
    } else if metadata.fail_on_error && response.status_code >= 400 {
        outcome.result = CURLE_HTTP_RETURNED_ERROR;
        true
    } else {
        false
    };

    if metadata.nobody || request.method.eq_ignore_ascii_case("HEAD") || ignore_body {
        if outcome.result == CURLE_RANGE_ERROR {
            perform::set_error_buffer(handle, "HTTP server did not provide requested range");
        } else if outcome.result == CURLE_HTTP_RETURNED_ERROR {
            perform::set_error_buffer(handle, "The requested URL returned error");
        }
        outcome.info.total_time_us = elapsed_us(request_started.elapsed());
        return Ok(outcome);
    }

    let mut low_speed = LowSpeedGuard::new(plan.low_speed);
    invoke_progress_callback(callbacks, 0, response.content_length)?;
    transfer_body(
        &mut stream,
        handle,
        callbacks,
        response.body_prefix,
        response.content_length,
        &mut low_speed,
    )?;
    outcome.info.total_time_us = elapsed_us(request_started.elapsed());
    Ok(outcome)
}

fn flush_cookie_jar(handle: *mut CURL, metadata: &EasyMetadata) {
    let Some(path) = metadata.cookie_jar.as_deref() else {
        return;
    };
    let _ = with_cookie_store_mut(handle, metadata, |store| store.flush_to_path(path));
}

pub(crate) fn transfer_body(
    stream: &mut TransportStream,
    handle: *mut CURL,
    callbacks: EasyCallbacks,
    mut body_prefix: Vec<u8>,
    content_length: Option<usize>,
    low_speed: &mut LowSpeedGuard,
) -> Result<(), CURLcode> {
    let mut delivered = 0usize;

    if !body_prefix.is_empty() {
        deliver_write(handle, callbacks, &mut body_prefix)?;
        delivered = delivered.saturating_add(body_prefix.len());
        low_speed.observe_progress(body_prefix.len())?;
        invoke_progress_callback(callbacks, delivered, content_length)?;
    }

    let mut buf = vec![0u8; 16 * 1024];
    loop {
        if let Some(limit) = content_length {
            if delivered >= limit {
                break;
            }
        }

        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(read) => {
                let chunk = &mut buf[..read];
                deliver_write(handle, callbacks, chunk)?;
                delivered = delivered.saturating_add(read);
                low_speed.observe_progress(read)?;
                invoke_progress_callback(callbacks, delivered, content_length)?;
            }
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                low_speed.observe_idle()?;
            }
            Err(_) => return Err(CURLE_READ_ERROR),
        }
    }

    Ok(())
}

fn connect_stream(
    request: &RequestContext,
    metadata: &EasyMetadata,
    resolve_overrides: &[ResolveOverride],
    callbacks: EasyCallbacks,
) -> Result<ConnectedStream, CURLcode> {
    let (host, port) = request
        .proxy
        .as_ref()
        .map(|(host, port)| (host.as_str(), *port))
        .unwrap_or((&request.target_host, request.target_port));
    let resolve_started = Instant::now();
    let addrs = resolve_addresses(host, port, resolve_overrides)?;
    let namelookup_time_us = elapsed_us(resolve_started.elapsed());

    let mut last_error = None;
    for addr in addrs {
        match connect_addr_stream(callbacks, &addr, metadata.tcp_nodelay) {
            Ok(stream) => {
                let connect_time_us = elapsed_us(resolve_started.elapsed());
                return Ok(ConnectedStream {
                    info: describe_connection(&stream, namelookup_time_us, connect_time_us),
                    stream,
                });
            }
            Err(error) => last_error = Some(error),
        }
    }

    let _ = last_error;
    Err(CURLE_COULDNT_CONNECT)
}

pub(crate) fn connect_protocol_transport(
    handle: *mut CURL,
    host: &str,
    port: u16,
    plan: &TransferPlan,
    metadata: &EasyMetadata,
    callbacks: EasyCallbacks,
) -> Result<ConnectedTransport, CURLcode> {
    let resolve_started = Instant::now();
    let addrs = resolve_addresses(host, port, &plan.resolve_overrides)?;
    let namelookup_time_us = elapsed_us(resolve_started.elapsed());

    let mut last_error = None;
    for addr in addrs {
        match connect_addr_stream(callbacks, &addr, metadata.tcp_nodelay) {
            Ok(stream) => {
                let connect_time_us = elapsed_us(resolve_started.elapsed());
                let info = describe_connection(&stream, namelookup_time_us, connect_time_us);
                let stream = if let Some(policy) = plan.tls.as_ref() {
                    let tls = crate::tls::connect(stream, host, port, metadata, policy)?;
                    record_certinfo(handle, policy.certinfo, &tls);
                    TransportStream::Tls(tls)
                } else {
                    TransportStream::Plain(stream)
                };
                return Ok(ConnectedTransport { stream, info });
            }
            Err(error) => last_error = Some(error),
        }
    }

    let _ = last_error;
    Err(CURLE_COULDNT_CONNECT)
}

pub(crate) struct ConnectedTransport {
    pub stream: TransportStream,
    pub info: RecordedTransferInfo,
}

pub(crate) fn close_transport(stream: TransportStream, callbacks: EasyCallbacks) {
    match stream {
        TransportStream::Plain(stream) => close_plain_stream(stream, callbacks),
        TransportStream::Tls(stream) => close_plain_stream(stream.into_plain_stream(), callbacks),
    }
}

fn record_certinfo(handle: *mut CURL, enabled: bool, connection: &crate::tls::TlsConnection) {
    if !enabled {
        return;
    }
    if let Some(certinfo) = crate::tls::certinfo::capture(connection) {
        crate::tls::certinfo::store(handle, certinfo);
    }
}

enum ConnectTarget {
    Callback {
        addr: crate::abi::sockaddr,
        addrlen: u32,
    },
    V4(sockaddr_in),
    V6(sockaddr_in6),
}

impl ConnectTarget {
    fn as_raw(&self) -> (*const crate::abi::sockaddr, u32) {
        match self {
            Self::Callback { addr, addrlen } => (addr as *const _, *addrlen),
            Self::V4(addr) => (
                addr as *const _ as *const crate::abi::sockaddr,
                core::mem::size_of::<sockaddr_in>() as u32,
            ),
            Self::V6(addr) => (
                addr as *const _ as *const crate::abi::sockaddr,
                core::mem::size_of::<sockaddr_in6>() as u32,
            ),
        }
    }
}

fn connect_target_for_addr(addr: &SocketAddr) -> ConnectTarget {
    match addr {
        SocketAddr::V4(v4) => ConnectTarget::V4(sockaddr_in {
            sin_family: AF_INET as u16,
            sin_port: v4.port().to_be(),
            sin_addr: in_addr {
                s_addr: u32::from_ne_bytes(v4.ip().octets()),
            },
            sin_zero: [0; 8],
        }),
        SocketAddr::V6(v6) => ConnectTarget::V6(sockaddr_in6 {
            sin6_family: AF_INET6 as u16,
            sin6_port: v6.port().to_be(),
            sin6_flowinfo: v6.flowinfo(),
            sin6_addr: in6_addr {
                s6_addr: v6.ip().octets(),
            },
            sin6_scope_id: v6.scope_id(),
        }),
    }
}

fn callback_sockaddr_for_addr(addr: &SocketAddr) -> crate::abi::curl_sockaddr {
    let family = match addr {
        SocketAddr::V4(_) => AF_INET,
        SocketAddr::V6(_) => AF_INET6,
    };
    let mut curl_addr = crate::abi::curl_sockaddr {
        family,
        socktype: SOCK_STREAM,
        protocol: IPPROTO_TCP,
        addrlen: core::mem::size_of::<crate::abi::sockaddr>() as u32,
        addr: crate::abi::sockaddr {
            sa_family: family as u16,
            sa_data: [0; 14],
        },
    };
    if let SocketAddr::V4(v4) = addr {
        let raw = sockaddr_in {
            sin_family: AF_INET as u16,
            sin_port: v4.port().to_be(),
            sin_addr: in_addr {
                s_addr: u32::from_ne_bytes(v4.ip().octets()),
            },
            sin_zero: [0; 8],
        };
        curl_addr.addr = unsafe { core::mem::transmute(raw) };
        curl_addr.addrlen = core::mem::size_of::<sockaddr_in>() as u32;
    }
    curl_addr
}

fn callback_connect_target(
    original: &SocketAddr,
    callback_addr: &crate::abi::curl_sockaddr,
) -> ConnectTarget {
    if callback_addr.family == AF_INET
        && callback_addr.addrlen as usize <= core::mem::size_of::<crate::abi::sockaddr>()
    {
        ConnectTarget::Callback {
            addr: crate::abi::sockaddr {
                sa_family: callback_addr.addr.sa_family,
                sa_data: callback_addr.addr.sa_data,
            },
            addrlen: callback_addr.addrlen,
        }
    } else {
        connect_target_for_addr(original)
    }
}

fn open_socket_stream(
    callbacks: EasyCallbacks,
    addr: &SocketAddr,
    tcp_nodelay: bool,
) -> Result<Option<TcpStream>, CURLcode> {
    let Some(callback) = callbacks.open_socket_function else {
        return Ok(None);
    };

    let mut sockaddr = callback_sockaddr_for_addr(addr);
    let fd = unsafe {
        callback(
            callbacks.open_socket_data as *mut c_void,
            CURLSOCKTYPE_IPCXN,
            &mut sockaddr,
        )
    };
    if fd == CURL_SOCKET_BAD {
        return Err(CURLE_COULDNT_CONNECT);
    }

    let stream = unsafe { TcpStream::from_raw_fd(fd as c_int) };
    if stream.peer_addr().is_ok() {
        return configure_tcp_stream(stream, tcp_nodelay).map(Some);
    }

    let target = callback_connect_target(addr, &sockaddr);
    let (raw_addr, raw_len) = target.as_raw();
    if unsafe { connect(stream.as_raw_fd() as c_int, raw_addr, raw_len) } == 0 {
        return configure_tcp_stream(stream, tcp_nodelay).map(Some);
    }

    close_plain_stream(stream, callbacks);
    Err(CURLE_COULDNT_CONNECT)
}

fn connect_addr_stream(
    callbacks: EasyCallbacks,
    addr: &SocketAddr,
    tcp_nodelay: bool,
) -> Result<TcpStream, CURLcode> {
    if let Some(stream) = open_socket_stream(callbacks, addr, tcp_nodelay)? {
        return Ok(stream);
    }
    let stream =
        TcpStream::connect_timeout(addr, CONNECT_TIMEOUT).map_err(|_| CURLE_COULDNT_CONNECT)?;
    configure_tcp_stream(stream, tcp_nodelay)
}

fn configure_tcp_stream(stream: TcpStream, tcp_nodelay: bool) -> Result<TcpStream, CURLcode> {
    if tcp_nodelay && stream.set_nodelay(true).is_err() {
        return Err(CURLE_COULDNT_CONNECT);
    }
    Ok(stream)
}

fn close_plain_stream(stream: TcpStream, callbacks: EasyCallbacks) {
    let fd = stream.into_raw_fd() as curl_socket_t;
    if let Some(callback) = callbacks.close_socket_function {
        let _ = unsafe { callback(callbacks.close_socket_data as *mut c_void, fd) };
    } else {
        unsafe {
            close(fd as c_int);
        }
    }
}

fn establish_proxy_tunnel(
    stream: &mut TcpStream,
    handle: *mut CURL,
    callbacks: EasyCallbacks,
    metadata: &EasyMetadata,
    request: &RequestContext,
) -> Result<Vec<u8>, CURLcode> {
    write_proxy_connect_request(stream, request)?;
    let (status_code, body_prefix) =
        read_proxy_connect_response(stream, handle, callbacks, metadata)?;
    if (200..300).contains(&status_code) {
        Ok(body_prefix)
    } else {
        perform::set_error_buffer(handle, "HTTP proxy CONNECT failed");
        Err(CURLE_COULDNT_CONNECT)
    }
}

fn write_proxy_connect_request(
    stream: &mut TcpStream,
    request: &RequestContext,
) -> Result<(), CURLcode> {
    let mut encoded = String::new();
    encoded.push_str("CONNECT ");
    encoded.push_str(&request.host_header);
    encoded.push_str(" HTTP/1.1\r\n");
    encoded.push_str("Host: ");
    encoded.push_str(&request.host_header);
    encoded.push_str("\r\n");
    if !has_header(&request.proxy_headers, "Proxy-Connection") {
        encoded.push_str("Proxy-Connection: Keep-Alive\r\n");
    }
    append_headers(&mut encoded, &request.proxy_headers);
    encoded.push_str("\r\n");

    stream
        .write_all(encoded.as_bytes())
        .map_err(|_| CURLE_SEND_ERROR)
}

fn write_request(stream: &mut TransportStream, request: &RequestContext) -> Result<(), CURLcode> {
    let mut encoded = String::new();
    encoded.push_str(&request.method);
    encoded.push(' ');
    encoded.push_str(&request.request_target);
    encoded.push_str(" HTTP/1.1\r\n");
    encoded.push_str("Host: ");
    encoded.push_str(&request.host_header);
    encoded.push_str("\r\n");
    encoded.push_str("Accept: */*\r\n");
    if request.proxy.is_some()
        && !request.tunnel_proxy
        && !has_header(&request.request_headers, "Proxy-Connection")
    {
        encoded.push_str("Proxy-Connection: Keep-Alive\r\n");
    }
    append_headers(&mut encoded, &request.request_headers);
    if request.use_chunked_upload {
        encoded.push_str("Transfer-Encoding: chunked\r\n");
    } else if let Some(body_length) = request.body_length {
        encoded.push_str("Content-Length: ");
        encoded.push_str(&body_length.to_string());
        encoded.push_str("\r\n");
    }
    if request.use_chunked_upload && !has_header(&request.request_headers, "Expect") {
        encoded.push_str("Expect: 100-continue\r\n");
    }
    if let Some(range) = request.range_header.as_ref() {
        encoded.push_str("Range: ");
        encoded.push_str(range);
        encoded.push_str("\r\n");
    }
    encoded.push_str("\r\n");

    stream
        .write_all(encoded.as_bytes())
        .map_err(|_| CURLE_SEND_ERROR)
}

fn append_headers(encoded: &mut String, headers: &[String]) {
    for header in headers {
        if let Some((_, value)) = header.split_once(':') {
            if value.trim().is_empty() {
                continue;
            }
        }
        encoded.push_str(header);
        encoded.push_str("\r\n");
    }
}

fn write_request_body(
    stream: &mut TransportStream,
    handle: *mut CURL,
    callbacks: EasyCallbacks,
    request: &RequestContext,
) -> Result<(), CURLcode> {
    if request.use_chunked_upload {
        return write_chunked_request_body(stream, handle, callbacks);
    }

    let mut remaining = request.body_length.unwrap_or(0);
    let mut buf = vec![0u8; 16 * 1024];
    while remaining > 0 {
        let chunk_len = remaining.min(buf.len());
        let read = read_request_body_chunk(handle, callbacks, &mut buf[..chunk_len])?;
        if read == 0 {
            perform::set_error_buffer(handle, "Failed reading upload data");
            return Err(CURLE_READ_ERROR);
        }
        stream
            .write_all(&buf[..read])
            .map_err(|_| CURLE_SEND_ERROR)?;
        remaining -= read;
    }
    stream.flush().map_err(|_| CURLE_SEND_ERROR)
}

fn write_chunked_request_body(
    stream: &mut TransportStream,
    handle: *mut CURL,
    callbacks: EasyCallbacks,
) -> Result<(), CURLcode> {
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        let read = read_request_body_chunk(handle, callbacks, &mut buf)?;
        if read == 0 {
            break;
        }
        let chunk_header = format!("{read:x}\r\n");
        stream
            .write_all(chunk_header.as_bytes())
            .map_err(|_| CURLE_SEND_ERROR)?;
        stream
            .write_all(&buf[..read])
            .map_err(|_| CURLE_SEND_ERROR)?;
        stream.write_all(b"\r\n").map_err(|_| CURLE_SEND_ERROR)?;
    }
    stream.write_all(b"0\r\n").map_err(|_| CURLE_SEND_ERROR)?;
    write_request_trailers(stream, callbacks)?;
    stream.write_all(b"\r\n").map_err(|_| CURLE_SEND_ERROR)?;
    stream.flush().map_err(|_| CURLE_SEND_ERROR)
}

fn write_request_trailers(
    stream: &mut TransportStream,
    callbacks: EasyCallbacks,
) -> Result<(), CURLcode> {
    let Some(callback) = callbacks.trailer_function else {
        return Ok(());
    };

    let mut list: *mut curl_slist = core::ptr::null_mut();
    let rc = unsafe { callback(&mut list, callbacks.trailer_data as *mut c_void) };
    let trailers = collect_slist_strings(list);
    unsafe { crate::slist::curl_slist_free_all(list) };
    if rc != 0 {
        return Err(CURLE_ABORTED_BY_CALLBACK);
    }

    for trailer in trailers {
        stream
            .write_all(trailer.as_bytes())
            .map_err(|_| CURLE_SEND_ERROR)?;
        stream.write_all(b"\r\n").map_err(|_| CURLE_SEND_ERROR)?;
    }
    Ok(())
}

fn read_proxy_connect_response(
    stream: &mut TcpStream,
    handle: *mut CURL,
    callbacks: EasyCallbacks,
    metadata: &EasyMetadata,
) -> Result<(u16, Vec<u8>), CURLcode> {
    let mut bytes = Vec::new();
    let started = Instant::now();
    let header_end = loop {
        if let Some(header_end) = find_header_end(&bytes) {
            break header_end;
        }
        if bytes.len() > response::MAX_RESPONSE_HEADERS_BYTES {
            perform::set_error_buffer(handle, "Too large response headers");
            return Err(CURLE_RECV_ERROR);
        }
        if started.elapsed() >= HEADER_WAIT_TIMEOUT {
            return Err(CURLE_OPERATION_TIMEDOUT);
        }

        let mut buf = [0u8; 1024];
        match stream.read(&mut buf) {
            Ok(0) if bytes.is_empty() => return Err(CURLE_READ_ERROR),
            Ok(0) => break bytes.len(),
            Ok(read) => bytes.extend_from_slice(&buf[..read]),
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => return Err(CURLE_READ_ERROR),
        }
    };

    let header_block = &bytes[..header_end];
    let body_prefix = bytes[header_end..].to_vec();
    let mut status_code = 0u16;
    for (index, raw_line) in split_header_lines(header_block).into_iter().enumerate() {
        deliver_header(handle, callbacks, metadata, raw_line)?;
        let text = String::from_utf8_lossy(raw_line);
        let trimmed = text.trim_end_matches(['\r', '\n']);
        if index == 0 {
            status_code = parse_status_code(trimmed).ok_or(CURLE_READ_ERROR)?;
        }
    }

    Ok((status_code, body_prefix))
}

fn read_response_meta_with_prefix(
    stream: &mut TransportStream,
    handle: *mut CURL,
    callbacks: EasyCallbacks,
    metadata: &EasyMetadata,
    request: &RequestContext,
    request_index: usize,
    mut bytes: Vec<u8>,
) -> Result<ResponseMeta, CURLcode> {
    let started = Instant::now();
    let header_end = loop {
        if let Some(header_end) = find_header_end(&bytes) {
            break header_end;
        }
        if bytes.len() > response::MAX_RESPONSE_HEADERS_BYTES {
            perform::set_error_buffer(handle, "Too large response headers");
            return Err(CURLE_RECV_ERROR);
        }
        if started.elapsed() >= HEADER_WAIT_TIMEOUT {
            return Err(CURLE_OPERATION_TIMEDOUT);
        }

        let mut buf = [0u8; 1024];
        match stream.read(&mut buf) {
            Ok(0) if bytes.is_empty() => return Err(CURLE_READ_ERROR),
            Ok(0) => break bytes.len(),
            Ok(read) => bytes.extend_from_slice(&buf[..read]),
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => return Err(CURLE_READ_ERROR),
        }
    };

    let body_prefix = bytes[header_end..].to_vec();
    let header_block = &bytes[..header_end];
    let mut status_code = 0u16;
    let mut content_length = None;
    let mut has_content_range = false;
    let mut retry_after = None;
    let mut location = None;
    let mut origin_flag = HEADER_ORIGIN_HEADER;
    let _ = perform::with_http_state_mut(handle, |state| {
        state.headers.set_latest_request(request_index)
    });

    for (index, raw_line) in split_header_lines(header_block).into_iter().enumerate() {
        deliver_header(handle, callbacks, metadata, raw_line)?;
        let text = String::from_utf8_lossy(raw_line);
        let trimmed = text.trim_end_matches(['\r', '\n']);
        if index == 0 {
            status_code = parse_status_code(trimmed).ok_or(CURLE_READ_ERROR)?;
            if status_code < 200 {
                origin_flag = HEADER_ORIGIN_1XX;
            }
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        let Some((name, value)) = trimmed.split_once(':') else {
            continue;
        };
        let name = name.trim();
        let value = value.trim();
        let _ = perform::with_http_state_mut(handle, |state| {
            state
                .headers
                .record(request_index, origin_flag, name, value);
            cookies::record_from_header(&mut state.cookies, &request.url, trimmed);
        });
        let _ = crate::share::with_shared_cookies_mut(metadata.share_handle, |store| {
            cookies::record_from_header(store, &request.url, trimmed);
        });
        let _ = with_hsts_store_mut(handle, metadata, |store| {
            hsts::record_from_header(store, &request.target_host, trimmed);
        });
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value.parse::<usize>().ok();
        } else if name.eq_ignore_ascii_case("content-range") {
            has_content_range = true;
        } else if name.eq_ignore_ascii_case("retry-after") {
            retry_after = parse_retry_after(value);
        } else if name.eq_ignore_ascii_case("location") {
            location = Some(value.to_string());
        }
    }
    Ok(ResponseMeta {
        status_code,
        content_length,
        has_content_range,
        retry_after,
        location,
        body_prefix,
    })
}

fn should_use_reference_http2_transport(
    handle: *mut CURL,
    metadata: &EasyMetadata,
    request: &RequestContext,
) -> bool {
    if request.websocket_style || metadata.connect_only {
        return false;
    }

    match metadata.http_version {
        CURL_HTTP_VERSION_1_0 | CURL_HTTP_VERSION_1_1 => false,
        CURL_HTTP_VERSION_2_0 | CURL_HTTP_VERSION_2_PRIOR_KNOWLEDGE => true,
        CURL_HTTP_VERSION_2TLS => request.scheme.eq_ignore_ascii_case("https"),
        0 => {
            request.scheme.eq_ignore_ascii_case("https")
                && metadata.ssl_enable_alpn
                && crate::easy::perform::attached_multi_for(handle).is_some()
        }
        _ => request.scheme.eq_ignore_ascii_case("https") && metadata.ssl_enable_alpn,
    }
}

fn resolve_addresses(
    host: &str,
    port: u16,
    resolve_overrides: &[ResolveOverride],
) -> Result<Vec<SocketAddr>, CURLcode> {
    if let Some(entry) = resolve_overrides
        .iter()
        .find(|entry| entry.host.eq_ignore_ascii_case(host) && entry.port == port)
    {
        if entry.remove || entry.addresses.is_empty() {
            return Err(CURLE_COULDNT_RESOLVE_HOST);
        }

        let mut addrs = Vec::new();
        for address in &entry.addresses {
            let resolved = (address.as_str(), port)
                .to_socket_addrs()
                .map_err(|_| CURLE_COULDNT_RESOLVE_HOST)?;
            addrs.extend(resolved);
        }
        if addrs.is_empty() {
            return Err(CURLE_COULDNT_RESOLVE_HOST);
        }
        return Ok(addrs);
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(ip, port)]);
    }

    let resolved = (host, port)
        .to_socket_addrs()
        .map_err(|_| CURLE_COULDNT_RESOLVE_HOST)?;
    let addrs: Vec<_> = resolved.collect();
    if addrs.is_empty() {
        Err(CURLE_COULDNT_RESOLVE_HOST)
    } else {
        Ok(addrs)
    }
}

pub(crate) fn deliver_write(
    handle: *mut CURL,
    callbacks: EasyCallbacks,
    buffer: &mut [u8],
) -> Result<(), CURLcode> {
    loop {
        wait_for_pause_clear(handle, CURLPAUSE_RECV);
        let wrote = if let Some(callback) = callbacks.write_function {
            let write_data = if callbacks.write_data == 0 {
                unsafe { stdout }
            } else {
                callbacks.write_data as *mut c_void
            };
            unsafe { callback(buffer.as_mut_ptr().cast(), 1, buffer.len(), write_data) }
        } else {
            let stream = if callbacks.write_data == 0 {
                unsafe { stdout }
            } else {
                callbacks.write_data as *mut c_void
            };
            unsafe { fwrite(buffer.as_ptr().cast(), 1, buffer.len(), stream) }
        };
        if wrote == buffer.len() {
            return Ok(());
        }
        if wrote == CURL_WRITEFUNC_PAUSE {
            add_pause_mask(handle, CURLPAUSE_RECV);
            continue;
        }
        perform::set_error_buffer(handle, "Failed writing received data");
        return Err(CURLE_WRITE_ERROR);
    }
}

fn describe_connection(
    stream: &TcpStream,
    namelookup_time_us: curl_off_t,
    connect_time_us: curl_off_t,
) -> RecordedTransferInfo {
    let peer_addr = stream.peer_addr().ok();
    let local_addr = stream.local_addr().ok();
    RecordedTransferInfo {
        primary_ip: peer_addr.as_ref().map(|addr| addr.ip().to_string()),
        primary_port: peer_addr.as_ref().map(|addr| addr.port()),
        local_ip: local_addr.as_ref().map(|addr| addr.ip().to_string()),
        local_port: local_addr.as_ref().map(|addr| addr.port()),
        namelookup_time_us,
        connect_time_us,
        ..RecordedTransferInfo::default()
    }
}

pub(crate) fn elapsed_us(duration: Duration) -> curl_off_t {
    duration.as_micros().min(curl_off_t::MAX as u128) as curl_off_t
}

fn requires_reference_backend(
    _metadata: &EasyMetadata,
    _route: crate::protocols::TransferRoute,
) -> bool {
    // Only the explicit HTTP/2 transport compatibility path may execute through the
    // reference backend. Public feature/state selection stays on the Rust-owned path.
    false
}

pub(crate) fn read_request_body_chunk(
    handle: *mut CURL,
    callbacks: EasyCallbacks,
    buffer: &mut [u8],
) -> Result<usize, CURLcode> {
    loop {
        wait_for_pause_clear(handle, CURLPAUSE_SEND);
        let read = if let Some(callback) = callbacks.read_function {
            unsafe {
                callback(
                    buffer.as_mut_ptr().cast(),
                    1,
                    buffer.len(),
                    callbacks.read_data as *mut c_void,
                )
            }
        } else {
            let stream = if callbacks.read_data == 0 {
                unsafe { stdin }
            } else {
                callbacks.read_data as *mut c_void
            };
            unsafe { fread(buffer.as_mut_ptr().cast(), 1, buffer.len(), stream) }
        };

        if read == CURL_READFUNC_PAUSE {
            add_pause_mask(handle, CURLPAUSE_SEND);
            continue;
        }
        if read <= buffer.len() {
            return Ok(read);
        }
        perform::set_error_buffer(handle, "Failed reading upload data");
        return Err(CURLE_READ_ERROR);
    }
}

pub(crate) fn deliver_header(
    handle: *mut CURL,
    callbacks: EasyCallbacks,
    metadata: &EasyMetadata,
    raw_line: &[u8],
) -> Result<(), CURLcode> {
    if let Some(callback) = callbacks.header_function {
        let mut line = raw_line.to_vec();
        let header_data = if callbacks.header_data == 0 {
            unsafe { stdout }
        } else {
            callbacks.header_data as *mut c_void
        };
        let wrote = unsafe { callback(line.as_mut_ptr().cast(), 1, line.len(), header_data) };
        if wrote == line.len() {
            return Ok(());
        }
        perform::set_error_buffer(handle, "Failed writing received header");
        return Err(CURLE_WRITE_ERROR);
    }

    if callbacks.header_data != 0 {
        let wrote = unsafe {
            fwrite(
                raw_line.as_ptr().cast(),
                1,
                raw_line.len(),
                callbacks.header_data as *mut c_void,
            )
        };
        if wrote == raw_line.len() {
            return Ok(());
        }
        perform::set_error_buffer(handle, "Failed writing received header");
        return Err(CURLE_WRITE_ERROR);
    }

    if metadata.header {
        let mut line = raw_line.to_vec();
        deliver_write(handle, callbacks, &mut line)?;
    }
    Ok(())
}

pub(crate) fn invoke_progress_callback(
    callbacks: EasyCallbacks,
    downloaded: usize,
    total: Option<usize>,
) -> Result<(), CURLcode> {
    if callbacks.no_progress {
        return Ok(());
    }
    let Some(callback) = callbacks.xferinfo_function else {
        return Ok(());
    };

    let rc = unsafe {
        callback(
            callbacks.xferinfo_data as *mut c_void,
            total.unwrap_or(0) as curl_off_t,
            downloaded as curl_off_t,
            0,
            0,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(CURLE_ABORTED_BY_CALLBACK)
    }
}

fn with_cookie_store_mut<R>(
    handle: *mut CURL,
    metadata: &EasyMetadata,
    f: impl FnOnce(&mut cookies::CookieStore) -> R,
) -> Option<R> {
    let mut f = Some(f);
    if let Some(result) = crate::share::with_shared_cookies_mut(metadata.share_handle, |store| {
        (f.take().expect("cookie closure already consumed"))(store)
    }) {
        Some(result)
    } else {
        perform::with_http_state_mut(handle, |state| {
            (f.take().expect("cookie closure already consumed"))(&mut state.cookies)
        })
    }
}

fn with_hsts_store_mut<R>(
    handle: *mut CURL,
    metadata: &EasyMetadata,
    f: impl FnOnce(&mut hsts::HstsStore) -> R,
) -> Option<R> {
    let mut f = Some(f);
    if let Some(result) = crate::share::with_shared_hsts_mut(metadata.share_handle, |store| {
        (f.take().expect("HSTS closure already consumed"))(store)
    }) {
        Some(result)
    } else {
        perform::with_http_state_mut(handle, |state| {
            (f.take().expect("HSTS closure already consumed"))(&mut state.hsts)
        })
    }
}

fn prepare_request_headers(
    handle: *mut CURL,
    metadata: &EasyMetadata,
    current_url: &str,
    initial_origin: Option<&Origin>,
    allow_cross_origin_auth: bool,
    referer_value: Option<&str>,
    request: &mut RequestContext,
) {
    if let Some(user_agent) = metadata.user_agent.as_deref() {
        if !has_header(&request.request_headers, "User-Agent") {
            request
                .request_headers
                .push(format!("User-Agent: {user_agent}"));
        }
    }

    let needs_auth_headers = metadata.xoauth2_bearer.is_some()
        || metadata.userpwd.is_some()
        || metadata.username.is_some()
        || metadata.password.is_some()
        || metadata.proxy_userpwd.is_some()
        || metadata.proxy_username.is_some()
        || metadata.proxy_password.is_some()
        || metadata.netrc_mode != 0
        || referer_value.is_some();
    if needs_auth_headers {
        let auth_headers = auth::request_auth_headers(
            metadata,
            current_url,
            initial_origin,
            allow_cross_origin_auth,
            referer_value,
        );
        if let Some(header) = auth_headers.authorization {
            if !has_header(&request.request_headers, "Authorization") {
                request.request_headers.push(header);
            }
        }
        if let Some(header) = auth_headers.proxy_authorization {
            let target_headers = if request.tunnel_proxy {
                &mut request.proxy_headers
            } else {
                &mut request.request_headers
            };
            if !has_header(target_headers, "Proxy-Authorization") {
                target_headers.push(header);
            }
        }
        if let Some(header) = auth_headers.referer {
            if !has_header(&request.request_headers, "Referer") {
                request.request_headers.push(header);
            }
        }
    }

    let uses_cookies = metadata.cookie.is_some()
        || metadata.cookie_file.is_some()
        || metadata.cookie_jar.is_some()
        || !metadata.cookie_list.is_empty()
        || metadata.share_handle.is_some();
    if uses_cookies {
        for item in metadata
            .cookie_list
            .iter()
            .filter(|item| item.as_str() != "SESS")
        {
            if let Some(value) = item.strip_prefix("Set-Cookie:") {
                let value = value.trim();
                let _ = perform::with_http_state_mut(handle, |state| {
                    state.cookies.store_set_cookie(current_url, value);
                });
                let _ = crate::share::with_shared_cookies_mut(metadata.share_handle, |store| {
                    store.store_set_cookie(current_url, value);
                });
            }
        }
        let cookie_header = crate::share::with_shared_cookies_mut(metadata.share_handle, |store| {
            store.apply_request(current_url, metadata.cookie.as_deref())
        })
        .flatten()
        .or_else(|| {
            perform::with_http_state_mut(handle, |state| {
                state
                    .cookies
                    .apply_request(current_url, metadata.cookie.as_deref())
            })
            .flatten()
        });
        if let Some(cookies) = cookie_header {
            if !has_header(&request.request_headers, "Cookie") {
                request.request_headers.push(format!("Cookie: {cookies}"));
            }
        }
    }
}

impl RequestContext {
    fn new(url: &str, metadata: &EasyMetadata) -> Result<Self, CURLcode> {
        let parsed = ParsedUrl::parse(url).ok_or(CURLE_URL_MALFORMAT)?;
        let websocket_style = crate::ws::websocket_mode_enabled(metadata.connect_mode)
            && matches!(parsed.scheme.as_str(), "ws" | "wss");
        let shared_http = matches!(parsed.scheme.as_str(), "http" | "https");
        if !shared_http && !websocket_style {
            return Err(CURLE_UNSUPPORTED_PROTOCOL);
        }
        if websocket_style && metadata.proxy.is_some() {
            return Err(CURLE_UNSUPPORTED_PROTOCOL);
        }

        let connect_override = metadata
            .connect_overrides
            .iter()
            .find(|candidate| candidate.matches(&parsed.host, parsed.port))
            .cloned();
        let target_host = connect_override
            .as_ref()
            .and_then(|candidate| candidate.target_host.clone())
            .unwrap_or_else(|| parsed.host.clone());
        let target_port = connect_override
            .as_ref()
            .and_then(|candidate| candidate.target_port)
            .unwrap_or(parsed.port);
        let proxy = metadata
            .proxy
            .as_deref()
            .and_then(|proxy| parse_proxy_authority(proxy, &parsed.scheme));
        let has_proxy = proxy.is_some();
        let separate_proxy_headers = (metadata.headeropt & CURLHEADER_SEPARATE) != 0;
        let scheme = parsed.scheme.clone();
        let host_header = parsed.host_header.clone();
        let path_and_query = parsed.path_and_query.clone();

        Ok(Self {
            url: url.to_string(),
            scheme,
            host_header: host_header.clone(),
            target_host,
            target_port,
            proxy,
            request_target: metadata.request_target.clone().unwrap_or_else(|| {
                if has_proxy && !metadata.tunnel_proxy {
                    format!("{}://{}{}", parsed.scheme, host_header, path_and_query)
                } else {
                    parsed.path_and_query
                }
            }),
            method: effective_method(metadata, websocket_style),
            request_headers: metadata.http_headers.clone(),
            proxy_headers: if has_proxy {
                if separate_proxy_headers {
                    metadata.proxy_headers.clone()
                } else {
                    let mut headers = metadata.http_headers.clone();
                    headers.extend(metadata.proxy_headers.iter().cloned());
                    headers
                }
            } else {
                Vec::new()
            },
            tunnel_proxy: metadata.tunnel_proxy,
            websocket_style,
            use_chunked_upload: metadata.upload && metadata.upload_size.is_none(),
            range_header: effective_range_header(metadata),
            body_length: if websocket_style || !metadata.upload {
                None
            } else {
                metadata.upload_size.map(|size| size.max(0) as usize)
            },
        })
    }
}

impl ParsedUrl {
    fn parse(url: &str) -> Option<Self> {
        let authority = parse_url_authority(url)?;
        let (scheme, rest) = url.split_once("://")?;
        let trimmed = rest.split('#').next().unwrap_or(rest);
        let path_start = trimmed.find(['/', '?']).unwrap_or(trimmed.len());
        let scheme_lower = scheme.to_ascii_lowercase();
        let host_header = format_host_header(&authority.host, authority.port, &scheme_lower);
        let suffix = &trimmed[path_start..];
        let path_and_query = if suffix.is_empty() {
            "/".to_string()
        } else if suffix.starts_with('/') {
            suffix.to_string()
        } else {
            format!("/{suffix}")
        };

        Some(Self {
            scheme: scheme_lower,
            host: authority.host,
            port: authority.port,
            host_header,
            path_and_query,
        })
    }
}

fn format_host_header(host: &str, port: u16, scheme: &str) -> String {
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    let default_port = match scheme {
        "http" | "ws" => 80,
        "https" | "wss" => 443,
        _ => 0,
    };
    if port == 0 || port == default_port {
        host
    } else {
        format!("{host}:{port}")
    }
}

fn hsts_expire_from_abi(entry: &curl_hstsentry) -> Option<String> {
    let end = entry
        .expire
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(entry.expire.len());
    let bytes = entry.expire[..end]
        .iter()
        .map(|byte| *byte as u8)
        .collect::<Vec<_>>();
    let text = String::from_utf8_lossy(&bytes).trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn fill_hsts_expire(entry: &mut curl_hstsentry, value: Option<&str>) {
    let text = value.unwrap_or("unlimited");
    for slot in &mut entry.expire {
        *slot = 0;
    }
    for (index, byte) in text
        .as_bytes()
        .iter()
        .copied()
        .take(entry.expire.len() - 1)
        .enumerate()
    {
        entry.expire[index] = byte as i8;
    }
}

fn effective_method(metadata: &EasyMetadata, websocket_style: bool) -> String {
    if websocket_style {
        return "GET".to_string();
    }
    if let Some(custom) = metadata.custom_request.as_ref() {
        return custom.clone();
    }
    if metadata.nobody {
        return "HEAD".to_string();
    }
    if metadata.upload {
        return "PUT".to_string();
    }
    "GET".to_string()
}

fn effective_range_header(metadata: &EasyMetadata) -> Option<String> {
    if metadata.resume_from > 0 {
        return Some(format!("bytes={}-", metadata.resume_from));
    }

    metadata.range.as_ref().map(|range| {
        if range.contains('=') {
            range.clone()
        } else {
            format!("bytes={range}")
        }
    })
}

fn redirected_url(
    current_url: &str,
    follow_location: bool,
    redirect_count: usize,
    status_code: u16,
    location: Option<&str>,
) -> Option<String> {
    if !follow_location || redirect_count >= REDIRECT_LIMIT {
        return None;
    }
    if !matches!(status_code, 301 | 302 | 303 | 307 | 308) {
        return None;
    }
    let location = location?;
    if location.contains("://") {
        return Some(location.to_string());
    }

    let parsed = ParsedUrl::parse(current_url)?;
    if location.starts_with('/') {
        return Some(format!(
            "{}://{}{}",
            parsed.scheme, parsed.host_header, location
        ));
    }

    let base = current_url
        .rsplit_once('/')
        .map(|(base, _)| base)
        .unwrap_or(current_url);
    Some(format!("{base}/{location}"))
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
        .or_else(|| {
            bytes
                .windows(2)
                .position(|window| window == b"\n\n")
                .map(|index| index + 2)
        })
}

fn split_header_lines(bytes: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut start = 0usize;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            lines.push(&bytes[start..=index]);
            start = index + 1;
        }
    }
    if start < bytes.len() {
        lines.push(&bytes[start..]);
    }
    lines
}

fn parse_status_code(line: &str) -> Option<u16> {
    let mut fields = line.split_whitespace();
    let _http_version = fields.next()?;
    fields.next()?.parse().ok()
}

fn parse_retry_after(value: &str) -> Option<curl_off_t> {
    if let Ok(seconds) = value.parse::<curl_off_t>() {
        return Some(seconds);
    }

    let timestamp = parse_http_date(value)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    Some((timestamp - now) as curl_off_t)
}

fn has_header(headers: &[String], name: &str) -> bool {
    headers.iter().any(|header| {
        header
            .split_once(':')
            .is_some_and(|(candidate, _)| candidate.trim().eq_ignore_ascii_case(name))
    })
}

fn collect_slist_strings(mut list: *mut curl_slist) -> Vec<String> {
    let mut values = Vec::new();
    while !list.is_null() {
        let data = unsafe { (*list).data };
        if !data.is_null() {
            values.push(
                unsafe { std::ffi::CStr::from_ptr(data) }
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        list = unsafe { (*list).next };
    }
    values
}

fn parse_http_date(value: &str) -> Option<i64> {
    let mut parts = value.split_whitespace();
    let weekday = parts.next()?;
    if !weekday.ends_with(',') {
        return None;
    }
    let day = parts.next()?.parse::<u32>().ok()?;
    let month = parse_month(parts.next()?)?;
    let year = parts.next()?.parse::<i32>().ok()?;
    let (hour, minute, second) = parse_hms(parts.next()?)?;
    if parts.next()? != "GMT" {
        return None;
    }
    Some(unix_timestamp_utc(year, month, day, hour, minute, second))
}

fn parse_month(value: &str) -> Option<u32> {
    match value {
        "Jan" => Some(1),
        "Feb" => Some(2),
        "Mar" => Some(3),
        "Apr" => Some(4),
        "May" => Some(5),
        "Jun" => Some(6),
        "Jul" => Some(7),
        "Aug" => Some(8),
        "Sep" => Some(9),
        "Oct" => Some(10),
        "Nov" => Some(11),
        "Dec" => Some(12),
        _ => None,
    }
}

fn parse_hms(value: &str) -> Option<(u32, u32, u32)> {
    let mut parts = value.split(':');
    let hour = parts.next()?.parse().ok()?;
    let minute = parts.next()?.parse().ok()?;
    let second = parts.next()?.parse().ok()?;
    Some((hour, minute, second))
}

fn unix_timestamp_utc(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> i64 {
    let days = days_from_civil(year, month, day);
    days * 86_400 + hour as i64 * 3_600 + minute as i64 * 60 + second as i64
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let adjusted_year = year - i32::from(month <= 2);
    let era = if adjusted_year >= 0 {
        adjusted_year / 400
    } else {
        (adjusted_year - 399) / 400
    };
    let year_of_era = adjusted_year - era * 400;
    let month_prime = month as i32 + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day as i32 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    (era * 146_097 + day_of_era - 719_468) as i64
}
