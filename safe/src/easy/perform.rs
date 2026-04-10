use crate::abi::{
    curl_off_t, curl_slist, curl_socket_t, CURLcode, CURLoption, CURL, CURLE_BAD_FUNCTION_ARGUMENT,
    CURLE_FAILED_INIT, CURLINFO, CURLM,
};
use crate::dns::{self, ConnectOverride, ResolveOverride};
use crate::multi::state::MultiState;
use crate::transfer::{map_multi_code, LowSpeedWindow, EASY_PERFORM_WAIT_TIMEOUT_MS};
use core::ffi::{c_char, c_int, c_long, c_void};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::sync::{Mutex, OnceLock};

pub(crate) type CurlWriteCallback =
    Option<unsafe extern "C" fn(*mut c_char, usize, usize, *mut c_void) -> usize>;
pub(crate) type CurlReadCallback =
    Option<unsafe extern "C" fn(*mut c_char, usize, usize, *mut c_void) -> usize>;
pub(crate) type CurlTrailerCallback =
    Option<unsafe extern "C" fn(*mut *mut curl_slist, *mut c_void) -> c_int>;
pub(crate) type CurlXferInfoCallback = Option<
    unsafe extern "C" fn(*mut c_void, curl_off_t, curl_off_t, curl_off_t, curl_off_t) -> c_int,
>;

const CURLOPT_READDATA: CURLoption = 10009;
const CURLOPT_WRITEDATA: CURLoption = 10001;
const CURLOPT_URL: CURLoption = 10002;
const CURLOPT_PROXY: CURLoption = 10004;
const CURLOPT_USERPWD: CURLoption = 10005;
const CURLOPT_RANGE: CURLoption = 10007;
const CURLOPT_ERRORBUFFER: CURLoption = 10010;
const CURLOPT_WRITEFUNCTION: CURLoption = 20011;
const CURLOPT_READFUNCTION: CURLoption = 20012;
const CURLOPT_HTTPHEADER: CURLoption = 10023;
const CURLOPT_CUSTOMREQUEST: CURLoption = 10036;
const CURLOPT_INFILESIZE: CURLoption = 14;
const CURLOPT_LOW_SPEED_LIMIT: CURLoption = 19;
const CURLOPT_LOW_SPEED_TIME: CURLoption = 20;
const CURLOPT_RESUME_FROM: CURLoption = 21;
const CURLOPT_HEADERDATA: CURLoption = 10029;
const CURLOPT_VERBOSE: CURLoption = 41;
const CURLOPT_HEADER: CURLoption = 42;
const CURLOPT_NOPROGRESS: CURLoption = 43;
const CURLOPT_NOBODY: CURLoption = 44;
const CURLOPT_FAILONERROR: CURLoption = 45;
const CURLOPT_UPLOAD: CURLoption = 46;
const CURLOPT_FOLLOWLOCATION: CURLoption = 52;
const CURLOPT_XFERINFODATA: CURLoption = 10057;
const CURLOPT_PROXYPORT: CURLoption = 59;
const CURLOPT_HTTPPROXYTUNNEL: CURLoption = 61;
const CURLOPT_SSL_VERIFYPEER: CURLoption = 64;
const CURLOPT_MAXCONNECTS: CURLoption = 71;
const CURLOPT_HEADERFUNCTION: CURLoption = 20079;
const CURLOPT_HTTPGET: CURLoption = 80;
const CURLOPT_SSL_VERIFYHOST: CURLoption = 81;
const CURLOPT_SHARE: CURLoption = 10100;
const CURLOPT_INFILESIZE_LARGE: CURLoption = 30115;
const CURLOPT_RESUME_FROM_LARGE: CURLoption = 30116;
const CURLOPT_CONNECT_ONLY: CURLoption = 141;
const CURLOPT_USERNAME: CURLoption = 10173;
const CURLOPT_PASSWORD: CURLoption = 10174;
const CURLOPT_PROXYUSERNAME: CURLoption = 10175;
const CURLOPT_PROXYPASSWORD: CURLoption = 10176;
const CURLOPT_RESOLVE: CURLoption = 10203;
const CURLOPT_XFERINFOFUNCTION: CURLoption = 20219;
const CURLOPT_XOAUTH2_BEARER: CURLoption = 10220;
const CURLOPT_PINNEDPUBLICKEY: CURLoption = 10230;
const CURLOPT_CONNECT_TO: CURLoption = 10243;
const CURLOPT_PRE_PROXY: CURLoption = 10262;
const CURLOPT_TRAILERFUNCTION: CURLoption = 20283;
const CURLOPT_TRAILERDATA: CURLoption = 10284;

const CURLINFO_RESPONSE_CODE: u32 = 0x200000 + 2;
const CURLINFO_PRIMARY_IP: u32 = 0x100000 + 32;
const CURLINFO_PRIMARY_PORT: u32 = 0x200000 + 40;
const CURLINFO_LOCAL_IP: u32 = 0x100000 + 41;
const CURLINFO_LOCAL_PORT: u32 = 0x200000 + 42;
const CURLINFO_SCHEME: u32 = 0x100000 + 49;
const CURLINFO_TOTAL_TIME_T: u32 = 0x600000 + 50;
const CURLINFO_NAMELOOKUP_TIME_T: u32 = 0x600000 + 51;
const CURLINFO_CONNECT_TIME_T: u32 = 0x600000 + 52;
const CURLINFO_PRETRANSFER_TIME_T: u32 = 0x600000 + 53;
const CURLINFO_STARTTRANSFER_TIME_T: u32 = 0x600000 + 54;
const CURLINFO_RETRY_AFTER: u32 = 0x600000 + 57;
const CURL_ERROR_SIZE: usize = 256;

#[derive(Clone)]
pub(crate) struct EasyMetadata {
    pub url: Option<String>,
    pub custom_request: Option<String>,
    pub http_headers: Vec<String>,
    pub range: Option<String>,
    pub resolve_overrides: Vec<ResolveOverride>,
    pub connect_overrides: Vec<ConnectOverride>,
    pub proxy: Option<String>,
    pub pre_proxy: Option<String>,
    pub proxy_port: Option<u16>,
    pub tunnel_proxy: bool,
    pub share_handle: Option<usize>,
    pub userpwd: Option<String>,
    pub proxy_userpwd: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub proxy_username: Option<String>,
    pub proxy_password: Option<String>,
    pub xoauth2_bearer: Option<String>,
    pub pinned_public_key: Option<String>,
    pub ssl_verify_peer: bool,
    pub ssl_verify_host: c_long,
    pub connect_only: bool,
    pub follow_location: bool,
    pub header: bool,
    pub nobody: bool,
    pub upload: bool,
    pub upload_size: Option<curl_off_t>,
    pub http_get: bool,
    pub verbose: bool,
    pub fail_on_error: bool,
    pub resume_from: i64,
    pub low_speed: LowSpeedWindow,
    pub maxconnects: Option<c_long>,
}

impl EasyMetadata {
    pub(crate) fn tls_peer_identity(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(pinned_key) = self.pinned_public_key.as_ref() {
            parts.push(format!("pinned={pinned_key}"));
        }
        parts.push(format!("verify_peer={}", self.ssl_verify_peer));
        parts.push(format!("verify_host={}", self.ssl_verify_host));
        Some(parts.join(";"))
    }

    pub(crate) fn auth_context(&self) -> Option<String> {
        let mut parts = Vec::new();
        push_auth_part(&mut parts, "userpwd", self.userpwd.as_deref());
        push_auth_part(&mut parts, "proxy_userpwd", self.proxy_userpwd.as_deref());
        push_auth_part(&mut parts, "username", self.username.as_deref());
        push_auth_part(&mut parts, "password", self.password.as_deref());
        push_auth_part(&mut parts, "proxy_username", self.proxy_username.as_deref());
        push_auth_part(&mut parts, "proxy_password", self.proxy_password.as_deref());
        push_auth_part(&mut parts, "bearer", self.xoauth2_bearer.as_deref());
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(";"))
        }
    }
}

impl Default for EasyMetadata {
    fn default() -> Self {
        Self {
            url: None,
            custom_request: None,
            http_headers: Vec::new(),
            range: None,
            resolve_overrides: Vec::new(),
            connect_overrides: Vec::new(),
            proxy: None,
            pre_proxy: None,
            proxy_port: None,
            tunnel_proxy: false,
            share_handle: None,
            userpwd: None,
            proxy_userpwd: None,
            username: None,
            password: None,
            proxy_username: None,
            proxy_password: None,
            xoauth2_bearer: None,
            pinned_public_key: None,
            ssl_verify_peer: true,
            ssl_verify_host: 2,
            connect_only: false,
            follow_location: false,
            header: false,
            nobody: false,
            upload: false,
            upload_size: None,
            http_get: false,
            verbose: false,
            fail_on_error: false,
            resume_from: 0,
            low_speed: LowSpeedWindow::default(),
            maxconnects: None,
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct EasyCallbacks {
    pub read_function: CurlReadCallback,
    pub read_data: usize,
    pub trailer_function: CurlTrailerCallback,
    pub trailer_data: usize,
    pub write_function: CurlWriteCallback,
    pub write_data: usize,
    pub header_function: CurlWriteCallback,
    pub header_data: usize,
    pub xferinfo_function: CurlXferInfoCallback,
    pub xferinfo_data: usize,
    pub error_buffer: usize,
    pub no_progress: bool,
}

#[derive(Clone, Default)]
struct EasyInfo {
    response_code: c_long,
    primary_ip: Option<CString>,
    primary_port: c_long,
    local_ip: Option<CString>,
    local_port: c_long,
    total_time_us: curl_off_t,
    namelookup_time_us: curl_off_t,
    connect_time_us: curl_off_t,
    pretransfer_time_us: curl_off_t,
    starttransfer_time_us: curl_off_t,
    retry_after: curl_off_t,
    retry_after_set: bool,
}

#[derive(Clone, Default)]
pub(crate) struct RecordedTransferInfo {
    pub response_code: c_long,
    pub primary_ip: Option<String>,
    pub primary_port: Option<u16>,
    pub local_ip: Option<String>,
    pub local_port: Option<u16>,
    pub total_time_us: curl_off_t,
    pub namelookup_time_us: curl_off_t,
    pub connect_time_us: curl_off_t,
    pub pretransfer_time_us: curl_off_t,
    pub starttransfer_time_us: curl_off_t,
    pub retry_after: Option<curl_off_t>,
}

#[derive(Clone)]
struct EasyShadow {
    private_multi: Option<usize>,
    attached_multi: Option<usize>,
    metadata: EasyMetadata,
    callbacks: EasyCallbacks,
    info: EasyInfo,
    state: MultiState,
}

impl Default for EasyShadow {
    fn default() -> Self {
        Self {
            private_multi: None,
            attached_multi: None,
            metadata: EasyMetadata::default(),
            callbacks: EasyCallbacks::default(),
            info: EasyInfo::default(),
            state: MultiState::Init,
        }
    }
}

fn registry() -> &'static Mutex<HashMap<usize, EasyShadow>> {
    static REGISTRY: OnceLock<Mutex<HashMap<usize, EasyShadow>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
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

    let shadow = registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get(&(source as usize))
        .cloned()
        .unwrap_or_default();

    registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .insert(
            duplicate as usize,
            EasyShadow {
                private_multi: None,
                attached_multi: None,
                metadata: shadow.metadata,
                callbacks: shadow.callbacks,
                info: EasyInfo::default(),
                state: MultiState::Init,
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
        shadow.metadata = EasyMetadata::default();
        shadow.callbacks = EasyCallbacks::default();
        shadow.info = EasyInfo::default();
        shadow.state = MultiState::Init;
    }
}

pub(crate) fn unregister_handle(handle: *mut CURL) -> Option<usize> {
    if handle.is_null() {
        return None;
    }

    let mut guard = registry().lock().expect("easy registry mutex poisoned");
    let private_multi = guard
        .remove(&(handle as usize))
        .and_then(|shadow| shadow.private_multi);
    if guard.is_empty() {
        guard.shrink_to_fit();
    }
    private_multi
}

pub(crate) fn observe_easy_setopt_long(handle: *mut CURL, option: CURLoption, value: c_long) {
    if handle.is_null() {
        return;
    }

    let mut guard = registry().lock().expect("easy registry mutex poisoned");
    let shadow = guard.entry(handle as usize).or_default();
    match option {
        CURLOPT_INFILESIZE => shadow.metadata.upload_size = (value >= 0).then_some(value as i64),
        CURLOPT_MAXCONNECTS => shadow.metadata.maxconnects = Some(value),
        CURLOPT_CONNECT_ONLY => shadow.metadata.connect_only = value != 0,
        CURLOPT_LOW_SPEED_LIMIT => shadow.metadata.low_speed.limit_bytes_per_second = value,
        CURLOPT_LOW_SPEED_TIME => shadow.metadata.low_speed.time_window_secs = value,
        CURLOPT_RESUME_FROM => shadow.metadata.resume_from = value as i64,
        CURLOPT_HEADER => shadow.metadata.header = value != 0,
        CURLOPT_VERBOSE => shadow.metadata.verbose = value != 0,
        CURLOPT_NOPROGRESS => shadow.callbacks.no_progress = value != 0,
        CURLOPT_NOBODY => shadow.metadata.nobody = value != 0,
        CURLOPT_FAILONERROR => shadow.metadata.fail_on_error = value != 0,
        CURLOPT_UPLOAD => shadow.metadata.upload = value != 0,
        CURLOPT_FOLLOWLOCATION => shadow.metadata.follow_location = value != 0,
        CURLOPT_HTTPGET => shadow.metadata.http_get = value != 0,
        CURLOPT_PROXYPORT => shadow.metadata.proxy_port = u16::try_from(value).ok(),
        CURLOPT_HTTPPROXYTUNNEL => shadow.metadata.tunnel_proxy = value != 0,
        CURLOPT_SSL_VERIFYPEER => shadow.metadata.ssl_verify_peer = value != 0,
        CURLOPT_SSL_VERIFYHOST => shadow.metadata.ssl_verify_host = value,
        _ => {}
    }
}

pub(crate) fn observe_easy_setopt_ptr(handle: *mut CURL, option: CURLoption, value: *mut c_void) {
    if handle.is_null() {
        return;
    }

    let mut guard = registry().lock().expect("easy registry mutex poisoned");
    let shadow = guard.entry(handle as usize).or_default();
    match option {
        CURLOPT_READDATA => shadow.callbacks.read_data = value as usize,
        CURLOPT_WRITEDATA => shadow.callbacks.write_data = value as usize,
        CURLOPT_URL => shadow.metadata.url = copy_c_string(value.cast()),
        CURLOPT_PROXY => shadow.metadata.proxy = copy_c_string(value.cast()),
        CURLOPT_USERPWD => shadow.metadata.userpwd = copy_c_string(value.cast()),
        CURLOPT_RANGE => shadow.metadata.range = copy_c_string(value.cast()),
        CURLOPT_HTTPHEADER => shadow.metadata.http_headers = collect_slist_strings(value.cast()),
        CURLOPT_ERRORBUFFER => shadow.callbacks.error_buffer = value as usize,
        CURLOPT_HEADERDATA => shadow.callbacks.header_data = value as usize,
        CURLOPT_CUSTOMREQUEST => shadow.metadata.custom_request = copy_c_string(value.cast()),
        CURLOPT_TRAILERDATA => shadow.callbacks.trailer_data = value as usize,
        CURLOPT_XFERINFODATA => shadow.callbacks.xferinfo_data = value as usize,
        CURLOPT_SHARE => {
            shadow.metadata.share_handle = (!value.is_null()).then_some(value as usize)
        }
        CURLOPT_USERNAME => shadow.metadata.username = copy_c_string(value.cast()),
        CURLOPT_PASSWORD => shadow.metadata.password = copy_c_string(value.cast()),
        CURLOPT_PROXYUSERNAME => shadow.metadata.proxy_username = copy_c_string(value.cast()),
        CURLOPT_PROXYPASSWORD => shadow.metadata.proxy_password = copy_c_string(value.cast()),
        CURLOPT_RESOLVE => {
            shadow.metadata.resolve_overrides = dns::collect_resolve_overrides(value.cast())
        }
        CURLOPT_XOAUTH2_BEARER => shadow.metadata.xoauth2_bearer = copy_c_string(value.cast()),
        CURLOPT_PINNEDPUBLICKEY => shadow.metadata.pinned_public_key = copy_c_string(value.cast()),
        CURLOPT_CONNECT_TO => {
            shadow.metadata.connect_overrides = dns::collect_connect_overrides(value.cast())
        }
        CURLOPT_PRE_PROXY => shadow.metadata.pre_proxy = copy_c_string(value.cast()),
        _ => {}
    }
}

pub(crate) fn observe_easy_setopt_function(
    handle: *mut CURL,
    option: CURLoption,
    value: Option<unsafe extern "C" fn()>,
) {
    if handle.is_null() {
        return;
    }

    let mut guard = registry().lock().expect("easy registry mutex poisoned");
    let shadow = guard.entry(handle as usize).or_default();
    match option {
        CURLOPT_READFUNCTION => {
            shadow.callbacks.read_function = unsafe { core::mem::transmute(value) }
        }
        CURLOPT_TRAILERFUNCTION => {
            shadow.callbacks.trailer_function = unsafe { core::mem::transmute(value) }
        }
        CURLOPT_WRITEFUNCTION => {
            shadow.callbacks.write_function = unsafe { core::mem::transmute(value) }
        }
        CURLOPT_HEADERFUNCTION => {
            shadow.callbacks.header_function = unsafe { core::mem::transmute(value) }
        }
        CURLOPT_XFERINFOFUNCTION => {
            shadow.callbacks.xferinfo_function = unsafe { core::mem::transmute(value) }
        }
        _ => {}
    }
}

pub(crate) fn observe_easy_setopt_off_t(handle: *mut CURL, option: CURLoption, value: curl_off_t) {
    if handle.is_null() {
        return;
    }
    let mut guard = registry().lock().expect("easy registry mutex poisoned");
    let metadata = &mut guard.entry(handle as usize).or_default().metadata;
    if option == CURLOPT_RESUME_FROM_LARGE {
        metadata.resume_from = value as i64;
    } else if option == CURLOPT_INFILESIZE_LARGE {
        metadata.upload_size = (value >= 0).then_some(value);
    }
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

pub(crate) fn snapshot_metadata(handle: *mut CURL) -> EasyMetadata {
    registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get(&(handle as usize))
        .map(|shadow| shadow.metadata.clone())
        .unwrap_or_default()
}

pub(crate) fn snapshot_callbacks(handle: *mut CURL) -> EasyCallbacks {
    registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get(&(handle as usize))
        .map(|shadow| shadow.callbacks)
        .unwrap_or_default()
}

pub(crate) fn clear_transfer_info(handle: *mut CURL) {
    if handle.is_null() {
        return;
    }
    if let Some(shadow) = registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get_mut(&(handle as usize))
    {
        shadow.info = EasyInfo::default();
    }
}

pub(crate) fn record_transfer_info(handle: *mut CURL, info: RecordedTransferInfo) {
    if handle.is_null() {
        return;
    }
    if let Some(shadow) = registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get_mut(&(handle as usize))
    {
        shadow.info.response_code = info.response_code;
        shadow.info.primary_ip = info.primary_ip.and_then(to_c_string);
        shadow.info.primary_port = info.primary_port.map(c_long::from).unwrap_or(0);
        shadow.info.local_ip = info.local_ip.and_then(to_c_string);
        shadow.info.local_port = info.local_port.map(c_long::from).unwrap_or(0);
        shadow.info.total_time_us = info.total_time_us;
        shadow.info.namelookup_time_us = info.namelookup_time_us;
        shadow.info.connect_time_us = info.connect_time_us;
        shadow.info.pretransfer_time_us = info.pretransfer_time_us;
        shadow.info.starttransfer_time_us = info.starttransfer_time_us;
        if let Some(retry_after) = info.retry_after {
            shadow.info.retry_after = retry_after;
            shadow.info.retry_after_set = true;
        }
    }
}

pub(crate) fn easy_getinfo_long(
    handle: *mut CURL,
    info: u32,
    value: *mut c_long,
) -> Option<CURLcode> {
    if handle.is_null() || value.is_null() {
        return Some(CURLE_BAD_FUNCTION_ARGUMENT);
    }
    let guard = registry().lock().expect("easy registry mutex poisoned");
    let info_values = guard.get(&(handle as usize)).map(|shadow| &shadow.info);
    let result = match info {
        CURLINFO_RESPONSE_CODE => info_values.map(|info| info.response_code).unwrap_or(0),
        CURLINFO_PRIMARY_PORT => info_values.map(|info| info.primary_port).unwrap_or(0),
        CURLINFO_LOCAL_PORT => info_values.map(|info| info.local_port).unwrap_or(0),
        _ => return None,
    };
    unsafe { *value = result };
    Some(crate::abi::CURLE_OK)
}

pub(crate) fn easy_getinfo_string(
    handle: *mut CURL,
    info: u32,
    value: *mut *mut c_char,
) -> Option<CURLcode> {
    if handle.is_null() || value.is_null() {
        return Some(CURLE_BAD_FUNCTION_ARGUMENT);
    }

    let guard = registry().lock().expect("easy registry mutex poisoned");
    let shadow = guard.get(&(handle as usize));
    unsafe {
        *value = match info {
            CURLINFO_SCHEME => {
                let scheme = shadow
                    .and_then(|shadow| shadow.metadata.url.as_deref())
                    .and_then(|url| url.split_once("://").map(|(scheme, _)| scheme))
                    .unwrap_or("http")
                    .to_ascii_lowercase();
                match scheme.as_str() {
                    "https" => c"https".as_ptr().cast_mut(),
                    _ => c"http".as_ptr().cast_mut(),
                }
            }
            CURLINFO_PRIMARY_IP => shadow
                .and_then(|shadow| shadow.info.primary_ip.as_ref())
                .map(|value| value.as_ptr().cast_mut())
                .unwrap_or_else(|| c"".as_ptr().cast_mut()),
            CURLINFO_LOCAL_IP => shadow
                .and_then(|shadow| shadow.info.local_ip.as_ref())
                .map(|value| value.as_ptr().cast_mut())
                .unwrap_or_else(|| c"".as_ptr().cast_mut()),
            _ => return None,
        };
    }
    Some(crate::abi::CURLE_OK)
}

pub(crate) fn easy_getinfo_off_t(
    handle: *mut CURL,
    info: u32,
    value: *mut curl_off_t,
) -> Option<CURLcode> {
    if handle.is_null() || value.is_null() {
        return Some(CURLE_BAD_FUNCTION_ARGUMENT);
    }
    let guard = registry().lock().expect("easy registry mutex poisoned");
    let shadow = guard.get(&(handle as usize));
    let result = match info {
        CURLINFO_RETRY_AFTER => shadow
            .and_then(|shadow| {
                shadow
                    .info
                    .retry_after_set
                    .then_some(shadow.info.retry_after)
            })
            .unwrap_or(0),
        CURLINFO_TOTAL_TIME_T => shadow.map(|shadow| shadow.info.total_time_us).unwrap_or(0),
        CURLINFO_NAMELOOKUP_TIME_T => shadow
            .map(|shadow| shadow.info.namelookup_time_us)
            .unwrap_or(0),
        CURLINFO_CONNECT_TIME_T => shadow
            .map(|shadow| shadow.info.connect_time_us)
            .unwrap_or(0),
        CURLINFO_PRETRANSFER_TIME_T => shadow
            .map(|shadow| shadow.info.pretransfer_time_us)
            .unwrap_or(0),
        CURLINFO_STARTTRANSFER_TIME_T => shadow
            .map(|shadow| shadow.info.starttransfer_time_us)
            .unwrap_or(0),
        _ => return None,
    };
    unsafe { *value = result };
    Some(crate::abi::CURLE_OK)
}

pub(crate) fn easy_getinfo_socket(
    handle: *mut CURL,
    info: CURLINFO,
    value: *mut curl_socket_t,
) -> Option<CURLcode> {
    if handle.is_null() || value.is_null() {
        return Some(CURLE_BAD_FUNCTION_ARGUMENT);
    }
    if info != crate::transfer::CURLINFO_ACTIVESOCKET {
        return None;
    }

    unsafe {
        *value = crate::transfer::active_socket(handle).unwrap_or(-1);
    }
    Some(crate::abi::CURLE_OK)
}

pub(crate) fn clear_error_buffer(handle: *mut CURL) {
    let error_buffer = snapshot_callbacks(handle).error_buffer;
    if error_buffer != 0 {
        unsafe { *(error_buffer as *mut c_char) = 0 };
    }
}

pub(crate) fn set_error_buffer(handle: *mut CURL, message: &str) {
    let error_buffer = snapshot_callbacks(handle).error_buffer;
    if error_buffer == 0 {
        return;
    }

    let bytes = message.as_bytes();
    let max_len = bytes.len().min(CURL_ERROR_SIZE.saturating_sub(1));
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), error_buffer as *mut u8, max_len);
        *((error_buffer as *mut u8).add(max_len)) = 0;
    }
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
        .and_then(|shadow| shadow.metadata.maxconnects)
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
    crate::transfer::release_handle_state(handle);
    clear_error_buffer(handle);
    clear_transfer_info(handle);

    if attached_multi_for(handle).is_some() {
        set_error_buffer(handle, "easy handle already used in multi handle");
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
    crate::transfer::pause_handle(handle, bitmask)
}

pub(crate) unsafe fn easy_recv(
    handle: *mut CURL,
    buffer: *mut c_void,
    buflen: usize,
    nread: *mut usize,
) -> CURLcode {
    crate::transfer::recv_handle(handle, buffer, buflen, nread)
}

pub(crate) unsafe fn easy_send(
    handle: *mut CURL,
    buffer: *const c_void,
    buflen: usize,
    nwritten: *mut usize,
) -> CURLcode {
    crate::transfer::send_handle(handle, buffer, buflen, nwritten)
}

pub(crate) unsafe fn easy_upkeep(handle: *mut CURL) -> CURLcode {
    crate::transfer::upkeep_handle(handle)
}

fn copy_c_string(value: *const c_char) -> Option<String> {
    if value.is_null() {
        None
    } else {
        Some(
            unsafe { CStr::from_ptr(value) }
                .to_string_lossy()
                .into_owned(),
        )
    }
}

fn collect_slist_strings(mut list: *mut curl_slist) -> Vec<String> {
    let mut values = Vec::new();
    while !list.is_null() {
        let data = unsafe { (*list).data };
        if !data.is_null() {
            values.push(
                unsafe { CStr::from_ptr(data) }
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        list = unsafe { (*list).next };
    }
    values
}

fn to_c_string(value: String) -> Option<CString> {
    CString::new(value).ok()
}

fn push_auth_part(parts: &mut Vec<String>, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        parts.push(format!("{label}={value}"));
    }
}
