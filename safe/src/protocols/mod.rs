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

use crate::abi::{curl_pushheaders, CURLcode, CURL};
use crate::easy::perform::{EasyCallbacks, EasyMetadata};
use crate::transfer::TransferPlan;
use core::ffi::{c_char, c_long};
use std::ffi::CStr;
use std::sync::OnceLock;

const CURLE_UNSUPPORTED_PROTOCOL: CURLcode = 1;
const CURLE_URL_MALFORMAT: CURLcode = 3;

type RefPushHeaderByNameFn =
    unsafe extern "C" fn(*mut curl_pushheaders, *const c_char) -> *mut c_char;
type RefPushHeaderByNumFn = unsafe extern "C" fn(*mut curl_pushheaders, usize) -> *mut c_char;

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
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ParsedProtocolUrl {
    pub raw_url: String,
    pub scheme: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub query: Option<String>,
}

impl ParsedProtocolUrl {
    pub(crate) fn parse(url: &str) -> Result<Self, CURLcode> {
        let (scheme, rest) = url.split_once("://").ok_or(CURLE_URL_MALFORMAT)?;
        let scheme = scheme.to_ascii_lowercase();
        let authority_and_path = rest.split('#').next().unwrap_or(rest);
        let (authority, remainder) = authority_and_path
            .split_once('/')
            .map(|(authority, tail)| (authority, format!("/{tail}")))
            .unwrap_or((authority_and_path, "/".to_string()));
        let authority = authority.trim();
        if authority.is_empty() {
            return Err(CURLE_URL_MALFORMAT);
        }
        let (userinfo, hostport) = authority
            .rsplit_once('@')
            .map(|(userinfo, hostport)| (Some(userinfo), hostport))
            .unwrap_or((None, authority));
        let (username, password) = userinfo.map(split_userinfo).unwrap_or((None, None));
        let (host, port) = split_host_port(hostport, default_port_for_scheme(&scheme))?;
        let (path, query) = remainder
            .split_once('?')
            .map(|(path, query)| (path.to_string(), Some(query.to_string())))
            .unwrap_or((remainder, None));

        Ok(Self {
            raw_url: url.to_string(),
            scheme,
            username,
            password,
            host,
            port,
            path,
            query,
        })
    }

    pub(crate) fn decoded_path(&self) -> Result<String, CURLcode> {
        percent_decode(self.path.as_bytes())
    }

    pub(crate) fn path_segments(&self) -> Result<Vec<String>, CURLcode> {
        Ok(self
            .decoded_path()?
            .trim_start_matches('/')
            .split('/')
            .filter(|segment| !segment.is_empty())
            .map(ToOwned::to_owned)
            .collect())
    }

    pub(crate) fn last_path_segment(&self) -> Result<Option<String>, CURLcode> {
        Ok(self.path_segments()?.into_iter().last())
    }
}

fn split_userinfo(userinfo: &str) -> (Option<String>, Option<String>) {
    let (username, password) = userinfo.split_once(':').unwrap_or((userinfo, ""));
    (
        (!username.is_empty()).then(|| username.to_string()),
        (!password.is_empty()).then(|| password.to_string()),
    )
}

fn split_host_port(input: &str, default_port: u16) -> Result<(String, u16), CURLcode> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CURLE_URL_MALFORMAT);
    }
    if let Some(rest) = trimmed.strip_prefix('[') {
        let end = rest.find(']').ok_or(CURLE_URL_MALFORMAT)?;
        let host = rest[..end].to_string();
        let port = if let Some(port_text) = rest[end + 1..].strip_prefix(':') {
            port_text.parse().map_err(|_| CURLE_URL_MALFORMAT)?
        } else {
            default_port
        };
        return Ok((host, port));
    }
    if let Some((host, port_text)) = trimmed.rsplit_once(':') {
        if !host.contains(':') && !port_text.is_empty() {
            return Ok((
                host.to_string(),
                port_text.parse().map_err(|_| CURLE_URL_MALFORMAT)?,
            ));
        }
    }
    Ok((trimmed.to_string(), default_port))
}

pub(crate) fn percent_decode(input: &[u8]) -> Result<String, CURLcode> {
    let mut out = Vec::with_capacity(input.len());
    let mut idx = 0usize;
    while idx < input.len() {
        match input[idx] {
            b'%' if idx + 2 < input.len() => {
                let hi = decode_hex(input[idx + 1])?;
                let lo = decode_hex(input[idx + 2])?;
                out.push((hi << 4) | lo);
                idx += 3;
            }
            byte => {
                out.push(byte);
                idx += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| CURLE_URL_MALFORMAT)
}

fn decode_hex(byte: u8) -> Result<u8, CURLcode> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(CURLE_URL_MALFORMAT),
    }
}

pub(crate) fn default_port_for_scheme(scheme: &str) -> u16 {
    match scheme.to_ascii_lowercase().as_str() {
        "http" | "ws" => 80,
        "https" | "wss" => 443,
        "ftp" => 21,
        "ftps" => 990,
        "imap" => 143,
        "imaps" => 993,
        "pop3" => 110,
        "pop3s" => 995,
        "smtp" => 25,
        "smtps" => 465,
        "ldap" => 389,
        "ldaps" => 636,
        "smb" | "smbs" => 445,
        "telnet" => 23,
        "tftp" => 69,
        "dict" => 2628,
        "gopher" => 70,
        "rtsp" => 554,
        "mqtt" => 1883,
        "scp" | "sftp" => 22,
        _ => 0,
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

pub(crate) fn perform_transfer(
    handle: *mut CURL,
    plan: &TransferPlan,
    metadata: &EasyMetadata,
    callbacks: EasyCallbacks,
) -> CURLcode {
    match plan.route.handler {
        SchemeHandler::Ftp => ftp::perform_transfer(handle, plan, metadata, callbacks),
        SchemeHandler::Imap => imap::perform_transfer(handle, plan, metadata, callbacks),
        SchemeHandler::Pop3 => pop3::perform_transfer(handle, plan, metadata, callbacks),
        SchemeHandler::Smtp => smtp::perform_transfer(handle, plan, metadata, callbacks),
        SchemeHandler::Ldap => ldap::perform_transfer(handle, plan, metadata, callbacks),
        SchemeHandler::Smb => smb::perform_transfer(handle, plan, metadata, callbacks),
        SchemeHandler::Telnet => telnet::perform_transfer(handle, plan, metadata, callbacks),
        SchemeHandler::Tftp => tftp::perform_transfer(handle, plan, metadata, callbacks),
        SchemeHandler::Dict => dict::perform_transfer(handle, plan, metadata, callbacks),
        SchemeHandler::Gopher => gopher::perform_transfer(handle, plan, metadata, callbacks),
        SchemeHandler::Rtsp => rtsp::perform_transfer(handle, plan, metadata, callbacks),
        SchemeHandler::Mqtt => mqtt::perform_transfer(handle, plan, metadata, callbacks),
        SchemeHandler::Scp | SchemeHandler::Sftp => {
            crate::ssh::perform_transfer(handle, plan, metadata, callbacks)
        }
        SchemeHandler::Unknown => unsupported(handle, "Unsupported protocol"),
        SchemeHandler::Http | SchemeHandler::WebSocket | SchemeHandler::File => {
            unsupported(handle, "Protocol route dispatched incorrectly")
        }
    }
}

pub(crate) fn unsupported(handle: *mut CURL, message: &str) -> CURLcode {
    crate::easy::perform::set_error_buffer(handle, message);
    CURLE_UNSUPPORTED_PROTOCOL
}

pub(crate) unsafe fn capture_push_headers(_headers: *mut curl_pushheaders, _num_headers: usize) {}

pub(crate) fn release_push_headers(_headers: *mut curl_pushheaders) {}

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
