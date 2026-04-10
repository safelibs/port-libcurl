use crate::abi::{CURLMcode, CURLcode, CURL};
use crate::conn::cache::{parse_proxy_authority, parse_url_authority, ConnectionCacheKey};
use crate::conn::filter::{ConnectionFilterChain, ConnectionFilterStep};
use crate::dns::{ConnectOverride, ResolveOverride, ResolverLease, ResolverOwner};
use crate::easy::perform::EasyMetadata;
use crate::global;
use core::ffi::{c_int, c_long, c_void};
use std::sync::OnceLock;

pub(crate) const EASY_PERFORM_WAIT_TIMEOUT_MS: c_int = 1000;
const CURLM_OUT_OF_MEMORY: CURLMcode = 3;

type CurlEasyPerformFn = unsafe extern "C" fn(*mut CURL) -> CURLcode;
type CurlEasyPauseFn = unsafe extern "C" fn(*mut CURL, c_int) -> CURLcode;
type CurlEasyRecvFn = unsafe extern "C" fn(*mut CURL, *mut c_void, usize, *mut usize) -> CURLcode;
type CurlEasySendFn = unsafe extern "C" fn(*mut CURL, *const c_void, usize, *mut usize) -> CURLcode;
type CurlEasyUpkeepFn = unsafe extern "C" fn(*mut CURL) -> CURLcode;

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
    filters.push(ConnectionFilterStep::ReferenceBackend);

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
            conn_to_port: connect_override.as_ref().and_then(|candidate| candidate.target_port),
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
    }
}

pub(crate) fn spawn_reference_transfer<F>(
    handle_key: usize,
    on_complete: F,
) -> std::thread::JoinHandle<()>
where
    F: FnOnce(CURLcode) + Send + 'static,
{
    std::thread::spawn(move || {
        let result = unsafe { easy_perform_backend(handle_key as *mut CURL) };
        on_complete(result);
    })
}

pub(crate) unsafe fn easy_perform_backend(handle: *mut CURL) -> CURLcode {
    unsafe { ref_easy_perform()(handle) }
}

pub(crate) unsafe fn easy_pause_backend(handle: *mut CURL, bitmask: c_int) -> CURLcode {
    unsafe { ref_easy_pause()(handle, bitmask) }
}

pub(crate) unsafe fn easy_recv_backend(
    handle: *mut CURL,
    buffer: *mut c_void,
    buflen: usize,
    nread: *mut usize,
) -> CURLcode {
    unsafe { ref_easy_recv()(handle, buffer, buflen, nread) }
}

pub(crate) unsafe fn easy_send_backend(
    handle: *mut CURL,
    buffer: *const c_void,
    buflen: usize,
    nwritten: *mut usize,
) -> CURLcode {
    unsafe { ref_easy_send()(handle, buffer, buflen, nwritten) }
}

pub(crate) unsafe fn easy_upkeep_backend(handle: *mut CURL) -> CURLcode {
    unsafe { ref_easy_upkeep()(handle) }
}

fn ref_easy_perform() -> CurlEasyPerformFn {
    static FN: OnceLock<CurlEasyPerformFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_easy_perform\0") })
}

fn ref_easy_pause() -> CurlEasyPauseFn {
    static FN: OnceLock<CurlEasyPauseFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_easy_pause\0") })
}

fn ref_easy_recv() -> CurlEasyRecvFn {
    static FN: OnceLock<CurlEasyRecvFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_easy_recv\0") })
}

fn ref_easy_send() -> CurlEasySendFn {
    static FN: OnceLock<CurlEasySendFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_easy_send\0") })
}

fn ref_easy_upkeep() -> CurlEasyUpkeepFn {
    static FN: OnceLock<CurlEasyUpkeepFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_easy_upkeep\0") })
}
