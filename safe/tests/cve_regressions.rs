use port_libcurl_safe::abi::{curl_header, curl_slist, curl_ws_frame, CURLHcode, CURLcode, CURL};
use serde_json::Value;
use std::collections::BTreeSet;
use std::ffi::{c_char, c_long, c_void, CStr, CString};
use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

const CURLOPT_URL: u32 = 10002;
const CURLOPT_FOLLOWLOCATION: u32 = 52;
const CURLOPT_XOAUTH2_BEARER: u32 = 10220;
const CURLOPT_AUTOREFERER: u32 = 58;
const CURLOPT_RESOLVE: u32 = 10203;
const CURLOPT_NETRC: u32 = 51;
const CURLOPT_NETRC_FILE: u32 = 10118;
const CURLOPT_CONNECT_ONLY: u32 = 141;

const CURL_GLOBAL_DEFAULT: c_long = 3;
const CURL_NETRC_OPTIONAL: c_long = 1;
const CURLH_HEADER: u32 = 1 << 0;
const CURLWS_TEXT: u32 = 1 << 0;
const CURLWS_CLOSE: u32 = 1 << 3;

const CURLE_OK: CURLcode = 0;
const CURLE_RECV_ERROR: CURLcode = 56;

unsafe extern "C" {
    fn curl_global_init(flags: c_long) -> CURLcode;
    fn curl_global_cleanup();
    fn curl_easy_init() -> *mut CURL;
    fn curl_easy_cleanup(handle: *mut CURL);
    fn curl_easy_perform(handle: *mut CURL) -> CURLcode;
    fn curl_easy_setopt(handle: *mut CURL, option: u32, ...) -> CURLcode;
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
        "redirect_credentials",
        "cookie_origin_scope",
        "headers_api_semantics",
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
                "reference_backend_method_state",
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
            "redirect_credentials" => run_redirect_credentials_case(),
            "cookie_origin_scope" => run_cookie_origin_scope_case(),
            "headers_api_semantics" => run_headers_api_case(),
            "websocket_mask_entropy" => run_websocket_mask_entropy_case(),
            "websocket_recv_progress" => run_websocket_recv_progress_case(),
            "response_header_limit" => run_response_header_limit_case(),
            other => panic!("unimplemented runtime case {other}"),
        }
    }
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
    let rc = unsafe {
        curl_ws_recv(
            handle.as_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut recv_len,
            &mut meta,
        )
    };
    assert_eq!(rc, CURLE_OK);
    assert_eq!(&buffer[..recv_len], b"ok");
    assert!(!meta.is_null());
    assert_eq!(unsafe { (*meta).len }, 2);
    join.join().expect("ws recv server");
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

struct ScriptedServer {
    port: u16,
    requests: std::sync::Arc<Mutex<Vec<String>>>,
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
