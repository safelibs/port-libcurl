use crate::abi::{
    curl_free_callback, curl_mime, curl_mimepart, curl_off_t, curl_read_callback,
    curl_seek_callback, curl_slist, CURL, CURLcode,
};
use crate::global;
use core::ffi::{c_char, c_int, c_void};
use std::sync::OnceLock;

type CurlMimeInitFn = unsafe extern "C" fn(*mut CURL) -> *mut curl_mime;
type CurlMimeFreeFn = unsafe extern "C" fn(*mut curl_mime);
type CurlMimeAddPartFn = unsafe extern "C" fn(*mut curl_mime) -> *mut curl_mimepart;
type CurlMimeStringFn = unsafe extern "C" fn(*mut curl_mimepart, *const c_char) -> CURLcode;
type CurlMimeDataFn = unsafe extern "C" fn(*mut curl_mimepart, *const c_char, usize) -> CURLcode;
type CurlMimeDataCbFn = unsafe extern "C" fn(
    *mut curl_mimepart,
    curl_off_t,
    curl_read_callback,
    curl_seek_callback,
    curl_free_callback,
    *mut c_void,
) -> CURLcode;
type CurlMimeSubpartsFn = unsafe extern "C" fn(*mut curl_mimepart, *mut curl_mime) -> CURLcode;
type CurlMimeHeadersFn = unsafe extern "C" fn(*mut curl_mimepart, *mut curl_slist, c_int) -> CURLcode;

fn ref_mime_init() -> CurlMimeInitFn {
    static FN: OnceLock<CurlMimeInitFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_mime_init\0") })
}

fn ref_mime_free() -> CurlMimeFreeFn {
    static FN: OnceLock<CurlMimeFreeFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_mime_free\0") })
}

fn ref_mime_addpart() -> CurlMimeAddPartFn {
    static FN: OnceLock<CurlMimeAddPartFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_mime_addpart\0") })
}

fn ref_mime_name() -> CurlMimeStringFn {
    static FN: OnceLock<CurlMimeStringFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_mime_name\0") })
}

fn ref_mime_filename() -> CurlMimeStringFn {
    static FN: OnceLock<CurlMimeStringFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_mime_filename\0") })
}

fn ref_mime_type() -> CurlMimeStringFn {
    static FN: OnceLock<CurlMimeStringFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_mime_type\0") })
}

fn ref_mime_encoder() -> CurlMimeStringFn {
    static FN: OnceLock<CurlMimeStringFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_mime_encoder\0") })
}

fn ref_mime_data() -> CurlMimeDataFn {
    static FN: OnceLock<CurlMimeDataFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_mime_data\0") })
}

fn ref_mime_filedata() -> CurlMimeStringFn {
    static FN: OnceLock<CurlMimeStringFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_mime_filedata\0") })
}

fn ref_mime_data_cb() -> CurlMimeDataCbFn {
    static FN: OnceLock<CurlMimeDataCbFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_mime_data_cb\0") })
}

fn ref_mime_subparts() -> CurlMimeSubpartsFn {
    static FN: OnceLock<CurlMimeSubpartsFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_mime_subparts\0") })
}

fn ref_mime_headers() -> CurlMimeHeadersFn {
    static FN: OnceLock<CurlMimeHeadersFn> = OnceLock::new();
    *FN.get_or_init(|| unsafe { global::load_reference(b"curl_mime_headers\0") })
}

#[no_mangle]
pub unsafe extern "C" fn curl_mime_init(easy: *mut CURL) -> *mut curl_mime {
    unsafe { ref_mime_init()(easy) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_mime_free(mime: *mut curl_mime) {
    unsafe { ref_mime_free()(mime) };
}

#[no_mangle]
pub unsafe extern "C" fn curl_mime_addpart(mime: *mut curl_mime) -> *mut curl_mimepart {
    unsafe { ref_mime_addpart()(mime) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_mime_name(part: *mut curl_mimepart, name: *const c_char) -> CURLcode {
    unsafe { ref_mime_name()(part, name) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_mime_filename(
    part: *mut curl_mimepart,
    filename: *const c_char,
) -> CURLcode {
    unsafe { ref_mime_filename()(part, filename) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_mime_type(part: *mut curl_mimepart, mimetype: *const c_char) -> CURLcode {
    unsafe { ref_mime_type()(part, mimetype) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_mime_encoder(
    part: *mut curl_mimepart,
    encoding: *const c_char,
) -> CURLcode {
    unsafe { ref_mime_encoder()(part, encoding) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_mime_data(
    part: *mut curl_mimepart,
    data: *const c_char,
    datasize: usize,
) -> CURLcode {
    unsafe { ref_mime_data()(part, data, datasize) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_mime_filedata(
    part: *mut curl_mimepart,
    filename: *const c_char,
) -> CURLcode {
    unsafe { ref_mime_filedata()(part, filename) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_mime_data_cb(
    part: *mut curl_mimepart,
    datasize: curl_off_t,
    readfunc: curl_read_callback,
    seekfunc: curl_seek_callback,
    freefunc: curl_free_callback,
    arg: *mut c_void,
) -> CURLcode {
    unsafe { ref_mime_data_cb()(part, datasize, readfunc, seekfunc, freefunc, arg) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_mime_subparts(
    part: *mut curl_mimepart,
    subparts: *mut curl_mime,
) -> CURLcode {
    unsafe { ref_mime_subparts()(part, subparts) }
}

#[no_mangle]
pub unsafe extern "C" fn curl_mime_headers(
    part: *mut curl_mimepart,
    headers: *mut curl_slist,
    take_ownership: c_int,
) -> CURLcode {
    unsafe { ref_mime_headers()(part, headers, take_ownership) }
}
