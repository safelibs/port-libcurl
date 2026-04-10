pub(crate) mod certinfo;
pub(crate) mod gnutls;
pub(crate) mod openssl;

use crate::easy::perform::EasyMetadata;

pub(crate) fn backend_name() -> &'static str {
    if cfg!(feature = "openssl-flavor") {
        openssl::NAME
    } else {
        gnutls::NAME
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

pub(crate) fn peer_identity(metadata: &EasyMetadata) -> Option<String> {
    let mut parts = vec![
        format!("backend={}", backend_name()),
        format!("verify_peer={}", metadata.ssl_verify_peer),
        format!("verify_host={}", metadata.ssl_verify_host),
        format!("alpn={}", metadata.ssl_enable_alpn),
        format!("certinfo={}", certinfo::requested(metadata.certinfo)),
        backend_cache_fragment().to_string(),
    ];
    if let Some(pinned_key) = metadata.pinned_public_key.as_ref() {
        parts.push(format!("pinned={pinned_key}"));
    }
    Some(parts.join(";"))
}
