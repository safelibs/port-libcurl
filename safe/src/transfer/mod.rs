use crate::abi::{CURLMcode, CURLcode};
use core::ffi::{c_int, c_long};

pub(crate) const EASY_PERFORM_WAIT_TIMEOUT_MS: c_int = 1000;
const CURLM_OUT_OF_MEMORY: CURLMcode = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LowSpeedWindow {
    pub limit_bytes_per_second: c_long,
    pub time_window_secs: c_long,
}

impl LowSpeedWindow {
    pub(crate) const fn enabled(self) -> bool {
        self.limit_bytes_per_second > 0 && self.time_window_secs > 0
    }
}

pub(crate) const fn map_multi_code(code: CURLMcode) -> CURLcode {
    if code == CURLM_OUT_OF_MEMORY {
        crate::abi::CURLE_OUT_OF_MEMORY
    } else {
        crate::abi::CURLE_BAD_FUNCTION_ARGUMENT
    }
}
