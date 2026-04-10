pub(crate) const NAME: &str = "openssl";
pub(crate) const UPSTREAM_SOURCES: &[&str] = &[
    "original/lib/vtls/openssl.c",
    "original/lib/vtls/vtls.c",
    "original/lib/vtls/x509asn1.c",
];

pub(crate) fn cache_fragment() -> &'static str {
    "openssl:partial-chain+pinning+alpn+session-cache"
}
