use crate::abi::{
    curl_calloc_callback, curl_free_callback, curl_malloc_callback, curl_off_t,
    curl_realloc_callback, curl_slist, curl_strdup_callback, CURLcode, CURLoption, CURL,
    CURLE_BAD_FUNCTION_ARGUMENT, CURLE_FAILED_INIT, CURLE_OK, CURL_GLOBAL_DEFAULT,
};
use core::ffi::{c_int, c_long, c_void};
use core::{mem, ptr};
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::{Mutex, OnceLock};

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
const CURLOPT_BUFFERSIZE: CURLoption = 98;
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
const CURLOPT_AWS_SIGV4: CURLoption = 10305;
const CURLOPT_PINNEDPUBLICKEY: CURLoption = 10230;
const CURLOPT_CONNECT_TO: CURLoption = 10243;
const CURLOPT_PRE_PROXY: CURLoption = 10262;
const CURLOPT_HEADEROPT: CURLoption = 229;
const CURLOPT_SUPPRESS_CONNECT_HEADERS: CURLoption = 265;
const CURLOPT_REQUEST_TARGET: CURLoption = 10266;
const CURLOPT_ALTSVC_CTRL: CURLoption = 286;
const CURLOPT_ALTSVC: CURLoption = 10287;
const CURLOPT_HTTP09_ALLOWED: CURLoption = 285;
const CURLOPT_HSTS_CTRL: CURLoption = 299;
const CURLOPT_HSTS: CURLoption = 10300;
const CURLOPT_HSTSREADFUNCTION: CURLoption = 20301;
const CURLOPT_HSTSREADDATA: CURLoption = 10302;
const CURLOPT_HSTSWRITEFUNCTION: CURLoption = 20303;
const CURLOPT_HSTSWRITEDATA: CURLoption = 10304;
const CURLOPT_WS_OPTIONS: CURLoption = 320;
const CURLOPT_QUICK_EXIT: CURLoption = 322;
const CURLOPT_TRAILERFUNCTION: CURLoption = 20283;
const CURLOPT_TRAILERDATA: CURLoption = 10284;
const CURLOPT_SSL_ENABLE_ALPN: CURLoption = 226;
const CURLOPT_DOH_URL: CURLoption = 10279;
const CURLOPT_OPENSOCKETFUNCTION: CURLoption = 20163;
const CURLOPT_RTSP_REQUEST: CURLoption = 189;
const CURLOPT_CLOSESOCKETFUNCTION: CURLoption = 20208;
const CURLOPT_CLOSESOCKETDATA: CURLoption = 10209;

type CurlEasyInitFn = unsafe extern "C" fn() -> *mut CURL;
type CurlEasyCleanupFn = unsafe extern "C" fn(*mut CURL);
type CurlEasyPauseFn = unsafe extern "C" fn(*mut CURL, c_int) -> CURLcode;
type CurlEasySetoptFn = unsafe extern "C" fn(*mut CURL, CURLoption, ...) -> CURLcode;
type CurlGlobalInitMemFn = unsafe extern "C" fn(
    c_long,
    curl_malloc_callback,
    curl_free_callback,
    curl_realloc_callback,
    curl_strdup_callback,
    curl_calloc_callback,
) -> CURLcode;
type CurlGlobalCleanupFn = unsafe extern "C" fn();

struct ActiveReference {
    raw: usize,
    owned_lists: Vec<usize>,
}

#[derive(Default)]
struct ReferenceRegistry {
    by_public: HashMap<usize, ActiveReference>,
    by_reference: HashMap<usize, usize>,
    runtime_initialized: bool,
}

fn registry() -> &'static Mutex<ReferenceRegistry> {
    static REGISTRY: OnceLock<Mutex<ReferenceRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(ReferenceRegistry::default()))
}

fn ref_easy_init() -> CurlEasyInitFn {
    static FN: OnceLock<CurlEasyInitFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { crate::global::load_reference(b"curl_easy_init\0") })
}

fn ref_easy_cleanup() -> CurlEasyCleanupFn {
    static FN: OnceLock<CurlEasyCleanupFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { crate::global::load_reference(b"curl_easy_cleanup\0") })
}

fn ref_easy_pause() -> CurlEasyPauseFn {
    static FN: OnceLock<CurlEasyPauseFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { crate::global::load_reference(b"curl_easy_pause\0") })
}

fn ref_easy_setopt() -> CurlEasySetoptFn {
    static FN: OnceLock<CurlEasySetoptFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { crate::global::load_reference(b"curl_easy_setopt\0") })
}

fn ref_global_init_mem() -> CurlGlobalInitMemFn {
    static FN: OnceLock<CurlGlobalInitMemFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { crate::global::load_reference(b"curl_global_init_mem\0") })
}

fn ref_global_cleanup() -> CurlGlobalCleanupFn {
    static FN: OnceLock<CurlGlobalCleanupFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { crate::global::load_reference(b"curl_global_cleanup\0") })
}

fn render_host_field(host: &str) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    }
}

fn render_resolve_override(entry: &crate::dns::ResolveOverride) -> String {
    let prefix = if entry.transient {
        "+"
    } else if entry.remove {
        "-"
    } else {
        ""
    };
    let addresses = if entry.remove {
        String::new()
    } else {
        entry.addresses.join(",")
    };
    format!(
        "{prefix}{}:{}:{addresses}",
        render_host_field(&entry.host),
        entry.port
    )
}

fn render_connect_override(entry: &crate::dns::ConnectOverride) -> String {
    let source_host = entry
        .source_host
        .as_deref()
        .map(render_host_field)
        .unwrap_or_default();
    let source_port = entry
        .source_port
        .map(|value| value.to_string())
        .unwrap_or_default();
    let target_host = entry
        .target_host
        .as_deref()
        .map(render_host_field)
        .unwrap_or_default();
    let target_port = entry
        .target_port
        .map(|value| value.to_string())
        .unwrap_or_default();
    format!("{source_host}:{source_port}:{target_host}:{target_port}")
}

fn ensure_runtime_initialized(registry: &mut ReferenceRegistry) -> Result<(), CURLcode> {
    if registry.runtime_initialized {
        return Ok(());
    }
    let alloc = crate::alloc::snapshot();
    let code = unsafe {
        ref_global_init_mem()(
            CURL_GLOBAL_DEFAULT,
            alloc.malloc,
            alloc.free,
            alloc.realloc,
            alloc.strdup,
            alloc.calloc,
        )
    };
    if code == CURLE_OK {
        registry.runtime_initialized = true;
        Ok(())
    } else {
        Err(code)
    }
}

fn insert_active(registry: &mut ReferenceRegistry, public: *mut CURL, active: ActiveReference) {
    registry.by_reference.insert(active.raw, public as usize);
    registry.by_public.insert(public as usize, active);
}

fn take_active(registry: &mut ReferenceRegistry, public: *mut CURL) -> Option<ActiveReference> {
    let active = registry.by_public.remove(&(public as usize))?;
    registry.by_reference.remove(&active.raw);
    Some(active)
}

unsafe fn cleanup_active(active: ActiveReference) {
    let raw = active.raw as *mut CURL;
    let owned_lists = active.owned_lists;
    crate::easy::perform::unregister_handle(raw);
    if !raw.is_null() {
        unsafe { ref_easy_cleanup()(raw) };
    }
    for list in owned_lists {
        let list = list as *mut curl_slist;
        if !list.is_null() {
            unsafe { crate::slist::curl_slist_free_all(list) };
        }
    }
}

unsafe fn setopt_long(
    reference: *mut CURL,
    option: CURLoption,
    value: c_long,
) -> Result<(), CURLcode> {
    let code = unsafe { ref_easy_setopt()(reference, option, value) };
    if code == CURLE_OK {
        Ok(())
    } else {
        Err(code)
    }
}

unsafe fn setopt_off_t(
    reference: *mut CURL,
    option: CURLoption,
    value: curl_off_t,
) -> Result<(), CURLcode> {
    let code = unsafe { ref_easy_setopt()(reference, option, value) };
    if code == CURLE_OK {
        Ok(())
    } else {
        Err(code)
    }
}

unsafe fn setopt_ptr(
    reference: *mut CURL,
    option: CURLoption,
    value: *mut c_void,
) -> Result<(), CURLcode> {
    let code = unsafe { ref_easy_setopt()(reference, option, value) };
    if code == CURLE_OK {
        Ok(())
    } else {
        Err(code)
    }
}

unsafe fn setopt_fn(
    reference: *mut CURL,
    option: CURLoption,
    value: Option<unsafe extern "C" fn()>,
) -> Result<(), CURLcode> {
    let code = unsafe { ref_easy_setopt()(reference, option, value) };
    if code == CURLE_OK {
        Ok(())
    } else {
        Err(code)
    }
}

unsafe fn setopt_string(
    reference: *mut CURL,
    option: CURLoption,
    value: Option<&str>,
) -> Result<(), CURLcode> {
    let Some(value) = value else {
        return Ok(());
    };
    let value = CString::new(value).map_err(|_| CURLE_BAD_FUNCTION_ARGUMENT)?;
    unsafe { setopt_ptr(reference, option, value.as_ptr().cast_mut().cast()) }
}

unsafe fn build_slist(entries: &[String]) -> Result<*mut curl_slist, CURLcode> {
    let mut list = ptr::null_mut();
    for entry in entries {
        let entry = CString::new(entry.as_str()).map_err(|_| CURLE_BAD_FUNCTION_ARGUMENT)?;
        let next = unsafe { crate::slist::curl_slist_append(list, entry.as_ptr()) };
        if next.is_null() {
            if !list.is_null() {
                unsafe { crate::slist::curl_slist_free_all(list) };
            }
            return Err(crate::abi::CURLE_OUT_OF_MEMORY);
        }
        list = next;
    }
    Ok(list)
}

unsafe fn configure_reference_handle(
    public: *mut CURL,
    reference: *mut CURL,
) -> Result<Vec<usize>, CURLcode> {
    let metadata = crate::easy::perform::snapshot_metadata(public);
    let callbacks = crate::easy::perform::snapshot_callbacks(public);
    let mut owned_lists = Vec::new();

    unsafe { setopt_string(reference, CURLOPT_URL, metadata.url.as_deref())? };
    unsafe { setopt_string(reference, CURLOPT_PROXY, metadata.proxy.as_deref())? };
    unsafe { setopt_string(reference, CURLOPT_NOPROXY, metadata.no_proxy.as_deref())? };
    unsafe { setopt_string(reference, CURLOPT_PRE_PROXY, metadata.pre_proxy.as_deref())? };
    unsafe { setopt_string(reference, CURLOPT_USERPWD, metadata.userpwd.as_deref())? };
    unsafe {
        setopt_string(
            reference,
            CURLOPT_PROXYUSERPWD,
            metadata.proxy_userpwd.as_deref(),
        )?
    };
    unsafe { setopt_string(reference, CURLOPT_RANGE, metadata.range.as_deref())? };
    unsafe {
        setopt_string(
            reference,
            CURLOPT_REQUEST_TARGET,
            metadata.request_target.as_deref(),
        )?
    };
    unsafe { setopt_string(reference, CURLOPT_REFERER, metadata.referer.as_deref())? };
    unsafe { setopt_string(reference, CURLOPT_USERAGENT, metadata.user_agent.as_deref())? };
    unsafe { setopt_string(reference, CURLOPT_COOKIE, metadata.cookie.as_deref())? };
    unsafe {
        setopt_string(
            reference,
            CURLOPT_COOKIEFILE,
            metadata.cookie_file.as_deref(),
        )?
    };
    unsafe { setopt_string(reference, CURLOPT_COOKIEJAR, metadata.cookie_jar.as_deref())? };
    unsafe {
        setopt_string(
            reference,
            CURLOPT_CUSTOMREQUEST,
            metadata.custom_request.as_deref(),
        )?
    };
    unsafe {
        setopt_string(
            reference,
            CURLOPT_NETRC_FILE,
            metadata.netrc_file.as_deref(),
        )?
    };
    unsafe { setopt_string(reference, CURLOPT_USERNAME, metadata.username.as_deref())? };
    unsafe { setopt_string(reference, CURLOPT_PASSWORD, metadata.password.as_deref())? };
    unsafe {
        setopt_string(
            reference,
            CURLOPT_PROXYUSERNAME,
            metadata.proxy_username.as_deref(),
        )?
    };
    unsafe {
        setopt_string(
            reference,
            CURLOPT_PROXYPASSWORD,
            metadata.proxy_password.as_deref(),
        )?
    };
    unsafe {
        setopt_string(
            reference,
            CURLOPT_XOAUTH2_BEARER,
            metadata.xoauth2_bearer.as_deref(),
        )?
    };
    unsafe { setopt_string(reference, CURLOPT_AWS_SIGV4, metadata.aws_sigv4.as_deref())? };
    unsafe {
        setopt_string(
            reference,
            CURLOPT_PINNEDPUBLICKEY,
            metadata.pinned_public_key.as_deref(),
        )?
    };
    unsafe { setopt_string(reference, CURLOPT_DOH_URL, metadata.doh_url.as_deref())? };
    unsafe {
        setopt_string(
            reference,
            CURLOPT_RTSP_SESSION_ID,
            metadata.rtsp_session_id.as_deref(),
        )?
    };
    unsafe {
        setopt_string(
            reference,
            CURLOPT_RTSP_STREAM_URI,
            metadata.rtsp_stream_uri.as_deref(),
        )?
    };
    unsafe {
        setopt_string(
            reference,
            CURLOPT_RTSP_TRANSPORT,
            metadata.rtsp_transport.as_deref(),
        )?
    };
    unsafe { setopt_string(reference, CURLOPT_ALTSVC, metadata.altsvc_file.as_deref())? };
    unsafe { setopt_string(reference, CURLOPT_HSTS, metadata.hsts_file.as_deref())? };

    if !metadata.http_headers.is_empty() {
        let list = unsafe { build_slist(&metadata.http_headers)? };
        unsafe { setopt_ptr(reference, CURLOPT_HTTPHEADER, list.cast())? };
        owned_lists.push(list as usize);
    }
    if !metadata.proxy_headers.is_empty() {
        let list = unsafe { build_slist(&metadata.proxy_headers)? };
        unsafe { setopt_ptr(reference, CURLOPT_PROXYHEADER, list.cast())? };
        owned_lists.push(list as usize);
    }
    if !metadata.resolve_overrides.is_empty() {
        let entries = metadata
            .resolve_overrides
            .iter()
            .map(render_resolve_override)
            .collect::<Vec<_>>();
        let list = unsafe { build_slist(&entries)? };
        unsafe { setopt_ptr(reference, CURLOPT_RESOLVE, list.cast())? };
        owned_lists.push(list as usize);
    }
    if !metadata.connect_overrides.is_empty() {
        let entries = metadata
            .connect_overrides
            .iter()
            .map(render_connect_override)
            .collect::<Vec<_>>();
        let list = unsafe { build_slist(&entries)? };
        unsafe { setopt_ptr(reference, CURLOPT_CONNECT_TO, list.cast())? };
        owned_lists.push(list as usize);
    }

    if let Some(share_handle) = metadata.share_handle {
        unsafe { setopt_ptr(reference, CURLOPT_SHARE, share_handle as *mut c_void)? };
    }
    if metadata.private_data != 0 {
        unsafe {
            setopt_ptr(
                reference,
                CURLOPT_PRIVATE,
                metadata.private_data as *mut c_void,
            )?
        };
    }
    if callbacks.read_data != 0 {
        unsafe {
            setopt_ptr(
                reference,
                CURLOPT_READDATA,
                callbacks.read_data as *mut c_void,
            )?
        };
    }
    if callbacks.write_data != 0 {
        unsafe {
            setopt_ptr(
                reference,
                CURLOPT_WRITEDATA,
                callbacks.write_data as *mut c_void,
            )?
        };
    }
    if callbacks.interleave_data != 0 {
        unsafe {
            setopt_ptr(
                reference,
                CURLOPT_INTERLEAVEDATA,
                callbacks.interleave_data as *mut c_void,
            )?
        };
    }
    if callbacks.seek_data != 0 {
        unsafe {
            setopt_ptr(
                reference,
                CURLOPT_SEEKDATA,
                callbacks.seek_data as *mut c_void,
            )?
        };
    }
    if callbacks.header_data != 0 {
        unsafe {
            setopt_ptr(
                reference,
                CURLOPT_HEADERDATA,
                callbacks.header_data as *mut c_void,
            )?
        };
    }
    if callbacks.trailer_data != 0 {
        unsafe {
            setopt_ptr(
                reference,
                CURLOPT_TRAILERDATA,
                callbacks.trailer_data as *mut c_void,
            )?
        };
    }
    if callbacks.xferinfo_data != 0 {
        unsafe {
            setopt_ptr(
                reference,
                CURLOPT_XFERINFODATA,
                callbacks.xferinfo_data as *mut c_void,
            )?
        };
    }
    if callbacks.error_buffer != 0 {
        unsafe {
            setopt_ptr(
                reference,
                CURLOPT_ERRORBUFFER,
                callbacks.error_buffer as *mut c_void,
            )?
        };
    }
    if callbacks.open_socket_data != 0 {
        unsafe {
            setopt_ptr(
                reference,
                CURLOPT_OPENSOCKETDATA,
                callbacks.open_socket_data as *mut c_void,
            )?
        };
    }
    if callbacks.hsts_read_data != 0 {
        unsafe {
            setopt_ptr(
                reference,
                CURLOPT_HSTSREADDATA,
                callbacks.hsts_read_data as *mut c_void,
            )?
        };
    }
    if callbacks.hsts_write_data != 0 {
        unsafe {
            setopt_ptr(
                reference,
                CURLOPT_HSTSWRITEDATA,
                callbacks.hsts_write_data as *mut c_void,
            )?
        };
    }
    if callbacks.close_socket_data != 0 {
        unsafe {
            setopt_ptr(
                reference,
                CURLOPT_CLOSESOCKETDATA,
                callbacks.close_socket_data as *mut c_void,
            )?
        };
    }

    if !metadata.cookie_list.is_empty() {
        for entry in &metadata.cookie_list {
            let entry = CString::new(entry.as_str()).map_err(|_| CURLE_BAD_FUNCTION_ARGUMENT)?;
            unsafe {
                setopt_ptr(
                    reference,
                    CURLOPT_COOKIELIST,
                    entry.as_ptr().cast_mut().cast(),
                )?
            };
        }
    }

    unsafe {
        setopt_fn(
            reference,
            CURLOPT_READFUNCTION,
            mem::transmute(callbacks.read_function),
        )?
    };
    unsafe {
        setopt_fn(
            reference,
            CURLOPT_WRITEFUNCTION,
            mem::transmute(callbacks.write_function),
        )?
    };
    unsafe {
        setopt_fn(
            reference,
            CURLOPT_SEEKFUNCTION,
            mem::transmute(callbacks.seek_function),
        )?
    };
    unsafe {
        setopt_fn(
            reference,
            CURLOPT_HEADERFUNCTION,
            mem::transmute(callbacks.header_function),
        )?
    };
    unsafe {
        setopt_fn(
            reference,
            CURLOPT_TRAILERFUNCTION,
            mem::transmute(callbacks.trailer_function),
        )?
    };
    unsafe {
        setopt_fn(
            reference,
            CURLOPT_XFERINFOFUNCTION,
            mem::transmute(callbacks.xferinfo_function),
        )?
    };
    unsafe {
        setopt_fn(
            reference,
            CURLOPT_HSTSREADFUNCTION,
            mem::transmute(callbacks.hsts_read_function),
        )?
    };
    unsafe {
        setopt_fn(
            reference,
            CURLOPT_HSTSWRITEFUNCTION,
            mem::transmute(callbacks.hsts_write_function),
        )?
    };
    unsafe {
        setopt_fn(
            reference,
            CURLOPT_OPENSOCKETFUNCTION,
            mem::transmute(callbacks.open_socket_function),
        )?
    };
    unsafe {
        setopt_fn(
            reference,
            CURLOPT_CLOSESOCKETFUNCTION,
            mem::transmute(callbacks.close_socket_function),
        )?
    };

    unsafe {
        setopt_long(
            reference,
            CURLOPT_LOW_SPEED_LIMIT,
            metadata.low_speed.limit_bytes_per_second,
        )?
    };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_LOW_SPEED_TIME,
            metadata.low_speed.time_window_secs,
        )?
    };
    unsafe { setopt_long(reference, CURLOPT_VERBOSE, metadata.verbose as c_long)? };
    unsafe { setopt_long(reference, CURLOPT_HEADER, metadata.header as c_long)? };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_NOPROGRESS,
            callbacks.no_progress as c_long,
        )?
    };
    unsafe { setopt_long(reference, CURLOPT_NOBODY, metadata.nobody as c_long)? };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_FAILONERROR,
            metadata.fail_on_error as c_long,
        )?
    };
    unsafe { setopt_long(reference, CURLOPT_UPLOAD, metadata.upload as c_long)? };
    unsafe { setopt_long(reference, CURLOPT_DIRLISTONLY, metadata.dirlistonly as c_long)? };
    unsafe { setopt_long(reference, CURLOPT_APPEND, metadata.append as c_long)? };
    unsafe { setopt_long(reference, CURLOPT_NETRC, metadata.netrc_mode)? };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_FOLLOWLOCATION,
            metadata.follow_location as c_long,
        )?
    };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_TRANSFERTEXT,
            metadata.transfer_text as c_long,
        )?
    };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_AUTOREFERER,
            metadata.auto_referer as c_long,
        )?
    };
    unsafe { setopt_long(reference, CURLOPT_HTTPGET, metadata.http_get as c_long)? };
    unsafe { setopt_long(reference, CURLOPT_HTTP_VERSION, metadata.http_version)? };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_PROXYPORT,
            metadata.proxy_port.unwrap_or_default() as c_long,
        )?
    };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_HTTPPROXYTUNNEL,
            metadata.tunnel_proxy as c_long,
        )?
    };
    unsafe { setopt_long(reference, CURLOPT_BUFFERSIZE, metadata.buffer_size)? };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_MAXREDIRS,
            metadata.max_redirs.unwrap_or(-1),
        )?
    };
    unsafe { setopt_long(reference, CURLOPT_POSTREDIR, metadata.postredir)? };
    unsafe { setopt_long(reference, CURLOPT_TIMEOUT_MS, metadata.timeout_ms)? };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_COOKIESESSION,
            metadata.cookie_session as c_long,
        )?
    };
    unsafe { setopt_long(reference, CURLOPT_CERTINFO, metadata.certinfo as c_long)? };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_SSL_VERIFYPEER,
            metadata.ssl_verify_peer as c_long,
        )?
    };
    unsafe { setopt_long(reference, CURLOPT_SSL_VERIFYHOST, metadata.ssl_verify_host)? };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_SSL_ENABLE_ALPN,
            metadata.ssl_enable_alpn as c_long,
        )?
    };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_UNRESTRICTED_AUTH,
            metadata.unrestricted_auth as c_long,
        )?
    };
    unsafe { setopt_long(reference, CURLOPT_HTTPAUTH, metadata.httpauth)? };
    unsafe { setopt_long(reference, CURLOPT_PROXYAUTH, metadata.proxyauth)? };
    unsafe { setopt_long(reference, CURLOPT_RTSP_REQUEST, metadata.rtsp_request)? };
    unsafe { setopt_long(reference, CURLOPT_HEADEROPT, metadata.headeropt)? };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_SUPPRESS_CONNECT_HEADERS,
            metadata.suppress_connect_headers as c_long,
        )?
    };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_HTTP09_ALLOWED,
            metadata.http09_allowed as c_long,
        )?
    };
    unsafe { setopt_long(reference, CURLOPT_ALTSVC_CTRL, metadata.altsvc_ctrl)? };
    unsafe { setopt_long(reference, CURLOPT_HSTS_CTRL, metadata.hsts_ctrl)? };
    unsafe { setopt_long(reference, CURLOPT_WS_OPTIONS, metadata.ws_options)? };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_QUICK_EXIT,
            metadata.quick_exit as c_long,
        )?
    };
    unsafe { setopt_long(reference, CURLOPT_CONNECT_ONLY, metadata.connect_mode)? };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_TCP_NODELAY,
            metadata.tcp_nodelay as c_long,
        )?
    };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_MAXCONNECTS,
            metadata.maxconnects.unwrap_or(0),
        )?
    };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_INFILESIZE,
            metadata.upload_size.unwrap_or(-1) as c_long,
        )?
    };
    unsafe {
        setopt_long(
            reference,
            CURLOPT_RESUME_FROM,
            metadata.resume_from as c_long,
        )?
    };
    unsafe {
        setopt_off_t(
            reference,
            CURLOPT_INFILESIZE_LARGE,
            metadata.upload_size.unwrap_or(-1),
        )?
    };
    unsafe { setopt_off_t(reference, CURLOPT_RESUME_FROM_LARGE, metadata.resume_from)? };

    let _ = metadata.httppost_handle;
    let _ = metadata.mimepost_handle;
    let _ = CURLOPT_HTTPPOST;

    Ok(owned_lists)
}

pub(crate) fn active_handle(handle: *mut CURL) -> *mut CURL {
    if handle.is_null() {
        return ptr::null_mut();
    }
    if !crate::easy::handle::is_public_handle(handle) {
        return handle;
    }
    registry()
        .lock()
        .expect("easy reference registry mutex poisoned")
        .by_public
        .get(&(handle as usize))
        .map(|active| active.raw as *mut CURL)
        .unwrap_or(ptr::null_mut())
}

pub(crate) unsafe fn setopt_long_on_active(
    handle: *mut CURL,
    option: CURLoption,
    value: c_long,
) -> CURLcode {
    let reference = active_handle(handle);
    if reference.is_null() {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    }
    unsafe { ref_easy_setopt()(reference, option, value) }
}

pub(crate) fn public_from_reference(reference: *mut CURL) -> *mut CURL {
    if reference.is_null() {
        return ptr::null_mut();
    }
    registry()
        .lock()
        .expect("easy reference registry mutex poisoned")
        .by_reference
        .get(&(reference as usize))
        .copied()
        .map(|value| value as *mut CURL)
        .unwrap_or(reference)
}

pub(crate) unsafe fn ensure_handle(public: *mut CURL) -> *mut CURL {
    if public.is_null() || !crate::easy::handle::is_public_handle(public) {
        return ptr::null_mut();
    }
    if let Some(raw) = registry()
        .lock()
        .expect("easy reference registry mutex poisoned")
        .by_public
        .get(&(public as usize))
        .map(|active| active.raw as *mut CURL)
    {
        return raw;
    }

    let mut guard = registry()
        .lock()
        .expect("easy reference registry mutex poisoned");
    if let Err(code) = ensure_runtime_initialized(&mut guard) {
        let _ = code;
        return ptr::null_mut();
    }
    drop(guard);

    let raw = unsafe { ref_easy_init()() };
    if raw.is_null() {
        return ptr::null_mut();
    }

    let owned_lists = match unsafe { configure_reference_handle(public, raw) } {
        Ok(owned_lists) => owned_lists,
        Err(_code) => {
            unsafe { ref_easy_cleanup()(raw) };
            return ptr::null_mut();
        }
    };

    let mut guard = registry()
        .lock()
        .expect("easy reference registry mutex poisoned");
    insert_active(
        &mut guard,
        public,
        ActiveReference {
            raw: raw as usize,
            owned_lists,
        },
    );
    raw
}

pub(crate) unsafe fn release_handle(public: *mut CURL) {
    if public.is_null() || !crate::easy::handle::is_public_handle(public) {
        return;
    }
    let active = {
        let mut guard = registry()
            .lock()
            .expect("easy reference registry mutex poisoned");
        take_active(&mut guard, public)
    };
    if let Some(active) = active {
        unsafe { cleanup_active(active) };
    }
}

pub(crate) unsafe fn cleanup_raw_reference(reference: *mut CURL) {
    if reference.is_null() {
        return;
    }
    let public = public_from_reference(reference);
    if public != reference {
        unsafe { release_handle(public) };
        return;
    }
    crate::easy::perform::unregister_handle(reference);
    unsafe { ref_easy_cleanup()(reference) };
}

pub(crate) unsafe fn clear_all() {
    let (actives, runtime_initialized) = {
        let mut guard = registry()
            .lock()
            .expect("easy reference registry mutex poisoned");
        let actives = guard
            .by_public
            .drain()
            .map(|(_, active)| active)
            .collect::<Vec<_>>();
        guard.by_reference.clear();
        let runtime_initialized = guard.runtime_initialized;
        guard.runtime_initialized = false;
        (actives, runtime_initialized)
    };
    for active in actives {
        unsafe { cleanup_active(active) };
    }
    if runtime_initialized {
        unsafe { ref_global_cleanup()() };
    }
}

pub(crate) unsafe fn pause_handle(handle: *mut CURL, bitmask: c_int) -> CURLcode {
    let reference = active_handle(handle);
    if reference.is_null() || reference == handle {
        return CURLE_FAILED_INIT;
    }
    unsafe { ref_easy_pause()(reference, bitmask) }
}
