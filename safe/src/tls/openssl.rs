pub(crate) const NAME: &str = "openssl";
pub(crate) const UPSTREAM_SOURCES: &[&str] = &[
    "original/lib/vtls/openssl.c",
    "original/lib/vtls/vtls.c",
    "original/lib/vtls/x509asn1.c",
];

pub(crate) struct OpenSslBackend;
pub(crate) const BACKEND: OpenSslBackend = OpenSslBackend;

pub(crate) fn cache_fragment() -> &'static str {
    "openssl:partial-chain+pinning+alpn+session-cache"
}

impl super::TlsBackendAdapter for OpenSslBackend {
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
            alpn: super::enable_http_alpn(scheme, metadata),
            certinfo: super::certinfo::requested(metadata.certinfo),
            pinned_public_key: metadata.pinned_public_key.clone(),
            session_cache_scope: cache_fragment().to_string(),
        }
    }

    fn session_cache_key(&self, policy: &super::TlsPolicy, host: &str, port: u16) -> String {
        format!(
            "{};openssl;{};{};verify={};host={};alpn={}",
            policy.session_cache_scope,
            host,
            port,
            policy.verify_peer,
            policy.verify_host,
            policy.alpn
        )
    }

    fn classify_connect_error(&self, message: &str) -> crate::abi::CURLcode {
        if message.contains("pinned public key") {
            90
        } else if message.contains("certificate")
            || message.contains("verify")
            || message.contains("host")
        {
            60
        } else {
            super::default_connect_error()
        }
    }
}
