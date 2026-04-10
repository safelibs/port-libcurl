use crate::abi::{CURLcode, CURL};
use crate::easy::perform::{EasyCallbacks, EasyMetadata};
use crate::http::auth;
use crate::protocols::ParsedProtocolUrl;
use crate::transfer::{self, LowSpeedGuard, TransferPlan, TransportStream};
use core::ffi::{c_char, c_int, c_void};
use std::ffi::CString;
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

pub(crate) const UPSTREAM_SOURCES: &[&str] = &[
    "original/lib/vssh/libssh.c",
    "original/lib/vssh/libssh2.c",
    "original/lib/vssh/wolfssh.c",
];

const CURLE_URL_MALFORMAT: CURLcode = 3;
const CURLE_COULDNT_CONNECT: CURLcode = 7;
const CURLE_REMOTE_ACCESS_DENIED: CURLcode = 9;
const CURLE_WRITE_ERROR: CURLcode = 23;
const CURLE_SEND_ERROR: CURLcode = 55;
const CURLE_RECV_ERROR: CURLcode = 56;
const CURLE_LOGIN_DENIED: CURLcode = 67;
const CURLE_REMOTE_FILE_NOT_FOUND: CURLcode = 78;

const CURL_SAFE_SSH_OK: c_int = 0;
const CURL_SAFE_SSH_CONNECT: c_int = 1;
const CURL_SAFE_SSH_AUTH: c_int = 2;
const CURL_SAFE_SSH_REMOTE_NOT_FOUND: c_int = 3;
const CURL_SAFE_SSH_REMOTE_ACCESS: c_int = 4;
const CURL_SAFE_SSH_SEND: c_int = 5;
const CURL_SAFE_SSH_RECV: c_int = 6;
const CURL_SAFE_SSH_CALLBACK: c_int = 7;

const IO_TIMEOUT: Duration = Duration::from_secs(30);
const UPLOAD_CHUNK_SIZE: usize = 16 * 1024;

unsafe extern "C" {
    fn curl_safe_ssh_transfer(
        fd: c_int,
        scheme: *const c_char,
        username: *const c_char,
        password: *const c_char,
        path: *const c_char,
        upload: c_int,
        upload_data: *const u8,
        upload_len: usize,
        write_cb: Option<unsafe extern "C" fn(*const c_char, usize, *mut c_void) -> isize>,
        write_ctx: *mut c_void,
        transferred: *mut u64,
        errbuf: *mut c_char,
        errlen: usize,
    ) -> c_int;
}

pub(crate) fn is_ssh_scheme(scheme: &str) -> bool {
    matches!(scheme, "scp" | "sftp")
}

pub(crate) fn perform_transfer(
    handle: *mut CURL,
    plan: &TransferPlan,
    metadata: &EasyMetadata,
    callbacks: EasyCallbacks,
) -> CURLcode {
    let Some(url) = metadata.url.as_deref() else {
        crate::easy::perform::set_error_buffer(handle, "No URL set");
        return CURLE_URL_MALFORMAT;
    };
    let parsed = match ParsedProtocolUrl::parse(url) {
        Ok(parsed) => parsed,
        Err(code) => return code,
    };
    let remote_path = match parsed.decoded_path() {
        Ok(path) if path != "/" => path,
        Ok(_) => {
            crate::easy::perform::set_error_buffer(handle, "SSH URL must include a remote path");
            return CURLE_URL_MALFORMAT;
        }
        Err(code) => return code,
    };
    let credentials = ssh_credentials(&parsed, metadata);
    let upload_data = if metadata.upload {
        match collect_upload_body(handle, callbacks, metadata.upload_size) {
            Ok(data) => data,
            Err(code) => return code,
        }
    } else {
        Vec::new()
    };

    let started = Instant::now();
    let connected = match transfer::connect_protocol_transport(
        handle,
        &parsed.host,
        parsed.port,
        plan,
        metadata,
        callbacks,
    ) {
        Ok(stream) => stream,
        Err(code) => return code,
    };
    let mut info = connected.info.clone();
    let stream = match connected.stream {
        TransportStream::Plain(stream) => stream,
        TransportStream::Tls(_) => {
            crate::easy::perform::set_error_buffer(handle, "SSH transport unexpectedly negotiated TLS");
            return CURLE_COULDNT_CONNECT;
        }
    };
    if stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .and_then(|_| stream.set_write_timeout(Some(IO_TIMEOUT)))
        .is_err()
    {
        transfer::close_transport(TransportStream::Plain(stream), callbacks);
        return CURLE_COULDNT_CONNECT;
    }

    let scheme_c = match CString::new(parsed.scheme.as_str()) {
        Ok(value) => value,
        Err(_) => {
            transfer::close_transport(TransportStream::Plain(stream), callbacks);
            return CURLE_URL_MALFORMAT;
        }
    };
    let user_c = credentials
        .username
        .as_deref()
        .map(CString::new)
        .transpose()
        .map_err(|_| CURLE_URL_MALFORMAT);
    let pass_c = credentials
        .password
        .as_deref()
        .map(CString::new)
        .transpose()
        .map_err(|_| CURLE_URL_MALFORMAT);
    let path_c = match CString::new(remote_path.as_str()) {
        Ok(value) => value,
        Err(_) => {
            transfer::close_transport(TransportStream::Plain(stream), callbacks);
            return CURLE_URL_MALFORMAT;
        }
    };
    let (user_c, pass_c) = match (user_c, pass_c) {
        (Ok(user_c), Ok(pass_c)) => (user_c, pass_c),
        _ => {
            transfer::close_transport(TransportStream::Plain(stream), callbacks);
            return CURLE_URL_MALFORMAT;
        }
    };

    let mut write_ctx = if metadata.upload {
        None
    } else {
        let context = Box::new(DownloadContext {
            handle,
            callbacks,
            low_speed: LowSpeedGuard::new(plan.low_speed),
            delivered: 0,
            last_error: None,
        });
        if let Err(code) = transfer::invoke_progress_callback(callbacks, 0, None) {
            transfer::close_transport(TransportStream::Plain(stream), callbacks);
            return code;
        }
        Some(context)
    };

    let mut transferred = 0u64;
    let mut errbuf = [0i8; 256];
    let rc = unsafe {
        curl_safe_ssh_transfer(
            stream.as_raw_fd(),
            scheme_c.as_ptr(),
            user_c.as_ref().map_or(core::ptr::null(), |value| value.as_ptr()),
            pass_c.as_ref().map_or(core::ptr::null(), |value| value.as_ptr()),
            path_c.as_ptr(),
            metadata.upload as c_int,
            upload_data.as_ptr(),
            upload_data.len(),
            if metadata.upload {
                None
            } else {
                Some(ssh_write_callback)
            },
            write_ctx
                .as_mut()
                .map_or(core::ptr::null_mut(), |ctx| ctx.as_mut() as *mut _ as *mut c_void),
            &mut transferred,
            errbuf.as_mut_ptr(),
            errbuf.len(),
        )
    };

    let code = if rc == CURL_SAFE_SSH_CALLBACK {
        write_ctx
            .as_ref()
            .and_then(|ctx| ctx.last_error)
            .unwrap_or(CURLE_WRITE_ERROR)
    } else if rc != CURL_SAFE_SSH_OK {
        let message = unsafe { std::ffi::CStr::from_ptr(errbuf.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        if !message.is_empty() {
            crate::easy::perform::set_error_buffer(handle, &message);
        }
        match rc {
            CURL_SAFE_SSH_AUTH => CURLE_LOGIN_DENIED,
            CURL_SAFE_SSH_REMOTE_NOT_FOUND => CURLE_REMOTE_FILE_NOT_FOUND,
            CURL_SAFE_SSH_REMOTE_ACCESS => CURLE_REMOTE_ACCESS_DENIED,
            CURL_SAFE_SSH_SEND => CURLE_SEND_ERROR,
            CURL_SAFE_SSH_RECV => CURLE_RECV_ERROR,
            CURL_SAFE_SSH_CONNECT => CURLE_COULDNT_CONNECT,
            _ => CURLE_RECV_ERROR,
        }
    } else {
        if metadata.upload {
            let _ = transfer::invoke_progress_callback(
                callbacks,
                transferred as usize,
                Some(upload_data.len()),
            );
        }
        info.pretransfer_time_us = info.connect_time_us;
        info.starttransfer_time_us = info.connect_time_us;
        info.total_time_us = transfer::elapsed_us(started.elapsed());
        crate::easy::perform::record_transfer_info(handle, info);
        crate::abi::CURLE_OK
    };

    transfer::close_transport(TransportStream::Plain(stream), callbacks);
    code
}

#[derive(Default)]
struct SshCredentials {
    username: Option<String>,
    password: Option<String>,
}

fn ssh_credentials(parsed: &ParsedProtocolUrl, metadata: &EasyMetadata) -> SshCredentials {
    if let Some(explicit) = auth::explicit_basic_credentials(metadata) {
        return SshCredentials {
            username: Some(explicit.username),
            password: Some(explicit.password),
        };
    }
    SshCredentials {
        username: parsed
            .username
            .clone()
            .or_else(|| std::env::var("USER").ok()),
        password: parsed.password.clone().or_else(|| metadata.password.clone()),
    }
}

fn collect_upload_body(
    handle: *mut CURL,
    callbacks: EasyCallbacks,
    advertised_size: Option<i64>,
) -> Result<Vec<u8>, CURLcode> {
    let capacity = advertised_size
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(UPLOAD_CHUNK_SIZE);
    let mut body = Vec::with_capacity(capacity);
    let mut chunk = vec![0u8; UPLOAD_CHUNK_SIZE];
    transfer::invoke_progress_callback(callbacks, 0, None)?;
    loop {
        let read = transfer::read_request_body_chunk(handle, callbacks, &mut chunk)?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    Ok(body)
}

struct DownloadContext {
    handle: *mut CURL,
    callbacks: EasyCallbacks,
    low_speed: LowSpeedGuard,
    delivered: usize,
    last_error: Option<CURLcode>,
}

unsafe extern "C" fn ssh_write_callback(
    buffer: *const c_char,
    len: usize,
    ctx: *mut c_void,
) -> isize {
    if buffer.is_null() || ctx.is_null() {
        return -1;
    }
    let context = unsafe { &mut *(ctx as *mut DownloadContext) };
    let mut body = unsafe { std::slice::from_raw_parts(buffer.cast::<u8>(), len) }.to_vec();
    if let Err(code) = transfer::deliver_write(context.handle, context.callbacks, &mut body) {
        context.last_error = Some(code);
        return -1;
    }
    if let Err(code) = context.low_speed.observe_progress(len) {
        context.last_error = Some(code);
        return -1;
    }
    context.delivered = context.delivered.saturating_add(len);
    if let Err(code) =
        transfer::invoke_progress_callback(context.callbacks, context.delivered, None)
    {
        context.last_error = Some(code);
        return -1;
    }
    len as isize
}
