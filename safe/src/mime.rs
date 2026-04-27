use crate::abi::{
    curl_free_callback, curl_mime, curl_mimepart, curl_off_t, curl_read_callback,
    curl_seek_callback, curl_slist, CURLcode, CURL, CURLE_BAD_FUNCTION_ARGUMENT, CURLE_OK,
};
use core::ffi::{c_char, c_int, c_void};
use core::ptr;
use std::ffi::CStr;

pub(crate) const CURL_ZERO_TERMINATED: usize = usize::MAX;
const MIME_BOUNDARY_PREFIX: &str = "------------------------port-libcurl-safe-mime-";
const SEEK_SET: c_int = 0;

#[derive(Default)]
pub(crate) struct HeaderStore {
    pub(crate) values: Vec<String>,
    pub(crate) owned_list: *mut curl_slist,
}

pub(crate) struct CallbackBody {
    pub(crate) size: curl_off_t,
    pub(crate) readfunc: curl_read_callback,
    pub(crate) seekfunc: curl_seek_callback,
    pub(crate) freefunc: curl_free_callback,
    pub(crate) arg: *mut c_void,
}

pub(crate) enum BodySource {
    None,
    Owned(Vec<u8>),
    Borrowed { ptr: *const u8, len: usize },
    FilePath(String),
    Callback(CallbackBody),
    Subparts(*mut curl_mime),
}

impl Default for BodySource {
    fn default() -> Self {
        Self::None
    }
}

#[repr(C)]
struct MimeHandle {
    easy: *mut CURL,
    parts: Vec<*mut MimePartHandle>,
    adopted: bool,
}

#[repr(C)]
struct MimePartHandle {
    name: Option<String>,
    filename: Option<String>,
    mime_type: Option<String>,
    encoder: Option<String>,
    headers: HeaderStore,
    body: BodySource,
}

fn mime_mut(mime: *mut curl_mime) -> Option<&'static mut MimeHandle> {
    if mime.is_null() {
        None
    } else {
        Some(unsafe { &mut *(mime as *mut MimeHandle) })
    }
}

fn part_mut(part: *mut curl_mimepart) -> Option<&'static mut MimePartHandle> {
    if part.is_null() {
        None
    } else {
        Some(unsafe { &mut *(part as *mut MimePartHandle) })
    }
}

fn c_string(value: *const c_char) -> Result<Option<String>, CURLcode> {
    if value.is_null() {
        Ok(None)
    } else {
        Ok(Some(
            unsafe { CStr::from_ptr(value) }
                .to_str()
                .map_err(|_| CURLE_BAD_FUNCTION_ARGUMENT)?
                .to_string(),
        ))
    }
}

fn duplicate_bytes(data: *const c_char, datasize: usize) -> Result<Vec<u8>, CURLcode> {
    if data.is_null() {
        return Ok(Vec::new());
    }
    let len = if datasize == CURL_ZERO_TERMINATED {
        unsafe { CStr::from_ptr(data) }.to_bytes().len()
    } else {
        datasize
    };
    Ok(unsafe { core::slice::from_raw_parts(data.cast::<u8>(), len) }.to_vec())
}

fn slist_strings(mut headers: *mut curl_slist) -> Vec<String> {
    let mut values = Vec::new();
    while !headers.is_null() {
        let data = unsafe { (*headers).data };
        if !data.is_null() {
            values.push(
                unsafe { CStr::from_ptr(data) }
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        headers = unsafe { (*headers).next };
    }
    values
}

fn read_callback_body(callback: &CallbackBody) -> Option<Vec<u8>> {
    let readfunc = callback.readfunc?;
    if let Some(seekfunc) = callback.seekfunc {
        let _ = unsafe { seekfunc(callback.arg, 0, SEEK_SET) };
    }

    let mut body = Vec::new();
    let mut remaining = (callback.size >= 0).then_some(callback.size as usize);
    loop {
        let chunk_len = remaining.map(|len| len.min(16 * 1024)).unwrap_or(16 * 1024);
        if chunk_len == 0 {
            break;
        }
        let start = body.len();
        body.resize(start + chunk_len, 0);
        let read = unsafe {
            readfunc(
                body[start..].as_mut_ptr().cast(),
                1,
                chunk_len,
                callback.arg,
            )
        };
        if read == 0 {
            body.truncate(start);
            break;
        }
        if read > chunk_len {
            return None;
        }
        body.truncate(start + read);
        if let Some(left) = remaining.as_mut() {
            *left = left.saturating_sub(read);
            if *left == 0 {
                break;
            }
        }
    }
    Some(body)
}

fn body_source_bytes(body: &BodySource) -> Option<Vec<u8>> {
    match body {
        BodySource::None => Some(Vec::new()),
        BodySource::Owned(bytes) => Some(bytes.clone()),
        BodySource::Borrowed { ptr, len } => {
            Some(unsafe { core::slice::from_raw_parts(*ptr, *len) }.to_vec())
        }
        BodySource::FilePath(path) => std::fs::read(path).ok(),
        BodySource::Callback(callback) => read_callback_body(callback),
        BodySource::Subparts(subparts) => render_multipart_body(*subparts, nested_boundary(*subparts))
            .map(|(bytes, _)| bytes),
    }
}

fn nested_boundary(mime: *mut curl_mime) -> String {
    format!("{MIME_BOUNDARY_PREFIX}{:x}", mime as usize)
}

fn append_part_headers(
    rendered: &mut Vec<u8>,
    part: &MimePartHandle,
    nested_content_type: Option<&str>,
) {
    rendered.extend_from_slice(b"Content-Disposition: form-data");
    if let Some(name) = part.name.as_deref() {
        rendered.extend_from_slice(b"; name=\"");
        rendered.extend_from_slice(name.as_bytes());
        rendered.extend_from_slice(b"\"");
    }
    if let Some(filename) = part.filename.as_deref() {
        rendered.extend_from_slice(b"; filename=\"");
        rendered.extend_from_slice(filename.as_bytes());
        rendered.extend_from_slice(b"\"");
    }
    rendered.extend_from_slice(b"\r\n");

    if let Some(content_type) = nested_content_type.or(part.mime_type.as_deref()) {
        rendered.extend_from_slice(b"Content-Type: ");
        rendered.extend_from_slice(content_type.as_bytes());
        rendered.extend_from_slice(b"\r\n");
    }
    if let Some(encoding) = part.encoder.as_deref() {
        rendered.extend_from_slice(b"Content-Transfer-Encoding: ");
        rendered.extend_from_slice(encoding.as_bytes());
        rendered.extend_from_slice(b"\r\n");
    }
    for header in &part.headers.values {
        rendered.extend_from_slice(header.as_bytes());
        rendered.extend_from_slice(b"\r\n");
    }
}

fn render_part_bytes(part: &MimePartHandle) -> Option<Vec<u8>> {
    match &part.body {
        BodySource::Subparts(subparts) => {
            let nested_boundary = nested_boundary(*subparts);
            let (bytes, _) = render_multipart_body(*subparts, nested_boundary.clone())?;
            let mut rendered = Vec::new();
            append_part_headers(
                &mut rendered,
                part,
                Some(&format!("multipart/mixed; boundary={nested_boundary}")),
            );
            rendered.extend_from_slice(b"\r\n");
            rendered.extend_from_slice(&bytes);
            Some(rendered)
        }
        body => {
            let mut rendered = Vec::new();
            append_part_headers(&mut rendered, part, None);
            rendered.extend_from_slice(b"\r\n");
            rendered.extend_from_slice(&body_source_bytes(body)?);
            Some(rendered)
        }
    }
}

fn render_multipart_body(mime: *mut curl_mime, boundary: String) -> Option<(Vec<u8>, String)> {
    let mime = mime_mut(mime)?;
    let mut rendered = Vec::new();
    for raw_part in &mime.parts {
        let part = part_mut((*raw_part).cast())?;
        rendered.extend_from_slice(b"--");
        rendered.extend_from_slice(boundary.as_bytes());
        rendered.extend_from_slice(b"\r\n");
        rendered.extend_from_slice(&render_part_bytes(part)?);
        rendered.extend_from_slice(b"\r\n");
    }
    rendered.extend_from_slice(b"--");
    rendered.extend_from_slice(boundary.as_bytes());
    rendered.extend_from_slice(b"--\r\n");
    Some((rendered, boundary))
}

pub(crate) unsafe fn cleanup_body_source(body: &mut BodySource) {
    match std::mem::take(body) {
        BodySource::Callback(callback) => {
            if let Some(freefunc) = callback.freefunc {
                unsafe { freefunc(callback.arg) };
            }
        }
        BodySource::Subparts(subparts) => {
            unsafe { free_mime_tree(subparts, true) };
        }
        BodySource::None
        | BodySource::Owned(_)
        | BodySource::Borrowed { .. }
        | BodySource::FilePath(_) => {}
    }
}

fn replace_body(part: &mut MimePartHandle, body: BodySource) {
    unsafe { cleanup_body_source(&mut part.body) };
    part.body = body;
}

unsafe fn free_part(part: *mut MimePartHandle) {
    if part.is_null() {
        return;
    }
    let mut part = unsafe { Box::from_raw(part) };
    unsafe { cleanup_body_source(&mut part.body) };
    if !part.headers.owned_list.is_null() {
        unsafe { crate::slist::curl_slist_free_all(part.headers.owned_list) };
    }
}

unsafe fn free_mime_tree(mime: *mut curl_mime, from_owner: bool) {
    let Some(mime_ref) = mime_mut(mime) else {
        return;
    };
    if mime_ref.adopted && !from_owner {
        return;
    }
    let parts = std::mem::take(&mut mime_ref.parts);
    for part in parts {
        unsafe { free_part(part) };
    }
    unsafe {
        drop(Box::from_raw(mime as *mut MimeHandle));
    }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_mime_init(easy: *mut CURL) -> *mut curl_mime {
    Box::into_raw(Box::new(MimeHandle {
        easy,
        parts: Vec::new(),
        adopted: false,
    }))
    .cast()
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_mime_free(mime: *mut curl_mime) {
    unsafe { free_mime_tree(mime, false) };
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_mime_addpart(
    mime: *mut curl_mime,
) -> *mut curl_mimepart {
    let Some(mime) = mime_mut(mime) else {
        return ptr::null_mut();
    };
    let part = Box::into_raw(Box::new(MimePartHandle {
        name: None,
        filename: None,
        mime_type: None,
        encoder: None,
        headers: HeaderStore::default(),
        body: BodySource::None,
    }));
    mime.parts.push(part);
    part.cast()
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_mime_name(
    part: *mut curl_mimepart,
    name: *const c_char,
) -> CURLcode {
    let Some(part) = part_mut(part) else {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    };
    part.name = match c_string(name) {
        Ok(value) => value,
        Err(code) => return code,
    };
    CURLE_OK
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_mime_filename(
    part: *mut curl_mimepart,
    filename: *const c_char,
) -> CURLcode {
    let Some(part) = part_mut(part) else {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    };
    part.filename = match c_string(filename) {
        Ok(value) => value,
        Err(code) => return code,
    };
    CURLE_OK
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_mime_type(
    part: *mut curl_mimepart,
    mimetype: *const c_char,
) -> CURLcode {
    let Some(part) = part_mut(part) else {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    };
    part.mime_type = match c_string(mimetype) {
        Ok(value) => value,
        Err(code) => return code,
    };
    CURLE_OK
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_mime_encoder(
    part: *mut curl_mimepart,
    encoding: *const c_char,
) -> CURLcode {
    let Some(part) = part_mut(part) else {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    };
    part.encoder = match c_string(encoding) {
        Ok(value) => value,
        Err(code) => return code,
    };
    CURLE_OK
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_mime_data(
    part: *mut curl_mimepart,
    data: *const c_char,
    datasize: usize,
) -> CURLcode {
    let Some(part) = part_mut(part) else {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    };
    let body = match duplicate_bytes(data, datasize) {
        Ok(bytes) => BodySource::Owned(bytes),
        Err(code) => return code,
    };
    replace_body(part, body);
    CURLE_OK
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_mime_filedata(
    part: *mut curl_mimepart,
    filename: *const c_char,
) -> CURLcode {
    let Some(part) = part_mut(part) else {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    };
    let Some(filename) = (match c_string(filename) {
        Ok(value) => value,
        Err(code) => return code,
    }) else {
        replace_body(part, BodySource::None);
        return CURLE_OK;
    };
    replace_body(part, BodySource::FilePath(filename));
    CURLE_OK
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_mime_data_cb(
    part: *mut curl_mimepart,
    datasize: curl_off_t,
    readfunc: curl_read_callback,
    seekfunc: curl_seek_callback,
    freefunc: curl_free_callback,
    arg: *mut c_void,
) -> CURLcode {
    let Some(part) = part_mut(part) else {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    };
    replace_body(
        part,
        BodySource::Callback(CallbackBody {
            size: datasize,
            readfunc,
            seekfunc,
            freefunc,
            arg,
        }),
    );
    CURLE_OK
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_mime_subparts(
    part: *mut curl_mimepart,
    subparts: *mut curl_mime,
) -> CURLcode {
    let Some(part) = part_mut(part) else {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    };
    if let Some(subparts_ref) = mime_mut(subparts) {
        subparts_ref.adopted = true;
    }
    replace_body(part, BodySource::Subparts(subparts));
    CURLE_OK
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_mime_headers(
    part: *mut curl_mimepart,
    headers: *mut curl_slist,
    take_ownership: c_int,
) -> CURLcode {
    let Some(part) = part_mut(part) else {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    };
    if !part.headers.owned_list.is_null() {
        unsafe { crate::slist::curl_slist_free_all(part.headers.owned_list) };
    }
    part.headers.values = slist_strings(headers);
    part.headers.owned_list = if take_ownership != 0 {
        headers
    } else {
        ptr::null_mut()
    };
    CURLE_OK
}

pub(crate) fn mime_summary(mime: *mut curl_mime) -> Option<(usize, *mut CURL)> {
    let mime = mime_mut(mime)?;
    Some((mime.parts.len(), mime.easy))
}

pub(crate) fn mime_post_bytes(mime: *mut curl_mime) -> Option<(Vec<u8>, String)> {
    let boundary = nested_boundary(mime);
    let (bytes, boundary) = render_multipart_body(mime, boundary)?;
    Some((bytes, format!("multipart/form-data; boundary={boundary}")))
}

pub(crate) fn part_body_bytes(part: *mut curl_mimepart) -> Option<Vec<u8>> {
    let part = part_mut(part)?;
    body_source_bytes(&part.body)
}
