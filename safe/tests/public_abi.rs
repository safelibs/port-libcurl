use port_libcurl_safe::abi::{
    curl_blob, curl_calloc_callback, curl_easyoption, curl_free_callback, curl_httppost,
    curl_malloc_callback, curl_mime, curl_mimepart, curl_off_t, curl_read_callback,
    curl_realloc_callback, curl_seek_callback, curl_slist, curl_ssl_backend, curl_sslbackend,
    curl_strdup_callback, curl_version_info_data, CURLFORMcode, CURLMcode, CURLSHcode,
    CURLSHoption, CURLUcode, CURLcode, CURLoption, CURLsslset, CURLversion, CURL, CURLE_OK, CURLM,
    CURLM_OK, CURLOT_STRING, CURLSH, CURLSHE_OK, CURLSHOPT_SHARE, CURLSSLBACKEND_GNUTLS,
    CURLSSLBACKEND_OPENSSL, CURLSSLSET_OK, CURLSSLSET_TOO_LATE, CURLU, CURLUE_OK, CURLUPART_HOST,
    CURLUPART_URL, CURLVERSION_NOW, CURL_GLOBAL_DEFAULT, CURL_LOCK_DATA_COOKIE,
};
use port_libcurl_safe::BUILD_FLAVOR;
use std::collections::HashSet;
use std::ffi::{c_char, c_int, c_long, c_void, CStr, CString};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::ptr;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Mutex, OnceLock};
use std::thread;

const CURLOPT_MIMEPOST: CURLoption = 10269;
const CURLOPT_HTTPPOST: CURLoption = 10024;
const CURLOPT_URL: CURLoption = 10002;
const CURLOPT_DOH_URL: CURLoption = 10279;
const CURLOPT_COOKIELIST: CURLoption = 10135;
const CURLOPT_CURLU: CURLoption = 10282;
const CURLOPT_FOLLOWLOCATION: CURLoption = 52;
const CURLOPT_NOBODY: CURLoption = 44;
const CURLOPT_CAINFO_BLOB: CURLoption = 40309;
const CURLINFO_EFFECTIVE_URL: u32 = 0x100000 + 1;
const CURLINFO_CONTENT_TYPE: u32 = 0x100000 + 18;
const CURLINFO_REDIRECT_COUNT: u32 = 0x200000 + 20;
const CURLINFO_REDIRECT_URL: u32 = 0x100000 + 31;
const CURLINFO_COOKIELIST: u32 = 0x400000 + 28;
const CURLINFO_SCHEME: u32 = 0x100000 + 49;
const CURLINFO_REDIRECT_TIME_T: u32 = 0x600000 + 55;
const CURLINFO_APPCONNECT_TIME_T: u32 = 0x600000 + 56;
const CURL_ZERO_TERMINATED: usize = usize::MAX;
const CURLUPART_QUERY: u32 = 8;
const CURLU_DEFAULT_SCHEME: u32 = 1 << 2;
const CURLU_URLENCODE: u32 = 1 << 7;
const CURLU_APPENDQUERY: u32 = 1 << 8;
const CURLU_PUNYCODE: u32 = 1 << 12;
const CURLU_PUNY2IDN: u32 = 1 << 13;
const CURL_FORMADD_OK: CURLFORMcode = 0;
const FORM_FLAG_PTR_CONTENTS: u32 = 1 << 0;

unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn calloc(nmemb: usize, size: usize) -> *mut c_void;
    fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);

    fn curl_global_init_mem(
        flags: c_long,
        malloc_cb: curl_malloc_callback,
        free_cb: curl_free_callback,
        realloc_cb: curl_realloc_callback,
        strdup_cb: curl_strdup_callback,
        calloc_cb: curl_calloc_callback,
    ) -> CURLcode;
    fn curl_global_trace(config: *const c_char) -> CURLcode;
    fn curl_global_sslset(
        id: curl_sslbackend,
        name: *const c_char,
        avail: *mut *const *const curl_ssl_backend,
    ) -> CURLsslset;
    fn curl_global_cleanup();

    fn curl_getenv(name: *const c_char) -> *mut c_char;
    fn curl_getdate(input: *const c_char, now: *const c_long) -> c_long;
    fn curl_strequal(lhs: *const c_char, rhs: *const c_char) -> c_int;
    fn curl_strnequal(lhs: *const c_char, rhs: *const c_char, len: usize) -> c_int;
    fn curl_version() -> *mut c_char;
    fn curl_version_info(age: CURLversion) -> *mut curl_version_info_data;
    fn curl_free(ptr: *mut c_void);

    fn curl_easy_init() -> *mut CURL;
    fn curl_easy_cleanup(handle: *mut CURL);
    fn curl_easy_duphandle(handle: *mut CURL) -> *mut CURL;
    fn curl_easy_perform(handle: *mut CURL) -> CURLcode;
    fn curl_easy_reset(handle: *mut CURL);
    fn curl_easy_setopt(handle: *mut CURL, option: CURLoption, ...) -> CURLcode;
    fn curl_easy_getinfo(handle: *mut CURL, info: u32, ...) -> CURLcode;
    fn curl_easy_escape(handle: *mut CURL, input: *const c_char, len: c_int) -> *mut c_char;
    fn curl_easy_unescape(
        handle: *mut CURL,
        input: *const c_char,
        len: c_int,
        out_len: *mut c_int,
    ) -> *mut c_char;
    fn curl_easy_option_by_name(name: *const c_char) -> *const curl_easyoption;
    fn curl_easy_option_by_id(id: CURLoption) -> *const curl_easyoption;
    fn curl_easy_option_next(prev: *const curl_easyoption) -> *const curl_easyoption;
    fn curl_mime_init(handle: *mut CURL) -> *mut curl_mime;
    fn curl_mime_free(mime: *mut curl_mime);
    fn curl_mime_addpart(mime: *mut curl_mime) -> *mut curl_mimepart;
    fn curl_mime_name(part: *mut curl_mimepart, name: *const c_char) -> CURLcode;
    fn curl_mime_filename(part: *mut curl_mimepart, filename: *const c_char) -> CURLcode;
    fn curl_mime_type(part: *mut curl_mimepart, mime_type: *const c_char) -> CURLcode;
    fn curl_mime_encoder(part: *mut curl_mimepart, encoding: *const c_char) -> CURLcode;
    fn curl_mime_data(part: *mut curl_mimepart, data: *const c_char, datasize: usize) -> CURLcode;
    fn curl_mime_headers(
        part: *mut curl_mimepart,
        headers: *mut curl_slist,
        take_ownership: c_int,
    ) -> CURLcode;
    fn port_safe_export_curl_mime_data_cb(
        part: *mut curl_mimepart,
        datasize: curl_off_t,
        readfunc: curl_read_callback,
        seekfunc: curl_seek_callback,
        freefunc: curl_free_callback,
        arg: *mut c_void,
    ) -> CURLcode;
    fn curl_slist_append(list: *mut curl_slist, string: *const c_char) -> *mut curl_slist;
    fn curl_slist_free_all(list: *mut curl_slist);
    fn port_safe_formadd_parsed(
        httppost: *mut *mut curl_httppost,
        last_post: *mut *mut curl_httppost,
        spec: *const TestFormSpec,
    ) -> CURLFORMcode;
    fn port_safe_export_curl_formfree(form: *mut curl_httppost);

    fn curl_multi_init() -> *mut CURLM;
    fn curl_multi_add_handle(multi: *mut CURLM, easy: *mut CURL) -> CURLMcode;
    fn curl_multi_remove_handle(multi: *mut CURLM, easy: *mut CURL) -> CURLMcode;
    fn curl_multi_cleanup(multi: *mut CURLM) -> CURLMcode;
    fn curl_multi_get_handles(multi: *mut CURLM) -> *mut *mut CURL;

    fn curl_url() -> *mut CURLU;
    fn curl_url_cleanup(handle: *mut CURLU);
    fn curl_url_dup(handle: *const CURLU) -> *mut CURLU;
    fn curl_url_get(
        handle: *const CURLU,
        what: u32,
        part: *mut *mut c_char,
        flags: u32,
    ) -> CURLUcode;
    fn curl_url_set(handle: *mut CURLU, what: u32, part: *const c_char, flags: u32) -> CURLUcode;
    fn curl_url_strerror(code: CURLUcode) -> *const c_char;

    fn curl_share_init() -> *mut CURLSH;
    fn curl_share_cleanup(handle: *mut CURLSH) -> CURLSHcode;
    fn curl_share_strerror(code: CURLSHcode) -> *const c_char;
    fn curl_share_setopt(share: *mut CURLSH, option: CURLSHoption, ...) -> CURLSHcode;

    fn curl_maprintf(format: *const c_char, ...) -> *mut c_char;
}

#[repr(C)]
struct TestFormSpec {
    name: *const c_char,
    namelength: c_long,
    contents: *const c_char,
    contentslength: c_long,
    contenttype: *const c_char,
    contentheader: *mut curl_slist,
    filename: *const c_char,
    filepath: *const c_char,
    buffer_name: *const c_char,
    buffer_ptr: *const c_char,
    buffer_length: usize,
    stream: *mut c_void,
    contentlen: curl_off_t,
    flags: u32,
}

#[derive(Default)]
struct TrackingState {
    live: HashSet<usize>,
}

fn tracking() -> &'static Mutex<TrackingState> {
    static TRACKING: OnceLock<Mutex<TrackingState>> = OnceLock::new();
    TRACKING.get_or_init(|| Mutex::new(TrackingState::default()))
}

fn clear_tracking() {
    tracking()
        .lock()
        .expect("tracking mutex poisoned")
        .live
        .clear();
}

fn track(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    tracking()
        .lock()
        .expect("tracking mutex poisoned")
        .live
        .insert(ptr as usize);
}

fn untrack(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    tracking()
        .lock()
        .expect("tracking mutex poisoned")
        .live
        .remove(&(ptr as usize));
}

fn is_tracked(ptr: *mut c_void) -> bool {
    if ptr.is_null() {
        return false;
    }
    tracking()
        .lock()
        .expect("tracking mutex poisoned")
        .live
        .contains(&(ptr as usize))
}

unsafe extern "C" fn test_malloc(size: usize) -> *mut c_void {
    let ptr = unsafe { malloc(size) };
    track(ptr);
    ptr
}

unsafe extern "C" fn test_calloc(nmemb: usize, size: usize) -> *mut c_void {
    let ptr = unsafe { calloc(nmemb, size) };
    track(ptr);
    ptr
}

unsafe extern "C" fn test_realloc(old: *mut c_void, size: usize) -> *mut c_void {
    let new_ptr = unsafe { realloc(old, size) };
    if new_ptr.is_null() {
        return ptr::null_mut();
    }
    if !old.is_null() {
        untrack(old);
    }
    track(new_ptr);
    new_ptr
}

unsafe extern "C" fn test_strdup(input: *const c_char) -> *mut c_char {
    if input.is_null() {
        return ptr::null_mut();
    }

    let len = unsafe { CStr::from_ptr(input).to_bytes_with_nul().len() };
    let copy = unsafe { malloc(len) }.cast::<c_char>();
    if copy.is_null() {
        return ptr::null_mut();
    }

    unsafe { ptr::copy_nonoverlapping(input, copy, len) };
    track(copy.cast());
    copy
}

unsafe extern "C" fn test_free(ptr: *mut c_void) {
    untrack(ptr);
    unsafe { free(ptr) };
}

fn c_ptr(bytes: &'static [u8]) -> *const c_char {
    bytes.as_ptr().cast()
}

fn c_string_value(ptr: *const c_char) -> String {
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

fn c_string_list(mut values: *const *const c_char) -> Vec<String> {
    let mut out = Vec::new();
    while !values.is_null() {
        let value = unsafe { *values };
        if value.is_null() {
            break;
        }
        out.push(c_string_value(value));
        values = unsafe { values.add(1) };
    }
    out
}

fn read_http_request(stream: &mut TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    let mut scratch = [0u8; 2048];
    let header_end = loop {
        let read = stream.read(&mut scratch).expect("read request");
        if read == 0 {
            panic!("fixture closed before headers");
        }
        request.extend_from_slice(&scratch[..read]);
        if let Some(index) = request.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let header_text = String::from_utf8_lossy(&request[..header_end]);
    let content_length = header_text
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("Content-Length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    while request.len() < header_end + content_length {
        let read = stream.read(&mut scratch).expect("read body");
        if read == 0 {
            break;
        }
        request.extend_from_slice(&scratch[..read]);
    }
    request
}

fn spawn_capture_fixture(
    expected_requests: usize,
) -> (String, Receiver<Vec<u8>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind fixture");
    let port = listener.local_addr().expect("fixture addr").port();
    let base_url = format!("http://127.0.0.1:{port}");
    let (tx, rx) = mpsc::channel();
    let join = thread::spawn(move || {
        for _ in 0..expected_requests {
            let (mut stream, _) = listener.accept().expect("accept fixture");
            let request = read_http_request(&mut stream);
            tx.send(request).expect("send capture");
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .expect("write response");
            let _ = stream.shutdown(Shutdown::Both);
        }
    });
    (base_url, rx, join)
}

fn build_doh_response(query: &[u8], answer: Option<Ipv4Addr>) -> Vec<u8> {
    let mut response = Vec::with_capacity(query.len() + 16);
    response.extend_from_slice(&query[..2]);
    response.extend_from_slice(&0x8180u16.to_be_bytes());
    response.extend_from_slice(&1u16.to_be_bytes());
    response.extend_from_slice(&(answer.is_some() as u16).to_be_bytes());
    response.extend_from_slice(&0u16.to_be_bytes());
    response.extend_from_slice(&0u16.to_be_bytes());
    response.extend_from_slice(&query[12..]);
    if let Some(ip) = answer {
        response.extend_from_slice(&0xc00cu16.to_be_bytes());
        response.extend_from_slice(&1u16.to_be_bytes());
        response.extend_from_slice(&1u16.to_be_bytes());
        response.extend_from_slice(&30u32.to_be_bytes());
        response.extend_from_slice(&4u16.to_be_bytes());
        response.extend_from_slice(&ip.octets());
    }
    response
}

fn spawn_doh_fixture() -> (String, Receiver<Vec<u8>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind fixture");
    let port = listener.local_addr().expect("fixture addr").port();
    let base_url = format!("http://127.0.0.1:{port}/dns-query");
    let (tx, rx) = mpsc::channel();
    let join = thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept fixture");
            let request = read_http_request(&mut stream);
            let header_end = request
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .expect("header end")
                + 4;
            let body = request[header_end..].to_vec();
            tx.send(body.clone()).expect("send capture");
            let qtype = if body.len() >= 4 {
                u16::from_be_bytes([body[body.len() - 4], body[body.len() - 3]])
            } else {
                0
            };
            let dns_response = if qtype == 1 {
                build_doh_response(&body, Some(Ipv4Addr::new(127, 0, 0, 1)))
            } else {
                build_doh_response(&body, None)
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/dns-message\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                dns_response.len()
            );
            stream
                .write_all(response.as_bytes())
                .and_then(|_| stream.write_all(&dns_response))
                .expect("write response");
            let _ = stream.shutdown(Shutdown::Both);
        }
    });
    (base_url, rx, join)
}

struct CallbackBody {
    bytes: &'static [u8],
    offset: usize,
}

unsafe extern "C" fn mime_read_callback(
    buffer: *mut c_char,
    size: usize,
    nmemb: usize,
    userp: *mut c_void,
) -> usize {
    let body = unsafe { &mut *(userp as *mut CallbackBody) };
    let capacity = size.saturating_mul(nmemb);
    if capacity == 0 || body.offset >= body.bytes.len() {
        return 0;
    }
    let remaining = body.bytes.len() - body.offset;
    let copied = remaining.min(capacity);
    unsafe {
        ptr::copy_nonoverlapping(
            body.bytes[body.offset..body.offset + copied]
                .as_ptr()
                .cast(),
            buffer,
            copied,
        );
    }
    body.offset += copied;
    copied
}

unsafe extern "C" fn mime_seek_callback(
    userp: *mut c_void,
    offset: curl_off_t,
    origin: c_int,
) -> c_int {
    let body = unsafe { &mut *(userp as *mut CallbackBody) };
    if origin != 0 || offset != 0 {
        return 1;
    }
    body.offset = 0;
    0
}

unsafe extern "C" fn mime_free_callback(userp: *mut c_void) {
    if userp.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(userp as *mut CallbackBody));
    }
}

fn spawn_getinfo_fixture() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind fixture");
    let port = listener.local_addr().expect("fixture addr").port();
    let base_url = format!("http://127.0.0.1:{port}");
    let join = thread::spawn(move || {
        for _ in 0..3 {
            let (mut stream, _) = listener.accept().expect("accept fixture");
            let mut request = Vec::new();
            let mut scratch = [0u8; 1024];
            loop {
                let read = stream.read(&mut scratch).expect("read request");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&scratch[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }

            let request_text = String::from_utf8_lossy(&request);
            let path = request_text.split_whitespace().nth(1).unwrap_or("/");
            let response = match path {
                "/redirect" => {
                    "HTTP/1.1 302 Found\r\nLocation: /target\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                }
                "/target" => {
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                }
                _ => {
                    "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                }
            };
            stream
                .write_all(response.as_bytes())
                .expect("write response");
            let _ = stream.shutdown(Shutdown::Both);
        }
    });
    (base_url, join)
}

#[test]
fn public_abi_smoke_and_allocator_contract() {
    assert!(!BUILD_FLAVOR.is_empty());
    clear_tracking();
    std::env::set_var("PORT_LIBCURL_SAFE_ALLOC_TEST", "allocator-smoke");

    unsafe {
        let expected_backend = if BUILD_FLAVOR == "openssl" {
            CURLSSLBACKEND_OPENSSL
        } else {
            CURLSSLBACKEND_GNUTLS
        };
        let expected_backend_name = if BUILD_FLAVOR == "openssl" {
            c"openssl"
        } else {
            c"gnutls"
        };
        let mut available_backends = ptr::null();
        assert_eq!(
            curl_global_sslset(expected_backend, ptr::null(), &mut available_backends),
            CURLSSLSET_OK
        );
        assert!(!available_backends.is_null());
        assert!(!(*available_backends).is_null());
        assert_eq!(
            CStr::from_ptr((**available_backends).name).to_bytes(),
            expected_backend_name.to_bytes()
        );

        assert_eq!(
            curl_global_init_mem(
                CURL_GLOBAL_DEFAULT,
                Some(test_malloc),
                Some(test_free),
                Some(test_realloc),
                Some(test_strdup),
                Some(test_calloc),
            ),
            CURLE_OK
        );
        assert_eq!(
            curl_global_sslset(expected_backend, ptr::null(), ptr::null_mut()),
            CURLSSLSET_TOO_LATE
        );
        assert_eq!(curl_global_trace(ptr::null()), CURLE_OK);

        let getenv_name = CString::new("PORT_LIBCURL_SAFE_ALLOC_TEST").expect("cstring");
        let getenv_value = curl_getenv(getenv_name.as_ptr());
        assert!(!getenv_value.is_null());
        assert_eq!(CStr::from_ptr(getenv_value).to_bytes(), b"allocator-smoke");
        assert!(is_tracked(getenv_value.cast()));
        curl_free(getenv_value.cast());
        assert!(!is_tracked(getenv_value.cast()));

        assert_eq!(
            curl_getdate(c_ptr(b"Sun, 06 Nov 1994 08:49:37 GMT\0"), ptr::null()),
            784111777
        );
        assert_eq!(curl_strequal(c_ptr(b"AbC\0"), c_ptr(b"aBc\0")), 1);
        assert_eq!(curl_strnequal(c_ptr(b"AbCd\0"), c_ptr(b"aBcE\0"), 3), 1);

        let version = curl_version();
        assert!(!version.is_null());
        assert!(is_tracked(version.cast()));
        assert_eq!(version, curl_version());
        let info = &*curl_version_info(CURLVERSION_NOW);
        assert!(!info.version.is_null());
        let ssl_version = c_string_value(info.ssl_version);
        let expected_protocols = vec![
            "dict", "file", "ftp", "ftps", "gopher", "gophers", "http", "https", "imap", "imaps",
            "mqtt", "pop3", "pop3s", "rtsp", "scp", "sftp", "smb", "smbs", "smtp", "smtps",
            "telnet", "tftp",
        ];
        let expected_features = vec![
            "alt-svc",
            "AsynchDNS",
            "HSTS",
            "HTTP2",
            "HTTPS-proxy",
            "Largefile",
            "libz",
            "NTLM",
            "SSL",
            "threadsafe",
            "TLS-SRP",
            "UnixSockets",
        ];
        let expected_version = format!(
            "libcurl/8.5.0 {} zlib/1.3 libssh2/1.11.0 nghttp2/1.59.0",
            ssl_version
        );
        assert_eq!(
            CStr::from_ptr(version).to_bytes(),
            expected_version.as_bytes()
        );
        assert_eq!(c_string_value(info.version), "8.5.0");
        assert_eq!(c_string_value(info.host), "x86_64-pc-linux-gnu");
        assert_eq!(info.features, 1_361_658_524);
        assert_eq!(c_string_value(info.libz_version), "1.3");
        assert_eq!(c_string_value(info.libssh_version), "libssh2/1.11.0");
        assert_eq!(c_string_value(info.nghttp2_version), "1.59.0");
        assert_eq!(c_string_list(info.protocols), expected_protocols);
        assert_eq!(c_string_list(info.feature_names), expected_features);
        match BUILD_FLAVOR {
            "openssl" => assert!(ssl_version.starts_with("OpenSSL/3.0.")),
            "gnutls" => assert!(ssl_version.starts_with("GnuTLS/3.8.")),
            other => panic!("unexpected BUILD_FLAVOR: {other}"),
        }

        let easy = curl_easy_init();
        assert!(!easy.is_null());
        let mut cainfo_bytes = *b"dummy-ca";
        let cainfo_blob = curl_blob {
            data: cainfo_bytes.as_mut_ptr().cast(),
            len: cainfo_bytes.len(),
            flags: 1,
        };
        assert_eq!(
            curl_easy_setopt(easy, CURLOPT_CAINFO_BLOB, &cainfo_blob),
            CURLE_OK
        );
        let header_text = CString::new("X-Test: native-mime").expect("cstring");
        let mime_headers = curl_slist_append(ptr::null_mut(), header_text.as_ptr());
        assert!(!mime_headers.is_null());
        let mime = curl_mime_init(easy);
        assert!(!mime.is_null());
        let part = curl_mime_addpart(mime);
        assert!(!part.is_null());
        assert_eq!(curl_mime_name(part, c_ptr(b"field\0")), CURLE_OK);
        assert_eq!(curl_mime_filename(part, c_ptr(b"value.txt\0")), CURLE_OK);
        assert_eq!(curl_mime_type(part, c_ptr(b"text/plain\0")), CURLE_OK);
        assert_eq!(curl_mime_encoder(part, c_ptr(b"binary\0")), CURLE_OK);
        assert_eq!(
            curl_mime_data(part, c_ptr(b"value\0"), CURL_ZERO_TERMINATED),
            CURLE_OK
        );
        assert_eq!(curl_mime_headers(part, mime_headers, 0), CURLE_OK);
        assert_eq!(curl_easy_setopt(easy, CURLOPT_MIMEPOST, mime), CURLE_OK);
        assert_eq!(
            curl_easy_setopt(easy, CURLOPT_MIMEPOST, ptr::null_mut::<curl_mime>()),
            CURLE_OK
        );

        let escaped = curl_easy_escape(easy, c_ptr(b"a/b?c=d\0"), 7);
        assert!(!escaped.is_null());
        assert!(is_tracked(escaped.cast()));

        let mut unescaped_len = 0;
        let unescaped = curl_easy_unescape(easy, escaped, 0, &mut unescaped_len);
        assert!(!unescaped.is_null());
        assert!(is_tracked(unescaped.cast()));
        assert_eq!(unescaped_len, 7);
        assert_eq!(
            std::slice::from_raw_parts(unescaped.cast::<u8>(), unescaped_len as usize),
            b"a/b?c=d"
        );
        curl_free(unescaped.cast());
        curl_free(escaped.cast());

        let option = curl_easy_option_by_name(c_ptr(b"url\0"));
        assert!(!option.is_null());
        assert_eq!(CStr::from_ptr((*option).name).to_bytes(), b"URL");
        assert_eq!((*option).type_, CURLOT_STRING);
        assert_eq!(curl_easy_option_by_id((*option).id), option);

        let mut option_count = 0usize;
        let mut cursor = ptr::null();
        loop {
            cursor = curl_easy_option_next(cursor);
            if cursor.is_null() {
                break;
            }
            option_count += 1;
        }
        assert!(option_count > 100);

        let share = curl_share_init();
        assert!(!share.is_null());
        assert_eq!(
            curl_share_setopt(share, CURLSHOPT_SHARE, CURL_LOCK_DATA_COOKIE as c_int),
            CURLSHE_OK
        );
        assert!(!curl_share_strerror(CURLSHE_OK).is_null());

        let url = curl_url();
        assert!(!url.is_null());
        let full_url =
            CString::new("https://user:pass@example.com:9443/base/path?x=1#frag").expect("cstring");
        assert_eq!(
            curl_url_set(url, CURLUPART_URL, full_url.as_ptr(), 0),
            CURLUE_OK
        );
        assert_eq!(curl_easy_setopt(easy, CURLOPT_CURLU, url), CURLE_OK);

        let mut scheme = ptr::null_mut();
        assert_eq!(
            curl_easy_getinfo(easy, CURLINFO_SCHEME, &mut scheme),
            CURLE_OK
        );
        assert_eq!(CStr::from_ptr(scheme).to_bytes(), b"https");

        assert_eq!(
            curl_easy_setopt(easy, CURLOPT_URL, c_ptr(b"http://example.test/plain\0")),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_getinfo(easy, CURLINFO_SCHEME, &mut scheme),
            CURLE_OK
        );
        assert_eq!(CStr::from_ptr(scheme).to_bytes(), b"http");

        assert_eq!(
            curl_easy_setopt(
                easy,
                CURLOPT_COOKIELIST,
                c_ptr(b"Set-Cookie: session=one; Path=/\0"),
            ),
            CURLE_OK
        );
        let mut cookie_list: *mut curl_slist = ptr::null_mut();
        assert_eq!(
            curl_easy_getinfo(easy, CURLINFO_COOKIELIST, &mut cookie_list),
            CURLE_OK
        );
        assert!(!cookie_list.is_null());
        assert!(is_tracked(cookie_list.cast()));
        assert_eq!(
            CStr::from_ptr((*cookie_list).data).to_bytes(),
            b"example.test\tFALSE\t/\tFALSE\t0\tsession\tone"
        );
        curl_slist_free_all(cookie_list);

        let mut host = ptr::null_mut();
        assert_eq!(curl_url_get(url, CURLUPART_HOST, &mut host, 0), CURLUE_OK);
        assert!(!host.is_null());
        assert!(is_tracked(host.cast()));
        assert_eq!(CStr::from_ptr(host).to_bytes(), b"example.com");
        curl_free(host.cast());

        let puny_url = curl_url();
        assert!(!puny_url.is_null());
        let idn_full_url = CString::new("https://räksmörgås.se/").expect("cstring");
        assert_eq!(
            curl_url_set(puny_url, CURLUPART_URL, idn_full_url.as_ptr(), 0),
            CURLUE_OK
        );
        let mut puny_host = ptr::null_mut();
        assert_eq!(
            curl_url_get(puny_url, CURLUPART_HOST, &mut puny_host, CURLU_PUNYCODE),
            CURLUE_OK
        );
        assert_eq!(
            CStr::from_ptr(puny_host).to_bytes(),
            b"xn--rksmrgs-5wao1o.se"
        );
        curl_free(puny_host.cast());

        let puny_input_url = CString::new("https://xn--rksmrgs-5wao1o.se/").expect("cstring");
        assert_eq!(
            curl_url_set(puny_url, CURLUPART_URL, puny_input_url.as_ptr(), 0),
            CURLUE_OK
        );
        let mut idn_host = ptr::null_mut();
        assert_eq!(
            curl_url_get(puny_url, CURLUPART_HOST, &mut idn_host, CURLU_PUNY2IDN),
            CURLUE_OK
        );
        assert_eq!(
            CStr::from_ptr(idn_host).to_str().expect("utf8"),
            "räksmörgås.se"
        );
        curl_free(idn_host.cast());

        let query_url = curl_url();
        assert!(!query_url.is_null());
        let default_scheme_url = CString::new("https://example.com/").expect("cstring");
        assert_eq!(
            curl_url_set(
                query_url,
                CURLUPART_URL,
                default_scheme_url.as_ptr(),
                CURLU_DEFAULT_SCHEME
            ),
            CURLUE_OK
        );
        let query_text = CString::new("first value").expect("cstring");
        assert_eq!(
            curl_url_set(
                query_url,
                CURLUPART_QUERY,
                query_text.as_ptr(),
                CURLU_APPENDQUERY | CURLU_URLENCODE,
            ),
            CURLUE_OK
        );
        let mut query_url_text = ptr::null_mut();
        assert_eq!(
            curl_url_get(query_url, CURLUPART_URL, &mut query_url_text, 0),
            CURLUE_OK
        );
        assert_eq!(
            CStr::from_ptr(query_url_text).to_bytes(),
            b"https://example.com/?first+value"
        );
        curl_free(query_url_text.cast());

        let url_copy = curl_url_dup(url);
        assert!(!url_copy.is_null());
        let mut url_text = ptr::null_mut();
        assert_eq!(
            curl_url_get(url_copy, CURLUPART_URL, &mut url_text, 0),
            CURLUE_OK
        );
        assert!(is_tracked(url_text.cast()));
        assert!(CStr::from_ptr(url_text).to_bytes().starts_with(b"https://"));
        curl_free(url_text.cast());
        assert!(!curl_url_strerror(CURLUE_OK).is_null());

        let dup = curl_easy_duphandle(easy);
        assert!(!dup.is_null());
        let multi = curl_multi_init();
        assert!(!multi.is_null());
        assert_eq!(curl_multi_add_handle(multi, dup), CURLM_OK);
        let handles = curl_multi_get_handles(multi);
        assert!(!handles.is_null());
        assert!(is_tracked(handles.cast()));
        assert_eq!(*handles, dup);
        assert!((*handles.add(1)).is_null());
        curl_free(handles.cast());
        assert_eq!(curl_multi_remove_handle(multi, dup), CURLM_OK);
        assert_eq!(curl_multi_cleanup(multi), CURLM_OK);
        curl_easy_reset(dup);
        curl_easy_cleanup(dup);

        let (base_url, fixture_join) = spawn_getinfo_fixture();
        let redirect_url = CString::new(format!("{base_url}/redirect")).expect("cstring");
        let target_url = format!("{base_url}/target");

        curl_easy_reset(easy);
        assert_eq!(
            curl_easy_setopt(easy, CURLOPT_URL, redirect_url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(easy, CURLOPT_FOLLOWLOCATION, 1 as c_long),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(easy, CURLOPT_NOBODY, 1 as c_long),
            CURLE_OK
        );
        assert_eq!(curl_easy_perform(easy), CURLE_OK);

        let mut effective_url = ptr::null_mut();
        assert_eq!(
            curl_easy_getinfo(easy, CURLINFO_EFFECTIVE_URL, &mut effective_url),
            CURLE_OK
        );
        assert_eq!(
            CStr::from_ptr(effective_url).to_bytes(),
            target_url.as_bytes()
        );

        let mut content_type = ptr::null_mut();
        assert_eq!(
            curl_easy_getinfo(easy, CURLINFO_CONTENT_TYPE, &mut content_type),
            CURLE_OK
        );
        assert_eq!(CStr::from_ptr(content_type).to_bytes(), b"text/plain");

        let mut redirect_count = 0i64 as c_long;
        assert_eq!(
            curl_easy_getinfo(easy, CURLINFO_REDIRECT_COUNT, &mut redirect_count),
            CURLE_OK
        );
        assert_eq!(redirect_count, 1);

        let mut redirect_time = 0 as curl_off_t;
        assert_eq!(
            curl_easy_getinfo(easy, CURLINFO_REDIRECT_TIME_T, &mut redirect_time),
            CURLE_OK
        );
        assert!(redirect_time >= 0);

        let mut appconnect_time = -1 as curl_off_t;
        assert_eq!(
            curl_easy_getinfo(easy, CURLINFO_APPCONNECT_TIME_T, &mut appconnect_time),
            CURLE_OK
        );
        assert_eq!(appconnect_time, 0);

        curl_easy_reset(easy);
        assert_eq!(
            curl_easy_setopt(easy, CURLOPT_URL, redirect_url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(easy, CURLOPT_NOBODY, 1 as c_long),
            CURLE_OK
        );
        assert_eq!(curl_easy_perform(easy), CURLE_OK);

        let mut redirect_target = ptr::null_mut();
        assert_eq!(
            curl_easy_getinfo(easy, CURLINFO_REDIRECT_URL, &mut redirect_target),
            CURLE_OK
        );
        assert_eq!(
            CStr::from_ptr(redirect_target).to_bytes(),
            target_url.as_bytes()
        );
        assert_eq!(
            curl_easy_getinfo(easy, CURLINFO_EFFECTIVE_URL, &mut effective_url),
            CURLE_OK
        );
        assert_eq!(
            CStr::from_ptr(effective_url).to_bytes(),
            redirect_url.as_bytes()
        );
        assert_eq!(
            curl_easy_getinfo(easy, CURLINFO_REDIRECT_COUNT, &mut redirect_count),
            CURLE_OK
        );
        assert_eq!(redirect_count, 0);
        fixture_join.join().expect("fixture join");

        let (doh_url, doh_rx, doh_join) = spawn_doh_fixture();
        let (doh_target_base_url, doh_target_rx, doh_target_join) = spawn_capture_fixture(1);
        let doh_target_port = doh_target_base_url.rsplit(':').next().expect("target port");
        let doh_target_url =
            CString::new(format!("http://native-doh.test:{doh_target_port}/doh")).expect("cstring");
        let doh_url = CString::new(doh_url).expect("cstring");
        let expected_qname = b"\x0anative-doh\x04test\x00";

        curl_easy_reset(easy);
        assert_eq!(
            curl_easy_setopt(easy, CURLOPT_URL, doh_target_url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(easy, CURLOPT_DOH_URL, doh_url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(easy, CURLOPT_NOBODY, 1 as c_long),
            CURLE_OK
        );
        assert_eq!(curl_easy_perform(easy), CURLE_OK);

        let first_doh = doh_rx.recv().expect("first doh request");
        let second_doh = doh_rx.recv().expect("second doh request");
        assert!(
            first_doh
                .windows(expected_qname.len())
                .any(|window| window == expected_qname)
                || second_doh
                    .windows(expected_qname.len())
                    .any(|window| window == expected_qname)
        );
        let target_request =
            String::from_utf8(doh_target_rx.recv().expect("doh target request")).expect("utf8");
        assert!(target_request.starts_with("HEAD /doh HTTP/1.1\r\n"));
        assert!(target_request.contains(&format!("Host: native-doh.test:{doh_target_port}\r\n")));
        doh_join.join().expect("doh fixture join");
        doh_target_join.join().expect("doh target fixture join");

        let (post_base_url, post_rx, post_join) = spawn_capture_fixture(2);

        curl_easy_reset(easy);
        let mime_url = CString::new(format!("{post_base_url}/mime")).expect("cstring");
        assert_eq!(
            curl_easy_setopt(easy, CURLOPT_URL, mime_url.as_ptr()),
            CURLE_OK
        );
        let mime_headers_text = CString::new("X-Test: native-mime").expect("cstring");
        let mime_upload_headers = curl_slist_append(ptr::null_mut(), mime_headers_text.as_ptr());
        assert!(!mime_upload_headers.is_null());
        let mime_upload = curl_mime_init(easy);
        assert!(!mime_upload.is_null());
        let mime_part = curl_mime_addpart(mime_upload);
        assert!(!mime_part.is_null());
        assert_eq!(curl_mime_name(mime_part, c_ptr(b"field\0")), CURLE_OK);
        assert_eq!(
            curl_mime_filename(mime_part, c_ptr(b"value.txt\0")),
            CURLE_OK
        );
        assert_eq!(curl_mime_type(mime_part, c_ptr(b"text/plain\0")), CURLE_OK);
        assert_eq!(curl_mime_encoder(mime_part, c_ptr(b"binary\0")), CURLE_OK);
        let callback_body = Box::new(CallbackBody {
            bytes: b"value from callback",
            offset: 0,
        });
        assert_eq!(
            port_safe_export_curl_mime_data_cb(
                mime_part,
                callback_body.bytes.len() as curl_off_t,
                Some(mime_read_callback),
                Some(mime_seek_callback),
                Some(mime_free_callback),
                Box::into_raw(callback_body).cast(),
            ),
            CURLE_OK
        );
        assert_eq!(
            curl_mime_headers(mime_part, mime_upload_headers, 0),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(easy, CURLOPT_MIMEPOST, mime_upload),
            CURLE_OK
        );
        assert_eq!(curl_easy_perform(easy), CURLE_OK);
        let mime_request = String::from_utf8(post_rx.recv().expect("mime request")).expect("utf8");
        assert!(mime_request.starts_with("POST /mime HTTP/1.1\r\n"));
        assert!(mime_request.contains("Content-Type: multipart/form-data; boundary=------------------------port-libcurl-safe-mime-"));
        assert!(mime_request
            .contains("Content-Disposition: form-data; name=\"field\"; filename=\"value.txt\""));
        assert!(mime_request.contains("Content-Type: text/plain"));
        assert!(mime_request.contains("Content-Transfer-Encoding: binary"));
        assert!(mime_request.contains("X-Test: native-mime"));
        assert!(mime_request.contains("value from callback"));
        assert_eq!(
            curl_easy_setopt(easy, CURLOPT_MIMEPOST, ptr::null_mut::<curl_mime>()),
            CURLE_OK
        );
        curl_mime_free(mime_upload);
        curl_slist_free_all(mime_upload_headers);

        curl_easy_reset(easy);
        let form_url = CString::new(format!("{post_base_url}/form")).expect("cstring");
        assert_eq!(
            curl_easy_setopt(easy, CURLOPT_URL, form_url.as_ptr()),
            CURLE_OK
        );
        let mut form = ptr::null_mut();
        let mut last = ptr::null_mut();
        let form_name = CString::new("legacy").expect("cstring");
        let form_value = CString::new("value").expect("cstring");
        let form_type = CString::new("text/plain").expect("cstring");
        let form_spec = TestFormSpec {
            name: form_name.as_ptr(),
            namelength: 0,
            contents: form_value.as_ptr(),
            contentslength: form_value.as_bytes().len() as c_long,
            contenttype: form_type.as_ptr(),
            contentheader: ptr::null_mut(),
            filename: ptr::null(),
            filepath: ptr::null(),
            buffer_name: ptr::null(),
            buffer_ptr: ptr::null(),
            buffer_length: 0,
            stream: ptr::null_mut(),
            contentlen: form_value.as_bytes().len() as curl_off_t,
            flags: FORM_FLAG_PTR_CONTENTS,
        };
        assert_eq!(
            port_safe_formadd_parsed(&mut form, &mut last, &form_spec),
            CURL_FORMADD_OK
        );
        assert_eq!(curl_easy_setopt(easy, CURLOPT_HTTPPOST, form), CURLE_OK);
        assert_eq!(curl_easy_perform(easy), CURLE_OK);
        let form_request = String::from_utf8(post_rx.recv().expect("form request")).expect("utf8");
        assert!(form_request.starts_with("POST /form HTTP/1.1\r\n"));
        assert!(form_request.contains(
            "Content-Type: multipart/form-data; boundary=------------------------port-libcurl-safe"
        ));
        assert!(form_request.contains("Content-Disposition: form-data; name=\"legacy\""));
        assert!(form_request.contains("Content-Type: text/plain"));
        assert!(form_request.contains("\r\n\r\nvalue\r\n"));
        post_join.join().expect("post fixture join");
        port_safe_export_curl_formfree(form);

        let fmt = CString::new("hello %s %d").expect("cstring");
        let world = CString::new("world").expect("cstring");
        let rendered = curl_maprintf(fmt.as_ptr(), world.as_ptr(), 7i32);
        assert!(!rendered.is_null());
        assert!(is_tracked(rendered.cast()));
        assert_eq!(CStr::from_ptr(rendered).to_bytes(), b"hello world 7");
        curl_free(rendered.cast());

        curl_mime_free(mime);
        curl_slist_free_all(mime_headers);
        curl_url_cleanup(query_url);
        curl_url_cleanup(puny_url);
        curl_url_cleanup(url_copy);
        curl_url_cleanup(url);
        assert_eq!(curl_share_cleanup(share), CURLSHE_OK);
        curl_easy_cleanup(easy);
        curl_global_cleanup();

        assert!(!is_tracked(version.cast()));
    }
}
