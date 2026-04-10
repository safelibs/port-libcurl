pub(crate) mod certinfo;
pub(crate) mod gnutls;
pub(crate) mod openssl;

use crate::abi::{CURLcode, CURL};
use crate::easy::perform::EasyMetadata;
use crate::protocols::TransferRoute;

const CURLE_UNSUPPORTED_PROTOCOL: CURLcode = 1;

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

trait TlsBackendAdapter {
    fn name(&self) -> &'static str;
    fn build_policy(&self, scheme: &'static str, metadata: &EasyMetadata) -> TlsPolicy;
    fn execute(&self, handle: *mut CURL, policy: &TlsPolicy) -> CURLcode;
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

pub(crate) fn execute_route(
    handle: *mut CURL,
    route: TransferRoute,
    metadata: &EasyMetadata,
    _callbacks: crate::easy::perform::EasyCallbacks,
) -> CURLcode {
    let Some(policy) = policy_for_route(route, metadata) else {
        return CURLE_UNSUPPORTED_PROTOCOL;
    };
    current_backend().execute(handle, &policy)
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
