pub(crate) const NAME: &str = "gnutls";
pub(crate) const UPSTREAM_SOURCES: &[&str] = &[
    "original/lib/vtls/gtls.c",
    "original/lib/vtls/vtls.c",
    "original/lib/vtls/x509asn1.c",
];

pub(crate) struct GnuTlsBackend;
pub(crate) const BACKEND: GnuTlsBackend = GnuTlsBackend;

pub(crate) fn cache_fragment() -> &'static str {
    "gnutls:ocsp+pinning+alpn+session-cache"
}

impl super::TlsBackendAdapter for GnuTlsBackend {
    fn name(&self) -> &'static str {
        NAME
    }

    fn build_policy(
        &self,
        scheme: &'static str,
        metadata: &crate::easy::perform::EasyMetadata,
    ) -> super::TlsPolicy {
        super::TlsPolicy {
            backend: NAME,
            scheme,
            verify_peer: metadata.ssl_verify_peer,
            verify_host: metadata.ssl_verify_host != 0,
            alpn: metadata.ssl_enable_alpn,
            certinfo: super::certinfo::requested(metadata.certinfo),
            pinned_public_key: metadata.pinned_public_key.clone(),
            session_cache_scope: cache_fragment().to_string(),
        }
    }

    fn execute(
        &self,
        handle: *mut crate::abi::CURL,
        _policy: &super::TlsPolicy,
    ) -> crate::abi::CURLcode {
        crate::protocols::perform_reference_bridge(handle)
    }
}
