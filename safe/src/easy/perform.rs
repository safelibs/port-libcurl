use crate::abi::{
    curl_hstsread_callback, curl_hstswrite_callback, curl_off_t, curl_slist, curl_sockaddr,
    curl_socket_t, CURLcode, CURLoption, CURL, CURLE_BAD_FUNCTION_ARGUMENT, CURLE_FAILED_INIT,
    CURLINFO, CURLM, CURLU, CURLUPART_URL,
};
use crate::dns::{self, ConnectOverride, ResolveOverride};
use crate::multi::state::MultiState;
use crate::transfer::{map_multi_code, LowSpeedWindow, EASY_PERFORM_WAIT_TIMEOUT_MS};
use core::ffi::{c_char, c_int, c_long, c_void};
use core::ptr;
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
pub(crate) type CurlOpenSocketCallback =
    Option<unsafe extern "C" fn(*mut c_void, c_int, *mut curl_sockaddr) -> curl_socket_t>;
pub(crate) type CurlCloseSocketCallback =
    Option<unsafe extern "C" fn(*mut c_void, curl_socket_t) -> c_int>;

const CURLOPT_READDATA: CURLoption = 10009;
const CURLOPT_WRITEDATA: CURLoption = 10001;
const CURLOPT_URL: CURLoption = 10002;
const CURLOPT_PROXY: CURLoption = 10004;
const CURLOPT_USERPWD: CURLoption = 10005;
const CURLOPT_PROXYUSERPWD: CURLoption = 10006;
const CURLOPT_RANGE: CURLoption = 10007;
const CURLOPT_ERRORBUFFER: CURLoption = 10010;
const CURLOPT_REFERER: CURLoption = 10016;
const CURLOPT_USERAGENT: CURLoption = 10018;
const CURLOPT_WRITEFUNCTION: CURLoption = 20011;
const CURLOPT_READFUNCTION: CURLoption = 20012;
const CURLOPT_COOKIE: CURLoption = 10022;
const CURLOPT_HTTPHEADER: CURLoption = 10023;
const CURLOPT_COOKIEFILE: CURLoption = 10031;
const CURLOPT_CUSTOMREQUEST: CURLoption = 10036;
const CURLOPT_PROXYHEADER: CURLoption = 10228;
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
const CURLOPT_NETRC: CURLoption = 51;
const CURLOPT_FOLLOWLOCATION: CURLoption = 52;
const CURLOPT_TRANSFERTEXT: CURLoption = 53;
const CURLOPT_AUTOREFERER: CURLoption = 58;
const CURLOPT_XFERINFODATA: CURLoption = 10057;
const CURLOPT_PROXYPORT: CURLoption = 59;
const CURLOPT_HTTPPROXYTUNNEL: CURLoption = 61;
const CURLOPT_HTTP_VERSION: CURLoption = 84;
const CURLOPT_SSL_VERIFYPEER: CURLoption = 64;
const CURLOPT_MAXREDIRS: CURLoption = 68;
const CURLOPT_MAXCONNECTS: CURLoption = 71;
const CURLOPT_CERTINFO: CURLoption = 172;
const CURLOPT_HEADERFUNCTION: CURLoption = 20079;
const CURLOPT_HTTPGET: CURLoption = 80;
const CURLOPT_COOKIEJAR: CURLoption = 10082;
const CURLOPT_SSL_VERIFYHOST: CURLoption = 81;
const CURLOPT_COOKIESESSION: CURLoption = 96;
const CURLOPT_SHARE: CURLoption = 10100;
const CURLOPT_UNRESTRICTED_AUTH: CURLoption = 105;
const CURLOPT_HTTPAUTH: CURLoption = 107;
const CURLOPT_PROXYAUTH: CURLoption = 111;
const CURLOPT_NETRC_FILE: CURLoption = 10118;
const CURLOPT_COOKIELIST: CURLoption = 10135;
const CURLOPT_INFILESIZE_LARGE: CURLoption = 30115;
const CURLOPT_RESUME_FROM_LARGE: CURLoption = 30116;
const CURLOPT_CONNECT_ONLY: CURLoption = 141;
const CURLOPT_OPENSOCKETDATA: CURLoption = 10164;
const CURLOPT_USERNAME: CURLoption = 10173;
const CURLOPT_PASSWORD: CURLoption = 10174;
const CURLOPT_PROXYUSERNAME: CURLoption = 10175;
const CURLOPT_PROXYPASSWORD: CURLoption = 10176;
const CURLOPT_RESOLVE: CURLoption = 10203;
const CURLOPT_RTSP_SESSION_ID: CURLoption = 10190;
const CURLOPT_RTSP_STREAM_URI: CURLoption = 10191;
const CURLOPT_RTSP_TRANSPORT: CURLoption = 10192;
const CURLOPT_XFERINFOFUNCTION: CURLoption = 20219;
const CURLOPT_XOAUTH2_BEARER: CURLoption = 10220;
const CURLOPT_PINNEDPUBLICKEY: CURLoption = 10230;
const CURLOPT_CONNECT_TO: CURLoption = 10243;
const CURLOPT_PRE_PROXY: CURLoption = 10262;
const CURLOPT_HEADEROPT: CURLoption = 229;
const CURLOPT_ALTSVC_CTRL: CURLoption = 286;
const CURLOPT_ALTSVC: CURLoption = 10287;
const CURLOPT_HSTS_CTRL: CURLoption = 299;
const CURLOPT_HSTS: CURLoption = 10300;
const CURLOPT_HSTSREADFUNCTION: CURLoption = 20301;
const CURLOPT_HSTSREADDATA: CURLoption = 10302;
const CURLOPT_HSTSWRITEFUNCTION: CURLoption = 20303;
const CURLOPT_HSTSWRITEDATA: CURLoption = 10304;
const CURLOPT_CURLU: CURLoption = 10282;
const CURLOPT_WS_OPTIONS: CURLoption = 320;
const CURLOPT_TRAILERFUNCTION: CURLoption = 20283;
const CURLOPT_TRAILERDATA: CURLoption = 10284;
const CURLOPT_SSL_ENABLE_ALPN: CURLoption = 226;
const CURLOPT_DOH_URL: CURLoption = 10279;
const CURLOPT_OPENSOCKETFUNCTION: CURLoption = 20163;
const CURLOPT_RTSP_REQUEST: CURLoption = 189;
const CURLOPT_CLOSESOCKETFUNCTION: CURLoption = 20208;
const CURLOPT_CLOSESOCKETDATA: CURLoption = 10209;

const CURLINFO_RESPONSE_CODE: u32 = 0x200000 + 2;
const CURLINFO_PRIMARY_IP: u32 = 0x100000 + 32;
const CURLINFO_PRIMARY_PORT: u32 = 0x200000 + 40;
const CURLINFO_LOCAL_IP: u32 = 0x100000 + 41;
const CURLINFO_LOCAL_PORT: u32 = 0x200000 + 42;
const CURLINFO_COOKIELIST: u32 = 0x400000 + 28;
const CURLINFO_CERTINFO: u32 = 0x400000 + 34;
const CURLINFO_SCHEME: u32 = 0x100000 + 49;
const CURLINFO_RTSP_SESSION_ID: u32 = 0x100000 + 36;
const CURLINFO_TOTAL_TIME_T: u32 = 0x600000 + 50;
const CURLINFO_NAMELOOKUP_TIME_T: u32 = 0x600000 + 51;
const CURLINFO_CONNECT_TIME_T: u32 = 0x600000 + 52;
const CURLINFO_PRETRANSFER_TIME_T: u32 = 0x600000 + 53;
const CURLINFO_STARTTRANSFER_TIME_T: u32 = 0x600000 + 54;
const CURLINFO_RETRY_AFTER: u32 = 0x600000 + 57;
const CURL_ERROR_SIZE: usize = 256;

unsafe extern "C" {
    fn curl_safe_reference_easy_getinfo_slist(
        handle: *mut CURL,
        info: u32,
        value: *mut *mut curl_slist,
    ) -> CURLcode;
}

type CurlEasyPauseFn = unsafe extern "C" fn(*mut CURL, c_int) -> CURLcode;

fn ref_easy_pause() -> CurlEasyPauseFn {
    static FN: OnceLock<CurlEasyPauseFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { crate::global::load_reference(b"curl_easy_pause\0") })
}

#[derive(Clone)]
pub(crate) struct EasyMetadata {
    pub url: Option<String>,
    pub custom_request: Option<String>,
    pub http_headers: Vec<String>,
    pub proxy_headers: Vec<String>,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
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
    pub cookie: Option<String>,
    pub cookie_file: Option<String>,
    pub cookie_jar: Option<String>,
    pub cookie_session: bool,
    pub cookie_list: Vec<String>,
    pub netrc_mode: c_long,
    pub netrc_file: Option<String>,
    pub unrestricted_auth: bool,
    pub auto_referer: bool,
    pub max_redirs: Option<c_long>,
    pub httpauth: c_long,
    pub proxyauth: c_long,
    pub transfer_text: bool,
    pub headeropt: c_long,
    pub connect_mode: c_long,
    pub ws_options: c_long,
    pub curlu_handle: Option<usize>,
    pub rtsp_request: c_long,
    pub rtsp_session_id: Option<String>,
    pub rtsp_stream_uri: Option<String>,
    pub rtsp_transport: Option<String>,
    pub hsts_file: Option<String>,
    pub hsts_ctrl: c_long,
    pub altsvc_file: Option<String>,
    pub altsvc_ctrl: c_long,
    pub doh_url: Option<String>,
    pub pinned_public_key: Option<String>,
    pub ssl_verify_peer: bool,
    pub ssl_verify_host: c_long,
    pub ssl_enable_alpn: bool,
    pub certinfo: bool,
    pub connect_only: bool,
    pub follow_location: bool,
    pub header: bool,
    pub nobody: bool,
    pub upload: bool,
    pub upload_size: Option<curl_off_t>,
    pub http_get: bool,
    pub http_version: c_long,
    pub verbose: bool,
    pub fail_on_error: bool,
    pub resume_from: i64,
    pub low_speed: LowSpeedWindow,
    pub maxconnects: Option<c_long>,
}

impl EasyMetadata {
    pub(crate) fn tls_peer_identity(&self) -> Option<String> {
        crate::tls::peer_identity(self)
    }

    pub(crate) fn auth_context(&self) -> Option<String> {
        crate::http::auth::build_auth_context(self)
    }
}

impl Default for EasyMetadata {
    fn default() -> Self {
        Self {
            url: None,
            custom_request: None,
            http_headers: Vec::new(),
            proxy_headers: Vec::new(),
            user_agent: None,
            referer: None,
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
            cookie: None,
            cookie_file: None,
            cookie_jar: None,
            cookie_session: false,
            cookie_list: Vec::new(),
            netrc_mode: 0,
            netrc_file: None,
            unrestricted_auth: false,
            auto_referer: false,
            max_redirs: None,
            httpauth: 0,
            proxyauth: 0,
            transfer_text: false,
            headeropt: 0,
            connect_mode: 0,
            ws_options: 0,
            curlu_handle: None,
            rtsp_request: 0,
            rtsp_session_id: None,
            rtsp_stream_uri: None,
            rtsp_transport: None,
            hsts_file: None,
            hsts_ctrl: 0,
            altsvc_file: None,
            altsvc_ctrl: 0,
            doh_url: None,
            pinned_public_key: None,
            ssl_verify_peer: true,
            ssl_verify_host: 2,
            ssl_enable_alpn: true,
            certinfo: false,
            connect_only: false,
            follow_location: false,
            header: false,
            nobody: false,
            upload: false,
            upload_size: None,
            http_get: false,
            http_version: 0,
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
    pub hsts_read_function: curl_hstsread_callback,
    pub hsts_read_data: usize,
    pub hsts_write_function: curl_hstswrite_callback,
    pub hsts_write_data: usize,
    pub open_socket_function: CurlOpenSocketCallback,
    pub open_socket_data: usize,
    pub close_socket_function: CurlCloseSocketCallback,
    pub close_socket_data: usize,
    pub no_progress: bool,
}

#[derive(Clone, Default)]
struct EasyInfo {
    response_code: c_long,
    primary_ip: Option<CString>,
    primary_port: c_long,
    local_ip: Option<CString>,
    local_port: c_long,
    rtsp_session_id: Option<CString>,
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
    http_state: crate::http::HandleHttpState,
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
            http_state: crate::http::HandleHttpState::default(),
            state: MultiState::Init,
        }
    }
}

fn registry() -> &'static Mutex<HashMap<usize, EasyShadow>> {
    static REGISTRY: OnceLock<Mutex<HashMap<usize, EasyShadow>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn clear_registry() {
    let mut guard = registry().lock().expect("easy registry mutex poisoned");
    *guard = HashMap::new();
    crate::tls::certinfo::clear_all();
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
                http_state: shadow.http_state,
                state: MultiState::Init,
            },
        );
}

pub(crate) fn reset_handle(handle: *mut CURL) {
    if handle.is_null() {
        return;
    }
    crate::tls::certinfo::clear(handle);
    if let Some(shadow) = registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get_mut(&(handle as usize))
    {
        shadow.metadata = EasyMetadata::default();
        shadow.callbacks = EasyCallbacks::default();
        shadow.info = EasyInfo::default();
        shadow.http_state = crate::http::HandleHttpState::default();
        shadow.state = MultiState::Init;
    }
}

pub(crate) fn unregister_handle(handle: *mut CURL) -> Option<usize> {
    if handle.is_null() {
        return None;
    }

    crate::tls::certinfo::clear(handle);

    let mut guard = registry().lock().expect("easy registry mutex poisoned");
    let private_multi = guard
        .remove(&(handle as usize))
        .and_then(|shadow| shadow.private_multi);
    if guard.is_empty() {
        *guard = HashMap::new();
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
        CURLOPT_CONNECT_ONLY => {
            shadow.metadata.connect_only = value != 0;
            shadow.metadata.connect_mode = value;
        }
        CURLOPT_LOW_SPEED_LIMIT => shadow.metadata.low_speed.limit_bytes_per_second = value,
        CURLOPT_LOW_SPEED_TIME => shadow.metadata.low_speed.time_window_secs = value,
        CURLOPT_RESUME_FROM => shadow.metadata.resume_from = value as i64,
        CURLOPT_HEADER => shadow.metadata.header = value != 0,
        CURLOPT_VERBOSE => shadow.metadata.verbose = value != 0,
        CURLOPT_NOPROGRESS => shadow.callbacks.no_progress = value != 0,
        CURLOPT_NOBODY => shadow.metadata.nobody = value != 0,
        CURLOPT_FAILONERROR => shadow.metadata.fail_on_error = value != 0,
        CURLOPT_UPLOAD => shadow.metadata.upload = value != 0,
        CURLOPT_NETRC => shadow.metadata.netrc_mode = value,
        CURLOPT_FOLLOWLOCATION => shadow.metadata.follow_location = value != 0,
        CURLOPT_TRANSFERTEXT => shadow.metadata.transfer_text = value != 0,
        CURLOPT_AUTOREFERER => shadow.metadata.auto_referer = value != 0,
        CURLOPT_HTTPGET => shadow.metadata.http_get = value != 0,
        CURLOPT_HTTP_VERSION => shadow.metadata.http_version = value,
        CURLOPT_PROXYPORT => shadow.metadata.proxy_port = u16::try_from(value).ok(),
        CURLOPT_HTTPPROXYTUNNEL => shadow.metadata.tunnel_proxy = value != 0,
        CURLOPT_MAXREDIRS => shadow.metadata.max_redirs = Some(value),
        CURLOPT_COOKIESESSION => shadow.metadata.cookie_session = value != 0,
        CURLOPT_CERTINFO => shadow.metadata.certinfo = value != 0,
        CURLOPT_SSL_VERIFYPEER => shadow.metadata.ssl_verify_peer = value != 0,
        CURLOPT_SSL_VERIFYHOST => shadow.metadata.ssl_verify_host = value,
        CURLOPT_SSL_ENABLE_ALPN => shadow.metadata.ssl_enable_alpn = value != 0,
        CURLOPT_UNRESTRICTED_AUTH => shadow.metadata.unrestricted_auth = value != 0,
        CURLOPT_HTTPAUTH => shadow.metadata.httpauth = value,
        CURLOPT_PROXYAUTH => shadow.metadata.proxyauth = value,
        CURLOPT_RTSP_REQUEST => shadow.metadata.rtsp_request = value,
        CURLOPT_HEADEROPT => shadow.metadata.headeropt = value,
        CURLOPT_ALTSVC_CTRL => {
            shadow.metadata.altsvc_ctrl = value;
            shadow.http_state.altsvc.ctrl_bits = value;
            shadow.http_state.altsvc.enabled = value != 0;
        }
        CURLOPT_HSTS_CTRL => shadow.metadata.hsts_ctrl = value,
        CURLOPT_WS_OPTIONS => shadow.metadata.ws_options = value,
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
        CURLOPT_PROXYUSERPWD => shadow.metadata.proxy_userpwd = copy_c_string(value.cast()),
        CURLOPT_RANGE => shadow.metadata.range = copy_c_string(value.cast()),
        CURLOPT_REFERER => shadow.metadata.referer = copy_c_string(value.cast()),
        CURLOPT_USERAGENT => shadow.metadata.user_agent = copy_c_string(value.cast()),
        CURLOPT_COOKIE => shadow.metadata.cookie = copy_c_string(value.cast()),
        CURLOPT_HTTPHEADER => shadow.metadata.http_headers = collect_slist_strings(value.cast()),
        CURLOPT_PROXYHEADER => shadow.metadata.proxy_headers = collect_slist_strings(value.cast()),
        CURLOPT_COOKIEFILE => shadow.metadata.cookie_file = copy_c_string(value.cast()),
        CURLOPT_ERRORBUFFER => shadow.callbacks.error_buffer = value as usize,
        CURLOPT_HEADERDATA => shadow.callbacks.header_data = value as usize,
        CURLOPT_CUSTOMREQUEST => shadow.metadata.custom_request = copy_c_string(value.cast()),
        CURLOPT_TRAILERDATA => shadow.callbacks.trailer_data = value as usize,
        CURLOPT_XFERINFODATA => shadow.callbacks.xferinfo_data = value as usize,
        CURLOPT_SHARE => {
            shadow.metadata.share_handle = (!value.is_null()).then_some(value as usize)
        }
        CURLOPT_OPENSOCKETDATA => shadow.callbacks.open_socket_data = value as usize,
        CURLOPT_USERNAME => shadow.metadata.username = copy_c_string(value.cast()),
        CURLOPT_PASSWORD => shadow.metadata.password = copy_c_string(value.cast()),
        CURLOPT_PROXYUSERNAME => shadow.metadata.proxy_username = copy_c_string(value.cast()),
        CURLOPT_PROXYPASSWORD => shadow.metadata.proxy_password = copy_c_string(value.cast()),
        CURLOPT_COOKIEJAR => shadow.metadata.cookie_jar = copy_c_string(value.cast()),
        CURLOPT_NETRC_FILE => shadow.metadata.netrc_file = copy_c_string(value.cast()),
        CURLOPT_COOKIELIST => {
            if let Some(value) = copy_c_string(value.cast()) {
                if value.eq_ignore_ascii_case("FLUSH") {
                    let path = shadow
                        .metadata
                        .cookie_jar
                        .clone()
                        .or_else(|| shadow.metadata.cookie_file.clone());
                    if let Some(path) = path {
                        let reference_lines = reference_cookie_lines(handle);
                        if !reference_lines.is_empty() {
                            let rendered =
                                crate::http::cookies::render_netscape_lines(&reference_lines);
                            let _ = std::fs::write(&path, rendered);
                        } else {
                            let mut wrote_fallback = false;
                            if let Some(url) = shadow.metadata.url.clone() {
                                let latest_headers =
                                    shadow.http_state.headers.latest_values("set-cookie");
                                if !latest_headers.is_empty() {
                                    let mut fallback_store =
                                        crate::http::cookies::CookieStore::default();
                                    for header in latest_headers {
                                        fallback_store.store_set_cookie(&url, &header);
                                    }
                                    let _ = fallback_store.flush_to_path(&path);
                                    wrote_fallback = true;
                                }
                            }
                            if !wrote_fallback
                                && crate::share::with_shared_cookies_mut(
                                    shadow.metadata.share_handle,
                                    |store| store.flush_to_path(&path),
                                )
                                .is_none()
                            {
                                let _ = shadow.http_state.cookies.flush_to_path(&path);
                            }
                        }
                    }
                } else {
                    shadow.metadata.cookie_list.push(value);
                }
            }
        }
        CURLOPT_RESOLVE => {
            shadow.metadata.resolve_overrides = dns::collect_resolve_overrides(value.cast())
        }
        CURLOPT_XOAUTH2_BEARER => shadow.metadata.xoauth2_bearer = copy_c_string(value.cast()),
        CURLOPT_PINNEDPUBLICKEY => shadow.metadata.pinned_public_key = copy_c_string(value.cast()),
        CURLOPT_DOH_URL => shadow.metadata.doh_url = copy_c_string(value.cast()),
        CURLOPT_RTSP_SESSION_ID => shadow.metadata.rtsp_session_id = copy_c_string(value.cast()),
        CURLOPT_RTSP_STREAM_URI => shadow.metadata.rtsp_stream_uri = copy_c_string(value.cast()),
        CURLOPT_RTSP_TRANSPORT => shadow.metadata.rtsp_transport = copy_c_string(value.cast()),
        CURLOPT_CONNECT_TO => {
            shadow.metadata.connect_overrides = dns::collect_connect_overrides(value.cast())
        }
        CURLOPT_PRE_PROXY => shadow.metadata.pre_proxy = copy_c_string(value.cast()),
        CURLOPT_ALTSVC => {
            shadow.metadata.altsvc_file = copy_c_string(value.cast());
            shadow.http_state.altsvc.path = shadow.metadata.altsvc_file.clone();
        }
        CURLOPT_CURLU => {
            shadow.metadata.curlu_handle = (!value.is_null()).then_some(value as usize);
            shadow.metadata.url = copy_url_from_curlu(value.cast::<CURLU>());
        }
        CURLOPT_HSTS => shadow.metadata.hsts_file = copy_c_string(value.cast()),
        CURLOPT_HSTSREADDATA => shadow.callbacks.hsts_read_data = value as usize,
        CURLOPT_HSTSWRITEDATA => shadow.callbacks.hsts_write_data = value as usize,
        CURLOPT_CLOSESOCKETDATA => shadow.callbacks.close_socket_data = value as usize,
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
        CURLOPT_HSTSREADFUNCTION => {
            shadow.callbacks.hsts_read_function = unsafe { core::mem::transmute(value) }
        }
        CURLOPT_HSTSWRITEFUNCTION => {
            shadow.callbacks.hsts_write_function = unsafe { core::mem::transmute(value) }
        }
        CURLOPT_OPENSOCKETFUNCTION => {
            shadow.callbacks.open_socket_function = unsafe { core::mem::transmute(value) }
        }
        CURLOPT_CLOSESOCKETFUNCTION => {
            shadow.callbacks.close_socket_function = unsafe { core::mem::transmute(value) }
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
    let mut metadata = registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get(&(handle as usize))
        .map(|shadow| shadow.metadata.clone())
        .unwrap_or_default();
    if let Some(curlu_handle) = metadata.curlu_handle {
        if let Some(url) = copy_url_from_curlu(curlu_handle as *mut CURLU) {
            metadata.url = Some(url);
        }
    }
    metadata
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
    crate::tls::certinfo::clear(handle);
    if let Some(shadow) = registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get_mut(&(handle as usize))
    {
        shadow.info = EasyInfo::default();
        shadow.http_state.clear_transient();
    }
}

pub(crate) fn with_http_state_mut<R>(
    handle: *mut CURL,
    f: impl FnOnce(&mut crate::http::HandleHttpState) -> R,
) -> Option<R> {
    if handle.is_null() {
        return None;
    }
    let mut guard = registry().lock().expect("easy registry mutex poisoned");
    let shadow = guard.get_mut(&(handle as usize))?;
    Some(f(&mut shadow.http_state))
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

pub(crate) fn record_rtsp_session_id(handle: *mut CURL, session_id: Option<&str>) {
    if handle.is_null() {
        return;
    }
    if let Some(shadow) = registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get_mut(&(handle as usize))
    {
        shadow.info.rtsp_session_id = session_id.map(str::to_string).and_then(to_c_string);
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
        CURLINFO_RESPONSE_CODE => match info_values.map(|info| info.response_code) {
            Some(code) if code != 0 => code,
            _ => return None,
        },
        CURLINFO_PRIMARY_PORT => match info_values.map(|info| info.primary_port) {
            Some(port) if port != 0 => port,
            _ => return None,
        },
        CURLINFO_LOCAL_PORT => match info_values.map(|info| info.local_port) {
            Some(port) if port != 0 => port,
            _ => return None,
        },
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
                    "dict" => c"dict".as_ptr().cast_mut(),
                    "file" => c"file".as_ptr().cast_mut(),
                    "ftp" => c"ftp".as_ptr().cast_mut(),
                    "ftps" => c"ftps".as_ptr().cast_mut(),
                    "gopher" => c"gopher".as_ptr().cast_mut(),
                    "http" => c"http".as_ptr().cast_mut(),
                    "https" => c"https".as_ptr().cast_mut(),
                    "imap" => c"imap".as_ptr().cast_mut(),
                    "imaps" => c"imaps".as_ptr().cast_mut(),
                    "ldap" => c"ldap".as_ptr().cast_mut(),
                    "ldaps" => c"ldaps".as_ptr().cast_mut(),
                    "mqtt" => c"mqtt".as_ptr().cast_mut(),
                    "pop3" => c"pop3".as_ptr().cast_mut(),
                    "pop3s" => c"pop3s".as_ptr().cast_mut(),
                    "rtsp" => c"rtsp".as_ptr().cast_mut(),
                    "scp" => c"scp".as_ptr().cast_mut(),
                    "smb" => c"smb".as_ptr().cast_mut(),
                    "smbs" => c"smbs".as_ptr().cast_mut(),
                    "sftp" => c"sftp".as_ptr().cast_mut(),
                    "smtp" => c"smtp".as_ptr().cast_mut(),
                    "smtps" => c"smtps".as_ptr().cast_mut(),
                    "telnet" => c"telnet".as_ptr().cast_mut(),
                    "tftp" => c"tftp".as_ptr().cast_mut(),
                    "wss" => c"wss".as_ptr().cast_mut(),
                    "ws" => c"ws".as_ptr().cast_mut(),
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
            CURLINFO_RTSP_SESSION_ID => shadow
                .and_then(|shadow| shadow.info.rtsp_session_id.as_ref())
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

pub(crate) fn easy_getinfo_ptr(
    handle: *mut CURL,
    info: u32,
    value: *mut *mut c_void,
) -> Option<CURLcode> {
    if handle.is_null() || value.is_null() {
        return Some(CURLE_BAD_FUNCTION_ARGUMENT);
    }
    if info != CURLINFO_CERTINFO {
        return None;
    }

    unsafe {
        *value = crate::tls::certinfo::lookup(handle)
            .map_or(ptr::null_mut(), |certinfo| certinfo.cast::<c_void>());
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
    if crate::transfer::has_connect_only_session(handle) {
        crate::transfer::pause_handle(handle, bitmask)
    } else {
        unsafe { ref_easy_pause()(handle, bitmask) }
    }
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

fn copy_url_from_curlu(value: *mut CURLU) -> Option<String> {
    if value.is_null() {
        return None;
    }

    let mut part = ptr::null_mut();
    let code = unsafe { crate::urlapi::url_get(value, CURLUPART_URL, &mut part, 0) };
    if code != 0 || part.is_null() {
        return None;
    }

    let copied = unsafe { CStr::from_ptr(part) }
        .to_string_lossy()
        .into_owned();
    unsafe { crate::alloc::free_ptr(part.cast()) };
    Some(copied)
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

fn reference_cookie_lines(handle: *mut CURL) -> Vec<String> {
    if handle.is_null() {
        return Vec::new();
    }

    let mut list = ptr::null_mut();
    let code =
        unsafe { curl_safe_reference_easy_getinfo_slist(handle, CURLINFO_COOKIELIST, &mut list) };
    if code != crate::abi::CURLE_OK || list.is_null() {
        return Vec::new();
    }

    let values = collect_slist_strings(list);
    unsafe { crate::slist::curl_slist_free_all(list) };
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
