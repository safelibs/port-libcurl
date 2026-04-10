pub(crate) mod dict;
pub(crate) mod file;
pub(crate) mod ftp;
pub(crate) mod gopher;
pub(crate) mod imap;
pub(crate) mod ldap;
pub(crate) mod mqtt;
pub(crate) mod pop3;
pub(crate) mod rtsp;
pub(crate) mod smb;
pub(crate) mod smtp;
pub(crate) mod telnet;
pub(crate) mod tftp;

use crate::abi::curl_pushheaders;
use core::ffi::c_char;
use core::ffi::c_long;
use std::sync::OnceLock;

type RefPushHeaderByNameFn =
    unsafe extern "C" fn(*mut curl_pushheaders, *const c_char) -> *mut c_char;
type RefPushHeaderByNumFn = unsafe extern "C" fn(*mut curl_pushheaders, usize) -> *mut c_char;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransferRouting {
    NativeHttp,
    NativeWebSocket,
    ReferenceBackend,
}

fn ref_pushheader_byname() -> RefPushHeaderByNameFn {
    static FN: OnceLock<RefPushHeaderByNameFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { crate::global::load_reference(b"curl_pushheader_byname\0") })
}

fn ref_pushheader_bynum() -> RefPushHeaderByNumFn {
    static FN: OnceLock<RefPushHeaderByNumFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { crate::global::load_reference(b"curl_pushheader_bynum\0") })
}

pub(crate) fn route_scheme(
    scheme: &str,
    connect_mode: c_long,
    http_version: c_long,
) -> TransferRouting {
    if crate::vquic::requires_reference_backend(http_version) {
        return TransferRouting::ReferenceBackend;
    }

    if scheme == "http" {
        return TransferRouting::NativeHttp;
    }

    if scheme == "ws" && crate::ws::websocket_mode_enabled(connect_mode as i64) {
        return TransferRouting::NativeWebSocket;
    }

    if crate::tls::is_tls_scheme(scheme)
        || crate::ssh::is_ssh_scheme(scheme)
        || file::matches(scheme)
        || ftp::matches(scheme)
        || imap::matches(scheme)
        || pop3::matches(scheme)
        || smtp::matches(scheme)
        || ldap::matches(scheme)
        || smb::matches(scheme)
        || telnet::matches(scheme)
        || tftp::matches(scheme)
        || dict::matches(scheme)
        || gopher::matches(scheme)
        || rtsp::matches(scheme)
        || mqtt::matches(scheme)
    {
        return TransferRouting::ReferenceBackend;
    }

    TransferRouting::ReferenceBackend
}

pub(crate) unsafe fn pushheader_byname(
    headers: *mut curl_pushheaders,
    name: *const c_char,
) -> *mut c_char {
    unsafe { ref_pushheader_byname()(headers, name) }
}

pub(crate) unsafe fn pushheader_bynum(headers: *mut curl_pushheaders, index: usize) -> *mut c_char {
    unsafe { ref_pushheader_bynum()(headers, index) }
}
