use core::ffi::c_long;

pub(crate) const UPSTREAM_SOURCES: &[&str] = &[
    "original/lib/vquic/vquic.c",
    "original/lib/vquic/curl_ngtcp2.c",
    "original/lib/vquic/curl_quiche.c",
    "original/lib/vquic/curl_msh3.c",
];

pub(crate) const ENABLED: bool = false;

pub(crate) fn requires_reference_backend(http_version: c_long) -> bool {
    http_version >= 3
}
