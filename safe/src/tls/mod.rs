pub(crate) mod certinfo;
pub(crate) mod gnutls;
pub(crate) mod openssl;

use crate::abi::CURLcode;
use crate::easy::perform::EasyMetadata;
use crate::protocols::TransferRoute;
use core::ffi::{c_char, c_int};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Mutex, OnceLock};

const CURLE_SSL_CONNECT_ERROR: CURLcode = 35;
const CURLE_PEER_FAILED_VERIFICATION: CURLcode = 60;
const CURLE_SSL_PINNEDPUBKEYNOTMATCH: CURLcode = 90;

#[derive(Clone, Debug)]
pub(crate) struct TlsPolicy {
    pub backend: &'static str,
    pub scheme: &'static str,
    pub verify_peer: bool,
    pub verify_host: bool,
    pub alpn: bool,
    pub certinfo: bool,
    pub pinned_public_key: Option<String>,
    pub session_cache_scope: String,
}

#[repr(C)]
struct SafeTlsConnection {
    _opaque: [u8; 0],
}

unsafe extern "C" {
    fn curl_safe_tls_connect(
        fd: c_int,
        host: *const c_char,
        verify_peer: c_int,
        verify_host: c_int,
        enable_alpn: c_int,
        pinned_public_key: *const c_char,
        session_data: *const u8,
        session_len: usize,
        out: *mut *mut SafeTlsConnection,
        out_session_data: *mut *mut u8,
        out_session_len: *mut usize,
        errbuf: *mut c_char,
        errlen: usize,
    ) -> c_int;
    fn curl_safe_tls_read(
        conn: *mut SafeTlsConnection,
        buf: *mut core::ffi::c_void,
        len: usize,
    ) -> isize;
    fn curl_safe_tls_write(
        conn: *mut SafeTlsConnection,
        buf: *const core::ffi::c_void,
        len: usize,
    ) -> isize;
    fn curl_safe_tls_close(conn: *mut SafeTlsConnection);
    fn curl_safe_tls_free_bytes(ptr: *mut u8);
}

trait TlsBackendAdapter {
    fn name(&self) -> &'static str;
    fn build_policy(&self, scheme: &'static str, metadata: &EasyMetadata) -> TlsPolicy;
    fn session_cache_key(&self, policy: &TlsPolicy, host: &str, port: u16) -> String;
    fn classify_connect_error(&self, message: &str) -> CURLcode;
}

pub(crate) struct TlsConnection {
    raw: *mut SafeTlsConnection,
    stream: TcpStream,
}

unsafe impl Send for TlsConnection {}

impl TlsConnection {
    pub(crate) fn set_read_timeout(
        &self,
        timeout: Option<std::time::Duration>,
    ) -> std::io::Result<()> {
        self.stream.set_read_timeout(timeout)
    }

    pub(crate) fn set_write_timeout(
        &self,
        timeout: Option<std::time::Duration>,
    ) -> std::io::Result<()> {
        self.stream.set_write_timeout(timeout)
    }
}

impl Drop for TlsConnection {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe { curl_safe_tls_close(self.raw) };
            self.raw = core::ptr::null_mut();
        }
    }
}

impl Read for TlsConnection {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let rc = unsafe { curl_safe_tls_read(self.raw, buf.as_mut_ptr().cast(), buf.len()) };
        if rc >= 0 {
            Ok(rc as usize)
        } else {
            Err(std::io::Error::other("tls read failed"))
        }
    }
}

impl Write for TlsConnection {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let rc = unsafe { curl_safe_tls_write(self.raw, buf.as_ptr().cast(), buf.len()) };
        if rc >= 0 {
            Ok(rc as usize)
        } else {
            Err(std::io::Error::other("tls write failed"))
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(crate) fn backend_name() -> &'static str {
    current_backend().name()
}

fn current_backend() -> &'static dyn TlsBackendAdapter {
    if cfg!(feature = "openssl-flavor") {
        &openssl::BACKEND
    } else {
        &gnutls::BACKEND
    }
}

pub(crate) fn backend_cache_fragment() -> &'static str {
    if cfg!(feature = "openssl-flavor") {
        openssl::cache_fragment()
    } else {
        gnutls::cache_fragment()
    }
}

pub(crate) fn is_tls_scheme(scheme: &str) -> bool {
    matches!(
        scheme,
        "https" | "wss" | "ftps" | "imaps" | "pop3s" | "smtps" | "ldaps"
    )
}

pub(crate) fn policy_for_route(route: TransferRoute, metadata: &EasyMetadata) -> Option<TlsPolicy> {
    if !route.tls {
        return None;
    }

    let scheme = match route.handler {
        crate::protocols::SchemeHandler::Http => {
            if route.websocket_mode {
                "wss"
            } else {
                "https"
            }
        }
        crate::protocols::SchemeHandler::WebSocket => "wss",
        crate::protocols::SchemeHandler::Ftp => "ftps",
        crate::protocols::SchemeHandler::Imap => "imaps",
        crate::protocols::SchemeHandler::Pop3 => "pop3s",
        crate::protocols::SchemeHandler::Smtp => "smtps",
        crate::protocols::SchemeHandler::Ldap => "ldaps",
        _ => "https",
    };

    Some(current_backend().build_policy(scheme, metadata))
}

pub(crate) fn peer_identity(metadata: &EasyMetadata) -> Option<String> {
    let policy = current_backend().build_policy("https", metadata);
    let mut parts = vec![
        format!("backend={}", policy.backend),
        format!("scheme={}", policy.scheme),
        format!("verify_peer={}", policy.verify_peer),
        format!("verify_host={}", policy.verify_host),
        format!("alpn={}", policy.alpn),
        format!("certinfo={}", policy.certinfo),
        format!("session-cache={}", policy.session_cache_scope),
    ];
    if let Some(pinned_key) = policy.pinned_public_key.as_ref() {
        parts.push(format!("pinned={pinned_key}"));
    }
    Some(parts.join(";"))
}

pub(crate) fn connect(
    stream: TcpStream,
    host: &str,
    port: u16,
    metadata: &EasyMetadata,
    policy: &TlsPolicy,
) -> Result<TlsConnection, CURLcode> {
    let session_key = current_backend().session_cache_key(policy, host, port);
    let cached_session = load_cached_session(metadata.share_handle, &session_key);
    let host_c = std::ffi::CString::new(host).map_err(|_| CURLE_PEER_FAILED_VERIFICATION)?;
    let pinned_c = policy
        .pinned_public_key
        .as_ref()
        .map(|value| std::ffi::CString::new(value.as_str()))
        .transpose()
        .map_err(|_| CURLE_SSL_PINNEDPUBKEYNOTMATCH)?;
    let mut raw = core::ptr::null_mut();
    let mut new_session = core::ptr::null_mut();
    let mut new_session_len = 0usize;
    let mut errbuf = [0i8; 256];
    let fd = std::os::fd::AsRawFd::as_raw_fd(&stream);
    let rc = unsafe {
        curl_safe_tls_connect(
            fd,
            host_c.as_ptr(),
            policy.verify_peer as c_int,
            policy.verify_host as c_int,
            policy.alpn as c_int,
            pinned_c
                .as_ref()
                .map_or(core::ptr::null(), |value| value.as_ptr()),
            cached_session
                .as_ref()
                .map_or(core::ptr::null(), |value| value.as_ptr()),
            cached_session.as_ref().map_or(0, Vec::len),
            &mut raw,
            &mut new_session,
            &mut new_session_len,
            errbuf.as_mut_ptr(),
            errbuf.len(),
        )
    };
    if rc != 0 || raw.is_null() {
        let message = unsafe { std::ffi::CStr::from_ptr(errbuf.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        return Err(current_backend().classify_connect_error(&message));
    }

    if !new_session.is_null() && new_session_len != 0 {
        let bytes = unsafe { std::slice::from_raw_parts(new_session, new_session_len) }.to_vec();
        store_cached_session(metadata.share_handle, session_key, bytes);
        unsafe { curl_safe_tls_free_bytes(new_session) };
    }

    Ok(TlsConnection { raw, stream })
}

fn shared_session_cache() -> &'static Mutex<HashMap<String, Vec<u8>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn load_cached_session(share_handle: Option<usize>, key: &str) -> Option<Vec<u8>> {
    if let Some(found) = crate::share::with_shared_ssl_sessions_mut(share_handle, |sessions| {
        sessions.get(key).cloned()
    }) {
        return found;
    }
    shared_session_cache()
        .lock()
        .expect("tls session cache mutex poisoned")
        .get(key)
        .cloned()
}

fn store_cached_session(share_handle: Option<usize>, key: String, session: Vec<u8>) {
    if crate::share::with_shared_ssl_sessions_mut(share_handle, |sessions| {
        sessions.insert(key.clone(), session.clone());
    })
    .is_some()
    {
        return;
    }
    shared_session_cache()
        .lock()
        .expect("tls session cache mutex poisoned")
        .insert(key, session);
}

pub(crate) fn classify_connect_error(message: &str) -> CURLcode {
    current_backend().classify_connect_error(message)
}

pub(crate) const fn default_connect_error() -> CURLcode {
    CURLE_SSL_CONNECT_ERROR
}
