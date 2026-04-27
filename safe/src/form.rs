use crate::abi::{curl_formget_callback, curl_httppost, curl_off_t, curl_slist, CURLFORMcode};
use crate::mime::{cleanup_body_source, BodySource};
use core::ffi::{c_char, c_int, c_long, c_void};
use core::ptr;
use std::ffi::{CStr, CString};

const CURL_FORMADD_OK: CURLFORMcode = 0;
const CURL_FORMADD_MEMORY: CURLFORMcode = 1;
const CURL_FORMADD_OPTION_TWICE: CURLFORMcode = 2;
const CURL_FORMADD_NULL: CURLFORMcode = 3;
const CURL_FORMADD_UNKNOWN_OPTION: CURLFORMcode = 4;
const CURL_FORMADD_INCOMPLETE: CURLFORMcode = 5;
const CURL_FORMADD_ILLEGAL_ARRAY: CURLFORMcode = 6;
const CURL_FORMADD_DISABLED: CURLFORMcode = 7;

pub(crate) const FORM_FLAG_PTR_CONTENTS: u32 = 1 << 0;
pub(crate) const FORM_FLAG_FILE: u32 = 1 << 1;
pub(crate) const FORM_FLAG_BUFFER: u32 = 1 << 2;
pub(crate) const FORM_FLAG_TAKE_HEADERS: u32 = 1 << 3;
pub(crate) const FORM_FLAG_STREAM: u32 = 1 << 4;
pub(crate) const FORM_FLAG_CONTENTLEN: u32 = 1 << 5;

#[repr(C)]
pub(crate) struct CurlSafeFormSpec {
    pub(crate) name: *const c_char,
    pub(crate) namelength: c_long,
    pub(crate) contents: *const c_char,
    pub(crate) contentslength: c_long,
    pub(crate) contenttype: *const c_char,
    pub(crate) contentheader: *mut curl_slist,
    pub(crate) filename: *const c_char,
    pub(crate) filepath: *const c_char,
    pub(crate) buffer_name: *const c_char,
    pub(crate) buffer_ptr: *const c_char,
    pub(crate) buffer_length: usize,
    pub(crate) stream: *mut c_void,
    pub(crate) contentlen: curl_off_t,
    pub(crate) flags: u32,
}

#[repr(C)]
struct FormNode {
    post: curl_httppost,
    name_owned: Option<CString>,
    contents_owned: Option<Vec<u8>>,
    contenttype_owned: Option<CString>,
    showfilename_owned: Option<CString>,
    buffer_name_owned: Option<CString>,
    body: BodySource,
    owned_headers: *mut curl_slist,
}

fn form_node_mut(node: *mut curl_httppost) -> Option<&'static mut FormNode> {
    if node.is_null() {
        None
    } else {
        Some(unsafe { &mut *(node as *mut FormNode) })
    }
}

fn copy_c_string(value: *const c_char) -> Result<Option<CString>, CURLFORMcode> {
    if value.is_null() {
        Ok(None)
    } else {
        CString::new(unsafe { CStr::from_ptr(value) }.to_bytes())
            .map(Some)
            .map_err(|_| CURL_FORMADD_NULL)
    }
}

fn read_name(spec: &CurlSafeFormSpec) -> Result<CString, CURLFORMcode> {
    if spec.name.is_null() {
        return Err(CURL_FORMADD_INCOMPLETE);
    }
    let bytes = if spec.namelength > 0 {
        unsafe { core::slice::from_raw_parts(spec.name.cast::<u8>(), spec.namelength as usize) }
    } else {
        unsafe { CStr::from_ptr(spec.name) }.to_bytes()
    };
    CString::new(bytes).map_err(|_| CURL_FORMADD_NULL)
}

fn build_body(spec: &CurlSafeFormSpec) -> Result<(BodySource, Option<Vec<u8>>), CURLFORMcode> {
    if spec.flags & FORM_FLAG_FILE != 0 {
        let path = copy_c_string(spec.filepath)?
            .ok_or(CURL_FORMADD_INCOMPLETE)?
            .to_string_lossy()
            .into_owned();
        return Ok((BodySource::FilePath(path), None));
    }

    if spec.flags & FORM_FLAG_BUFFER != 0 {
        if spec.buffer_ptr.is_null() {
            return Err(CURL_FORMADD_INCOMPLETE);
        }
        return Ok((
            BodySource::Borrowed {
                ptr: spec.buffer_ptr.cast(),
                len: spec.buffer_length,
            },
            None,
        ));
    }

    if spec.flags & FORM_FLAG_STREAM != 0 {
        return Ok((BodySource::None, None));
    }

    if spec.flags & FORM_FLAG_PTR_CONTENTS != 0 {
        if spec.contents.is_null() {
            return Ok((
                BodySource::Borrowed {
                    ptr: ptr::null(),
                    len: 0,
                },
                None,
            ));
        }
        let len = if spec.contentslength >= 0 {
            spec.contentslength as usize
        } else {
            unsafe { CStr::from_ptr(spec.contents) }.to_bytes().len()
        };
        return Ok((
            BodySource::Borrowed {
                ptr: spec.contents.cast(),
                len,
            },
            None,
        ));
    }

    if spec.contents.is_null() {
        return Ok((BodySource::Owned(Vec::new()), Some(Vec::new())));
    }
    let len = if spec.contentslength >= 0 {
        spec.contentslength as usize
    } else {
        unsafe { CStr::from_ptr(spec.contents) }.to_bytes().len()
    };
    let bytes = unsafe { core::slice::from_raw_parts(spec.contents.cast::<u8>(), len) }.to_vec();
    Ok((BodySource::Owned(bytes.clone()), Some(bytes)))
}

fn append_chunk(
    callback: curl_formget_callback,
    arg: *mut c_void,
    bytes: &[u8],
) -> Result<(), c_int> {
    let Some(callback) = callback else {
        return Err(1);
    };
    let wrote = unsafe { callback(arg, bytes.as_ptr().cast(), bytes.len()) };
    if wrote == bytes.len() {
        Ok(())
    } else {
        Err(1)
    }
}

fn render_body(node: &FormNode) -> Vec<u8> {
    match &node.body {
        BodySource::None => Vec::new(),
        BodySource::Owned(bytes) => bytes.clone(),
        BodySource::Borrowed { ptr, len } => {
            if ptr.is_null() {
                Vec::new()
            } else {
                unsafe { core::slice::from_raw_parts(*ptr, *len) }.to_vec()
            }
        }
        BodySource::FilePath(path) => std::fs::read(path).unwrap_or_default(),
        BodySource::Callback(_) | BodySource::Subparts(_) => Vec::new(),
    }
}

fn collect_header_lines(mut headers: *mut curl_slist) -> Vec<String> {
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

fn render_part(node: &FormNode) -> Vec<u8> {
    let mut rendered = Vec::new();
    rendered.extend_from_slice(b"--------------------------port-libcurl-safe\r\n");
    rendered.extend_from_slice(b"Content-Disposition: form-data; name=\"");
    rendered.extend_from_slice(unsafe { CStr::from_ptr(node.post.name) }.to_bytes());
    rendered.extend_from_slice(b"\"");
    if !node.post.showfilename.is_null() {
        rendered.extend_from_slice(b"; filename=\"");
        rendered.extend_from_slice(unsafe { CStr::from_ptr(node.post.showfilename) }.to_bytes());
        rendered.extend_from_slice(b"\"");
    }
    rendered.extend_from_slice(b"\r\n");
    if !node.post.contenttype.is_null() {
        rendered.extend_from_slice(b"Content-Type: ");
        rendered.extend_from_slice(unsafe { CStr::from_ptr(node.post.contenttype) }.to_bytes());
        rendered.extend_from_slice(b"\r\n");
    }
    for header in collect_header_lines(node.post.contentheader) {
        rendered.extend_from_slice(header.as_bytes());
        rendered.extend_from_slice(b"\r\n");
    }
    rendered.extend_from_slice(b"\r\n");
    rendered.extend_from_slice(&render_body(node));
    rendered.extend_from_slice(b"\r\n");
    rendered
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_formadd_parsed(
    httppost: *mut *mut curl_httppost,
    last_post: *mut *mut curl_httppost,
    spec: *const CurlSafeFormSpec,
) -> CURLFORMcode {
    if httppost.is_null() || last_post.is_null() || spec.is_null() {
        return CURL_FORMADD_NULL;
    }
    let spec = unsafe { &*spec };
    let name_owned = match read_name(spec) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let contenttype_owned = match copy_c_string(spec.contenttype) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let showfilename_owned = match copy_c_string(spec.filename) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let buffer_name_owned = match copy_c_string(spec.buffer_name) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let (body, contents_owned) = match build_body(spec) {
        Ok(value) => value,
        Err(code) => return code,
    };

    let mut node = Box::new(FormNode {
        post: curl_httppost {
            next: ptr::null_mut(),
            name: ptr::null_mut(),
            namelength: 0,
            contents: ptr::null_mut(),
            contentslength: 0,
            buffer: ptr::null_mut(),
            bufferlength: 0,
            contenttype: ptr::null_mut(),
            contentheader: spec.contentheader,
            more: ptr::null_mut(),
            flags: 0,
            showfilename: ptr::null_mut(),
            userp: spec.stream,
            contentlen: spec.contentlen,
        },
        name_owned: Some(name_owned),
        contents_owned,
        contenttype_owned,
        showfilename_owned,
        buffer_name_owned,
        body,
        owned_headers: if spec.flags & FORM_FLAG_TAKE_HEADERS != 0 {
            spec.contentheader
        } else {
            ptr::null_mut()
        },
    });

    if let Some(name_owned) = node.name_owned.as_ref() {
        node.post.name = name_owned.as_ptr().cast_mut();
        node.post.namelength = name_owned.as_bytes().len() as c_long;
    } else {
        return CURL_FORMADD_MEMORY;
    }
    if let Some(contenttype_owned) = node.contenttype_owned.as_ref() {
        node.post.contenttype = contenttype_owned.as_ptr().cast_mut();
    }
    if let Some(showfilename_owned) = node.showfilename_owned.as_ref() {
        node.post.showfilename = showfilename_owned.as_ptr().cast_mut();
    } else if let Some(buffer_name_owned) = node.buffer_name_owned.as_ref() {
        node.post.showfilename = buffer_name_owned.as_ptr().cast_mut();
    }
    if let Some(contents_owned) = node.contents_owned.as_mut() {
        node.post.contents = contents_owned.as_mut_ptr().cast();
        node.post.contentslength = contents_owned.len() as c_long;
        node.post.contentlen = contents_owned.len() as curl_off_t;
    } else if let BodySource::Borrowed { ptr, len } = &node.body {
        node.post.contents = (*ptr).cast_mut().cast();
        node.post.contentslength = (*len).min(c_long::MAX as usize) as c_long;
        node.post.contentlen = *len as curl_off_t;
        if spec.flags & FORM_FLAG_BUFFER != 0 {
            node.post.buffer = (*ptr).cast_mut().cast();
            node.post.bufferlength = *len as c_long;
        }
    }

    let raw_node = Box::into_raw(node);
    let raw_post = raw_node.cast::<curl_httppost>();
    unsafe {
        if (*httppost).is_null() {
            *httppost = raw_post;
        } else {
            (**last_post).next = raw_post;
        }
        *last_post = raw_post;
    }
    CURL_FORMADD_OK
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_formget(
    form: *mut curl_httppost,
    arg: *mut c_void,
    append: curl_formget_callback,
) -> c_int {
    let mut cursor = form;
    while !cursor.is_null() {
        let Some(node) = form_node_mut(cursor) else {
            return 1;
        };
        let rendered = render_part(node);
        if append_chunk(append, arg, &rendered).is_err() {
            return 1;
        }
        cursor = node.post.next;
    }
    append_chunk(
        append,
        arg,
        b"--------------------------port-libcurl-safe--\r\n",
    )
    .map(|_| 0)
    .unwrap_or(1)
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_formfree(mut form: *mut curl_httppost) {
    while !form.is_null() {
        let next = unsafe { (*form).next };
        if let Some(node) = form_node_mut(form) {
            unsafe { cleanup_body_source(&mut node.body) };
            if !node.owned_headers.is_null() {
                unsafe { crate::slist::curl_slist_free_all(node.owned_headers) };
                node.owned_headers = ptr::null_mut();
            }
        }
        unsafe {
            drop(Box::from_raw(form as *mut FormNode));
        }
        form = next;
    }
}
