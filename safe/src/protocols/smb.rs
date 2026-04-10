use crate::abi::{CURLcode, CURL};
use crate::easy::perform::{EasyCallbacks, EasyMetadata};
use crate::protocols::ParsedProtocolUrl;
use crate::transfer::TransferPlan;

pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/smb.c"];

const CURLE_URL_MALFORMAT: CURLcode = 3;

pub(crate) fn matches(scheme: &str) -> bool {
    matches!(scheme, "smb" | "smbs")
}

pub(crate) fn perform_transfer(
    handle: *mut CURL,
    _plan: &TransferPlan,
    metadata: &EasyMetadata,
    _callbacks: EasyCallbacks,
) -> CURLcode {
    let Some(url) = metadata.url.as_deref() else {
        crate::easy::perform::set_error_buffer(handle, "No URL set");
        return CURLE_URL_MALFORMAT;
    };
    let _ = ParsedProtocolUrl::parse(url);
    crate::protocols::unsupported(
        handle,
        "SMB framing is not implemented in the shared engine",
    )
}
