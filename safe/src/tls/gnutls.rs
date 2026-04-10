pub(crate) const NAME: &str = "gnutls";
pub(crate) const UPSTREAM_SOURCES: &[&str] = &[
    "original/lib/vtls/gtls.c",
    "original/lib/vtls/vtls.c",
    "original/lib/vtls/x509asn1.c",
];

pub(crate) fn cache_fragment() -> &'static str {
    "gnutls:ocsp+pinning+alpn+session-cache"
}
