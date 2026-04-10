pub(crate) const UPSTREAM_SOURCES: &[&str] = &[
    "original/lib/vtls/openssl.c",
    "original/lib/vtls/gtls.c",
    "original/lib/vtls/x509asn1.c",
];

pub(crate) const fn requested(enabled: bool) -> bool {
    enabled
}
