use crate::abi::{curl_version_info_data, size_t, time_t, CURLversion, CURLVERSION_NOW};
use crate::{alloc, BUILD_FLAVOR};
use core::ffi::{c_char, c_void};
use core::ptr;
use std::ffi::CStr;
use std::sync::{Mutex, OnceLock};

unsafe extern "C" {
    fn getenv(name: *const c_char) -> *mut c_char;
}

const LIBCURL_VERSION_NUM: u32 = 0x080500;
const CURL_VERSION_IPV6: i32 = 1 << 0;
const CURL_VERSION_SSL: i32 = 1 << 2;
const CURL_VERSION_LARGEFILE: i32 = 1 << 9;
const CURL_VERSION_UNIX_SOCKETS: i32 = 1 << 19;
const CURL_VERSION_ALTSVC: i32 = 1 << 24;
const CURL_VERSION_HSTS: i32 = 1 << 28;
const CURL_VERSION_THREADSAFE: i32 = 1 << 30;
const FEATURES: i32 = CURL_VERSION_IPV6
    | CURL_VERSION_SSL
    | CURL_VERSION_LARGEFILE
    | CURL_VERSION_UNIX_SOCKETS
    | CURL_VERSION_ALTSVC
    | CURL_VERSION_HSTS
    | CURL_VERSION_THREADSAFE;

static VERSION_CACHE: Mutex<Option<usize>> = Mutex::new(None);

struct SyncCharPtrArray<const N: usize>([*const c_char; N]);
unsafe impl<const N: usize> Sync for SyncCharPtrArray<N> {}

struct SyncVersionInfo(curl_version_info_data);
unsafe impl Sync for SyncVersionInfo {}
unsafe impl Send for SyncVersionInfo {}

static PROTOCOLS: SyncCharPtrArray<26> = SyncCharPtrArray([
    c"dict".as_ptr(),
    c"file".as_ptr(),
    c"ftp".as_ptr(),
    c"ftps".as_ptr(),
    c"gopher".as_ptr(),
    c"http".as_ptr(),
    c"https".as_ptr(),
    c"imap".as_ptr(),
    c"imaps".as_ptr(),
    c"ldap".as_ptr(),
    c"ldaps".as_ptr(),
    c"mqtt".as_ptr(),
    c"pop3".as_ptr(),
    c"pop3s".as_ptr(),
    c"rtsp".as_ptr(),
    c"scp".as_ptr(),
    c"sftp".as_ptr(),
    c"smb".as_ptr(),
    c"smbs".as_ptr(),
    c"smtp".as_ptr(),
    c"smtps".as_ptr(),
    c"telnet".as_ptr(),
    c"tftp".as_ptr(),
    c"ws".as_ptr(),
    c"wss".as_ptr(),
    ptr::null(),
]);

static FEATURE_NAMES: SyncCharPtrArray<8> = SyncCharPtrArray([
    c"IPv6".as_ptr(),
    c"SSL".as_ptr(),
    c"Largefile".as_ptr(),
    c"UnixSockets".as_ptr(),
    c"alt-svc".as_ptr(),
    c"HSTS".as_ptr(),
    c"threadsafe".as_ptr(),
    ptr::null(),
]);

fn ssl_version_string() -> *const c_char {
    if BUILD_FLAVOR == "openssl" {
        c"OpenSSL".as_ptr()
    } else {
        c"GnuTLS".as_ptr()
    }
}

fn version_info() -> &'static curl_version_info_data {
    static VALUE: OnceLock<SyncVersionInfo> = OnceLock::new();
    &VALUE
        .get_or_init(|| {
            SyncVersionInfo(curl_version_info_data {
                age: CURLVERSION_NOW,
                version: c"8.5.0".as_ptr(),
                version_num: LIBCURL_VERSION_NUM,
                host: c"unknown".as_ptr(),
                features: FEATURES,
                ssl_version: ssl_version_string(),
                ssl_version_num: 0,
                libz_version: ptr::null(),
                protocols: PROTOCOLS.0.as_ptr(),
                ares: ptr::null(),
                ares_num: 0,
                libidn: ptr::null(),
                iconv_ver_num: 0,
                libssh_version: c"libssh2".as_ptr(),
                brotli_ver_num: 0,
                brotli_version: ptr::null(),
                nghttp2_ver_num: 0,
                nghttp2_version: ptr::null(),
                quic_version: ptr::null(),
                cainfo: ptr::null(),
                capath: ptr::null(),
                zstd_ver_num: 0,
                zstd_version: ptr::null(),
                hyper_version: ptr::null(),
                gsasl_version: ptr::null(),
                feature_names: FEATURE_NAMES.0.as_ptr(),
            })
        })
        .0
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
pub unsafe extern "C" fn curl_getenv(variable: *const c_char) -> *mut c_char {
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
pub unsafe extern "C" fn curl_getdate(input: *const c_char, _unused: *const time_t) -> time_t {
    if input.is_null() {
        return -1;
    }
    let rendered = unsafe { CStr::from_ptr(input) }.to_string_lossy();
    parse_http_date(rendered.trim()).unwrap_or(-1)
}

#[no_mangle]
pub unsafe extern "C" fn curl_strequal(s1: *const c_char, s2: *const c_char) -> i32 {
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
pub unsafe extern "C" fn curl_strnequal(s1: *const c_char, s2: *const c_char, n: size_t) -> i32 {
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
pub unsafe extern "C" fn curl_version() -> *mut c_char {
    if let Some(ptr) = *VERSION_CACHE.lock().expect("version cache mutex poisoned") {
        return ptr as *mut c_char;
    }

    let rendered = if BUILD_FLAVOR == "openssl" {
        "libcurl/8.5.0 OpenSSL"
    } else {
        "libcurl/8.5.0 GnuTLS"
    };
    let copy = unsafe { alloc::alloc_and_copy(rendered.as_bytes()) };
    if copy.is_null() {
        return ptr::null_mut();
    }

    *VERSION_CACHE.lock().expect("version cache mutex poisoned") = Some(copy as usize);
    copy
}

#[no_mangle]
pub unsafe extern "C" fn curl_version_info(_stamp: CURLversion) -> *mut curl_version_info_data {
    version_info() as *const curl_version_info_data as *mut curl_version_info_data
}
