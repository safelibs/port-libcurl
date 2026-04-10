use crate::abi::CURLMcode;
use core::ffi::{c_int, c_long};

pub(crate) const fn clamp_timeout(requested_ms: c_int, internal_ms: c_long) -> c_int {
    if internal_ms >= 0 && internal_ms < requested_ms as c_long {
        internal_ms as c_int
    } else {
        requested_ms
    }
}

pub(crate) const fn validate_timeout(
    timeout_ms: c_int,
    bad_argument: CURLMcode,
) -> Result<c_int, CURLMcode> {
    if timeout_ms < 0 {
        Err(bad_argument)
    } else {
        Ok(timeout_ms)
    }
}
