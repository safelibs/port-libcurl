use crate::abi::{CURLcode, CURL};
use crate::easy::perform::{EasyCallbacks, EasyMetadata, RecordedTransferInfo};
use crate::http::auth;
use crate::protocols::ParsedProtocolUrl;
use crate::transfer::{self, LowSpeedGuard, TransferPlan, TransportStream};
use std::io::{ErrorKind, Read, Write};
use std::time::{Duration, Instant};

pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/rtsp.c"];

const CURLE_URL_MALFORMAT: CURLcode = 3;
const CURLE_COULDNT_CONNECT: CURLcode = 7;
const CURLE_REMOTE_ACCESS_DENIED: CURLcode = 9;
const CURLE_OPERATION_TIMEDOUT: CURLcode = 28;
const CURLE_SEND_ERROR: CURLcode = 55;
const CURLE_RECV_ERROR: CURLcode = 56;

const CONTROL_TIMEOUT: Duration = Duration::from_secs(30);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) fn matches(scheme: &str) -> bool {
    scheme == "rtsp"
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
        Err(code) => {
            crate::easy::perform::set_error_buffer(handle, "Malformed RTSP URL");
            return code;
        }
    };
    let method = rtsp_method(metadata);
    let request_uri = metadata
        .rtsp_stream_uri
        .clone()
        .unwrap_or_else(|| parsed.raw_url.clone());
    let started = Instant::now();
    let mut control = match transfer::connect_protocol_transport(
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
    if control
        .stream
        .set_read_timeout(Some(CONTROL_TIMEOUT))
        .and_then(|_| control.stream.set_write_timeout(Some(WRITE_TIMEOUT)))
        .is_err()
    {
        transfer::close_transport(control.stream, callbacks);
        return CURLE_COULDNT_CONNECT;
    }

    let mut info = control.info.clone();
    info.pretransfer_time_us = info.connect_time_us;
    let result = perform_transfer_inner(
        handle,
        metadata,
        callbacks,
        &method,
        &request_uri,
        &mut control.stream,
        &mut info,
        started,
    );
    transfer::close_transport(control.stream, callbacks);
    result
}

fn perform_transfer_inner(
    handle: *mut CURL,
    metadata: &EasyMetadata,
    callbacks: EasyCallbacks,
    method: &str,
    request_uri: &str,
    stream: &mut TransportStream,
    info: &mut RecordedTransferInfo,
    started: Instant,
) -> CURLcode {
    let authorization = auth::resolve_basic_credentials(metadata, request_uri).map(|credentials| {
        let token = auth::base64_encode(
            format!("{}:{}", credentials.username, credentials.password).as_bytes(),
        );
        format!("Authorization: Basic {token}")
    });

    let mut cseq = 1usize;
    let mut sent_authorization = None;
    loop {
        if let Err(code) = write_request(
            stream,
            metadata,
            method,
            request_uri,
            cseq,
            sent_authorization.as_deref(),
        ) {
            return code;
        }
        let response = match read_response(stream, handle, callbacks, metadata) {
            Ok(response) => response,
            Err(code) => return code,
        };
        crate::easy::perform::record_rtsp_session_id(handle, response.session_id.as_deref());
        info.starttransfer_time_us = transfer::elapsed_us(started.elapsed());

        if response.status_code == 401
            && sent_authorization.is_none()
            && response.basic_auth_requested
            && authorization.is_some()
        {
            sent_authorization = authorization.clone();
            cseq += 1;
            continue;
        }
        if !(200..300).contains(&response.status_code) {
            crate::easy::perform::set_error_buffer(handle, "RTSP request failed");
            return CURLE_REMOTE_ACCESS_DENIED;
        }

        let mut low_speed = LowSpeedGuard::new(metadata.low_speed);
        if let Err(code) = transfer::invoke_progress_callback(callbacks, 0, response.content_length)
        {
            return code;
        }
        if let Err(code) = transfer::transfer_body(
            stream,
            handle,
            callbacks,
            response.body_prefix,
            response.content_length,
            &mut low_speed,
        ) {
            return code;
        }

        info.response_code = response.status_code as i64;
        info.total_time_us = transfer::elapsed_us(started.elapsed());
        crate::easy::perform::record_transfer_info(handle, info.clone());
        return crate::abi::CURLE_OK;
    }
}

fn rtsp_method(metadata: &EasyMetadata) -> String {
    if let Some(custom) = metadata.custom_request.as_deref() {
        if !custom.trim().is_empty() {
            return custom.trim().to_string();
        }
    }
    match metadata.rtsp_request {
        1 => "OPTIONS",
        2 => "DESCRIBE",
        3 => "ANNOUNCE",
        4 => "SETUP",
        5 => "PLAY",
        6 => "PAUSE",
        7 => "TEARDOWN",
        8 => "GET_PARAMETER",
        9 => "SET_PARAMETER",
        10 => "RECORD",
        11 => "RECEIVE",
        _ => "OPTIONS",
    }
    .to_string()
}

fn write_request(
    stream: &mut TransportStream,
    metadata: &EasyMetadata,
    method: &str,
    request_uri: &str,
    cseq: usize,
    authorization: Option<&str>,
) -> Result<(), CURLcode> {
    let mut request = String::new();
    request.push_str(method);
    request.push(' ');
    request.push_str(request_uri);
    request.push_str(" RTSP/1.0\r\n");
    request.push_str("CSeq: ");
    request.push_str(&cseq.to_string());
    request.push_str("\r\n");
    if method.eq_ignore_ascii_case("DESCRIBE") {
        request.push_str("Accept: application/sdp\r\n");
    }
    if let Some(session_id) = metadata.rtsp_session_id.as_deref() {
        request.push_str("Session: ");
        request.push_str(session_id);
        request.push_str("\r\n");
    }
    if let Some(transport) = metadata.rtsp_transport.as_deref() {
        request.push_str("Transport: ");
        request.push_str(transport);
        request.push_str("\r\n");
    }
    if let Some(agent) = metadata.user_agent.as_deref() {
        request.push_str("User-Agent: ");
        request.push_str(agent);
        request.push_str("\r\n");
    }
    if let Some(authorization) = authorization {
        request.push_str(authorization);
        request.push_str("\r\n");
    }
    request.push_str("\r\n");

    stream
        .write_all(request.as_bytes())
        .and_then(|_| stream.flush())
        .map_err(map_write_error)
}

fn read_response(
    stream: &mut TransportStream,
    handle: *mut CURL,
    callbacks: EasyCallbacks,
    metadata: &EasyMetadata,
) -> Result<RtspResponse, CURLcode> {
    let mut bytes = Vec::new();
    let header_end = loop {
        if let Some(header_end) = find_header_end(&bytes) {
            break header_end;
        }
        let mut buf = [0u8; 1024];
        match stream.read(&mut buf) {
            Ok(0) if bytes.is_empty() => return Err(CURLE_RECV_ERROR),
            Ok(0) => return Err(CURLE_RECV_ERROR),
            Ok(read) => bytes.extend_from_slice(&buf[..read]),
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                return Err(CURLE_OPERATION_TIMEDOUT);
            }
            Err(_) => return Err(CURLE_RECV_ERROR),
        }
    };

    let header_block = &bytes[..header_end];
    let body_prefix = bytes[header_end..].to_vec();
    let mut status_code = 0u16;
    let mut content_length = None;
    let mut session_id = None;
    let mut basic_auth_requested = false;

    for (index, raw_line) in split_header_lines(header_block).into_iter().enumerate() {
        transfer::deliver_header(handle, callbacks, metadata, raw_line)?;
        let line = String::from_utf8_lossy(raw_line)
            .trim_end_matches(['\r', '\n'])
            .to_string();
        if index == 0 {
            status_code = parse_status_code(&line).ok_or(CURLE_RECV_ERROR)?;
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            let lower = name.trim().to_ascii_lowercase();
            let value = value.trim();
            match lower.as_str() {
                "content-length" => content_length = value.parse::<usize>().ok(),
                "www-authenticate" => {
                    basic_auth_requested = value.to_ascii_lowercase().contains("basic")
                }
                "session" => {
                    session_id = Some(value.split(';').next().unwrap_or(value).to_string());
                }
                _ => {}
            }
        }
    }

    Ok(RtspResponse {
        status_code,
        content_length,
        session_id,
        basic_auth_requested,
        body_prefix,
    })
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
        .or_else(|| {
            bytes
                .windows(2)
                .position(|window| window == b"\n\n")
                .map(|index| index + 2)
        })
}

fn split_header_lines(bytes: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut start = 0usize;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            lines.push(&bytes[start..=index]);
            start = index + 1;
        }
    }
    if start < bytes.len() {
        lines.push(&bytes[start..]);
    }
    lines
}

fn parse_status_code(line: &str) -> Option<u16> {
    let mut fields = line.split_whitespace();
    let _proto = fields.next()?;
    fields.next()?.parse().ok()
}

fn map_write_error(error: std::io::Error) -> CURLcode {
    match error.kind() {
        ErrorKind::WouldBlock | ErrorKind::TimedOut => CURLE_OPERATION_TIMEDOUT,
        ErrorKind::BrokenPipe => CURLE_SEND_ERROR,
        _ => CURLE_SEND_ERROR,
    }
}

struct RtspResponse {
    status_code: u16,
    content_length: Option<usize>,
    session_id: Option<String>,
    basic_auth_requested: bool,
    body_prefix: Vec<u8>,
}
