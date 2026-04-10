pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/smtp.c"];

pub(crate) fn matches(scheme: &str) -> bool {
    matches!(scheme, "smtp" | "smtps")
}

pub(crate) fn execute(
    handle: *mut crate::abi::CURL,
    _metadata: &crate::easy::perform::EasyMetadata,
    _callbacks: crate::easy::perform::EasyCallbacks,
) -> crate::abi::CURLcode {
    crate::protocols::perform_reference_bridge(handle)
}
