use port_libcurl_safe::abi::{
    curl_calloc_callback, curl_easyoption, curl_free_callback, curl_malloc_callback, curl_mime,
    curl_mimepart, curl_off_t, curl_realloc_callback, curl_slist, curl_ssl_backend,
    curl_sslbackend, curl_strdup_callback, curl_version_info_data, CURLMcode, CURLSHcode,
    CURLSHoption, CURLUcode, CURLcode, CURLoption, CURLsslset, CURLversion, CURL, CURLE_OK, CURLM,
    CURLM_OK, CURLOT_STRING, CURLSH, CURLSHE_OK, CURLSHOPT_SHARE, CURLSSLBACKEND_GNUTLS,
    CURLSSLBACKEND_OPENSSL, CURLSSLSET_OK, CURLSSLSET_TOO_LATE, CURLU, CURLUE_OK, CURLUPART_HOST,
    CURLUPART_URL, CURLVERSION_NOW, CURL_GLOBAL_DEFAULT, CURL_LOCK_DATA_COOKIE,
};
use port_libcurl_safe::BUILD_FLAVOR;
use std::collections::HashSet;
use std::ffi::{c_char, c_int, c_long, c_void, CStr, CString};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener};
use std::ptr;
use std::sync::{Mutex, OnceLock};
use std::thread;

const CURLOPT_MIMEPOST: CURLoption = 10269;
const CURLOPT_URL: CURLoption = 10002;
const CURLOPT_COOKIELIST: CURLoption = 10135;
const CURLOPT_CURLU: CURLoption = 10282;
const CURLOPT_FOLLOWLOCATION: CURLoption = 52;
const CURLOPT_NOBODY: CURLoption = 44;
const CURLINFO_EFFECTIVE_URL: u32 = 0x100000 + 1;
const CURLINFO_CONTENT_TYPE: u32 = 0x100000 + 18;
const CURLINFO_REDIRECT_COUNT: u32 = 0x200000 + 20;
const CURLINFO_REDIRECT_URL: u32 = 0x100000 + 31;
const CURLINFO_COOKIELIST: u32 = 0x400000 + 28;
const CURLINFO_SCHEME: u32 = 0x100000 + 49;
const CURLINFO_REDIRECT_TIME_T: u32 = 0x600000 + 55;
const CURLINFO_APPCONNECT_TIME_T: u32 = 0x600000 + 56;
const CURL_ZERO_TERMINATED: usize = usize::MAX;

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
    fn curl_slist_append(list: *mut curl_slist, string: *const c_char) -> *mut curl_slist;
    fn curl_slist_free_all(list: *mut curl_slist);

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
        assert!(!(*curl_version_info(CURLVERSION_NOW)).version.is_null());

        let easy = curl_easy_init();
        assert!(!easy.is_null());
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
        fixture_join.join().expect("fixture join");

        let fmt = CString::new("hello %s %d").expect("cstring");
        let world = CString::new("world").expect("cstring");
        let rendered = curl_maprintf(fmt.as_ptr(), world.as_ptr(), 7i32);
        assert!(!rendered.is_null());
        assert!(is_tracked(rendered.cast()));
        assert_eq!(CStr::from_ptr(rendered).to_bytes(), b"hello world 7");
        curl_free(rendered.cast());

        curl_mime_free(mime);
        curl_slist_free_all(mime_headers);
        curl_url_cleanup(url_copy);
        curl_url_cleanup(url);
        assert_eq!(curl_share_cleanup(share), CURLSHE_OK);
        curl_easy_cleanup(easy);
        curl_global_cleanup();

        assert!(!is_tracked(version.cast()));
    }
}
