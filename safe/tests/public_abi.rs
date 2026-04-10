use port_libcurl_safe::abi::{
    curl_calloc_callback, curl_easyoption, curl_free_callback, curl_malloc_callback,
    curl_realloc_callback, curl_strdup_callback, curl_version_info_data, CURL, CURLM, CURLM_OK,
    CURLSH, CURLSHcode, CURLSHoption, CURLU, CURLUcode, CURLUPART_HOST, CURLUPART_URL, CURLcode,
    CURLoption, CURLversion, CURLMcode, CURLSHOPT_SHARE, CURLSHE_OK, CURLUE_OK, CURLVERSION_NOW,
    CURLE_OK, CURL_GLOBAL_DEFAULT, CURL_LOCK_DATA_COOKIE, CURLOT_STRING,
};
use port_libcurl_safe::BUILD_FLAVOR;
use std::collections::HashSet;
use std::ffi::{c_char, c_int, c_long, c_void, CStr, CString};
use std::ptr;
use std::sync::{Mutex, OnceLock};

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
    fn curl_easy_reset(handle: *mut CURL);
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
    tracking().lock().expect("tracking mutex poisoned").live.clear();
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

#[test]
fn public_abi_smoke_and_allocator_contract() {
    assert!(!BUILD_FLAVOR.is_empty());
    clear_tracking();
    std::env::set_var("PORT_LIBCURL_SAFE_ALLOC_TEST", "allocator-smoke");

    unsafe {
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

        let getenv_name = CString::new("PORT_LIBCURL_SAFE_ALLOC_TEST").expect("cstring");
        let getenv_value = curl_getenv(getenv_name.as_ptr());
        assert!(!getenv_value.is_null());
        assert_eq!(CStr::from_ptr(getenv_value).to_bytes(), b"allocator-smoke");
        assert!(is_tracked(getenv_value.cast()));
        curl_free(getenv_value.cast());
        assert!(!is_tracked(getenv_value.cast()));

        assert_eq!(curl_getdate(c_ptr(b"Sun, 06 Nov 1994 08:49:37 GMT\0"), ptr::null()), 784111777);
        assert_eq!(curl_strequal(c_ptr(b"AbC\0"), c_ptr(b"aBc\0")), 1);
        assert_eq!(curl_strnequal(c_ptr(b"AbCd\0"), c_ptr(b"aBcE\0"), 3), 1);

        let version = curl_version();
        assert!(!version.is_null());
        assert!(is_tracked(version.cast()));
        assert_eq!(version, curl_version());
        assert!(!(*curl_version_info(CURLVERSION_NOW)).version.is_null());

        let easy = curl_easy_init();
        assert!(!easy.is_null());

        let escaped = curl_easy_escape(easy, c_ptr(b"a/b?c=d\0"), 7);
        assert!(!escaped.is_null());
        assert!(is_tracked(escaped.cast()));

        let mut unescaped_len = 0;
        let unescaped = curl_easy_unescape(easy, escaped, 0, &mut unescaped_len);
        assert!(!unescaped.is_null());
        assert!(is_tracked(unescaped.cast()));
        assert_eq!(unescaped_len, 7);
        assert_eq!(std::slice::from_raw_parts(unescaped.cast::<u8>(), unescaped_len as usize), b"a/b?c=d");
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
        let full_url = CString::new("https://user:pass@example.com:9443/base/path?x=1#frag")
            .expect("cstring");
        assert_eq!(curl_url_set(url, CURLUPART_URL, full_url.as_ptr(), 0), CURLUE_OK);

        let mut host = ptr::null_mut();
        assert_eq!(curl_url_get(url, CURLUPART_HOST, &mut host, 0), CURLUE_OK);
        assert!(!host.is_null());
        assert!(is_tracked(host.cast()));
        assert_eq!(CStr::from_ptr(host).to_bytes(), b"example.com");
        curl_free(host.cast());

        let url_copy = curl_url_dup(url);
        assert!(!url_copy.is_null());
        let mut url_text = ptr::null_mut();
        assert_eq!(curl_url_get(url_copy, CURLUPART_URL, &mut url_text, 0), CURLUE_OK);
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

        let fmt = CString::new("hello %s %d").expect("cstring");
        let world = CString::new("world").expect("cstring");
        let rendered = curl_maprintf(fmt.as_ptr(), world.as_ptr(), 7i32);
        assert!(!rendered.is_null());
        assert!(is_tracked(rendered.cast()));
        assert_eq!(CStr::from_ptr(rendered).to_bytes(), b"hello world 7");
        curl_free(rendered.cast());

        curl_url_cleanup(url_copy);
        curl_url_cleanup(url);
        assert_eq!(curl_share_cleanup(share), CURLSHE_OK);
        curl_easy_cleanup(easy);
        curl_global_cleanup();

        assert!(!is_tracked(version.cast()));
    }
}
