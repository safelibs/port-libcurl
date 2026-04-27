use port_libcurl_safe::abi::{
    curl_header, curl_pushheaders, curl_slist, curl_waitfd, curl_ws_frame, CURLHcode, CURLMcode,
    CURLMoption, CURLMsg, CURLcode, CURLM, CURL,
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::ffi::{c_char, c_long, c_void, CStr, CString};
use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::ptr;
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CURLOPT_URL: u32 = 10002;
const CURLOPT_FOLLOWLOCATION: u32 = 52;
const CURLOPT_READDATA: u32 = 10009;
const CURLOPT_READFUNCTION: u32 = 20012;
const CURLOPT_XOAUTH2_BEARER: u32 = 10220;
const CURLOPT_AUTOREFERER: u32 = 58;
const CURLOPT_RESOLVE: u32 = 10203;
const CURLOPT_NETRC: u32 = 51;
const CURLOPT_NETRC_FILE: u32 = 10118;
const CURLOPT_PROXY: u32 = 10004;
const CURLOPT_PROXYUSERPWD: u32 = 10006;
const CURLOPT_COOKIEFILE: u32 = 10031;
const CURLOPT_HTTPHEADER: u32 = 10023;
const CURLOPT_POSTFIELDS: u32 = 10015;
const CURLOPT_CONNECT_ONLY: u32 = 141;
const CURLOPT_ALTSVC_CTRL: u32 = 286;
const CURLOPT_ALTSVC: u32 = 10287;
const CURLOPT_HSTS_CTRL: u32 = 299;
const CURLOPT_HSTS: u32 = 10300;
const CURLOPT_WRITEFUNCTION: u32 = 20011;
const CURLOPT_SSL_VERIFYPEER: u32 = 64;
const CURLOPT_SSL_VERIFYHOST: u32 = 81;
const CURLOPT_UPLOAD: u32 = 46;
const CURLOPT_INFILESIZE_LARGE: u32 = 30115;

const CURL_GLOBAL_DEFAULT: c_long = 3;
const CURL_NETRC_OPTIONAL: c_long = 1;
const CURLH_HEADER: u32 = 1 << 0;
const CURLH_TRAILER: u32 = 1 << 1;
const CURLWS_TEXT: u32 = 1 << 0;
const CURLWS_CLOSE: u32 = 1 << 3;
const CURLPAUSE_RECV: i32 = 1 << 0;
const CURLPAUSE_SEND: i32 = 1 << 2;
const CURLMOPT_PIPELINING: CURLMoption = 3;
const CURLMOPT_PUSHFUNCTION: CURLMoption = 20014;
const CURLMOPT_PUSHDATA: CURLMoption = 10015;
const CURLPIPE_MULTIPLEX: c_long = 2;
const CURLMSG_DONE: u32 = 1;

const CURLE_OK: CURLcode = 0;
const CURLE_BAD_CONTENT_ENCODING: CURLcode = 61;
const CURLE_AGAIN: CURLcode = 81;
const CURLE_RECV_ERROR: CURLcode = 56;
const CURLM_OK: CURLMcode = 0;
const CURL_PUSH_OK: i32 = 0;

unsafe extern "C" {
    fn curl_global_init(flags: c_long) -> CURLcode;
    fn curl_global_cleanup();
    fn curl_easy_init() -> *mut CURL;
    fn curl_easy_cleanup(handle: *mut CURL);
    fn curl_easy_perform(handle: *mut CURL) -> CURLcode;
    fn curl_easy_pause(handle: *mut CURL, bitmask: i32) -> CURLcode;
    fn curl_easy_recv(
        handle: *mut CURL,
        buffer: *mut c_void,
        buflen: usize,
        n: *mut usize,
    ) -> CURLcode;
    fn curl_easy_send(
        handle: *mut CURL,
        buffer: *const c_void,
        buflen: usize,
        n: *mut usize,
    ) -> CURLcode;
    fn curl_easy_setopt(handle: *mut CURL, option: u32, ...) -> CURLcode;
    fn curl_easy_upkeep(handle: *mut CURL) -> CURLcode;
    fn curl_easy_header(
        easy: *mut CURL,
        name: *const c_char,
        index: usize,
        origin: u32,
        request: i32,
        hout: *mut *mut curl_header,
    ) -> CURLHcode;
    fn curl_easy_nextheader(
        easy: *mut CURL,
        origin: u32,
        request: i32,
        prev: *mut curl_header,
    ) -> *mut curl_header;
    fn curl_slist_append(list: *mut curl_slist, data: *const c_char) -> *mut curl_slist;
    fn curl_slist_free_all(list: *mut curl_slist);
    fn curl_multi_init() -> *mut CURLM;
    fn curl_multi_cleanup(multi: *mut CURLM) -> CURLMcode;
    fn curl_multi_add_handle(multi: *mut CURLM, easy: *mut CURL) -> CURLMcode;
    fn curl_multi_remove_handle(multi: *mut CURLM, easy: *mut CURL) -> CURLMcode;
    fn curl_multi_perform(multi: *mut CURLM, running_handles: *mut i32) -> CURLMcode;
    fn curl_multi_poll(
        multi: *mut CURLM,
        extra_fds: *mut curl_waitfd,
        extra_nfds: u32,
        timeout_ms: i32,
        ret: *mut i32,
    ) -> CURLMcode;
    fn curl_multi_info_read(multi: *mut CURLM, msgs_in_queue: *mut i32) -> *mut CURLMsg;
    fn curl_multi_setopt(multi: *mut CURLM, option: CURLMoption, ...) -> CURLMcode;
    fn curl_pushheader_byname(
        headers: *mut curl_pushheaders,
        name: *const c_char,
    ) -> *mut c_char;
    fn curl_pushheader_bynum(headers: *mut curl_pushheaders, index: usize) -> *mut c_char;
    fn curl_ws_send(
        curl: *mut CURL,
        buffer: *const c_void,
        buflen: usize,
        sent: *mut usize,
        fragsize: i64,
        flags: u32,
    ) -> CURLcode;
    fn curl_ws_recv(
        curl: *mut CURL,
        buffer: *mut c_void,
        buflen: usize,
        recv: *mut usize,
        metap: *mut *const curl_ws_frame,
    ) -> CURLcode;
}

fn safe_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn serialized_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct CurlGuard;

impl CurlGuard {
    fn new() -> Self {
        let code = unsafe { curl_global_init(CURL_GLOBAL_DEFAULT) };
        assert_eq!(code, CURLE_OK);
        Self
    }
}

impl Drop for CurlGuard {
    fn drop(&mut self) {
        unsafe { curl_global_cleanup() };
    }
}

struct EasyHandle(*mut CURL);

impl EasyHandle {
    fn new() -> Self {
        let handle = unsafe { curl_easy_init() };
        assert!(!handle.is_null());
        Self(handle)
    }

    fn as_ptr(&self) -> *mut CURL {
        self.0
    }
}

impl Drop for EasyHandle {
    fn drop(&mut self) {
        unsafe { curl_easy_cleanup(self.0) };
    }
}

struct Slist {
    head: *mut curl_slist,
    keepalive: Vec<CString>,
}

impl Slist {
    fn new() -> Self {
        Self {
            head: ptr::null_mut(),
            keepalive: Vec::new(),
        }
    }

    fn push(&mut self, value: impl Into<String>) {
        let value = CString::new(value.into()).expect("cstring");
        self.head = unsafe { curl_slist_append(self.head, value.as_ptr()) };
        assert!(!self.head.is_null());
        self.keepalive.push(value);
    }

    fn as_ptr(&self) -> *mut curl_slist {
        self.head
    }
}

impl Drop for Slist {
    fn drop(&mut self) {
        unsafe { curl_slist_free_all(self.head) };
    }
}

struct MultiHandle(*mut CURLM);

impl MultiHandle {
    fn new() -> Self {
        let handle = unsafe { curl_multi_init() };
        assert!(!handle.is_null());
        Self(handle)
    }

    fn as_ptr(&self) -> *mut CURLM {
        self.0
    }
}

impl Drop for MultiHandle {
    fn drop(&mut self) {
        unsafe {
            assert_eq!(curl_multi_cleanup(self.0), CURLM_OK);
        }
    }
}

#[derive(Default)]
struct PushCapture {
    count: usize,
    path: Option<String>,
    first_header: Option<String>,
}

struct HttpsFixture {
    port: u16,
    child: Child,
}

impl Drop for HttpsFixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

unsafe extern "C" fn sink_write(
    _buffer: *mut c_char,
    size: usize,
    nmemb: usize,
    _userdata: *mut c_void,
) -> usize {
    size.saturating_mul(nmemb)
}

struct UploadSource {
    body: Vec<u8>,
    offset: usize,
    calls: usize,
}

unsafe extern "C" fn upload_read(
    buffer: *mut c_char,
    size: usize,
    nmemb: usize,
    userdata: *mut c_void,
) -> usize {
    if buffer.is_null() || userdata.is_null() {
        return 0;
    }
    let source = unsafe { &mut *(userdata as *mut UploadSource) };
    source.calls = source.calls.saturating_add(1);
    let capacity = size.saturating_mul(nmemb);
    let remaining = &source.body[source.offset.min(source.body.len())..];
    let amount = remaining.len().min(capacity);
    unsafe {
        ptr::copy_nonoverlapping(remaining.as_ptr(), buffer.cast::<u8>(), amount);
    }
    source.offset = source.offset.saturating_add(amount);
    amount
}

unsafe extern "C" fn capture_push_callback(
    _parent: *mut CURL,
    easy: *mut CURL,
    _num_headers: usize,
    headers: *mut curl_pushheaders,
    userp: *mut c_void,
) -> i32 {
    if userp.is_null() {
        return 1;
    }
    let state = unsafe { &mut *(userp as *mut PushCapture) };
    let path_name = CString::new(":path").expect("path name");
    let path = unsafe { curl_pushheader_byname(headers, path_name.as_ptr()) };
    if !path.is_null() {
        state.path = Some(
            unsafe { CStr::from_ptr(path) }
                .to_str()
                .expect("push path")
                .to_string(),
        );
    }
    let first = unsafe { curl_pushheader_bynum(headers, 0) };
    if !first.is_null() {
        state.first_header = Some(
            unsafe { CStr::from_ptr(first) }
                .to_str()
                .expect("push header")
                .to_string(),
        );
    }
    state.count += 1;

    if unsafe {
        curl_easy_setopt(
            easy,
            CURLOPT_WRITEFUNCTION,
            sink_write as unsafe extern "C" fn(*mut c_char, usize, usize, *mut c_void) -> usize,
        )
    } != CURLE_OK
    {
        return 1;
    }
    CURL_PUSH_OK
}

#[derive(Debug)]
struct CaseDescriptor {
    id: String,
    kind: String,
}

#[derive(Debug)]
struct MappingEntry {
    cve_id: String,
    case_file: String,
    shared_case: bool,
    justification: Option<String>,
}

fn mapping_json() -> &'static Value {
    static VALUE: OnceLock<Value> = OnceLock::new();
    VALUE.get_or_init(|| {
        serde_json::from_str(include_str!("../metadata/cve-to-test.json")).expect("mapping")
    })
}

fn manifest_json() -> &'static Value {
    static VALUE: OnceLock<Value> = OnceLock::new();
    VALUE.get_or_init(|| {
        serde_json::from_str(include_str!("../metadata/cve-manifest.json")).expect("manifest")
    })
}

fn mapping_entries() -> Vec<MappingEntry> {
    mapping_json()["mappings"]
        .as_array()
        .expect("mappings")
        .iter()
        .map(|entry| MappingEntry {
            cve_id: entry["cve_id"].as_str().expect("cve_id").to_string(),
            case_file: entry["case_file"].as_str().expect("case_file").to_string(),
            shared_case: entry["shared_case"].as_bool().expect("shared_case"),
            justification: entry["justification"].as_str().map(str::to_string),
        })
        .collect()
}

fn case_descriptor(path: &Path) -> CaseDescriptor {
    let value: Value =
        serde_json::from_str(&fs::read_to_string(path).expect("case file")).expect("case json");
    CaseDescriptor {
        id: value["id"].as_str().expect("case id").to_string(),
        kind: value["kind"].as_str().expect("case kind").to_string(),
    }
}

fn case_root() -> PathBuf {
    safe_dir().join("tests").join("cve_cases")
}

fn runtime_case_ids() -> BTreeSet<String> {
    [
        "proxy_auth_reuse",
        "redirect_credentials",
        "cookie_origin_scope",
        "headers_api_semantics",
        "hsts_domain_scope",
        "http_content_encoding_limit",
        "http_method_state",
        "websocket_mask_entropy",
        "websocket_recv_progress",
        "response_header_limit",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

#[test]
fn mapping_and_case_inventory_stay_in_sync() {
    let mapping = mapping_entries();
    let mapped_cves = mapping
        .iter()
        .map(|entry| entry.cve_id.clone())
        .collect::<BTreeSet<_>>();
    let curated_cves = manifest_json()["curated_relevant_cves"]["cves"]
        .as_array()
        .expect("curated cves")
        .iter()
        .map(|entry| entry["cve_id"].as_str().expect("cve_id").to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(mapped_cves, curated_cves);

    let mapped_files = mapping
        .iter()
        .map(|entry| entry.case_file.clone())
        .collect::<BTreeSet<_>>();
    let actual_files = fs::read_dir(case_root())
        .expect("case dir")
        .map(|entry| {
            entry
                .expect("case dir entry")
                .file_name()
                .into_string()
                .expect("utf-8")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(mapped_files, actual_files);

    let known_case_ids = runtime_case_ids()
        .into_iter()
        .chain(
            [
                "proxy_auth_reuse",
                "hsts_domain_scope",
                "reference_backend_transport",
                "reference_backend_parsing",
                "reference_backend_resource",
                "reference_backend_platform",
                "reference_backend_permissions",
                "reference_backend_ssh_auth",
                "reference_backend_concurrency",
            ]
            .into_iter()
            .map(str::to_string),
        )
        .collect::<BTreeSet<_>>();

    for entry in &mapping {
        let path = case_root().join(&entry.case_file);
        assert!(path.is_file(), "missing case file {}", path.display());
        let descriptor = case_descriptor(&path);
        assert!(
            known_case_ids.contains(&descriptor.id),
            "unknown case id {} from {}",
            descriptor.id,
            path.display()
        );
        if entry.shared_case {
            assert!(
                entry
                    .justification
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|value| !value.is_empty()),
                "shared mapping missing justification for {}",
                entry.cve_id
            );
        }
    }
}

#[test]
fn runtime_cases_execute_from_mapping() {
    let _guard = serialized_test_lock().lock().expect("test lock");
    let _curl = CurlGuard::new();

    let mut executed = BTreeSet::new();
    for case_file in mapping_entries()
        .into_iter()
        .map(|entry| entry.case_file)
        .collect::<BTreeSet<_>>()
    {
        let descriptor = case_descriptor(&case_root().join(&case_file));
        if descriptor.kind != "runtime" {
            continue;
        }
        if !executed.insert(descriptor.id.clone()) {
            continue;
        }
        match descriptor.id.as_str() {
            "proxy_auth_reuse" => run_proxy_auth_reuse_case(),
            "redirect_credentials" => run_redirect_credentials_case(),
            "cookie_origin_scope" => run_cookie_origin_scope_case(),
            "headers_api_semantics" => run_headers_api_case(),
            "hsts_domain_scope" => run_hsts_domain_scope_case(),
            "http_content_encoding_limit" => run_http_content_encoding_limit_case(),
            "http_method_state" => run_http_method_state_case(),
            "websocket_mask_entropy" => run_websocket_mask_entropy_case(),
            "websocket_recv_progress" => run_websocket_recv_progress_case(),
            "response_header_limit" => run_response_header_limit_case(),
            other => panic!("unimplemented runtime case {other}"),
        }
    }
}

fn run_proxy_auth_reuse_case() {
    let server = spawn_scripted_http_server(vec![
        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_string(),
    ]);
    let url = CString::new(format!("http://127.0.0.1:{}/direct", server.port)).expect("url");
    let mut headers = Slist::new();
    headers.push("Proxy-Authorization: Basic Zm9vOmJhcg==");
    let handle = EasyHandle::new();
    unsafe {
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_URL, url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_HTTPHEADER, headers.as_ptr()),
            CURLE_OK
        );
        assert_eq!(curl_easy_perform(handle.as_ptr()), CURLE_OK);
    }
    server.join.join().expect("direct server");
    let requests = server.requests.lock().expect("requests").clone();
    assert!(header(&requests[0], "Proxy-Authorization").is_none());

    let proxy = spawn_scripted_http_server(vec![
        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_string(),
    ]);
    let url = CString::new("http://origin.test/resource").expect("url");
    let proxy_url =
        CString::new(format!("http://127.0.0.1:{}", proxy.port)).expect("proxy url");
    let proxy_userpwd = CString::new("alice:secret").expect("proxy creds");
    let handle = EasyHandle::new();
    unsafe {
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_URL, url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_PROXY, proxy_url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_PROXYUSERPWD, proxy_userpwd.as_ptr()),
            CURLE_OK
        );
        assert_eq!(curl_easy_perform(handle.as_ptr()), CURLE_OK);
    }
    proxy.join.join().expect("proxy server");
    let requests = proxy.requests.lock().expect("proxy requests").clone();
    assert!(
        requests[0].starts_with("GET http://origin.test/resource HTTP/1.1"),
        "unexpected proxied request line: {}",
        requests[0].lines().next().unwrap_or_default()
    );
    assert!(header(&requests[0], "Proxy-Authorization").is_some());
}

fn run_redirect_credentials_case() {
    let server = spawn_scripted_http_server_with_port(|port| {
        vec![
            format!(
                "HTTP/1.1 302 Found\r\nContent-Length: 0\r\nConnection: close\r\nLocation: http://b.test:{port}/target\r\n\r\n"
            ),
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_string(),
        ]
    });
    let port = server.port;

    let mut resolve = Slist::new();
    resolve.push(format!("a.test:{port}:127.0.0.1"));
    resolve.push(format!("b.test:{port}:127.0.0.1"));

    let url = CString::new(format!("http://user:pass@a.test:{port}/start")).expect("url");
    let token = CString::new("SECRET_TOKEN").expect("token");
    let handle = EasyHandle::new();
    unsafe {
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_URL, url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_RESOLVE, resolve.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_XOAUTH2_BEARER, token.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_FOLLOWLOCATION, 1 as c_long),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_AUTOREFERER, 1 as c_long),
            CURLE_OK
        );
        assert_eq!(curl_easy_perform(handle.as_ptr()), CURLE_OK);
    }
    server.join.join().expect("redirect server");
    let requests = server.requests.lock().expect("requests").clone();
    assert_eq!(requests.len(), 2);
    assert!(header(&requests[0], "Authorization").is_some());
    assert!(header(&requests[1], "Authorization").is_none());
    let expected_referer = format!("http://a.test:{port}/start");
    assert_eq!(
        header(&requests[1], "Referer").as_deref(),
        Some(expected_referer.as_str())
    );

    let server = spawn_scripted_http_server_with_port(|port| {
        vec![
            format!(
                "HTTP/1.1 302 Found\r\nContent-Length: 0\r\nConnection: close\r\nLocation: http://b.test:{port}/target\r\n\r\n"
            ),
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_string(),
        ]
    });
    let port = server.port;
    let mut resolve = Slist::new();
    resolve.push(format!("a.test:{port}:127.0.0.1"));
    resolve.push(format!("b.test:{port}:127.0.0.1"));
    let netrc_path = temp_path("netrc");
    fs::write(
        &netrc_path,
        "machine a.test login alice password alicespassword\ndefault\n",
    )
    .expect("write netrc");
    let netrc_path_c = CString::new(netrc_path.to_string_lossy().into_owned()).expect("netrc");
    let url = CString::new(format!("http://a.test:{port}/netrc")).expect("url");
    let handle = EasyHandle::new();
    unsafe {
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_URL, url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_RESOLVE, resolve.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_NETRC, CURL_NETRC_OPTIONAL),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_NETRC_FILE, netrc_path_c.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_FOLLOWLOCATION, 1 as c_long),
            CURLE_OK
        );
        assert_eq!(curl_easy_perform(handle.as_ptr()), CURLE_OK);
    }
    server.join.join().expect("netrc redirect server");
    let requests = server.requests.lock().expect("requests").clone();
    assert_eq!(requests.len(), 2);
    assert!(header(&requests[0], "Authorization").is_some());
    assert!(header(&requests[1], "Authorization").is_none());
}

fn run_cookie_origin_scope_case() {
    let server = spawn_scripted_http_server(vec![
        "HTTP/1.1 200 OK\r\nSet-Cookie: sid=one; Domain=a.test; Path=/\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_string(),
        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_string(),
        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_string(),
    ]);
    let port = server.port;

    let mut resolve = Slist::new();
    resolve.push(format!("a.test:{port}:127.0.0.1"));
    resolve.push(format!("b.test:{port}:127.0.0.1"));

    let handle = EasyHandle::new();
    for url in [
        format!("http://a.test:{port}/set"),
        format!("http://a.test:{port}/check"),
        format!("http://b.test:{port}/check"),
    ] {
        let url = CString::new(url).expect("url");
        unsafe {
            assert_eq!(
                curl_easy_setopt(handle.as_ptr(), CURLOPT_URL, url.as_ptr()),
                CURLE_OK
            );
            assert_eq!(
                curl_easy_setopt(handle.as_ptr(), CURLOPT_RESOLVE, resolve.as_ptr()),
                CURLE_OK
            );
            assert_eq!(curl_easy_perform(handle.as_ptr()), CURLE_OK);
        }
    }

    server.join.join().expect("cookie server");
    let requests = server.requests.lock().expect("requests").clone();
    assert_eq!(requests.len(), 3);
    assert!(header(&requests[1], "Cookie").is_some());
    assert!(header(&requests[2], "Cookie").is_none());
}

fn run_headers_api_case() {
    let server = spawn_scripted_http_server(vec![
        "HTTP/1.1 200 OK\r\nSet-Cookie: one=data\r\nSet-Cookie: two=more\r\nX-Test: yes\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_string(),
    ]);
    let url = CString::new(format!("http://127.0.0.1:{}/headers", server.port)).expect("url");
    let handle = EasyHandle::new();
    unsafe {
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_URL, url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(curl_easy_perform(handle.as_ptr()), CURLE_OK);
    }
    server.join.join().expect("headers server");

    let mut header_ptr = ptr::null_mut();
    let name = CString::new("Set-Cookie").expect("name");
    let code = unsafe {
        curl_easy_header(
            handle.as_ptr(),
            name.as_ptr(),
            1,
            CURLH_HEADER,
            -1,
            &mut header_ptr,
        )
    };
    assert_eq!(code, 0);
    assert!(!header_ptr.is_null());
    assert_eq!(unsafe { (*header_ptr).amount }, 2);
    assert_eq!(unsafe { (*header_ptr).index }, 1);
    assert_eq!(
        unsafe { CStr::from_ptr((*header_ptr).value) }
            .to_str()
            .expect("value"),
        "two=more"
    );

    let mut seen = Vec::new();
    let mut cursor = ptr::null_mut();
    loop {
        cursor = unsafe { curl_easy_nextheader(handle.as_ptr(), CURLH_HEADER, -1, cursor) };
        if cursor.is_null() {
            break;
        }
        seen.push(
            unsafe { CStr::from_ptr((*cursor).name) }
                .to_str()
                .expect("name")
                .to_string(),
        );
    }
    assert!(seen.contains(&"Set-Cookie".to_string()));
    assert!(seen.contains(&"X-Test".to_string()));

    let server = spawn_scripted_http_server(vec![
        "HTTP/1.1 200 OK\r\nLink: </push/asset.txt>; rel=preload\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_string(),
        "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nasset".to_string(),
    ]);
    let mut resolve = Slist::new();
    resolve.push(format!("push.test:{}:127.0.0.1", server.port));
    let url = CString::new(format!("http://push.test:{}/push", server.port)).expect("url");
    let multi = MultiHandle::new();
    let handle = EasyHandle::new();
    let mut push_state = Box::new(PushCapture::default());
    unsafe {
        assert_eq!(
            curl_multi_setopt(
                multi.as_ptr(),
                CURLMOPT_PIPELINING,
                CURLPIPE_MULTIPLEX as c_long
            ),
            CURLM_OK
        );
        assert_eq!(
            curl_multi_setopt(
                multi.as_ptr(),
                CURLMOPT_PUSHFUNCTION,
                capture_push_callback as unsafe extern "C" fn(
                    *mut CURL,
                    *mut CURL,
                    usize,
                    *mut curl_pushheaders,
                    *mut c_void,
                ) -> i32,
            ),
            CURLM_OK
        );
        assert_eq!(
            curl_multi_setopt(
                multi.as_ptr(),
                CURLMOPT_PUSHDATA,
                (&mut *push_state) as *mut PushCapture as *mut c_void
            ),
            CURLM_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_URL, url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_RESOLVE, resolve.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(
                handle.as_ptr(),
                CURLOPT_WRITEFUNCTION,
                sink_write as unsafe extern "C" fn(*mut c_char, usize, usize, *mut c_void) -> usize,
            ),
            CURLE_OK
        );
        assert_eq!(
            curl_multi_add_handle(multi.as_ptr(), handle.as_ptr()),
            CURLM_OK
        );
    }

    let start = std::time::Instant::now();
    let mut running = 1;
    let mut primary_done = false;
    while !primary_done || running > 0 {
        unsafe {
            assert_eq!(
                curl_multi_perform(multi.as_ptr(), &mut running),
                CURLM_OK
            );
            let mut messages = 0;
            loop {
                let message = curl_multi_info_read(multi.as_ptr(), &mut messages);
                if message.is_null() {
                    break;
                }
                if (*message).msg != CURLMSG_DONE {
                    continue;
                }
                let easy = (*message).easy_handle;
                assert_eq!(curl_multi_remove_handle(multi.as_ptr(), easy), CURLM_OK);
                if easy == handle.as_ptr() {
                    primary_done = true;
                } else {
                    curl_easy_cleanup(easy);
                }
            }
            if running > 0 {
                let mut ready = 0;
                assert_eq!(
                    curl_multi_poll(multi.as_ptr(), ptr::null_mut(), 0, 100, &mut ready),
                    CURLM_OK
                );
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "timed out waiting for synthetic push transfer"
        );
    }

    server.join.join().expect("push server");
    assert_eq!(push_state.count, 1);
    assert_eq!(push_state.path.as_deref(), Some("/push/asset.txt"));
    assert!(
        push_state
            .first_header
            .as_deref()
            .is_some_and(|value| value.contains(":method: GET"))
    );
}

fn run_hsts_domain_scope_case() {
    let https = spawn_https_fixture();
    let hsts_file = temp_path("hsts-runtime");
    fs::write(&hsts_file, ".hsts.test \"20991231 23:59:59\"\n").expect("write hsts");
    let mut resolve = Slist::new();
    resolve.push(format!("a.hsts.test:{}:127.0.0.1", https.port));
    let url = CString::new(format!("http://a.hsts.test:{}/upgrade", https.port)).expect("url");
    let hsts_file_c = CString::new(hsts_file.to_string_lossy().into_owned()).expect("hsts");
    let handle = EasyHandle::new();
    unsafe {
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_URL, url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_RESOLVE, resolve.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_HSTS, hsts_file_c.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_HSTS_CTRL, 1 as c_long),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_SSL_VERIFYPEER, 0 as c_long),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_SSL_VERIFYHOST, 0 as c_long),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(
                handle.as_ptr(),
                CURLOPT_WRITEFUNCTION,
                sink_write as unsafe extern "C" fn(*mut c_char, usize, usize, *mut c_void) -> usize,
            ),
            CURLE_OK
        );
        assert_eq!(curl_easy_perform(handle.as_ptr()), CURLE_OK);
    }
    let text = fs::read_to_string(&hsts_file).expect("read hsts");
    assert!(text.contains(".hsts.test"));
    let _ = fs::remove_file(hsts_file);
}

fn run_websocket_mask_entropy_case() {
    let (tx, rx) = mpsc::channel();
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind ws");
    let port = listener.local_addr().expect("addr").port();
    let join = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept ws");
        let _ = read_http_headers(&mut stream);
        stream
            .write_all(
                b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Accept: ignored\r\n\r\n",
            )
            .expect("write handshake");
        let first = read_ws_frame(&mut stream).expect("first frame");
        let second = read_ws_frame(&mut stream).expect("second frame");
        let _ = read_ws_frame(&mut stream);
        tx.send((first.mask, second.mask)).expect("send masks");
        let _ = stream.shutdown(Shutdown::Both);
    });

    let url = CString::new(format!("ws://127.0.0.1:{port}/echo")).expect("url");
    let handle = EasyHandle::new();
    unsafe {
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_URL, url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_CONNECT_ONLY, 2 as c_long),
            CURLE_OK
        );
        assert_eq!(curl_easy_perform(handle.as_ptr()), CURLE_OK);
    }

    let mut sent = 0usize;
    unsafe {
        assert_eq!(
            curl_ws_send(
                handle.as_ptr(),
                b"one".as_ptr().cast(),
                3,
                &mut sent,
                0,
                CURLWS_TEXT,
            ),
            CURLE_OK
        );
        assert_eq!(
            curl_ws_send(
                handle.as_ptr(),
                b"two".as_ptr().cast(),
                3,
                &mut sent,
                0,
                CURLWS_TEXT,
            ),
            CURLE_OK
        );
        assert_eq!(
            curl_ws_send(handle.as_ptr(), ptr::null(), 0, &mut sent, 0, CURLWS_CLOSE),
            CURLE_OK
        );
    }

    join.join().expect("ws mask server");
    let (first, second) = rx.recv().expect("masks");
    assert_ne!(first, second);
}

fn run_websocket_recv_progress_case() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind ws");
    let port = listener.local_addr().expect("addr").port();
    let join = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept ws");
        let _ = read_http_headers(&mut stream);
        stream
            .write_all(
                b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Accept: ignored\r\n\r\n",
            )
            .expect("write handshake");
        stream.write_all(&[0x89, 0x00]).expect("write ping");
        stream
            .write_all(&[0x81, 0x02, b'o', b'k'])
            .expect("write text");
        stream.flush().expect("flush ws");
        let _ = stream.shutdown(Shutdown::Both);
    });

    let url = CString::new(format!("ws://127.0.0.1:{port}/echo")).expect("url");
    let handle = EasyHandle::new();
    unsafe {
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_URL, url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_CONNECT_ONLY, 2 as c_long),
            CURLE_OK
        );
        assert_eq!(curl_easy_perform(handle.as_ptr()), CURLE_OK);
    }

    let mut recv_len = 0usize;
    let mut meta = ptr::null();
    let mut buffer = [0u8; 16];
    let start = std::time::Instant::now();
    loop {
        let rc = unsafe {
            curl_ws_recv(
                handle.as_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut recv_len,
                &mut meta,
            )
        };
        if rc == CURLE_OK {
            break;
        }
        assert_eq!(rc, CURLE_AGAIN);
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "timed out waiting for websocket payload"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(&buffer[..recv_len], b"ok");
    assert!(!meta.is_null());
    assert_eq!(unsafe { (*meta).len }, 2);
    join.join().expect("ws recv server");

    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind masked ws");
    let port = listener.local_addr().expect("addr").port();
    let join = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept ws");
        let _ = read_http_headers(&mut stream);
        stream
            .write_all(
                b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Accept: ignored\r\n\r\n",
            )
            .expect("write handshake");
        stream
            .write_all(&[0x81, 0x82, 1, 2, 3, 4, b'o' ^ 1, b'k' ^ 2])
            .expect("write masked text");
        stream.flush().expect("flush masked ws");
        let _ = stream.shutdown(Shutdown::Both);
    });

    let url = CString::new(format!("ws://127.0.0.1:{port}/echo")).expect("url");
    let handle = EasyHandle::new();
    unsafe {
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_URL, url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_CONNECT_ONLY, 2 as c_long),
            CURLE_OK
        );
        assert_eq!(curl_easy_perform(handle.as_ptr()), CURLE_OK);
    }
    let mut recv_len = 0usize;
    let mut meta = ptr::null();
    let mut buffer = [0u8; 16];
    let code = unsafe {
        curl_ws_recv(
            handle.as_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut recv_len,
            &mut meta,
        )
    };
    assert_eq!(code, CURLE_RECV_ERROR);
    join.join().expect("masked ws server");
}

fn run_response_header_limit_case() {
    let huge = "x".repeat(310_000);
    let server = spawn_scripted_http_server(vec![format!(
        "HTTP/1.1 200 OK\r\nX-Huge: {huge}\r\nConnection: close\r\n\r\n"
    )]);
    let url = CString::new(format!("http://127.0.0.1:{}/huge", server.port)).expect("url");
    let handle = EasyHandle::new();
    let code = unsafe {
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_URL, url.as_ptr()),
            CURLE_OK
        );
        curl_easy_perform(handle.as_ptr())
    };
    assert_eq!(code, CURLE_RECV_ERROR);
    server.join.join().expect("huge headers server");
}

fn run_http_content_encoding_limit_case() {
    let server = spawn_scripted_http_server(vec![concat!(
        "HTTP/1.1 200 OK\r\n",
        "Content-Encoding: gzip\r\n",
        "Transfer-Encoding: deflate\r\n",
        "Content-Encoding: br\r\n",
        "Transfer-Encoding: gzip\r\n",
        "Content-Encoding: zstd\r\n",
        "Transfer-Encoding: compress\r\n",
        "Connection: close\r\n\r\n"
    )
    .to_string()]);
    let url = CString::new(format!("http://127.0.0.1:{}/encoded", server.port)).expect("url");
    let handle = EasyHandle::new();
    let code = unsafe {
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_URL, url.as_ptr()),
            CURLE_OK
        );
        curl_easy_perform(handle.as_ptr())
    };
    assert_eq!(code, CURLE_BAD_CONTENT_ENCODING);
    server.join.join().expect("content encoding server");
}

fn run_http_method_state_case() {
    let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_string();
    let server = spawn_body_recording_server(vec![response.clone(), response]);
    let url = CString::new(format!("http://127.0.0.1:{}/state", server.port)).expect("url");
    let post_body = CString::new("post-body").expect("post body");
    let put_body = b"put-body".to_vec();
    let mut upload = UploadSource {
        body: put_body.clone(),
        offset: 0,
        calls: 0,
    };
    let handle = EasyHandle::new();
    unsafe {
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_URL, url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(
                handle.as_ptr(),
                CURLOPT_WRITEFUNCTION,
                sink_write as unsafe extern "C" fn(*mut c_char, usize, usize, *mut c_void) -> usize,
            ),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(
                handle.as_ptr(),
                CURLOPT_READDATA,
                (&mut upload as *mut UploadSource).cast::<c_void>(),
            ),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(
                handle.as_ptr(),
                CURLOPT_READFUNCTION,
                upload_read as unsafe extern "C" fn(*mut c_char, usize, usize, *mut c_void) -> usize,
            ),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(
                handle.as_ptr(),
                CURLOPT_INFILESIZE_LARGE,
                put_body.len() as i64,
            ),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_UPLOAD, 1 as c_long),
            CURLE_OK
        );
        assert_eq!(curl_easy_perform(handle.as_ptr()), CURLE_OK);
    }
    let calls_after_put = upload.calls;
    assert!(calls_after_put > 0, "PUT transfer never used the read callback");

    unsafe {
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_POSTFIELDS, post_body.as_ptr()),
            CURLE_OK
        );
        assert_eq!(curl_easy_perform(handle.as_ptr()), CURLE_OK);
    }
    assert_eq!(upload.calls, calls_after_put);

    server.join.join().expect("method state server");
    let requests = server.requests.lock().expect("requests").clone();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].head.starts_with("PUT /state HTTP/1.1\r\n"));
    assert_eq!(requests[0].body, put_body);
    assert!(requests[1].head.starts_with("POST /state HTTP/1.1\r\n"));
    assert_eq!(requests[1].body, post_body.as_bytes());
}

#[derive(Clone, Debug)]
struct RecordedRequest {
    head: String,
    body: Vec<u8>,
}

struct ScriptedServer {
    port: u16,
    requests: std::sync::Arc<Mutex<Vec<String>>>,
    join: thread::JoinHandle<()>,
}

struct BodyRecordingServer {
    port: u16,
    requests: std::sync::Arc<Mutex<Vec<RecordedRequest>>>,
    join: thread::JoinHandle<()>,
}

fn spawn_scripted_http_server(responses: Vec<String>) -> ScriptedServer {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind server");
    let port = listener.local_addr().expect("addr").port();
    let requests = std::sync::Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    let join = thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_http_headers(&mut stream);
            captured.lock().expect("requests").push(request);
            stream
                .write_all(response.as_bytes())
                .expect("write response");
            stream.flush().expect("flush response");
            let _ = stream.shutdown(Shutdown::Both);
        }
    });
    ScriptedServer {
        port,
        requests,
        join,
    }
}

fn spawn_body_recording_server(responses: Vec<String>) -> BodyRecordingServer {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind server");
    let port = listener.local_addr().expect("addr").port();
    let requests = std::sync::Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    let join = thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_http_request(&mut stream);
            captured.lock().expect("requests").push(request);
            stream
                .write_all(response.as_bytes())
                .expect("write response");
            stream.flush().expect("flush response");
            let _ = stream.shutdown(Shutdown::Both);
        }
    });
    BodyRecordingServer {
        port,
        requests,
        join,
    }
}

fn spawn_scripted_http_server_with_port(
    build_responses: impl FnOnce(u16) -> Vec<String> + Send + 'static,
) -> ScriptedServer {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind server");
    let port = listener.local_addr().expect("addr").port();
    let responses = build_responses(port);
    let requests = std::sync::Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    let join = thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_http_headers(&mut stream);
            captured.lock().expect("requests").push(request);
            stream
                .write_all(response.as_bytes())
                .expect("write response");
            stream.flush().expect("flush response");
            let _ = stream.shutdown(Shutdown::Both);
        }
    });
    ScriptedServer {
        port,
        requests,
        join,
    }
}

fn read_http_headers(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut buf = [0u8; 1024];
    loop {
        let read = stream.read(&mut buf).expect("read request");
        assert!(read != 0, "connection closed before complete request");
        bytes.extend_from_slice(&buf[..read]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8(bytes).expect("utf-8 request")
}

fn read_http_request(stream: &mut TcpStream) -> RecordedRequest {
    let mut bytes = Vec::new();
    let mut buf = [0u8; 1024];
    let header_end = loop {
        let read = stream.read(&mut buf).expect("read request");
        assert!(read != 0, "connection closed before complete request");
        bytes.extend_from_slice(&buf[..read]);
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let head = String::from_utf8(bytes[..header_end].to_vec()).expect("utf-8 request");
    let content_length = header(&head, "Content-Length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = bytes[header_end..].to_vec();
    while body.len() < content_length {
        let read = stream.read(&mut buf).expect("read request body");
        assert!(read != 0, "connection closed before complete request body");
        body.extend_from_slice(&buf[..read]);
    }
    body.truncate(content_length);
    RecordedRequest { head, body }
}

fn header(request: &str, name: &str) -> Option<String> {
    request
        .lines()
        .find_map(|line| {
            line.split_once(':')
                .filter(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        })
        .map(|(_, value)| value.trim().to_string())
}

#[derive(Debug)]
struct WsFrame {
    mask: [u8; 4],
}

fn read_ws_frame(stream: &mut TcpStream) -> Option<WsFrame> {
    let mut header = [0u8; 2];
    if stream.read_exact(&mut header).is_err() {
        return None;
    }
    let mut payload_len = (header[1] & 0x7f) as usize;
    if payload_len == 126 {
        let mut extended = [0u8; 2];
        stream.read_exact(&mut extended).expect("extended");
        payload_len = u16::from_be_bytes(extended) as usize;
    } else if payload_len == 127 {
        let mut extended = [0u8; 8];
        stream.read_exact(&mut extended).expect("extended");
        payload_len = u64::from_be_bytes(extended) as usize;
    }
    let mut mask = [0u8; 4];
    stream.read_exact(&mut mask).expect("mask");
    let mut payload = vec![0u8; payload_len];
    stream.read_exact(&mut payload).expect("payload");
    Some(WsFrame { mask })
}

fn temp_path(stem: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("port-libcurl-safe-{stem}-{nanos}.tmp"))
}

fn pick_unused_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("bind port picker")
        .local_addr()
        .expect("port picker addr")
        .port()
}

fn wait_for_port(port: u16, child: &mut Child) {
    for _ in 0..100 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        if child.try_wait().expect("poll https child").is_some() {
            panic!("openssl s_server exited before becoming ready");
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("openssl s_server did not become ready on port {port}");
}

fn spawn_https_fixture() -> HttpsFixture {
    let port = pick_unused_port();
    let cert_dir = safe_dir().join("vendor").join("upstream").join("tests").join("certs");
    let cert = cert_dir.join("Server-localhost-sv.pem");
    let key = cert_dir.join("Server-localhost-sv.key");
    let mut child = Command::new("openssl")
        .arg("s_server")
        .arg("-accept")
        .arg(port.to_string())
        .arg("-cert")
        .arg(&cert)
        .arg("-key")
        .arg(&key)
        .arg("-www")
        .arg("-quiet")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn openssl s_server");
    wait_for_port(port, &mut child);
    HttpsFixture { port, child }
}

#[test]
fn cookiefile_preloads_into_native_requests() {
    let _guard = serialized_test_lock().lock().expect("test lock");
    let _curl = CurlGuard::new();

    let cookie_file = temp_path("cookiefile");
    fs::write(
        &cookie_file,
        "# Netscape HTTP Cookie File\nexample.test\tFALSE\t/\tFALSE\t2147483647\tsid\tone\n",
    )
    .expect("cookie file");
    let server = spawn_scripted_http_server(vec![
        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_string(),
    ]);
    let mut resolve = Slist::new();
    resolve.push(format!("example.test:{}:127.0.0.1", server.port));
    let url = CString::new(format!("http://example.test:{}/check", server.port)).expect("url");
    let cookie_file_c =
        CString::new(cookie_file.to_string_lossy().into_owned()).expect("cookie file");
    let handle = EasyHandle::new();
    unsafe {
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_URL, url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_RESOLVE, resolve.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_COOKIEFILE, cookie_file_c.as_ptr()),
            CURLE_OK
        );
        assert_eq!(curl_easy_perform(handle.as_ptr()), CURLE_OK);
    }
    server.join.join().expect("cookiefile server");
    let requests = server.requests.lock().expect("requests").clone();
    assert_eq!(header(&requests[0], "Cookie").as_deref(), Some("sid=one"));
    let _ = fs::remove_file(cookie_file);
}

#[test]
fn chunked_response_trailers_reach_header_api() {
    let _guard = serialized_test_lock().lock().expect("test lock");
    let _curl = CurlGuard::new();

    let server = spawn_scripted_http_server(vec![
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n2\r\nok\r\n0\r\nX-Trailer: done\r\n\r\n".to_string(),
    ]);
    let url = CString::new(format!("http://127.0.0.1:{}/trailers", server.port)).expect("url");
    let handle = EasyHandle::new();
    unsafe {
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_URL, url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(curl_easy_perform(handle.as_ptr()), CURLE_OK);
    }
    server.join.join().expect("trailer server");

    let mut header_ptr = ptr::null_mut();
    let name = CString::new("X-Trailer").expect("name");
    let code = unsafe {
        curl_easy_header(
            handle.as_ptr(),
            name.as_ptr(),
            0,
            CURLH_TRAILER,
            -1,
            &mut header_ptr,
        )
    };
    assert_eq!(code, 0);
    assert_eq!(
        unsafe { CStr::from_ptr((*header_ptr).value) }
            .to_str()
            .expect("value"),
        "done"
    );
}

#[test]
fn hsts_and_altsvc_headers_persist_from_native_transfer() {
    let _guard = serialized_test_lock().lock().expect("test lock");
    let _curl = CurlGuard::new();

    let hsts_file = temp_path("hsts");
    let altsvc_file = temp_path("altsvc");
    let server = spawn_scripted_http_server(vec![
        "HTTP/1.1 200 OK\r\nStrict-Transport-Security: max-age=31536000; includeSubDomains\r\nAlt-Svc: h2=\":8443\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
    ]);
    let mut resolve = Slist::new();
    resolve.push(format!("example.test:{}:127.0.0.1", server.port));
    let url = CString::new(format!("http://example.test:{}/state", server.port)).expect("url");
    let hsts_file_c = CString::new(hsts_file.to_string_lossy().into_owned()).expect("hsts");
    let altsvc_file_c = CString::new(altsvc_file.to_string_lossy().into_owned()).expect("altsvc");
    let handle = EasyHandle::new();
    unsafe {
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_URL, url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_RESOLVE, resolve.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_HSTS, hsts_file_c.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_HSTS_CTRL, 1 as c_long),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_ALTSVC, altsvc_file_c.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_ALTSVC_CTRL, 1 as c_long),
            CURLE_OK
        );
        assert_eq!(curl_easy_perform(handle.as_ptr()), CURLE_OK);
    }
    server.join.join().expect("state server");
    let hsts_text = fs::read_to_string(&hsts_file).expect("read hsts");
    let altsvc_text = fs::read_to_string(&altsvc_file).expect("read altsvc");
    assert!(hsts_text.contains(".example.test"));
    assert!(altsvc_text.contains("h2=\"example.test:8443\""));
    let _ = fs::remove_file(hsts_file);
    let _ = fs::remove_file(altsvc_file);
}

#[test]
fn native_connect_only_send_recv_pause_and_upkeep() {
    let _guard = serialized_test_lock().lock().expect("test lock");
    let _curl = CurlGuard::new();

    let (tx, rx) = mpsc::channel();
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let join = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buf = [0u8; 4];
        stream.read_exact(&mut buf).expect("read ping");
        tx.send(buf).expect("send ping");
        stream.write_all(b"pong").expect("write pong");
        stream.flush().expect("flush pong");
        let _ = stream.shutdown(Shutdown::Both);
    });

    let url = CString::new(format!("http://127.0.0.1:{port}/raw")).expect("url");
    let handle = EasyHandle::new();
    unsafe {
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_URL, url.as_ptr()),
            CURLE_OK
        );
        assert_eq!(
            curl_easy_setopt(handle.as_ptr(), CURLOPT_CONNECT_ONLY, 1 as c_long),
            CURLE_OK
        );
        assert_eq!(curl_easy_perform(handle.as_ptr()), CURLE_OK);
        assert_eq!(curl_easy_upkeep(handle.as_ptr()), CURLE_OK);
        assert_eq!(curl_easy_pause(handle.as_ptr(), CURLPAUSE_SEND), CURLE_OK);
    }

    let mut written = 0usize;
    let rc = unsafe { curl_easy_send(handle.as_ptr(), b"ping".as_ptr().cast(), 4, &mut written) };
    assert_eq!(rc, CURLE_AGAIN);
    assert_eq!(written, 0);

    unsafe {
        assert_eq!(curl_easy_pause(handle.as_ptr(), 0), CURLE_OK);
    }
    let start = std::time::Instant::now();
    loop {
        let rc =
            unsafe { curl_easy_send(handle.as_ptr(), b"ping".as_ptr().cast(), 4, &mut written) };
        if rc == CURLE_OK {
            break;
        }
        assert_eq!(rc, CURLE_AGAIN);
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "timed out waiting to send"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(written, 4);
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("server saw ping"),
        *b"ping"
    );

    unsafe {
        assert_eq!(curl_easy_pause(handle.as_ptr(), CURLPAUSE_RECV), CURLE_OK);
    }
    let mut recv_len = 0usize;
    let mut buffer = [0u8; 16];
    let rc = unsafe {
        curl_easy_recv(
            handle.as_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut recv_len,
        )
    };
    assert_eq!(rc, CURLE_AGAIN);
    assert_eq!(recv_len, 0);

    unsafe {
        assert_eq!(curl_easy_pause(handle.as_ptr(), 0), CURLE_OK);
    }
    let start = std::time::Instant::now();
    loop {
        let rc = unsafe {
            curl_easy_recv(
                handle.as_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut recv_len,
            )
        };
        if rc == CURLE_OK && recv_len > 0 {
            break;
        }
        assert_eq!(rc, CURLE_AGAIN);
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "timed out waiting to recv"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(&buffer[..recv_len], b"pong");

    join.join().expect("connect-only server");
}
