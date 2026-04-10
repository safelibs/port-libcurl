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

use crate::abi::{CURLcode, CURL};
use crate::abi::curl_pushheaders;
use crate::easy::perform::{EasyCallbacks, EasyMetadata};
use core::ffi::c_char;
use core::ffi::c_long;
use std::ffi::CStr;
use std::sync::OnceLock;

type RefPushHeaderByNameFn =
    unsafe extern "C" fn(*mut curl_pushheaders, *const c_char) -> *mut c_char;
type RefPushHeaderByNumFn = unsafe extern "C" fn(*mut curl_pushheaders, usize) -> *mut c_char;
const CURLE_UNSUPPORTED_PROTOCOL: CURLcode = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SchemeHandler {
    Http,
    WebSocket,
    File,
    Ftp,
    Imap,
    Pop3,
    Smtp,
    Ldap,
    Smb,
    Telnet,
    Tftp,
    Dict,
    Gopher,
    Rtsp,
    Mqtt,
    Scp,
    Sftp,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TransferRoute {
    pub handler: SchemeHandler,
    pub tls: bool,
    pub websocket_mode: bool,
}

impl TransferRoute {
    pub(crate) const fn uses_shared_http(self) -> bool {
        matches!(self.handler, SchemeHandler::Http) && !self.tls
    }

    pub(crate) const fn uses_shared_websocket(self) -> bool {
        matches!(self.handler, SchemeHandler::WebSocket) && self.websocket_mode && !self.tls
    }

    pub(crate) const fn is_http_family(self) -> bool {
        matches!(self.handler, SchemeHandler::Http | SchemeHandler::WebSocket)
    }

    pub(crate) fn requires_reference_multi(self, http_version: c_long) -> bool {
        crate::vquic::requires_reference_backend(http_version)
            || (self.is_http_family() && http_version >= 2)
            || matches!(self.handler, SchemeHandler::Ftp | SchemeHandler::Rtsp)
    }
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
    _http_version: c_long,
) -> TransferRoute {
    let websocket_mode = crate::ws::websocket_mode_enabled(connect_mode as i64);
    match scheme {
        "http" => TransferRoute {
            handler: SchemeHandler::Http,
            tls: false,
            websocket_mode: false,
        },
        "https" => TransferRoute {
            handler: SchemeHandler::Http,
            tls: true,
            websocket_mode: false,
        },
        "ws" => TransferRoute {
            handler: SchemeHandler::WebSocket,
            tls: false,
            websocket_mode,
        },
        "wss" => TransferRoute {
            handler: SchemeHandler::WebSocket,
            tls: true,
            websocket_mode,
        },
        "file" => TransferRoute {
            handler: SchemeHandler::File,
            tls: false,
            websocket_mode: false,
        },
        "ftp" | "ftps" => TransferRoute {
            handler: SchemeHandler::Ftp,
            tls: scheme.ends_with('s'),
            websocket_mode: false,
        },
        "imap" | "imaps" => TransferRoute {
            handler: SchemeHandler::Imap,
            tls: scheme.ends_with('s'),
            websocket_mode: false,
        },
        "pop3" | "pop3s" => TransferRoute {
            handler: SchemeHandler::Pop3,
            tls: scheme.ends_with('s'),
            websocket_mode: false,
        },
        "smtp" | "smtps" => TransferRoute {
            handler: SchemeHandler::Smtp,
            tls: scheme.ends_with('s'),
            websocket_mode: false,
        },
        "ldap" | "ldaps" => TransferRoute {
            handler: SchemeHandler::Ldap,
            tls: scheme.ends_with('s'),
            websocket_mode: false,
        },
        "smb" | "smbs" => TransferRoute {
            handler: SchemeHandler::Smb,
            tls: scheme.ends_with('s'),
            websocket_mode: false,
        },
        "telnet" => TransferRoute {
            handler: SchemeHandler::Telnet,
            tls: false,
            websocket_mode: false,
        },
        "tftp" => TransferRoute {
            handler: SchemeHandler::Tftp,
            tls: false,
            websocket_mode: false,
        },
        "dict" => TransferRoute {
            handler: SchemeHandler::Dict,
            tls: false,
            websocket_mode: false,
        },
        "gopher" => TransferRoute {
            handler: SchemeHandler::Gopher,
            tls: false,
            websocket_mode: false,
        },
        "rtsp" => TransferRoute {
            handler: SchemeHandler::Rtsp,
            tls: false,
            websocket_mode: false,
        },
        "mqtt" => TransferRoute {
            handler: SchemeHandler::Mqtt,
            tls: false,
            websocket_mode: false,
        },
        "scp" => TransferRoute {
            handler: SchemeHandler::Scp,
            tls: false,
            websocket_mode: false,
        },
        "sftp" => TransferRoute {
            handler: SchemeHandler::Sftp,
            tls: false,
            websocket_mode: false,
        },
        _ => TransferRoute {
            handler: SchemeHandler::Unknown,
            tls: false,
            websocket_mode: false,
        },
    }
}

pub(crate) unsafe fn capture_push_headers(_headers: *mut curl_pushheaders, _num_headers: usize) {}

pub(crate) fn release_push_headers(_headers: *mut curl_pushheaders) {}

pub(crate) fn execute_route(
    handle: *mut CURL,
    route: TransferRoute,
    metadata: &EasyMetadata,
    callbacks: EasyCallbacks,
) -> Option<CURLcode> {
    match route.handler {
        SchemeHandler::Http | SchemeHandler::WebSocket if !route.tls => None,
        SchemeHandler::Http | SchemeHandler::WebSocket => {
            Some(crate::tls::execute_route(handle, route, metadata, callbacks))
        }
        SchemeHandler::File => Some(file::execute(handle, metadata, callbacks)),
        SchemeHandler::Ftp => Some(ftp::execute(handle, metadata, callbacks)),
        SchemeHandler::Imap => Some(imap::execute(handle, metadata, callbacks)),
        SchemeHandler::Pop3 => Some(pop3::execute(handle, metadata, callbacks)),
        SchemeHandler::Smtp => Some(smtp::execute(handle, metadata, callbacks)),
        SchemeHandler::Ldap => Some(ldap::execute(handle, metadata, callbacks)),
        SchemeHandler::Smb => Some(smb::execute(handle, metadata, callbacks)),
        SchemeHandler::Telnet => Some(telnet::execute(handle, metadata, callbacks)),
        SchemeHandler::Tftp => Some(tftp::execute(handle, metadata, callbacks)),
        SchemeHandler::Dict => Some(dict::execute(handle, metadata, callbacks)),
        SchemeHandler::Gopher => Some(gopher::execute(handle, metadata, callbacks)),
        SchemeHandler::Rtsp => Some(rtsp::execute(handle, metadata, callbacks)),
        SchemeHandler::Mqtt => Some(mqtt::execute(handle, metadata, callbacks)),
        SchemeHandler::Scp | SchemeHandler::Sftp => {
            Some(crate::ssh::execute(handle, route, metadata, callbacks))
        }
        SchemeHandler::Unknown => Some(CURLE_UNSUPPORTED_PROTOCOL),
    }
}

pub(crate) fn perform_reference_bridge(handle: *mut CURL) -> CURLcode {
    type RefEasyPerformFn = unsafe extern "C" fn(*mut CURL) -> CURLcode;

    fn ref_easy_perform() -> RefEasyPerformFn {
        static FN: OnceLock<RefEasyPerformFn> = OnceLock::new();
        *FN.get_or_init(|| unsafe { crate::global::load_reference(b"curl_easy_perform\0") })
    }

    unsafe { ref_easy_perform()(handle) }
}

pub(crate) unsafe fn pushheader_byname(
    headers: *mut curl_pushheaders,
    name: *const c_char,
) -> *mut c_char {
    if headers.is_null() || name.is_null() {
        return core::ptr::null_mut();
    }

    let Ok(query) = unsafe { CStr::from_ptr(name) }.to_str() else {
        return core::ptr::null_mut();
    };
    if query.is_empty() || query == ":" || query[1..].contains(':') {
        return core::ptr::null_mut();
    }
    unsafe { ref_pushheader_byname()(headers, name) }
}

pub(crate) unsafe fn pushheader_bynum(headers: *mut curl_pushheaders, index: usize) -> *mut c_char {
    if headers.is_null() {
        return core::ptr::null_mut();
    }
    unsafe { ref_pushheader_bynum()(headers, index) }
}
