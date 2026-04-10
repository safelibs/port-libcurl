pub(crate) const UPSTREAM_SOURCES: &[&str] = &[
    "original/lib/vssh/libssh.c",
    "original/lib/vssh/libssh2.c",
    "original/lib/vssh/wolfssh.c",
];

pub(crate) fn is_ssh_scheme(scheme: &str) -> bool {
    matches!(scheme, "scp" | "sftp")
}

pub(crate) fn execute(
    handle: *mut crate::abi::CURL,
    _route: crate::protocols::TransferRoute,
    _metadata: &crate::easy::perform::EasyMetadata,
    _callbacks: crate::easy::perform::EasyCallbacks,
) -> crate::abi::CURLcode {
    crate::protocols::perform_reference_bridge(handle)
}
