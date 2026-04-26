use crate::abi::{
    CURLUPart, CURLUcode, CURLU, CURLUE_OK, CURLUE_OUT_OF_MEMORY, CURLUPART_FRAGMENT,
    CURLUPART_HOST, CURLUPART_OPTIONS, CURLUPART_PASSWORD, CURLUPART_PATH, CURLUPART_PORT,
    CURLUPART_QUERY, CURLUPART_SCHEME, CURLUPART_URL, CURLUPART_USER, CURLUPART_ZONEID,
};
use crate::alloc;
use core::ffi::c_char;
use core::ptr;
use std::ffi::CStr;

const CURLUE_BAD_HANDLE: CURLUcode = 1;
const CURLUE_BAD_PARTPOINTER: CURLUcode = 2;
const CURLUE_MALFORMED_INPUT: CURLUcode = 3;
const CURLUE_BAD_PORT_NUMBER: CURLUcode = 4;
const CURLUE_UNKNOWN_PART: CURLUcode = 9;
const CURLUE_NO_SCHEME: CURLUcode = 10;
const CURLUE_NO_USER: CURLUcode = 11;
const CURLUE_NO_PASSWORD: CURLUcode = 12;
const CURLUE_NO_OPTIONS: CURLUcode = 13;
const CURLUE_NO_HOST: CURLUcode = 14;
const CURLUE_NO_PORT: CURLUcode = 15;
const CURLUE_NO_QUERY: CURLUcode = 16;
const CURLUE_NO_FRAGMENT: CURLUcode = 17;
const CURLUE_NO_ZONEID: CURLUcode = 18;

const CURLU_DEFAULT_PORT: u32 = 1 << 0;
const CURLU_NO_DEFAULT_PORT: u32 = 1 << 1;
const CURLU_DEFAULT_SCHEME: u32 = 1 << 2;
const CURLU_URLDECODE: u32 = 1 << 6;
const CURLU_URLENCODE: u32 = 1 << 7;
const CURLU_APPENDQUERY: u32 = 1 << 8;

#[derive(Clone, Default)]
struct UrlState {
    scheme: Option<String>,
    user: Option<String>,
    password: Option<String>,
    options: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    port_explicit: bool,
    path: Option<String>,
    query: Option<String>,
    fragment: Option<String>,
    zoneid: Option<String>,
}

#[repr(C)]
#[derive(Clone, Default)]
struct UrlHandle {
    state: UrlState,
}

fn handle_ref(handle: *const CURLU) -> Option<&'static UrlHandle> {
    if handle.is_null() {
        None
    } else {
        Some(unsafe { &*(handle as *const UrlHandle) })
    }
}

fn handle_mut(handle: *mut CURLU) -> Option<&'static mut UrlHandle> {
    if handle.is_null() {
        None
    } else {
        Some(unsafe { &mut *(handle as *mut UrlHandle) })
    }
}

fn read_part(part: *const c_char) -> Result<Option<String>, CURLUcode> {
    if part.is_null() {
        Ok(None)
    } else {
        Ok(Some(
            unsafe { CStr::from_ptr(part) }
                .to_str()
                .map_err(|_| CURLUE_MALFORMED_INPUT)?
                .to_string(),
        ))
    }
}

fn default_port_for_scheme(scheme: &str) -> Option<u16> {
    let port = crate::protocols::default_port_for_scheme(scheme);
    (port != 0).then_some(port)
}

fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(
                char::from_digit((byte >> 4) as u32, 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
            out.push(
                char::from_digit((byte & 0x0f) as u32, 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
        }
    }
    out
}

fn percent_decode(text: &str) -> Result<String, CURLUcode> {
    crate::protocols::percent_decode(text.as_bytes()).map_err(|_| CURLUE_MALFORMED_INPUT)
}

fn parse_port_text(text: &str) -> Result<u16, CURLUcode> {
    text.parse().map_err(|_| CURLUE_BAD_PORT_NUMBER)
}

fn parse_full_url(input: &str) -> Result<UrlState, CURLUcode> {
    let (without_fragment, fragment) = input
        .split_once('#')
        .map(|(left, right)| (left, Some(right.to_string())))
        .unwrap_or((input, None));
    let (scheme, remainder) = without_fragment.split_once("://").ok_or(CURLUE_NO_SCHEME)?;
    if scheme.is_empty() {
        return Err(CURLUE_NO_SCHEME);
    }

    let authority_end = remainder.find(['/', '?']).unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    let suffix = &remainder[authority_end..];
    if authority.is_empty() {
        return Err(CURLUE_NO_HOST);
    }

    let (userinfo, hostport) = authority
        .rsplit_once('@')
        .map(|(left, right)| (Some(left), right))
        .unwrap_or((None, authority));

    let mut state = UrlState {
        scheme: Some(scheme.to_ascii_lowercase()),
        fragment,
        ..UrlState::default()
    };

    if let Some(userinfo) = userinfo {
        let (login, options) = userinfo
            .split_once(';')
            .map(|(left, right)| (left, Some(right.to_string())))
            .unwrap_or((userinfo, None));
        let (user, password) = login
            .split_once(':')
            .map(|(left, right)| (Some(left.to_string()), Some(right.to_string())))
            .unwrap_or((Some(login.to_string()), None));
        state.user = user.filter(|value| !value.is_empty());
        state.password = password.filter(|value| !value.is_empty());
        state.options = options.filter(|value| !value.is_empty());
    }

    if let Some(rest) = hostport.strip_prefix('[') {
        let end = rest.find(']').ok_or(CURLUE_MALFORMED_INPUT)?;
        let inside = &rest[..end];
        let tail = &rest[end + 1..];
        let (host, zoneid) = inside
            .split_once("%25")
            .map(|(host, zone)| (host.to_string(), Some(zone.to_string())))
            .unwrap_or((inside.to_string(), None));
        state.host = Some(host);
        state.zoneid = zoneid;
        if let Some(port_text) = tail.strip_prefix(':') {
            state.port = Some(parse_port_text(port_text)?);
            state.port_explicit = true;
        }
    } else if let Some((host, port_text)) = hostport.rsplit_once(':') {
        if !host.contains(':') && !port_text.is_empty() {
            state.host = Some(host.to_string());
            state.port = Some(parse_port_text(port_text)?);
            state.port_explicit = true;
        } else {
            state.host = Some(hostport.to_string());
        }
    } else {
        state.host = Some(hostport.to_string());
    }

    let (path, query) = suffix
        .split_once('?')
        .map(|(left, right)| {
            (
                if left.is_empty() { "/" } else { left }.to_string(),
                Some(right.to_string()),
            )
        })
        .unwrap_or_else(|| {
            (
                if suffix.is_empty() { "/" } else { suffix }.to_string(),
                None,
            )
        });
    state.path = Some(path);
    state.query = query.filter(|value| !value.is_empty());

    if state.host.as_deref().unwrap_or_default().is_empty() {
        return Err(CURLUE_NO_HOST);
    }
    Ok(state)
}

fn render_host(state: &UrlState) -> Result<String, CURLUcode> {
    let host = state.host.clone().ok_or(CURLUE_NO_HOST)?;
    if host.contains(':') && !host.starts_with('[') {
        if let Some(zoneid) = state.zoneid.as_deref() {
            Ok(format!("[{host}%25{zoneid}]"))
        } else {
            Ok(format!("[{host}]"))
        }
    } else {
        Ok(host)
    }
}

fn render_url(state: &UrlState, flags: u32) -> Result<String, CURLUcode> {
    let scheme = state
        .scheme
        .clone()
        .or_else(|| (flags & CURLU_DEFAULT_SCHEME != 0).then(|| "https".to_string()))
        .ok_or(CURLUE_NO_SCHEME)?;
    let mut rendered = format!("{scheme}://");
    if let Some(user) = state.user.as_deref() {
        rendered.push_str(user);
        if let Some(password) = state.password.as_deref() {
            rendered.push(':');
            rendered.push_str(password);
        }
        if let Some(options) = state.options.as_deref() {
            rendered.push(';');
            rendered.push_str(options);
        }
        rendered.push('@');
    }
    rendered.push_str(&render_host(state)?);

    let default_port = default_port_for_scheme(&scheme);
    if let Some(port) = state.port {
        if state.port_explicit
            && !(flags & CURLU_NO_DEFAULT_PORT != 0 && Some(port) == default_port)
        {
            rendered.push(':');
            rendered.push_str(&port.to_string());
        }
    }

    rendered.push_str(state.path.as_deref().unwrap_or("/"));
    if let Some(query) = state.query.as_deref() {
        rendered.push('?');
        rendered.push_str(query);
    }
    if let Some(fragment) = state.fragment.as_deref() {
        rendered.push('#');
        rendered.push_str(fragment);
    }
    Ok(rendered)
}

fn part_string(state: &UrlState, what: CURLUPart, flags: u32) -> Result<String, CURLUcode> {
    let value = match what {
        CURLUPART_URL => render_url(state, flags)?,
        CURLUPART_SCHEME => state
            .scheme
            .clone()
            .or_else(|| (flags & CURLU_DEFAULT_SCHEME != 0).then(|| "https".to_string()))
            .ok_or(CURLUE_NO_SCHEME)?,
        CURLUPART_USER => state.user.clone().ok_or(CURLUE_NO_USER)?,
        CURLUPART_PASSWORD => state.password.clone().ok_or(CURLUE_NO_PASSWORD)?,
        CURLUPART_OPTIONS => state.options.clone().ok_or(CURLUE_NO_OPTIONS)?,
        CURLUPART_HOST => render_host(state)?,
        CURLUPART_PORT => {
            if state.port_explicit {
                state.port.unwrap().to_string()
            } else if flags & CURLU_DEFAULT_PORT != 0 {
                default_port_for_scheme(state.scheme.as_deref().ok_or(CURLUE_NO_SCHEME)?)
                    .ok_or(CURLUE_NO_PORT)?
                    .to_string()
            } else {
                return Err(CURLUE_NO_PORT);
            }
        }
        CURLUPART_PATH => state.path.clone().unwrap_or_else(|| "/".to_string()),
        CURLUPART_QUERY => state.query.clone().ok_or(CURLUE_NO_QUERY)?,
        CURLUPART_FRAGMENT => state.fragment.clone().ok_or(CURLUE_NO_FRAGMENT)?,
        CURLUPART_ZONEID => state.zoneid.clone().ok_or(CURLUE_NO_ZONEID)?,
        _ => return Err(CURLUE_UNKNOWN_PART),
    };

    if flags & CURLU_URLDECODE != 0 {
        percent_decode(&value)
    } else {
        Ok(value)
    }
}

fn alloc_result(text: &str, out: *mut *mut c_char) -> CURLUcode {
    let ptr = unsafe { alloc::alloc_and_copy(text.as_bytes()) };
    if ptr.is_null() {
        unsafe {
            *out = ptr::null_mut();
        }
        CURLUE_OUT_OF_MEMORY
    } else {
        unsafe {
            *out = ptr;
        }
        CURLUE_OK
    }
}

fn url_error_string(code: CURLUcode) -> *const c_char {
    match code {
        CURLUE_OK => c"No error".as_ptr(),
        CURLUE_BAD_HANDLE => c"Bad URL handle".as_ptr(),
        CURLUE_BAD_PARTPOINTER => c"Bad URL part pointer".as_ptr(),
        CURLUE_MALFORMED_INPUT => c"Malformed input".as_ptr(),
        CURLUE_BAD_PORT_NUMBER => c"Bad port number".as_ptr(),
        CURLUE_OUT_OF_MEMORY => c"Out of memory".as_ptr(),
        CURLUE_UNKNOWN_PART => c"Unknown URL part".as_ptr(),
        CURLUE_NO_SCHEME => c"No scheme part".as_ptr(),
        CURLUE_NO_USER => c"No user part".as_ptr(),
        CURLUE_NO_PASSWORD => c"No password part".as_ptr(),
        CURLUE_NO_OPTIONS => c"No options part".as_ptr(),
        CURLUE_NO_HOST => c"No host part".as_ptr(),
        CURLUE_NO_PORT => c"No port part".as_ptr(),
        CURLUE_NO_QUERY => c"No query part".as_ptr(),
        CURLUE_NO_FRAGMENT => c"No fragment part".as_ptr(),
        CURLUE_NO_ZONEID => c"No zone id part".as_ptr(),
        _ => c"Unknown URL error".as_ptr(),
    }
}

pub(crate) unsafe fn url() -> *mut CURLU {
    Box::into_raw(Box::new(UrlHandle::default())).cast()
}

pub(crate) unsafe fn url_cleanup(handle: *mut CURLU) {
    if handle.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle as *mut UrlHandle));
    }
}

pub(crate) unsafe fn url_dup(handle: *const CURLU) -> *mut CURLU {
    let Some(handle) = handle_ref(handle) else {
        return ptr::null_mut();
    };
    Box::into_raw(Box::new(handle.clone())).cast()
}

pub(crate) unsafe fn url_get(
    handle: *const CURLU,
    what: CURLUPart,
    part: *mut *mut c_char,
    flags: u32,
) -> CURLUcode {
    let Some(handle) = handle_ref(handle) else {
        return CURLUE_BAD_HANDLE;
    };
    if part.is_null() {
        return CURLUE_BAD_PARTPOINTER;
    }
    let value = match part_string(&handle.state, what, flags) {
        Ok(value) => value,
        Err(code) => {
            unsafe {
                *part = ptr::null_mut();
            }
            return code;
        }
    };
    alloc_result(&value, part)
}

pub(crate) unsafe fn url_set(
    handle: *mut CURLU,
    what: CURLUPart,
    part: *const c_char,
    flags: u32,
) -> CURLUcode {
    let Some(handle) = handle_mut(handle) else {
        return CURLUE_BAD_HANDLE;
    };
    let mut value = match read_part(part) {
        Ok(value) => value,
        Err(code) => return code,
    };
    if flags & CURLU_URLENCODE != 0 {
        value = value.map(|text| percent_encode(&text));
    }

    if what == CURLUPART_URL {
        return match value {
            Some(text) => match parse_full_url(&text) {
                Ok(state) => {
                    handle.state = state;
                    CURLUE_OK
                }
                Err(code) => code,
            },
            None => {
                handle.state = UrlState::default();
                CURLUE_OK
            }
        };
    }

    match what {
        CURLUPART_SCHEME => handle.state.scheme = value.filter(|text| !text.is_empty()),
        CURLUPART_USER => handle.state.user = value.filter(|text| !text.is_empty()),
        CURLUPART_PASSWORD => handle.state.password = value.filter(|text| !text.is_empty()),
        CURLUPART_OPTIONS => handle.state.options = value.filter(|text| !text.is_empty()),
        CURLUPART_HOST => handle.state.host = value.filter(|text| !text.is_empty()),
        CURLUPART_PORT => {
            if let Some(text) = value.as_deref() {
                handle.state.port = match parse_port_text(text) {
                    Ok(port) => Some(port),
                    Err(code) => return code,
                };
                handle.state.port_explicit = true;
            } else {
                handle.state.port = None;
                handle.state.port_explicit = false;
            }
        }
        CURLUPART_PATH => {
            handle.state.path = value
                .map(|text| {
                    if text.is_empty() {
                        "/".to_string()
                    } else {
                        text
                    }
                })
                .or_else(|| Some("/".to_string()));
        }
        CURLUPART_QUERY => {
            if flags & CURLU_APPENDQUERY != 0 {
                if let Some(text) = value {
                    let current = handle.state.query.take().unwrap_or_default();
                    handle.state.query = Some(if current.is_empty() {
                        text
                    } else {
                        format!("{current}&{text}")
                    });
                }
            } else {
                handle.state.query = value.filter(|text| !text.is_empty());
            }
        }
        CURLUPART_FRAGMENT => handle.state.fragment = value.filter(|text| !text.is_empty()),
        CURLUPART_ZONEID => handle.state.zoneid = value.filter(|text| !text.is_empty()),
        _ => return CURLUE_UNKNOWN_PART,
    }
    CURLUE_OK
}

pub(crate) unsafe fn url_strerror(code: CURLUcode) -> *const c_char {
    url_error_string(code)
}
