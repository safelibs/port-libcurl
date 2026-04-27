use crate::abi::{curl_off_t, curl_pushheaders, CURLMcode, CURLMoption, CURLMsg, CURL, CURLM};
use core::ffi::{c_char, c_int, c_long, c_uint, c_void};

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_multi_init() -> *mut CURLM {
    unsafe { crate::multi::init_handle() }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_multi_cleanup(multi_handle: *mut CURLM) -> CURLMcode {
    unsafe { crate::multi::cleanup_handle(multi_handle) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_multi_add_handle(
    multi_handle: *mut CURLM,
    curl_handle: *mut CURL,
) -> CURLMcode {
    unsafe { crate::multi::add_handle(multi_handle, curl_handle) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_multi_remove_handle(
    multi_handle: *mut CURLM,
    curl_handle: *mut CURL,
) -> CURLMcode {
    unsafe { crate::multi::remove_handle(multi_handle, curl_handle) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_multi_fdset(
    multi_handle: *mut CURLM,
    read_fd_set: *mut c_void,
    write_fd_set: *mut c_void,
    exc_fd_set: *mut c_void,
    max_fd: *mut c_int,
) -> CURLMcode {
    unsafe {
        crate::multi::fdset_handle(
            multi_handle,
            read_fd_set.cast(),
            write_fd_set.cast(),
            exc_fd_set.cast(),
            max_fd,
        )
    }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_multi_perform(
    multi_handle: *mut CURLM,
    running_handles: *mut c_int,
) -> CURLMcode {
    unsafe { crate::multi::perform_handle(multi_handle, running_handles) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_multi_wait(
    multi_handle: *mut CURLM,
    extra_fds: *mut crate::abi::curl_waitfd,
    extra_nfds: c_uint,
    timeout_ms: c_int,
    ret: *mut c_int,
) -> CURLMcode {
    unsafe { crate::multi::wait_handle(multi_handle, extra_fds, extra_nfds, timeout_ms, ret) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_multi_poll(
    multi_handle: *mut CURLM,
    extra_fds: *mut crate::abi::curl_waitfd,
    extra_nfds: c_uint,
    timeout_ms: c_int,
    ret: *mut c_int,
) -> CURLMcode {
    unsafe { crate::multi::poll_handle(multi_handle, extra_fds, extra_nfds, timeout_ms, ret) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_multi_timeout(
    multi_handle: *mut CURLM,
    milliseconds: *mut c_long,
) -> CURLMcode {
    unsafe { crate::multi::timeout_handle(multi_handle, milliseconds) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_multi_wakeup(multi_handle: *mut CURLM) -> CURLMcode {
    unsafe { crate::multi::wakeup_handle(multi_handle) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_multi_info_read(
    multi_handle: *mut CURLM,
    msgs_in_queue: *mut c_int,
) -> *mut CURLMsg {
    unsafe { crate::multi::info_read_handle(multi_handle, msgs_in_queue) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_multi_socket(
    multi_handle: *mut CURLM,
    socket: crate::abi::curl_socket_t,
    running_handles: *mut c_int,
) -> CURLMcode {
    unsafe { crate::multi::socket_handle(multi_handle, socket, running_handles) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_multi_socket_all(
    multi_handle: *mut CURLM,
    running_handles: *mut c_int,
) -> CURLMcode {
    unsafe { crate::multi::socket_all_handle(multi_handle, running_handles) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_multi_socket_action(
    multi_handle: *mut CURLM,
    socket: crate::abi::curl_socket_t,
    ev_bitmask: c_int,
    running_handles: *mut c_int,
) -> CURLMcode {
    unsafe { crate::multi::socket_action_handle(multi_handle, socket, ev_bitmask, running_handles) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_multi_assign(
    multi_handle: *mut CURLM,
    socket: crate::abi::curl_socket_t,
    socketp: *mut c_void,
) -> CURLMcode {
    unsafe { crate::multi::assign_handle(multi_handle, socket, socketp) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_multi_strerror(code: CURLMcode) -> *const c_char {
    unsafe { crate::multi::multi_strerror(code) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_multi_setopt_long(
    multi_handle: *mut CURLM,
    option: CURLMoption,
    value: c_long,
) -> CURLMcode {
    unsafe { crate::multi::dispatch_setopt_long(multi_handle, option, value) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_multi_setopt_ptr(
    multi_handle: *mut CURLM,
    option: CURLMoption,
    value: *mut c_void,
) -> CURLMcode {
    unsafe { crate::multi::dispatch_setopt_ptr(multi_handle, option, value) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_multi_setopt_function(
    multi_handle: *mut CURLM,
    option: CURLMoption,
    value: Option<unsafe extern "C" fn()>,
) -> CURLMcode {
    unsafe { crate::multi::dispatch_setopt_function(multi_handle, option, value) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_multi_setopt_off_t(
    multi_handle: *mut CURLM,
    option: CURLMoption,
    value: curl_off_t,
) -> CURLMcode {
    unsafe { crate::multi::dispatch_setopt_off_t(multi_handle, option, value) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_pushheader_byname(
    headers: *mut curl_pushheaders,
    name: *const c_char,
) -> *mut c_char {
    unsafe { crate::protocols::pushheader_byname(headers, name) }
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_pushheader_bynum(
    headers: *mut curl_pushheaders,
    index: usize,
) -> *mut c_char {
    unsafe { crate::protocols::pushheader_bynum(headers, index) }
}
