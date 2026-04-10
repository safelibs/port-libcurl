use crate::abi::{
    curl_off_t, curl_slist, curl_socket_t, CURLMcode, CURLcode, CURL, CURLE_BAD_FUNCTION_ARGUMENT,
    CURLINFO,
};
use crate::conn::cache::{parse_proxy_authority, parse_url_authority, ConnectionCacheKey};
use crate::conn::filter::{ConnectionFilterChain, ConnectionFilterStep};
use crate::dns::{ConnectOverride, ResolveOverride, ResolverLease, ResolverOwner};
use crate::easy::perform::{self, EasyCallbacks, EasyMetadata, RecordedTransferInfo};
use core::ffi::{c_int, c_long, c_void};
use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
use std::os::fd::AsRawFd;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(crate) const EASY_PERFORM_WAIT_TIMEOUT_MS: c_int = 1000;
pub(crate) const CURLINFO_ACTIVESOCKET: CURLINFO = 0x500000 + 44;

const CURLM_OUT_OF_MEMORY: CURLMcode = 3;

const CURLE_UNSUPPORTED_PROTOCOL: CURLcode = 1;
const CURLE_URL_MALFORMAT: CURLcode = 3;
const CURLE_COULDNT_RESOLVE_HOST: CURLcode = 6;
const CURLE_COULDNT_CONNECT: CURLcode = 7;
const CURLE_HTTP_RETURNED_ERROR: CURLcode = 22;
const CURLE_WRITE_ERROR: CURLcode = 23;
const CURLE_READ_ERROR: CURLcode = 26;
const CURLE_OPERATION_TIMEDOUT: CURLcode = 28;
const CURLE_RANGE_ERROR: CURLcode = 33;
const CURLE_ABORTED_BY_CALLBACK: CURLcode = 42;
const CURLE_SEND_ERROR: CURLcode = 55;
const CURLE_RECV_ERROR: CURLcode = 56;
const CURLE_AGAIN: CURLcode = 81;

const CURLPAUSE_RECV: c_int = 1 << 0;
const CURLPAUSE_SEND: c_int = 1 << 2;
const CURLPAUSE_ALL: c_int = CURLPAUSE_RECV | CURLPAUSE_SEND;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const IO_POLL_INTERVAL: Duration = Duration::from_millis(200);
const HEADER_WAIT_TIMEOUT: Duration = Duration::from_secs(30);
const REDIRECT_LIMIT: usize = 8;

unsafe extern "C" {
    static mut stdin: *mut c_void;
    static mut stdout: *mut c_void;
    fn fread(ptr: *mut c_void, size: usize, nmemb: usize, stream: *mut c_void) -> usize;
    fn fwrite(ptr: *const c_void, size: usize, nmemb: usize, stream: *mut c_void) -> usize;
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
    pub filters: ConnectionFilterChain,
    pub low_speed: LowSpeedWindow,
    pub connect_only: bool,
    pub reference_backend: bool,
}

struct ConnectOnlySession {
    stream: TcpStream,
    paused: c_int,
}

struct ConnectedStream {
    stream: TcpStream,
    info: RecordedTransferInfo,
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

struct LowSpeedGuard {
    window: LowSpeedWindow,
    window_start: Instant,
    window_bytes: usize,
}

impl LowSpeedGuard {
    fn new(window: LowSpeedWindow) -> Self {
        Self {
            window,
            window_start: Instant::now(),
            window_bytes: 0,
        }
    }

    fn observe_idle(&mut self) -> Result<(), CURLcode> {
        self.check(Instant::now())
    }

    fn observe_progress(&mut self, count: usize) -> Result<(), CURLcode> {
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
    let connect_override = metadata
        .connect_overrides
        .iter()
        .find(|candidate| candidate.matches(&authority.host, authority.port))
        .cloned();
    let target_host = connect_override
        .as_ref()
        .and_then(|candidate| candidate.target_host.clone())
        .unwrap_or_else(|| authority.host.clone());
    let target_port = connect_override
        .as_ref()
        .and_then(|candidate| candidate.target_port)
        .unwrap_or(authority.port);
    let proxy = metadata
        .proxy
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
            host: target_host,
            port: target_port,
            proxy_host: proxy.as_ref().map(|(host, _)| host.clone()),
            proxy_port: proxy.as_ref().map(|(_, port)| *port),
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
        filters,
        low_speed: metadata.low_speed,
        connect_only: metadata.connect_only,
        reference_backend: requires_reference_backend(metadata),
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

pub(crate) fn release_handle_state(handle: *mut CURL) {
    if handle.is_null() {
        return;
    }

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

pub(crate) unsafe fn pause_handle(handle: *mut CURL, bitmask: c_int) -> CURLcode {
    if handle.is_null() {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    }

    if let Some(session) = connect_only_registry()
        .lock()
        .expect("connect-only registry mutex poisoned")
        .get_mut(&(handle as usize))
    {
        session.paused = bitmask & CURLPAUSE_ALL;
    }
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

fn perform_transfer(handle_key: usize, plan: TransferPlan) -> CURLcode {
    let handle = handle_key as *mut CURL;
    release_handle_state(handle);

    let metadata = perform::snapshot_metadata(handle);
    let callbacks = perform::snapshot_callbacks(handle);
    let Some(initial_url) = metadata.url.clone() else {
        perform::set_error_buffer(handle, "No URL set");
        return CURLE_URL_MALFORMAT;
    };

    let mut current_url = initial_url;
    crate::share::touch_connect_callbacks(handle, metadata.share_handle, 6);
    let redirect_limit = if metadata.follow_location {
        REDIRECT_LIMIT
    } else {
        0
    };

    for redirect_count in 0..=redirect_limit {
        let request = match RequestContext::new(&current_url, &metadata) {
            Ok(request) => request,
            Err(code) => return code,
        };

        let outcome = if plan.connect_only {
            connect_only_transfer(handle, &request)
        } else {
            execute_http_transfer(handle, &request, &plan, &metadata, callbacks)
        };

        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(code) => return code,
        };

        let mut recorded_info = outcome.info.clone();
        recorded_info.response_code = outcome.response_code as c_long;
        recorded_info.retry_after = outcome.retry_after;
        perform::record_transfer_info(handle, recorded_info);

        if let Some(next_url) = redirected_url(
            &current_url,
            metadata.follow_location,
            redirect_count,
            outcome.response_code,
            outcome.location.as_deref(),
        ) {
            current_url = next_url;
            continue;
        }

        return outcome.result;
    }

    perform::set_error_buffer(handle, "Maximum redirects followed");
    CURLE_BAD_FUNCTION_ARGUMENT
}

fn connect_only_transfer(
    handle: *mut CURL,
    request: &RequestContext,
) -> Result<TransferOutcome, CURLcode> {
    let ConnectedStream { stream, mut info } = connect_stream(request, &[])?;
    stream
        .set_nonblocking(true)
        .map_err(|_| CURLE_COULDNT_CONNECT)?;
    info.pretransfer_time_us = info.connect_time_us;
    info.starttransfer_time_us = info.connect_time_us;
    info.total_time_us = info.connect_time_us;
    connect_only_registry()
        .lock()
        .expect("connect-only registry mutex poisoned")
        .insert(handle as usize, ConnectOnlySession { stream, paused: 0 });

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
) -> Result<TransferOutcome, CURLcode> {
    let request_started = Instant::now();
    let ConnectedStream {
        mut stream,
        mut info,
    } = connect_stream(request, &plan.resolve_overrides)?;
    stream
        .set_read_timeout(Some(IO_POLL_INTERVAL))
        .map_err(|_| CURLE_COULDNT_CONNECT)?;
    stream
        .set_write_timeout(Some(CONNECT_TIMEOUT))
        .map_err(|_| CURLE_COULDNT_CONNECT)?;
    write_request(&mut stream, request)?;
    if metadata.upload {
        write_request_body(&mut stream, handle, callbacks, request)?;
    }

    let response = read_response_meta(&mut stream, handle, callbacks, metadata)?;
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

fn transfer_body(
    stream: &mut TcpStream,
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
                let mut chunk = buf[..read].to_vec();
                deliver_write(handle, callbacks, &mut chunk)?;
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
    resolve_overrides: &[ResolveOverride],
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
        match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
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

fn write_request(stream: &mut TcpStream, request: &RequestContext) -> Result<(), CURLcode> {
    let mut encoded = String::new();
    encoded.push_str(&request.method);
    encoded.push(' ');
    encoded.push_str(&request.request_target);
    encoded.push_str(" HTTP/1.1\r\n");
    encoded.push_str("Host: ");
    encoded.push_str(&request.host_header);
    encoded.push_str("\r\n");
    encoded.push_str("Accept: */*\r\n");
    if request.use_chunked_upload {
        encoded.push_str("Transfer-Encoding: chunked\r\n");
    } else if let Some(body_length) = request.body_length {
        encoded.push_str("Content-Length: ");
        encoded.push_str(&body_length.to_string());
        encoded.push_str("\r\n");
    }
    for header in &request.request_headers {
        encoded.push_str(header);
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

fn write_request_body(
    stream: &mut TcpStream,
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
    stream: &mut TcpStream,
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
    stream: &mut TcpStream,
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

fn read_response_meta(
    stream: &mut TcpStream,
    handle: *mut CURL,
    callbacks: EasyCallbacks,
    metadata: &EasyMetadata,
) -> Result<ResponseMeta, CURLcode> {
    let mut bytes = Vec::new();
    let started = Instant::now();
    let header_end = loop {
        if let Some(header_end) = find_header_end(&bytes) {
            break header_end;
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

    for (index, raw_line) in split_header_lines(header_block).into_iter().enumerate() {
        deliver_header(handle, callbacks, metadata, raw_line)?;
        let text = String::from_utf8_lossy(raw_line);
        let trimmed = text.trim_end_matches(['\r', '\n']);
        if index == 0 {
            status_code = parse_status_code(trimmed).ok_or(CURLE_READ_ERROR)?;
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        let Some((name, value)) = trimmed.split_once(':') else {
            continue;
        };
        let lower = name.trim().to_ascii_lowercase();
        let value = value.trim();
        match lower.as_str() {
            "content-length" => content_length = value.parse::<usize>().ok(),
            "content-range" => has_content_range = true,
            "retry-after" => retry_after = parse_retry_after(value),
            "location" => location = Some(value.to_string()),
            _ => {}
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

fn deliver_write(
    handle: *mut CURL,
    callbacks: EasyCallbacks,
    buffer: &mut [u8],
) -> Result<(), CURLcode> {
    let wrote = if let Some(callback) = callbacks.write_function {
        unsafe {
            callback(
                buffer.as_mut_ptr().cast(),
                1,
                buffer.len(),
                callbacks.write_data as *mut c_void,
            )
        }
    } else {
        let stream = if callbacks.write_data == 0 {
            unsafe { stdout }
        } else {
            callbacks.write_data as *mut c_void
        };
        unsafe { fwrite(buffer.as_ptr().cast(), 1, buffer.len(), stream) }
    };
    if wrote == buffer.len() {
        Ok(())
    } else {
        perform::set_error_buffer(handle, "Failed writing received data");
        Err(CURLE_WRITE_ERROR)
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

fn elapsed_us(duration: Duration) -> curl_off_t {
    duration.as_micros().min(curl_off_t::MAX as u128) as curl_off_t
}

fn requires_reference_backend(metadata: &EasyMetadata) -> bool {
    let Some(url) = metadata.url.as_deref() else {
        return false;
    };
    let Some(authority) = parse_url_authority(url) else {
        return false;
    };

    authority.scheme != "http" || metadata.http_version >= 3
}

fn read_request_body_chunk(
    handle: *mut CURL,
    callbacks: EasyCallbacks,
    buffer: &mut [u8],
) -> Result<usize, CURLcode> {
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

    if read <= buffer.len() {
        Ok(read)
    } else {
        perform::set_error_buffer(handle, "Failed reading upload data");
        Err(CURLE_READ_ERROR)
    }
}

fn deliver_header(
    handle: *mut CURL,
    callbacks: EasyCallbacks,
    metadata: &EasyMetadata,
    raw_line: &[u8],
) -> Result<(), CURLcode> {
    if let Some(callback) = callbacks.header_function {
        let mut line = raw_line.to_vec();
        let wrote = unsafe {
            callback(
                line.as_mut_ptr().cast(),
                1,
                line.len(),
                callbacks.header_data as *mut c_void,
            )
        };
        if wrote == line.len() {
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

fn invoke_progress_callback(
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

impl RequestContext {
    fn new(url: &str, metadata: &EasyMetadata) -> Result<Self, CURLcode> {
        let parsed = ParsedUrl::parse(url).ok_or(CURLE_URL_MALFORMAT)?;
        if parsed.scheme != "http" {
            return Err(CURLE_UNSUPPORTED_PROTOCOL);
        }
        if metadata.tunnel_proxy {
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

        Ok(Self {
            url: url.to_string(),
            scheme: parsed.scheme,
            host_header: parsed.host_header,
            target_host,
            target_port,
            proxy,
            request_target: if has_proxy {
                url.to_string()
            } else {
                parsed.path_and_query
            },
            method: effective_method(metadata),
            request_headers: metadata.http_headers.clone(),
            use_chunked_upload: metadata.upload && metadata.upload_size.is_none(),
            range_header: effective_range_header(metadata),
            body_length: metadata
                .upload
                .then(|| {
                    if metadata.upload_size.is_none() {
                        0
                    } else {
                        metadata.upload_size.unwrap_or(0).max(0) as usize
                    }
                })
                .filter(|_| !metadata.upload_size.is_none()),
        })
    }
}

impl ParsedUrl {
    fn parse(url: &str) -> Option<Self> {
        let authority = parse_url_authority(url)?;
        let (scheme, rest) = url.split_once("://")?;
        let trimmed = rest.split('#').next().unwrap_or(rest);
        let path_start = trimmed.find(['/', '?']).unwrap_or(trimmed.len());
        let authority_text = &trimmed[..path_start];
        let host_header = authority_text
            .rsplit_once('@')
            .map(|(_, host)| host)
            .unwrap_or(authority_text)
            .to_string();
        let suffix = &trimmed[path_start..];
        let path_and_query = if suffix.is_empty() {
            "/".to_string()
        } else if suffix.starts_with('/') {
            suffix.to_string()
        } else {
            format!("/{suffix}")
        };

        Some(Self {
            scheme: scheme.to_ascii_lowercase(),
            host: authority.host,
            port: authority.port,
            host_header,
            path_and_query,
        })
    }
}

fn effective_method(metadata: &EasyMetadata) -> String {
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
