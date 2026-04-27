use crate::abi::{curl_version_info_data, size_t, time_t, CURLversion, CURLVERSION_NOW};
use crate::alloc;
use core::ffi::{c_char, c_int, c_void};
use core::ptr;
use std::ffi::{CStr, CString};
use std::sync::{Mutex, OnceLock};

unsafe extern "C" {
    fn getenv(name: *const c_char) -> *mut c_char;
    fn zlibVersion() -> *const c_char;
    fn libssh2_version(req_version_num: c_int) -> *const c_char;
    fn nghttp2_version(least_version: c_int) -> *const NgHttp2Info;
    #[cfg(feature = "openssl-flavor")]
    fn OpenSSL_version(t: c_int) -> *const c_char;
    #[cfg(feature = "gnutls-flavor")]
    fn gnutls_check_version(req_version: *const c_char) -> *const c_char;
}

const LIBCURL_VERSION_NUM: u32 = 0x080500;
const CURL_VERSION_SSL: i32 = 1 << 2;
const CURL_VERSION_LIBZ: i32 = 1 << 3;
const CURL_VERSION_NTLM: i32 = 1 << 4;
const CURL_VERSION_ASYNCHDNS: i32 = 1 << 7;
const CURL_VERSION_LARGEFILE: i32 = 1 << 9;
const CURL_VERSION_TLSAUTH_SRP: i32 = 1 << 14;
const CURL_VERSION_HTTP2: i32 = 1 << 16;
const CURL_VERSION_UNIX_SOCKETS: i32 = 1 << 19;
const CURL_VERSION_HTTPS_PROXY: i32 = 1 << 21;
const CURL_VERSION_ALTSVC: i32 = 1 << 24;
const CURL_VERSION_HSTS: i32 = 1 << 28;
const CURL_VERSION_THREADSAFE: i32 = 1 << 30;
const FEATURES: i32 = CURL_VERSION_SSL
    | CURL_VERSION_LIBZ
    | CURL_VERSION_NTLM
    | CURL_VERSION_ASYNCHDNS
    | CURL_VERSION_LARGEFILE
    | CURL_VERSION_TLSAUTH_SRP
    | CURL_VERSION_HTTP2
    | CURL_VERSION_UNIX_SOCKETS
    | CURL_VERSION_HTTPS_PROXY
    | CURL_VERSION_ALTSVC
    | CURL_VERSION_HSTS
    | CURL_VERSION_THREADSAFE;

static VERSION_CACHE: Mutex<Option<usize>> = Mutex::new(None);

// Keep this inventory aligned with the reference curl-config script used by the
// compat harness so `curl --version` and `curl-config --protocols` agree.
static PROTOCOL_NAMES: [&CStr; 22] = [
    c"dict",
    c"file",
    c"ftp",
    c"ftps",
    c"gopher",
    c"gophers",
    c"http",
    c"https",
    c"imap",
    c"imaps",
    c"mqtt",
    c"pop3",
    c"pop3s",
    c"rtsp",
    c"scp",
    c"sftp",
    c"smb",
    c"smbs",
    c"smtp",
    c"smtps",
    c"telnet",
    c"tftp",
];

static FEATURE_NAME_STRINGS: [&CStr; 12] = [
    c"alt-svc",
    c"AsynchDNS",
    c"HSTS",
    c"HTTP2",
    c"HTTPS-proxy",
    c"Largefile",
    c"libz",
    c"NTLM",
    c"SSL",
    c"threadsafe",
    c"TLS-SRP",
    c"UnixSockets",
];

#[repr(C)]
struct NgHttp2Info {
    age: c_int,
    version_num: c_int,
    version_str: *const c_char,
    proto_str: *const c_char,
}

struct VersionRuntime {
    info: curl_version_info_data,
    curl_version_text: CString,
    version_short: CString,
    host: CString,
    ssl_version: CString,
    libz_version: CString,
    libssh_version: CString,
    nghttp2_version: CString,
    protocols: [*const c_char; PROTOCOL_NAMES.len() + 1],
    feature_names: [*const c_char; FEATURE_NAME_STRINGS.len() + 1],
}

unsafe impl Sync for VersionRuntime {}
unsafe impl Send for VersionRuntime {}

fn cstring_or_fallback(text: String, fallback: &CStr) -> CString {
    CString::new(text).unwrap_or_else(|_| fallback.to_owned())
}

fn runtime_host() -> CString {
    let target = env!("PORT_LIBCURL_SAFE_TARGET");
    let normalized = if let Some(rest) = target.strip_prefix("x86_64-unknown-linux-") {
        format!("x86_64-pc-linux-{rest}")
    } else {
        target.to_string()
    };
    cstring_or_fallback(
        normalized,
        c"unknown",
    )
}

fn ssl_version_string() -> CString {
    #[cfg(feature = "openssl-flavor")]
    {
        const OPENSSL_VERSION_STRING: c_int = 6;
        let version = unsafe { OpenSSL_version(OPENSSL_VERSION_STRING) };
        if !version.is_null() {
            let text = unsafe { CStr::from_ptr(version) }.to_string_lossy();
            return cstring_or_fallback(format!("OpenSSL/{text}"), c"OpenSSL");
        }
        c"OpenSSL".to_owned()
    }
    #[cfg(feature = "gnutls-flavor")]
    {
        let version = unsafe { gnutls_check_version(ptr::null()) };
        if !version.is_null() {
            let text = unsafe { CStr::from_ptr(version) }.to_string_lossy();
            return cstring_or_fallback(format!("GnuTLS/{text}"), c"GnuTLS");
        }
        c"GnuTLS".to_owned()
    }
}

fn library_version(ptr: *const c_char, prefix: &str, fallback: &CStr) -> CString {
    if !ptr.is_null() {
        let text = unsafe { CStr::from_ptr(ptr) }.to_string_lossy();
        return cstring_or_fallback(format!("{prefix}{text}"), fallback);
    }
    fallback.to_owned()
}

fn libz_version_string() -> CString {
    library_version(unsafe { zlibVersion() }, "", c"")
}

fn libssh_version_string() -> CString {
    library_version(unsafe { libssh2_version(0) }, "libssh2/", c"libssh2")
}

fn nghttp2_version_fields() -> (CString, u32) {
    let info = unsafe { nghttp2_version(0) };
    if info.is_null() {
        return (c"".to_owned(), 0);
    }
    let info = unsafe { &*info };
    let text = if info.version_str.is_null() {
        c"".to_owned()
    } else {
        library_version(info.version_str, "", c"")
    };
    (text, info.version_num.max(0) as u32)
}

fn version_runtime() -> &'static VersionRuntime {
    static VALUE: OnceLock<Box<VersionRuntime>> = OnceLock::new();
    VALUE
        .get_or_init(|| {
            let ssl_version = ssl_version_string();
            let libz_version = libz_version_string();
            let libssh_version = libssh_version_string();
            let (nghttp2_version, nghttp2_ver_num) = nghttp2_version_fields();
            let curl_version_text = cstring_or_fallback(
                format!(
                    "libcurl/8.5.0 {} zlib/{} {} nghttp2/{}",
                    ssl_version.to_string_lossy(),
                    libz_version.to_string_lossy(),
                    libssh_version.to_string_lossy(),
                    nghttp2_version.to_string_lossy()
                ),
                c"libcurl/8.5.0",
            );

            let mut runtime = Box::new(VersionRuntime {
                info: curl_version_info_data {
                    age: CURLVERSION_NOW,
                    version: ptr::null(),
                    version_num: LIBCURL_VERSION_NUM,
                    host: ptr::null(),
                    features: FEATURES,
                    ssl_version: ptr::null(),
                    ssl_version_num: 0,
                    libz_version: ptr::null(),
                    protocols: ptr::null(),
                    ares: ptr::null(),
                    ares_num: 0,
                    libidn: ptr::null(),
                    iconv_ver_num: 0,
                    libssh_version: ptr::null(),
                    brotli_ver_num: 0,
                    brotli_version: ptr::null(),
                    nghttp2_ver_num,
                    nghttp2_version: ptr::null(),
                    quic_version: ptr::null(),
                    cainfo: ptr::null(),
                    capath: ptr::null(),
                    zstd_ver_num: 0,
                    zstd_version: ptr::null(),
                    hyper_version: ptr::null(),
                    gsasl_version: ptr::null(),
                    feature_names: ptr::null(),
                },
                curl_version_text,
                version_short: c"8.5.0".to_owned(),
                host: runtime_host(),
                ssl_version,
                libz_version,
                libssh_version,
                nghttp2_version,
                protocols: [ptr::null(); PROTOCOL_NAMES.len() + 1],
                feature_names: [ptr::null(); FEATURE_NAME_STRINGS.len() + 1],
            });

            for (slot, name) in runtime.protocols.iter_mut().zip(PROTOCOL_NAMES.iter()) {
                *slot = name.as_ptr();
            }
            for (slot, name) in runtime
                .feature_names
                .iter_mut()
                .zip(FEATURE_NAME_STRINGS.iter())
            {
                *slot = name.as_ptr();
            }

            runtime.info.version = runtime.version_short.as_ptr();
            runtime.info.host = runtime.host.as_ptr();
            runtime.info.ssl_version = runtime.ssl_version.as_ptr();
            runtime.info.libz_version = runtime.libz_version.as_ptr();
            runtime.info.protocols = runtime.protocols.as_ptr();
            runtime.info.libssh_version = runtime.libssh_version.as_ptr();
            runtime.info.nghttp2_version = runtime.nghttp2_version.as_ptr();
            runtime.info.feature_names = runtime.feature_names.as_ptr();
            runtime
        })
        .as_ref()
}

fn version_info() -> &'static curl_version_info_data {
    &version_runtime().info
}

pub(crate) fn clear_cached_version() {
    let cached = VERSION_CACHE
        .lock()
        .expect("version cache mutex poisoned")
        .take();
    if let Some(ptr) = cached {
        unsafe { alloc::free_ptr((ptr as *mut c_char).cast::<c_void>()) };
    }
}

fn ascii_lower(byte: u8) -> u8 {
    if byte.is_ascii_uppercase() {
        byte + 32
    } else {
        byte
    }
}

fn parse_decimal(input: &str) -> Option<i32> {
    (!input.is_empty()).then_some(())?;
    input.parse().ok()
}

fn month_number(text: &str) -> Option<u32> {
    match text.to_ascii_lowercase().as_str() {
        "jan" => Some(1),
        "feb" => Some(2),
        "mar" => Some(3),
        "apr" => Some(4),
        "may" => Some(5),
        "jun" => Some(6),
        "jul" => Some(7),
        "aug" => Some(8),
        "sep" => Some(9),
        "oct" => Some(10),
        "nov" => Some(11),
        "dec" => Some(12),
        _ => None,
    }
}

fn parse_hms(text: &str) -> Option<(u32, u32, u32)> {
    let mut parts = text.split(':');
    let hour = parts.next()?.parse().ok()?;
    let minute = parts.next()?.parse().ok()?;
    let second = parts.next()?.parse().ok()?;
    Some((hour, minute, second))
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = (year - era * 400) as u32;
    let month_index = (month as i32 + if month > 2 { -3 } else { 9 }) as u32;
    let doy = (153 * month_index + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era as i64 * 146097 + doe as i64 - 719468
}

fn epoch_seconds(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> Option<time_t> {
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let days = days_from_civil(year, month, day);
    Some((days * 86_400 + hour as i64 * 3_600 + minute as i64 * 60 + second as i64) as time_t)
}

fn parse_rfc1123(text: &str) -> Option<time_t> {
    let (weekday, rest) = text.split_once(',')?;
    if weekday.len() != 3 {
        return None;
    }
    let mut parts = rest.split_whitespace();
    let day = parts.next()?.parse().ok()?;
    let month = month_number(parts.next()?)?;
    let year = parse_decimal(parts.next()?)?;
    let (hour, minute, second) = parse_hms(parts.next()?)?;
    let zone = parts.next()?;
    if !zone.eq_ignore_ascii_case("GMT") && !zone.eq_ignore_ascii_case("UTC") {
        return None;
    }
    epoch_seconds(year, month, day, hour, minute, second)
}

fn parse_rfc850(text: &str) -> Option<time_t> {
    let (_, rest) = text.split_once(',')?;
    let mut parts = rest.split_whitespace();
    let date = parts.next()?;
    let (day, rest) = date.split_once('-')?;
    let (month, year) = rest.rsplit_once('-')?;
    let year = parse_decimal(year)?;
    let year = if year >= 70 { 1900 + year } else { 2000 + year };
    let month = month_number(month)?;
    let (hour, minute, second) = parse_hms(parts.next()?)?;
    let zone = parts.next()?;
    if !zone.eq_ignore_ascii_case("GMT") && !zone.eq_ignore_ascii_case("UTC") {
        return None;
    }
    epoch_seconds(year, month, day.parse().ok()?, hour, minute, second)
}

fn parse_asctime(text: &str) -> Option<time_t> {
    let mut parts = text.split_whitespace();
    let _weekday = parts.next()?;
    let month = month_number(parts.next()?)?;
    let day = parts.next()?.parse().ok()?;
    let (hour, minute, second) = parse_hms(parts.next()?)?;
    let year = parse_decimal(parts.next()?)?;
    epoch_seconds(year, month, day, hour, minute, second)
}

fn parse_http_date(text: &str) -> Option<time_t> {
    parse_rfc1123(text)
        .or_else(|| parse_rfc850(text))
        .or_else(|| parse_asctime(text))
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_getenv(variable: *const c_char) -> *mut c_char {
    if variable.is_null() {
        return ptr::null_mut();
    }

    let value = unsafe { getenv(variable) };
    if value.is_null() {
        return ptr::null_mut();
    }

    unsafe { alloc::strdup_bytes(value) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_getdate(
    input: *const c_char,
    _unused: *const time_t,
) -> time_t {
    if input.is_null() {
        return -1;
    }
    let rendered = unsafe { CStr::from_ptr(input) }.to_string_lossy();
    parse_http_date(rendered.trim()).unwrap_or(-1)
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_strequal(
    s1: *const c_char,
    s2: *const c_char,
) -> i32 {
    if s1.is_null() || s2.is_null() {
        return 0;
    }

    let mut idx = 0usize;
    loop {
        let a = unsafe { *s1.add(idx) } as u8;
        let b = unsafe { *s2.add(idx) } as u8;
        if ascii_lower(a) != ascii_lower(b) {
            return 0;
        }
        if a == 0 {
            return 1;
        }
        idx += 1;
    }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_strnequal(
    s1: *const c_char,
    s2: *const c_char,
    n: size_t,
) -> i32 {
    if n == 0 {
        return 1;
    }
    if s1.is_null() || s2.is_null() {
        return 0;
    }

    let mut idx = 0usize;
    while idx < n {
        let a = unsafe { *s1.add(idx) } as u8;
        let b = unsafe { *s2.add(idx) } as u8;
        if ascii_lower(a) != ascii_lower(b) {
            return 0;
        }
        if a == 0 {
            return 1;
        }
        idx += 1;
    }
    1
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_version() -> *mut c_char {
    if let Some(ptr) = *VERSION_CACHE.lock().expect("version cache mutex poisoned") {
        return ptr as *mut c_char;
    }

    let copy = unsafe {
        alloc::alloc_and_copy(version_runtime().curl_version_text.as_c_str().to_bytes())
    };
    if copy.is_null() {
        return ptr::null_mut();
    }

    *VERSION_CACHE.lock().expect("version cache mutex poisoned") = Some(copy as usize);
    copy
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_version_info(
    _stamp: CURLversion,
) -> *mut curl_version_info_data {
    version_info() as *const curl_version_info_data as *mut curl_version_info_data
}
