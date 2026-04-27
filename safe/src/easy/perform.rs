use crate::abi::{
    curl_hstsread_callback, curl_hstswrite_callback, curl_off_t, curl_slist, curl_sockaddr,
    curl_socket_t, CURLcode, CURLoption, CURL, CURLE_BAD_FUNCTION_ARGUMENT, CURLE_FAILED_INIT,
    CURLE_OK, CURLE_OUT_OF_MEMORY, CURLE_UNKNOWN_OPTION, CURLINFO, CURLM, CURLU, CURLUPART_URL,
};
use crate::dns::{self, ConnectOverride, ResolveOverride, ResolverOwner};
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
pub(crate) type CurlSeekCallback = crate::abi::curl_seek_callback;
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
const CURLOPT_APPEND: CURLoption = 50;
const CURLOPT_INTERLEAVEDATA: CURLoption = 10195;
const CURLOPT_SEEKFUNCTION: CURLoption = 20167;
const CURLOPT_SEEKDATA: CURLoption = 10168;
const CURLOPT_COOKIE: CURLoption = 10022;
const CURLOPT_HTTPPOST: CURLoption = 10024;
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
const CURLOPT_DIRLISTONLY: CURLoption = 48;
const CURLOPT_NETRC: CURLoption = 51;
const CURLOPT_FOLLOWLOCATION: CURLoption = 52;
const CURLOPT_TRANSFERTEXT: CURLoption = 53;
const CURLOPT_AUTOREFERER: CURLoption = 58;
const CURLOPT_XFERINFODATA: CURLoption = 10057;
const CURLOPT_PROXYPORT: CURLoption = 59;
const CURLOPT_HTTPPROXYTUNNEL: CURLoption = 61;
const CURLOPT_HTTP_VERSION: CURLoption = 84;
const CURLOPT_TIMEOUT_MS: CURLoption = 155;
const CURLOPT_POSTREDIR: CURLoption = 161;
const CURLOPT_SSL_VERIFYPEER: CURLoption = 64;
const CURLOPT_MAXREDIRS: CURLoption = 68;
const CURLOPT_MAXCONNECTS: CURLoption = 71;
const CURLOPT_BUFFERSIZE: CURLoption = 98;
const CURLOPT_CERTINFO: CURLoption = 172;
const CURLOPT_HEADERFUNCTION: CURLoption = 20079;
const CURLOPT_HTTPGET: CURLoption = 80;
const CURLOPT_COOKIEJAR: CURLoption = 10082;
const CURLOPT_SSL_VERIFYHOST: CURLoption = 81;
const CURLOPT_COOKIESESSION: CURLoption = 96;
const CURLOPT_SHARE: CURLoption = 10100;
const CURLOPT_PRIVATE: CURLoption = 10103;
const CURLOPT_UNRESTRICTED_AUTH: CURLoption = 105;
const CURLOPT_HTTPAUTH: CURLoption = 107;
const CURLOPT_PROXYAUTH: CURLoption = 111;
const CURLOPT_NETRC_FILE: CURLoption = 10118;
const CURLOPT_COOKIELIST: CURLoption = 10135;
const CURLOPT_INFILESIZE_LARGE: CURLoption = 30115;
const CURLOPT_RESUME_FROM_LARGE: CURLoption = 30116;
const CURLOPT_CONNECT_ONLY: CURLoption = 141;
const CURLOPT_OPENSOCKETDATA: CURLoption = 10164;
const CURLOPT_TCP_NODELAY: CURLoption = 121;
const CURLOPT_USERNAME: CURLoption = 10173;
const CURLOPT_PASSWORD: CURLoption = 10174;
const CURLOPT_PROXYUSERNAME: CURLoption = 10175;
const CURLOPT_PROXYPASSWORD: CURLoption = 10176;
const CURLOPT_NOPROXY: CURLoption = 10177;
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
const CURLOPT_SUPPRESS_CONNECT_HEADERS: CURLoption = 265;
const CURLOPT_REQUEST_TARGET: CURLoption = 10266;
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
const CURLOPT_AWS_SIGV4: CURLoption = 10305;
const CURLOPT_HTTP09_ALLOWED: CURLoption = 285;
const CURLOPT_MIMEPOST: CURLoption = 10269;
const CURLOPT_OPENSOCKETFUNCTION: CURLoption = 20163;
const CURLOPT_RTSP_REQUEST: CURLoption = 189;
const CURLOPT_CLOSESOCKETFUNCTION: CURLoption = 20208;
const CURLOPT_CLOSESOCKETDATA: CURLoption = 10209;
const CURLOPT_QUICK_EXIT: CURLoption = 322;

const CURLINFO_EFFECTIVE_URL: u32 = 0x100000 + 1;
const CURLINFO_RESPONSE_CODE: u32 = 0x200000 + 2;
const CURLINFO_TOTAL_TIME: u32 = 0x300000 + 3;
const CURLINFO_NAMELOOKUP_TIME: u32 = 0x300000 + 4;
const CURLINFO_CONNECT_TIME: u32 = 0x300000 + 5;
const CURLINFO_PRETRANSFER_TIME: u32 = 0x300000 + 6;
const CURLINFO_SSL_VERIFYRESULT: u32 = 0x200000 + 13;
const CURLINFO_FILETIME: u32 = 0x200000 + 14;
const CURLINFO_STARTTRANSFER_TIME: u32 = 0x300000 + 17;
const CURLINFO_CONTENT_TYPE: u32 = 0x100000 + 18;
const CURLINFO_REDIRECT_TIME: u32 = 0x300000 + 19;
const CURLINFO_REDIRECT_COUNT: u32 = 0x200000 + 20;
const CURLINFO_HTTP_CONNECTCODE: u32 = 0x200000 + 22;
const CURLINFO_OS_ERRNO: u32 = 0x200000 + 25;
const CURLINFO_NUM_CONNECTS: u32 = 0x200000 + 26;
const CURLINFO_LASTSOCKET: u32 = 0x200000 + 29;
const CURLINFO_REDIRECT_URL: u32 = 0x100000 + 31;
const CURLINFO_APPCONNECT_TIME: u32 = 0x300000 + 33;
const CURLINFO_CONDITION_UNMET: u32 = 0x200000 + 35;
const CURLINFO_PRIMARY_IP: u32 = 0x100000 + 32;
const CURLINFO_PRIMARY_PORT: u32 = 0x200000 + 40;
const CURLINFO_LOCAL_IP: u32 = 0x100000 + 41;
const CURLINFO_LOCAL_PORT: u32 = 0x200000 + 42;
const CURLINFO_COOKIELIST: u32 = 0x400000 + 28;
const CURLINFO_CERTINFO: u32 = 0x400000 + 34;
const CURLINFO_TLS_SESSION: u32 = 0x400000 + 43;
const CURLINFO_PRIVATE: u32 = 0x100000 + 21;
const CURLINFO_TLS_SSL_PTR: u32 = 0x400000 + 45;
const CURLINFO_HTTP_VERSION: u32 = 0x200000 + 46;
const CURLINFO_PROXY_SSL_VERIFYRESULT: u32 = 0x200000 + 47;
const CURLINFO_PROTOCOL: u32 = 0x200000 + 48;
const CURLINFO_SCHEME: u32 = 0x100000 + 49;
const CURLINFO_RTSP_SESSION_ID: u32 = 0x100000 + 36;
const CURLINFO_FILETIME_T: u32 = 0x600000 + 14;
const CURLINFO_TOTAL_TIME_T: u32 = 0x600000 + 50;
const CURLINFO_NAMELOOKUP_TIME_T: u32 = 0x600000 + 51;
const CURLINFO_CONNECT_TIME_T: u32 = 0x600000 + 52;
const CURLINFO_PRETRANSFER_TIME_T: u32 = 0x600000 + 53;
const CURLINFO_STARTTRANSFER_TIME_T: u32 = 0x600000 + 54;
const CURLINFO_REDIRECT_TIME_T: u32 = 0x600000 + 55;
const CURLINFO_APPCONNECT_TIME_T: u32 = 0x600000 + 56;
const CURLINFO_RETRY_AFTER: u32 = 0x600000 + 57;
const CURLINFO_EFFECTIVE_METHOD: u32 = 0x100000 + 58;
const CURLINFO_REFERER: u32 = 0x100000 + 60;
const CURL_ERROR_SIZE: usize = 256;

const CURL_HTTP_VERSION_1_0: c_long = 1;
const CURL_HTTP_VERSION_1_1: c_long = 2;

const CURLPROTO_HTTP: c_long = 1 << 0;
const CURLPROTO_HTTPS: c_long = 1 << 1;
const CURLPROTO_FTP: c_long = 1 << 2;
const CURLPROTO_FTPS: c_long = 1 << 3;
const CURLPROTO_SCP: c_long = 1 << 4;
const CURLPROTO_SFTP: c_long = 1 << 5;
const CURLPROTO_TELNET: c_long = 1 << 6;
const CURLPROTO_LDAP: c_long = 1 << 7;
const CURLPROTO_LDAPS: c_long = 1 << 8;
const CURLPROTO_DICT: c_long = 1 << 9;
const CURLPROTO_FILE: c_long = 1 << 10;
const CURLPROTO_TFTP: c_long = 1 << 11;
const CURLPROTO_IMAP: c_long = 1 << 12;
const CURLPROTO_IMAPS: c_long = 1 << 13;
const CURLPROTO_POP3: c_long = 1 << 14;
const CURLPROTO_POP3S: c_long = 1 << 15;
const CURLPROTO_SMTP: c_long = 1 << 16;
const CURLPROTO_SMTPS: c_long = 1 << 17;
const CURLPROTO_RTSP: c_long = 1 << 18;
const CURLPROTO_GOPHER: c_long = 1 << 25;
const CURLPROTO_SMB: c_long = 1 << 26;
const CURLPROTO_SMBS: c_long = 1 << 27;
const CURLPROTO_MQTT: c_long = 1 << 28;

#[derive(Clone)]
pub(crate) struct EasyMetadata {
    pub url: Option<String>,
    pub custom_request: Option<String>,
    pub http_headers: Vec<String>,
    pub proxy_headers: Vec<String>,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
    pub range: Option<String>,
    pub request_target: Option<String>,
    pub resolve_overrides: Vec<ResolveOverride>,
    pub connect_overrides: Vec<ConnectOverride>,
    pub proxy: Option<String>,
    pub no_proxy: Option<String>,
    pub pre_proxy: Option<String>,
    pub proxy_port: Option<u16>,
    pub tunnel_proxy: bool,
    pub suppress_connect_headers: bool,
    pub share_handle: Option<usize>,
    pub userpwd: Option<String>,
    pub proxy_userpwd: Option<String>,
    pub private_data: usize,
    pub username: Option<String>,
    pub password: Option<String>,
    pub proxy_username: Option<String>,
    pub proxy_password: Option<String>,
    pub xoauth2_bearer: Option<String>,
    pub aws_sigv4: Option<String>,
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
    pub postredir: c_long,
    pub httpauth: c_long,
    pub proxyauth: c_long,
    pub transfer_text: bool,
    pub dirlistonly: bool,
    pub append: bool,
    pub headeropt: c_long,
    pub connect_mode: c_long,
    pub ws_options: c_long,
    pub quick_exit: bool,
    pub curlu_handle: Option<usize>,
    pub mimepost_handle: Option<usize>,
    pub httppost_handle: Option<usize>,
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
    pub tcp_nodelay: bool,
    pub follow_location: bool,
    pub header: bool,
    pub nobody: bool,
    pub upload: bool,
    pub upload_size: Option<curl_off_t>,
    pub http_get: bool,
    pub http_version: c_long,
    pub http09_allowed: bool,
    pub verbose: bool,
    pub fail_on_error: bool,
    pub timeout_ms: c_long,
    pub buffer_size: c_long,
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
            request_target: None,
            resolve_overrides: Vec::new(),
            connect_overrides: Vec::new(),
            proxy: None,
            no_proxy: None,
            pre_proxy: None,
            proxy_port: None,
            tunnel_proxy: false,
            suppress_connect_headers: false,
            share_handle: None,
            userpwd: None,
            proxy_userpwd: None,
            private_data: 0,
            username: None,
            password: None,
            proxy_username: None,
            proxy_password: None,
            xoauth2_bearer: None,
            aws_sigv4: None,
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
            postredir: 0,
            httpauth: 0,
            proxyauth: 0,
            transfer_text: false,
            dirlistonly: false,
            append: false,
            headeropt: 0,
            connect_mode: 0,
            ws_options: 0,
            quick_exit: false,
            curlu_handle: None,
            mimepost_handle: None,
            httppost_handle: None,
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
            tcp_nodelay: true,
            follow_location: false,
            header: false,
            nobody: false,
            upload: false,
            upload_size: None,
            http_get: false,
            http_version: 0,
            http09_allowed: false,
            verbose: false,
            fail_on_error: false,
            timeout_ms: 0,
            buffer_size: 0,
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
    pub interleave_data: usize,
    pub seek_function: CurlSeekCallback,
    pub seek_data: usize,
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

#[derive(Clone)]
struct EasyInfo {
    response_code: c_long,
    effective_url: Option<CString>,
    content_type: Option<CString>,
    redirect_url: Option<CString>,
    effective_method: Option<CString>,
    referer: Option<CString>,
    primary_ip: Option<CString>,
    primary_port: c_long,
    local_ip: Option<CString>,
    local_port: c_long,
    rtsp_session_id: Option<CString>,
    total_time_us: curl_off_t,
    namelookup_time_us: curl_off_t,
    connect_time_us: curl_off_t,
    appconnect_time_us: curl_off_t,
    pretransfer_time_us: curl_off_t,
    starttransfer_time_us: curl_off_t,
    redirect_time_us: curl_off_t,
    retry_after: curl_off_t,
    retry_after_set: bool,
    redirect_count: c_long,
    ssl_verify_result: c_long,
    proxy_ssl_verify_result: c_long,
    http_connect_code: c_long,
    os_errno: c_long,
    num_connects: c_long,
    http_version: c_long,
    protocol: c_long,
    filetime: curl_off_t,
}

impl Default for EasyInfo {
    fn default() -> Self {
        Self {
            response_code: 0,
            effective_url: None,
            content_type: None,
            redirect_url: None,
            effective_method: None,
            referer: None,
            primary_ip: None,
            primary_port: 0,
            local_ip: None,
            local_port: 0,
            rtsp_session_id: None,
            total_time_us: 0,
            namelookup_time_us: 0,
            connect_time_us: 0,
            appconnect_time_us: 0,
            pretransfer_time_us: 0,
            starttransfer_time_us: 0,
            redirect_time_us: 0,
            retry_after: 0,
            retry_after_set: false,
            redirect_count: 0,
            ssl_verify_result: 0,
            proxy_ssl_verify_result: 0,
            http_connect_code: 0,
            os_errno: 0,
            num_connects: 0,
            http_version: 0,
            protocol: 0,
            filetime: -1,
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
struct StoredTlsSessionInfo {
    backend: crate::abi::curl_sslbackend,
    internals: *mut c_void,
}

unsafe impl Send for StoredTlsSessionInfo {}

#[derive(Clone)]
pub(crate) struct RecordedTransferInfo {
    pub response_code: c_long,
    pub effective_url: Option<String>,
    pub content_type: Option<String>,
    pub redirect_url: Option<String>,
    pub effective_method: Option<String>,
    pub referer: Option<String>,
    pub primary_ip: Option<String>,
    pub primary_port: Option<u16>,
    pub local_ip: Option<String>,
    pub local_port: Option<u16>,
    pub total_time_us: curl_off_t,
    pub namelookup_time_us: curl_off_t,
    pub connect_time_us: curl_off_t,
    pub appconnect_time_us: curl_off_t,
    pub pretransfer_time_us: curl_off_t,
    pub starttransfer_time_us: curl_off_t,
    pub redirect_time_us: curl_off_t,
    pub retry_after: Option<curl_off_t>,
    pub redirect_count: c_long,
    pub ssl_verify_result: c_long,
    pub proxy_ssl_verify_result: c_long,
    pub http_connect_code: c_long,
    pub os_errno: c_long,
    pub num_connects: c_long,
    pub http_version: c_long,
    pub protocol: c_long,
    pub filetime: curl_off_t,
}

impl Default for RecordedTransferInfo {
    fn default() -> Self {
        Self {
            response_code: 0,
            effective_url: None,
            content_type: None,
            redirect_url: None,
            effective_method: None,
            referer: None,
            primary_ip: None,
            primary_port: None,
            local_ip: None,
            local_port: None,
            total_time_us: 0,
            namelookup_time_us: 0,
            connect_time_us: 0,
            appconnect_time_us: 0,
            pretransfer_time_us: 0,
            starttransfer_time_us: 0,
            redirect_time_us: 0,
            retry_after: None,
            redirect_count: 0,
            ssl_verify_result: 0,
            proxy_ssl_verify_result: 0,
            http_connect_code: 0,
            os_errno: 0,
            num_connects: 0,
            http_version: 0,
            protocol: 0,
            filetime: -1,
        }
    }
}

#[derive(Clone)]
struct EasyShadow {
    private_multi: Option<usize>,
    attached_multi: Option<usize>,
    metadata: EasyMetadata,
    cached_multi_plan: Option<crate::transfer::TransferPlan>,
    callbacks: EasyCallbacks,
    info: EasyInfo,
    tls_session_info: Box<StoredTlsSessionInfo>,
    http_state: crate::http::HandleHttpState,
    state: MultiState,
}

impl Default for EasyShadow {
    fn default() -> Self {
        Self {
            private_multi: None,
            attached_multi: None,
            metadata: EasyMetadata::default(),
            cached_multi_plan: None,
            callbacks: EasyCallbacks::default(),
            info: EasyInfo::default(),
            tls_session_info: Box::new(default_tls_session_info()),
            http_state: crate::http::HandleHttpState::default(),
            state: MultiState::Init,
        }
    }
}

fn default_tls_session_info() -> StoredTlsSessionInfo {
    StoredTlsSessionInfo {
        backend: crate::global::compiled_ssl_backend_id(),
        internals: ptr::null_mut(),
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
                cached_multi_plan: None,
                callbacks: shadow.callbacks,
                info: EasyInfo::default(),
                tls_session_info: shadow.tls_session_info,
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

fn with_shadow_mut<R>(
    handle: *mut CURL,
    f: impl FnOnce(&mut EasyShadow) -> R,
) -> Result<R, CURLcode> {
    if handle.is_null() || !crate::easy::handle::is_public_handle(handle) {
        return Err(CURLE_BAD_FUNCTION_ARGUMENT);
    }

    let mut guard = registry().lock().expect("easy registry mutex poisoned");
    let shadow = guard.entry(handle as usize).or_default();
    shadow.cached_multi_plan = None;
    Ok(f(shadow))
}

fn flush_cookie_store(shadow: &EasyShadow) {
    let path = shadow
        .metadata
        .cookie_jar
        .clone()
        .or_else(|| shadow.metadata.cookie_file.clone());
    let Some(path) = path else {
        return;
    };

    if crate::share::with_shared_cookies(shadow.metadata.share_handle, |store| {
        store.flush_to_path(&path)
    })
    .is_some()
    {
        return;
    }

    let _ = shadow.http_state.cookies.flush_to_path(&path);
}

fn apply_cookie_list_item(shadow: &mut EasyShadow, item: String) {
    if item.eq_ignore_ascii_case("FLUSH") {
        flush_cookie_store(shadow);
        return;
    }

    if let Some(value) = item.strip_prefix("Set-Cookie:").map(str::trim) {
        if let Some(current_url) = shadow.metadata.url.as_deref() {
            shadow
                .http_state
                .cookies
                .store_set_cookie(current_url, value);
            let _ = crate::share::with_shared_cookies_mut(shadow.metadata.share_handle, |store| {
                store.store_set_cookie(current_url, value);
            });
        }
    }

    shadow.metadata.cookie_list.push(item);
}

pub(crate) fn easy_setopt_long(handle: *mut CURL, option: CURLoption, value: c_long) -> CURLcode {
    match with_shadow_mut(handle, |shadow| match option {
        CURLOPT_INFILESIZE => {
            shadow.metadata.upload_size = (value >= 0).then_some(value as i64);
            CURLE_OK
        }
        CURLOPT_MAXCONNECTS => {
            shadow.metadata.maxconnects = Some(value);
            CURLE_OK
        }
        CURLOPT_CONNECT_ONLY => {
            shadow.metadata.connect_only = value != 0;
            shadow.metadata.connect_mode = value;
            CURLE_OK
        }
        CURLOPT_TCP_NODELAY => {
            shadow.metadata.tcp_nodelay = value != 0;
            CURLE_OK
        }
        CURLOPT_LOW_SPEED_LIMIT => {
            shadow.metadata.low_speed.limit_bytes_per_second = value;
            CURLE_OK
        }
        CURLOPT_LOW_SPEED_TIME => {
            shadow.metadata.low_speed.time_window_secs = value;
            CURLE_OK
        }
        CURLOPT_RESUME_FROM => {
            shadow.metadata.resume_from = value as i64;
            CURLE_OK
        }
        CURLOPT_HEADER => {
            shadow.metadata.header = value != 0;
            CURLE_OK
        }
        CURLOPT_VERBOSE => {
            shadow.metadata.verbose = value != 0;
            CURLE_OK
        }
        CURLOPT_NOPROGRESS => {
            shadow.callbacks.no_progress = value != 0;
            CURLE_OK
        }
        CURLOPT_NOBODY => {
            shadow.metadata.nobody = value != 0;
            CURLE_OK
        }
        CURLOPT_FAILONERROR => {
            shadow.metadata.fail_on_error = value != 0;
            CURLE_OK
        }
        CURLOPT_UPLOAD => {
            shadow.metadata.upload = value != 0;
            CURLE_OK
        }
        CURLOPT_DIRLISTONLY => {
            shadow.metadata.dirlistonly = value != 0;
            CURLE_OK
        }
        CURLOPT_APPEND => {
            shadow.metadata.append = value != 0;
            CURLE_OK
        }
        CURLOPT_NETRC => {
            shadow.metadata.netrc_mode = value;
            CURLE_OK
        }
        CURLOPT_FOLLOWLOCATION => {
            shadow.metadata.follow_location = value != 0;
            CURLE_OK
        }
        CURLOPT_TRANSFERTEXT => {
            shadow.metadata.transfer_text = value != 0;
            CURLE_OK
        }
        CURLOPT_AUTOREFERER => {
            shadow.metadata.auto_referer = value != 0;
            CURLE_OK
        }
        CURLOPT_HTTPGET => {
            shadow.metadata.http_get = value != 0;
            CURLE_OK
        }
        CURLOPT_HTTP_VERSION => {
            shadow.metadata.http_version = value;
            CURLE_OK
        }
        CURLOPT_PROXYPORT => {
            shadow.metadata.proxy_port = u16::try_from(value).ok();
            CURLE_OK
        }
        CURLOPT_HTTPPROXYTUNNEL => {
            shadow.metadata.tunnel_proxy = value != 0;
            CURLE_OK
        }
        CURLOPT_BUFFERSIZE => {
            shadow.metadata.buffer_size = value;
            CURLE_OK
        }
        CURLOPT_MAXREDIRS => {
            shadow.metadata.max_redirs = Some(value);
            CURLE_OK
        }
        CURLOPT_POSTREDIR => {
            shadow.metadata.postredir = value;
            CURLE_OK
        }
        CURLOPT_COOKIESESSION => {
            shadow.metadata.cookie_session = value != 0;
            CURLE_OK
        }
        CURLOPT_CERTINFO => {
            shadow.metadata.certinfo = value != 0;
            CURLE_OK
        }
        CURLOPT_SSL_VERIFYPEER => {
            shadow.metadata.ssl_verify_peer = value != 0;
            CURLE_OK
        }
        CURLOPT_SSL_VERIFYHOST => {
            shadow.metadata.ssl_verify_host = value;
            CURLE_OK
        }
        CURLOPT_SSL_ENABLE_ALPN => {
            shadow.metadata.ssl_enable_alpn = value != 0;
            CURLE_OK
        }
        CURLOPT_UNRESTRICTED_AUTH => {
            shadow.metadata.unrestricted_auth = value != 0;
            CURLE_OK
        }
        CURLOPT_HTTPAUTH => {
            shadow.metadata.httpauth = value;
            CURLE_OK
        }
        CURLOPT_PROXYAUTH => {
            shadow.metadata.proxyauth = value;
            CURLE_OK
        }
        CURLOPT_RTSP_REQUEST => {
            shadow.metadata.rtsp_request = value;
            CURLE_OK
        }
        CURLOPT_HEADEROPT => {
            shadow.metadata.headeropt = value;
            CURLE_OK
        }
        CURLOPT_TIMEOUT_MS => {
            shadow.metadata.timeout_ms = value;
            CURLE_OK
        }
        CURLOPT_SUPPRESS_CONNECT_HEADERS => {
            shadow.metadata.suppress_connect_headers = value != 0;
            CURLE_OK
        }
        CURLOPT_HTTP09_ALLOWED => {
            shadow.metadata.http09_allowed = value != 0;
            CURLE_OK
        }
        CURLOPT_ALTSVC_CTRL => {
            shadow.metadata.altsvc_ctrl = value;
            shadow.http_state.altsvc.ctrl_bits = value;
            shadow.http_state.altsvc.enabled = value != 0;
            CURLE_OK
        }
        CURLOPT_HSTS_CTRL => {
            shadow.metadata.hsts_ctrl = value;
            CURLE_OK
        }
        CURLOPT_WS_OPTIONS => {
            shadow.metadata.ws_options = value;
            CURLE_OK
        }
        CURLOPT_QUICK_EXIT => {
            shadow.metadata.quick_exit = value != 0;
            CURLE_OK
        }
        _ => CURLE_UNKNOWN_OPTION,
    }) {
        Ok(code) => code,
        Err(code) => code,
    }
}

pub(crate) fn easy_setopt_ptr(
    handle: *mut CURL,
    option: CURLoption,
    value: *mut c_void,
) -> CURLcode {
    match with_shadow_mut(handle, |shadow| match option {
        CURLOPT_READDATA => {
            shadow.callbacks.read_data = value as usize;
            CURLE_OK
        }
        CURLOPT_WRITEDATA => {
            shadow.callbacks.write_data = value as usize;
            CURLE_OK
        }
        CURLOPT_INTERLEAVEDATA => {
            shadow.callbacks.interleave_data = value as usize;
            CURLE_OK
        }
        CURLOPT_URL => {
            shadow.metadata.url = copy_c_string(value.cast());
            shadow.metadata.curlu_handle = None;
            CURLE_OK
        }
        CURLOPT_PROXY => {
            shadow.metadata.proxy = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_NOPROXY => {
            shadow.metadata.no_proxy = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_USERPWD => {
            shadow.metadata.userpwd = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_PROXYUSERPWD => {
            shadow.metadata.proxy_userpwd = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_RANGE => {
            shadow.metadata.range = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_REQUEST_TARGET => {
            shadow.metadata.request_target = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_REFERER => {
            shadow.metadata.referer = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_USERAGENT => {
            shadow.metadata.user_agent = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_COOKIE => {
            shadow.metadata.cookie = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_HTTPPOST => {
            shadow.metadata.httppost_handle = (!value.is_null()).then_some(value as usize);
            CURLE_OK
        }
        CURLOPT_HTTPHEADER => {
            shadow.metadata.http_headers = collect_slist_strings(value.cast());
            CURLE_OK
        }
        CURLOPT_PROXYHEADER => {
            shadow.metadata.proxy_headers = collect_slist_strings(value.cast());
            CURLE_OK
        }
        CURLOPT_COOKIEFILE => {
            shadow.metadata.cookie_file = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_ERRORBUFFER => {
            shadow.callbacks.error_buffer = value as usize;
            CURLE_OK
        }
        CURLOPT_HEADERDATA => {
            shadow.callbacks.header_data = value as usize;
            CURLE_OK
        }
        CURLOPT_CUSTOMREQUEST => {
            shadow.metadata.custom_request = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_TRAILERDATA => {
            shadow.callbacks.trailer_data = value as usize;
            CURLE_OK
        }
        CURLOPT_SEEKDATA => {
            shadow.callbacks.seek_data = value as usize;
            CURLE_OK
        }
        CURLOPT_XFERINFODATA => {
            shadow.callbacks.xferinfo_data = value as usize;
            CURLE_OK
        }
        CURLOPT_SHARE => {
            if !value.is_null() && !crate::share::is_public_handle(value.cast()) {
                return CURLE_BAD_FUNCTION_ARGUMENT;
            }
            shadow.metadata.share_handle = (!value.is_null()).then_some(value as usize);
            CURLE_OK
        }
        CURLOPT_PRIVATE => {
            shadow.metadata.private_data = value as usize;
            CURLE_OK
        }
        CURLOPT_OPENSOCKETDATA => {
            shadow.callbacks.open_socket_data = value as usize;
            CURLE_OK
        }
        CURLOPT_USERNAME => {
            shadow.metadata.username = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_PASSWORD => {
            shadow.metadata.password = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_PROXYUSERNAME => {
            shadow.metadata.proxy_username = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_PROXYPASSWORD => {
            shadow.metadata.proxy_password = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_COOKIEJAR => {
            shadow.metadata.cookie_jar = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_NETRC_FILE => {
            shadow.metadata.netrc_file = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_COOKIELIST => {
            if let Some(item) = copy_c_string(value.cast()) {
                apply_cookie_list_item(shadow, item);
            }
            CURLE_OK
        }
        CURLOPT_RESOLVE => {
            shadow.metadata.resolve_overrides = dns::collect_resolve_overrides(value.cast());
            CURLE_OK
        }
        CURLOPT_XOAUTH2_BEARER => {
            shadow.metadata.xoauth2_bearer = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_AWS_SIGV4 => {
            shadow.metadata.aws_sigv4 = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_PINNEDPUBLICKEY => {
            shadow.metadata.pinned_public_key = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_DOH_URL => {
            shadow.metadata.doh_url = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_RTSP_SESSION_ID => {
            shadow.metadata.rtsp_session_id = copy_c_string(value.cast());
            shadow.info.rtsp_session_id = shadow
                .metadata
                .rtsp_session_id
                .clone()
                .and_then(to_c_string);
            CURLE_OK
        }
        CURLOPT_RTSP_STREAM_URI => {
            shadow.metadata.rtsp_stream_uri = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_RTSP_TRANSPORT => {
            shadow.metadata.rtsp_transport = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_CONNECT_TO => {
            shadow.metadata.connect_overrides = dns::collect_connect_overrides(value.cast());
            CURLE_OK
        }
        CURLOPT_PRE_PROXY => {
            shadow.metadata.pre_proxy = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_ALTSVC => {
            shadow.metadata.altsvc_file = copy_c_string(value.cast());
            shadow.http_state.altsvc.path = shadow.metadata.altsvc_file.clone();
            CURLE_OK
        }
        CURLOPT_MIMEPOST => {
            shadow.metadata.mimepost_handle = (!value.is_null()).then_some(value as usize);
            CURLE_OK
        }
        CURLOPT_CURLU => {
            if value.is_null() {
                shadow.metadata.curlu_handle = None;
                shadow.metadata.url = None;
                return CURLE_OK;
            }
            let Some(url) = copy_url_from_curlu(value.cast::<CURLU>()) else {
                return CURLE_BAD_FUNCTION_ARGUMENT;
            };
            shadow.metadata.curlu_handle = Some(value as usize);
            shadow.metadata.url = Some(url);
            CURLE_OK
        }
        CURLOPT_HSTS => {
            shadow.metadata.hsts_file = copy_c_string(value.cast());
            CURLE_OK
        }
        CURLOPT_HSTSREADDATA => {
            shadow.callbacks.hsts_read_data = value as usize;
            CURLE_OK
        }
        CURLOPT_HSTSWRITEDATA => {
            shadow.callbacks.hsts_write_data = value as usize;
            CURLE_OK
        }
        CURLOPT_CLOSESOCKETDATA => {
            shadow.callbacks.close_socket_data = value as usize;
            CURLE_OK
        }
        _ => CURLE_UNKNOWN_OPTION,
    }) {
        Ok(code) => code,
        Err(code) => code,
    }
}

pub(crate) fn easy_setopt_function(
    handle: *mut CURL,
    option: CURLoption,
    value: Option<unsafe extern "C" fn()>,
) -> CURLcode {
    match with_shadow_mut(handle, |shadow| match option {
        CURLOPT_READFUNCTION => {
            shadow.callbacks.read_function = unsafe { core::mem::transmute(value) };
            CURLE_OK
        }
        CURLOPT_SEEKFUNCTION => {
            shadow.callbacks.seek_function = unsafe { core::mem::transmute(value) };
            CURLE_OK
        }
        CURLOPT_TRAILERFUNCTION => {
            shadow.callbacks.trailer_function = unsafe { core::mem::transmute(value) };
            CURLE_OK
        }
        CURLOPT_WRITEFUNCTION => {
            shadow.callbacks.write_function = unsafe { core::mem::transmute(value) };
            CURLE_OK
        }
        CURLOPT_HEADERFUNCTION => {
            shadow.callbacks.header_function = unsafe { core::mem::transmute(value) };
            CURLE_OK
        }
        CURLOPT_XFERINFOFUNCTION => {
            shadow.callbacks.xferinfo_function = unsafe { core::mem::transmute(value) };
            CURLE_OK
        }
        CURLOPT_HSTSREADFUNCTION => {
            shadow.callbacks.hsts_read_function = unsafe { core::mem::transmute(value) };
            CURLE_OK
        }
        CURLOPT_HSTSWRITEFUNCTION => {
            shadow.callbacks.hsts_write_function = unsafe { core::mem::transmute(value) };
            CURLE_OK
        }
        CURLOPT_OPENSOCKETFUNCTION => {
            shadow.callbacks.open_socket_function = unsafe { core::mem::transmute(value) };
            CURLE_OK
        }
        CURLOPT_CLOSESOCKETFUNCTION => {
            shadow.callbacks.close_socket_function = unsafe { core::mem::transmute(value) };
            CURLE_OK
        }
        _ => CURLE_UNKNOWN_OPTION,
    }) {
        Ok(code) => code,
        Err(code) => code,
    }
}

pub(crate) fn easy_setopt_off_t(
    handle: *mut CURL,
    option: CURLoption,
    value: curl_off_t,
) -> CURLcode {
    match with_shadow_mut(handle, |shadow| {
        let metadata = &mut shadow.metadata;
        match option {
            CURLOPT_RESUME_FROM_LARGE => {
                metadata.resume_from = value as i64;
                CURLE_OK
            }
            CURLOPT_INFILESIZE_LARGE => {
                metadata.upload_size = (value >= 0).then_some(value);
                CURLE_OK
            }
            _ => CURLE_UNKNOWN_OPTION,
        }
    }) {
        Ok(code) => code,
        Err(code) => code,
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

pub(crate) fn plan_for_multi(
    handle: *mut CURL,
    metadata: &EasyMetadata,
) -> crate::transfer::TransferPlan {
    if metadata.curlu_handle.is_none() {
        if let Some(plan) = registry()
            .lock()
            .expect("easy registry mutex poisoned")
            .get(&(handle as usize))
            .and_then(|shadow| shadow.cached_multi_plan.clone())
        {
            return plan;
        }
    }

    let plan = crate::transfer::build_plan(metadata, ResolverOwner::Multi);
    if metadata.curlu_handle.is_none() {
        if let Some(shadow) = registry()
            .lock()
            .expect("easy registry mutex poisoned")
            .get_mut(&(handle as usize))
        {
            shadow.cached_multi_plan = Some(plan.clone());
        }
    }
    plan
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
        shadow.info.effective_url = info.effective_url.and_then(to_c_string);
        shadow.info.content_type = info.content_type.and_then(to_c_string);
        shadow.info.redirect_url = info.redirect_url.and_then(to_c_string);
        shadow.info.effective_method = info.effective_method.and_then(to_c_string);
        shadow.info.referer = info.referer.and_then(to_c_string);
        shadow.info.primary_ip = info.primary_ip.and_then(to_c_string);
        shadow.info.primary_port = info.primary_port.map(c_long::from).unwrap_or(0);
        shadow.info.local_ip = info.local_ip.and_then(to_c_string);
        shadow.info.local_port = info.local_port.map(c_long::from).unwrap_or(0);
        shadow.info.total_time_us = info.total_time_us;
        shadow.info.namelookup_time_us = info.namelookup_time_us;
        shadow.info.connect_time_us = info.connect_time_us;
        shadow.info.appconnect_time_us = info.appconnect_time_us;
        shadow.info.pretransfer_time_us = info.pretransfer_time_us;
        shadow.info.starttransfer_time_us = info.starttransfer_time_us;
        shadow.info.redirect_time_us = info.redirect_time_us;
        shadow.info.redirect_count = info.redirect_count;
        shadow.info.ssl_verify_result = info.ssl_verify_result;
        shadow.info.proxy_ssl_verify_result = info.proxy_ssl_verify_result;
        shadow.info.http_connect_code = info.http_connect_code;
        shadow.info.os_errno = info.os_errno;
        shadow.info.num_connects = info.num_connects;
        shadow.info.http_version = info.http_version;
        shadow.info.protocol = info.protocol;
        shadow.info.filetime = info.filetime;
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

fn timing_seconds(micros: curl_off_t) -> f64 {
    micros as f64 / 1_000_000.0
}

fn effective_method_from_metadata(metadata: &EasyMetadata) -> String {
    if let Some(custom_request) = metadata.custom_request.as_ref() {
        return custom_request.clone();
    }
    if metadata.nobody {
        return "HEAD".to_string();
    }
    if metadata.upload {
        return "PUT".to_string();
    }
    if metadata.mimepost_handle.is_some() || metadata.httppost_handle.is_some() {
        return "POST".to_string();
    }
    if metadata.http_get {
        return "GET".to_string();
    }
    "GET".to_string()
}

pub(crate) fn protocol_from_url(url: Option<&str>) -> c_long {
    let Some(url) = url else {
        return 0;
    };
    let Some((scheme, _)) = url.split_once("://") else {
        return 0;
    };
    match scheme.to_ascii_lowercase().as_str() {
        "dict" => CURLPROTO_DICT,
        "file" => CURLPROTO_FILE,
        "ftp" => CURLPROTO_FTP,
        "ftps" => CURLPROTO_FTPS,
        "gopher" => CURLPROTO_GOPHER,
        "http" | "ws" => CURLPROTO_HTTP,
        "https" | "wss" => CURLPROTO_HTTPS,
        "imap" => CURLPROTO_IMAP,
        "imaps" => CURLPROTO_IMAPS,
        "ldap" => CURLPROTO_LDAP,
        "ldaps" => CURLPROTO_LDAPS,
        "mqtt" => CURLPROTO_MQTT,
        "pop3" => CURLPROTO_POP3,
        "pop3s" => CURLPROTO_POP3S,
        "rtsp" => CURLPROTO_RTSP,
        "scp" => CURLPROTO_SCP,
        "smb" => CURLPROTO_SMB,
        "smbs" => CURLPROTO_SMBS,
        "sftp" => CURLPROTO_SFTP,
        "smtp" => CURLPROTO_SMTP,
        "smtps" => CURLPROTO_SMTPS,
        "telnet" => CURLPROTO_TELNET,
        "tftp" => CURLPROTO_TFTP,
        _ => 0,
    }
}

fn scheme_ptr(url: Option<&str>) -> *mut c_char {
    let Some(url) = url else {
        return ptr::null_mut();
    };
    let Some((scheme, _)) = url.split_once("://") else {
        return ptr::null_mut();
    };
    match scheme.to_ascii_lowercase().as_str() {
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
        _ => ptr::null_mut(),
    }
}

fn cookie_lines_from_store(store: &crate::http::cookies::CookieStore) -> Vec<String> {
    store
        .serialize_netscape()
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

fn cookie_lines_for_handle(handle: *mut CURL) -> Vec<String> {
    if handle.is_null() {
        return Vec::new();
    }

    let (share_handle, local_lines) = {
        let guard = registry().lock().expect("easy registry mutex poisoned");
        let Some(shadow) = guard.get(&(handle as usize)) else {
            return Vec::new();
        };
        (
            shadow.metadata.share_handle,
            cookie_lines_from_store(&shadow.http_state.cookies),
        )
    };

    crate::share::with_shared_cookies(share_handle, cookie_lines_from_store).unwrap_or(local_lines)
}

fn build_owned_slist(entries: &[String]) -> Result<*mut curl_slist, CURLcode> {
    let mut list = ptr::null_mut();
    for entry in entries {
        let Ok(entry) = CString::new(entry.as_str()) else {
            unsafe { crate::slist::curl_slist_free_all(list) };
            return Err(CURLE_BAD_FUNCTION_ARGUMENT);
        };
        let next = unsafe { crate::slist::curl_slist_append(list, entry.as_ptr()) };
        if next.is_null() {
            unsafe { crate::slist::curl_slist_free_all(list) };
            return Err(CURLE_OUT_OF_MEMORY);
        }
        list = next;
    }
    Ok(list)
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
    let shadow = guard.get(&(handle as usize));
    let info_values = shadow.map(|shadow| &shadow.info);
    let result = match info {
        CURLINFO_RESPONSE_CODE => info_values.map(|info| info.response_code).unwrap_or(0),
        CURLINFO_SSL_VERIFYRESULT => info_values.map(|info| info.ssl_verify_result).unwrap_or(0),
        CURLINFO_FILETIME => {
            let filetime = info_values.map(|info| info.filetime).unwrap_or(-1);
            filetime.clamp(c_long::MIN as curl_off_t, c_long::MAX as curl_off_t) as c_long
        }
        CURLINFO_REDIRECT_COUNT => info_values.map(|info| info.redirect_count).unwrap_or(0),
        CURLINFO_HTTP_CONNECTCODE => info_values.map(|info| info.http_connect_code).unwrap_or(0),
        CURLINFO_OS_ERRNO => info_values.map(|info| info.os_errno).unwrap_or(0),
        CURLINFO_NUM_CONNECTS => info_values.map(|info| info.num_connects).unwrap_or(0),
        CURLINFO_LASTSOCKET => crate::transfer::active_socket(handle)
            .map(|socket| socket as c_long)
            .unwrap_or(-1),
        CURLINFO_CONDITION_UNMET => info_values
            .map(|info| (info.response_code == 304) as c_long)
            .unwrap_or(0),
        CURLINFO_PRIMARY_PORT => info_values.map(|info| info.primary_port).unwrap_or(0),
        CURLINFO_LOCAL_PORT => info_values.map(|info| info.local_port).unwrap_or(0),
        CURLINFO_HTTP_VERSION => info_values.map(|info| info.http_version).unwrap_or(0),
        CURLINFO_PROXY_SSL_VERIFYRESULT => info_values
            .map(|info| info.proxy_ssl_verify_result)
            .unwrap_or(0),
        CURLINFO_PROTOCOL => info_values
            .map(|info| info.protocol)
            .or_else(|| shadow.map(|shadow| protocol_from_url(shadow.metadata.url.as_deref())))
            .unwrap_or(0),
        _ => return None,
    };
    unsafe { *value = result };
    Some(CURLE_OK)
}

pub(crate) fn easy_getinfo_double(
    handle: *mut CURL,
    info: u32,
    value: *mut f64,
) -> Option<CURLcode> {
    if handle.is_null() || value.is_null() {
        return Some(CURLE_BAD_FUNCTION_ARGUMENT);
    }
    let guard = registry().lock().expect("easy registry mutex poisoned");
    let shadow = guard.get(&(handle as usize));
    let result = match info {
        CURLINFO_TOTAL_TIME => {
            timing_seconds(shadow.map(|shadow| shadow.info.total_time_us).unwrap_or(0))
        }
        CURLINFO_NAMELOOKUP_TIME => timing_seconds(
            shadow
                .map(|shadow| shadow.info.namelookup_time_us)
                .unwrap_or(0),
        ),
        CURLINFO_CONNECT_TIME => timing_seconds(
            shadow
                .map(|shadow| shadow.info.connect_time_us)
                .unwrap_or(0),
        ),
        CURLINFO_APPCONNECT_TIME => timing_seconds(
            shadow
                .map(|shadow| shadow.info.appconnect_time_us)
                .unwrap_or(0),
        ),
        CURLINFO_PRETRANSFER_TIME => timing_seconds(
            shadow
                .map(|shadow| shadow.info.pretransfer_time_us)
                .unwrap_or(0),
        ),
        CURLINFO_STARTTRANSFER_TIME => timing_seconds(
            shadow
                .map(|shadow| shadow.info.starttransfer_time_us)
                .unwrap_or(0),
        ),
        CURLINFO_REDIRECT_TIME => timing_seconds(
            shadow
                .map(|shadow| shadow.info.redirect_time_us)
                .unwrap_or(0),
        ),
        _ => return None,
    };
    unsafe { *value = result };
    Some(CURLE_OK)
}

pub(crate) fn easy_getinfo_string(
    handle: *mut CURL,
    info: u32,
    value: *mut *mut c_char,
) -> Option<CURLcode> {
    if handle.is_null() || value.is_null() {
        return Some(CURLE_BAD_FUNCTION_ARGUMENT);
    }

    let mut guard = registry().lock().expect("easy registry mutex poisoned");
    let shadow = guard.get_mut(&(handle as usize));
    unsafe {
        *value = match info {
            CURLINFO_EFFECTIVE_URL => shadow
                .map(|shadow| {
                    if shadow.info.effective_url.is_none() {
                        shadow.info.effective_url =
                            shadow.metadata.url.clone().and_then(to_c_string);
                    }
                    shadow
                        .info
                        .effective_url
                        .as_ref()
                        .map(|value| value.as_ptr().cast_mut())
                        .unwrap_or(c"".as_ptr().cast_mut())
                })
                .unwrap_or(c"".as_ptr().cast_mut()),
            CURLINFO_CONTENT_TYPE => shadow
                .and_then(|shadow| shadow.info.content_type.as_ref())
                .map(|value| value.as_ptr().cast_mut())
                .unwrap_or(ptr::null_mut()),
            CURLINFO_PRIVATE => shadow
                .map(|shadow| shadow.metadata.private_data as *mut c_char)
                .unwrap_or(ptr::null_mut()),
            CURLINFO_REDIRECT_URL => shadow
                .and_then(|shadow| shadow.info.redirect_url.as_ref())
                .map(|value| value.as_ptr().cast_mut())
                .unwrap_or(ptr::null_mut()),
            CURLINFO_SCHEME => scheme_ptr(shadow.and_then(|shadow| shadow.metadata.url.as_deref())),
            CURLINFO_EFFECTIVE_METHOD => shadow
                .map(|shadow| {
                    if shadow.info.effective_method.is_none() {
                        shadow.info.effective_method =
                            to_c_string(effective_method_from_metadata(&shadow.metadata));
                    }
                    shadow
                        .info
                        .effective_method
                        .as_ref()
                        .map(|value| value.as_ptr().cast_mut())
                        .unwrap_or(ptr::null_mut())
                })
                .unwrap_or(ptr::null_mut()),
            CURLINFO_REFERER => shadow
                .map(|shadow| {
                    if shadow.info.referer.is_none() {
                        shadow.info.referer = shadow.metadata.referer.clone().and_then(to_c_string);
                    }
                    shadow
                        .info
                        .referer
                        .as_ref()
                        .map(|value| value.as_ptr().cast_mut())
                        .unwrap_or(ptr::null_mut())
                })
                .unwrap_or(ptr::null_mut()),
            CURLINFO_PRIMARY_IP => shadow
                .and_then(|shadow| shadow.info.primary_ip.as_ref())
                .map(|value| value.as_ptr().cast_mut())
                .unwrap_or(ptr::null_mut()),
            CURLINFO_LOCAL_IP => shadow
                .and_then(|shadow| shadow.info.local_ip.as_ref())
                .map(|value| value.as_ptr().cast_mut())
                .unwrap_or(ptr::null_mut()),
            CURLINFO_RTSP_SESSION_ID => shadow
                .and_then(|shadow| shadow.info.rtsp_session_id.as_ref())
                .map(|value| value.as_ptr().cast_mut())
                .unwrap_or(ptr::null_mut()),
            _ => return None,
        };
    }
    Some(CURLE_OK)
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
        CURLINFO_FILETIME_T => shadow.map(|shadow| shadow.info.filetime).unwrap_or(-1),
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
        CURLINFO_APPCONNECT_TIME_T => shadow
            .map(|shadow| shadow.info.appconnect_time_us)
            .unwrap_or(0),
        CURLINFO_PRETRANSFER_TIME_T => shadow
            .map(|shadow| shadow.info.pretransfer_time_us)
            .unwrap_or(0),
        CURLINFO_STARTTRANSFER_TIME_T => shadow
            .map(|shadow| shadow.info.starttransfer_time_us)
            .unwrap_or(0),
        CURLINFO_REDIRECT_TIME_T => shadow
            .map(|shadow| shadow.info.redirect_time_us)
            .unwrap_or(0),
        _ => return None,
    };
    unsafe { *value = result };
    Some(CURLE_OK)
}

pub(crate) fn easy_getinfo_slist(
    handle: *mut CURL,
    info: u32,
    value: *mut *mut curl_slist,
) -> Option<CURLcode> {
    if handle.is_null() || value.is_null() {
        return Some(CURLE_BAD_FUNCTION_ARGUMENT);
    }

    if info != CURLINFO_COOKIELIST {
        return None;
    }

    let lines = cookie_lines_for_handle(handle);
    let list = match build_owned_slist(&lines) {
        Ok(list) => list,
        Err(code) => return Some(code),
    };
    unsafe { *value = list };
    Some(CURLE_OK)
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
    Some(CURLE_OK)
}

pub(crate) fn easy_getinfo_ptr(
    handle: *mut CURL,
    info: u32,
    value: *mut *mut c_void,
) -> Option<CURLcode> {
    if handle.is_null() || value.is_null() {
        return Some(CURLE_BAD_FUNCTION_ARGUMENT);
    }
    match info {
        CURLINFO_PRIVATE => {
            unsafe {
                *value = registry()
                    .lock()
                    .expect("easy registry mutex poisoned")
                    .get(&(handle as usize))
                    .map(|shadow| shadow.metadata.private_data as *mut c_void)
                    .unwrap_or(ptr::null_mut());
            }
            Some(CURLE_OK)
        }
        CURLINFO_TLS_SESSION | CURLINFO_TLS_SSL_PTR => {
            unsafe {
                *value = registry()
                    .lock()
                    .expect("easy registry mutex poisoned")
                    .get(&(handle as usize))
                    .map(|shadow| shadow.tls_session_info.as_ref() as *const _ as *mut c_void)
                    .unwrap_or(ptr::null_mut());
            }
            Some(CURLE_OK)
        }
        CURLINFO_CERTINFO => {
            unsafe {
                *value = crate::tls::certinfo::lookup(handle)
                    .map_or(ptr::null_mut(), |certinfo| certinfo.cast::<c_void>());
            }
            Some(CURLE_OK)
        }
        _ => None,
    }
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

    let metadata = snapshot_metadata(handle);
    let plan = plan_for_multi(handle, &metadata);
    if !plan.reference_backend {
        let callbacks = snapshot_callbacks(handle);
        on_transfer_progress(handle, MultiState::Performing);
        let result = crate::transfer::perform_transfer_sync_with(handle, plan, metadata, callbacks);
        on_transfer_progress(handle, MultiState::Done);
        return result;
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
    }

    let _ = unsafe { crate::multi::remove_handle(multi, handle) };
    result
}

pub(crate) unsafe fn easy_pause(handle: *mut CURL, bitmask: c_int) -> CURLcode {
    let ref_rc = unsafe { crate::easy::reference::pause_handle(handle, bitmask) };
    let rc = crate::transfer::pause_handle(handle, bitmask);
    if rc != crate::abi::CURLE_OK {
        return rc;
    }
    if let Some(multi) = attached_multi_for(handle) {
        let _ = unsafe { crate::multi::wakeup_handle(multi as *mut crate::abi::CURLM) };
    }
    if ref_rc == crate::abi::CURLE_OK {
        crate::abi::CURLE_OK
    } else {
        rc
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

fn to_c_string(value: String) -> Option<CString> {
    CString::new(value).ok()
}

fn push_auth_part(parts: &mut Vec<String>, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        parts.push(format!("{label}={value}"));
    }
}
