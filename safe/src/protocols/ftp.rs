use crate::abi::{CURLcode, CURL};
use crate::easy::perform::{EasyCallbacks, EasyMetadata, RecordedTransferInfo};
use crate::http::auth;
use crate::protocols::ParsedProtocolUrl;
use crate::transfer::{self, LowSpeedGuard, TransferPlan, TransportStream};
use std::io::{ErrorKind, Read, Write};
use std::time::{Duration, Instant};

pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/ftp.c"];

const CURLE_URL_MALFORMAT: CURLcode = 3;
const CURLE_COULDNT_CONNECT: CURLcode = 7;
const CURLE_REMOTE_ACCESS_DENIED: CURLcode = 9;
const CURLE_WRITE_ERROR: CURLcode = 23;
const CURLE_OPERATION_TIMEDOUT: CURLcode = 28;
const CURLE_READ_ERROR: CURLcode = 26;
const CURLE_SEND_ERROR: CURLcode = 55;
const CURLE_RECV_ERROR: CURLcode = 56;
const CURLE_LOGIN_DENIED: CURLcode = 67;
const CURLE_REMOTE_FILE_NOT_FOUND: CURLcode = 78;

const CONTROL_TIMEOUT: Duration = Duration::from_secs(30);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) fn matches(scheme: &str) -> bool {
    matches!(scheme, "ftp" | "ftps")
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
            crate::easy::perform::set_error_buffer(handle, "Malformed FTP URL");
            return code;
        }
    };
    let object = match ftp_object_path(&parsed) {
        Ok(object) => object,
        Err(code) => {
            crate::easy::perform::set_error_buffer(handle, "FTP path is missing a file name");
            return code;
        }
    };
    let started = Instant::now();
    let mut pending = Vec::new();
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
    let mut info = control.info.clone();
    if control
        .stream
        .set_read_timeout(Some(CONTROL_TIMEOUT))
        .and_then(|_| control.stream.set_write_timeout(Some(WRITE_TIMEOUT)))
        .is_err()
    {
        transfer::close_transport(control.stream, callbacks);
        return CURLE_COULDNT_CONNECT;
    }

    let result = perform_transfer_inner(
        handle,
        plan,
        metadata,
        callbacks,
        &parsed,
        &object,
        &mut control.stream,
        &mut pending,
        &mut info,
        started,
    );
    transfer::close_transport(control.stream, callbacks);
    result
}

fn perform_transfer_inner(
    handle: *mut CURL,
    plan: &TransferPlan,
    metadata: &EasyMetadata,
    callbacks: EasyCallbacks,
    parsed: &ParsedProtocolUrl,
    object: &str,
    control: &mut TransportStream,
    pending: &mut Vec<u8>,
    info: &mut RecordedTransferInfo,
    started: Instant,
) -> CURLcode {
    let welcome = match read_response(control, pending) {
        Ok(response) => response,
        Err(code) => return code,
    };
    if !welcome.is_positive() {
        crate::easy::perform::set_error_buffer(
            handle,
            "FTP server rejected the control connection",
        );
        return CURLE_COULDNT_CONNECT;
    }

    let (username, password) = ftp_credentials(parsed, metadata);
    let user = match send_command(control, pending, &format!("USER {username}\r\n")) {
        Ok(response) => response,
        Err(code) => return code,
    };
    match user.code {
        230 => {}
        331 => {
            let pass = match send_command(control, pending, &format!("PASS {password}\r\n")) {
                Ok(response) => response,
                Err(code) => return code,
            };
            if !pass.is_positive() {
                crate::easy::perform::set_error_buffer(handle, "FTP login failed");
                return CURLE_LOGIN_DENIED;
            }
        }
        530 => {
            crate::easy::perform::set_error_buffer(handle, "FTP login denied");
            return CURLE_LOGIN_DENIED;
        }
        _ => {
            crate::easy::perform::set_error_buffer(handle, "FTP login failed");
            return CURLE_LOGIN_DENIED;
        }
    }

    let pwd = match send_command(control, pending, "PWD\r\n") {
        Ok(response) => response,
        Err(code) => return code,
    };
    if !pwd.is_positive() {
        crate::easy::perform::set_error_buffer(handle, "FTP PWD command failed");
        return CURLE_REMOTE_ACCESS_DENIED;
    }

    let passive = match send_command(control, pending, "EPSV\r\n") {
        Ok(response) if response.is_positive() => match parse_epsv_port(&response) {
            Some(port) => PassiveAddress {
                host: parsed.host.clone(),
                port,
            },
            None => {
                crate::easy::perform::set_error_buffer(handle, "FTP EPSV response was malformed");
                return CURLE_COULDNT_CONNECT;
            }
        },
        Ok(_) => {
            let response = match send_command(control, pending, "PASV\r\n") {
                Ok(response) => response,
                Err(code) => return code,
            };
            match parse_pasv_address(&response) {
                Some(address) => address,
                None => {
                    crate::easy::perform::set_error_buffer(
                        handle,
                        "FTP PASV response was malformed",
                    );
                    return CURLE_COULDNT_CONNECT;
                }
            }
        }
        Err(code) => return code,
    };

    let mut data = match transfer::connect_protocol_transport(
        handle,
        &passive.host,
        passive.port,
        plan,
        metadata,
        callbacks,
    ) {
        Ok(stream) => stream,
        Err(code) => return code,
    };
    if data
        .stream
        .set_read_timeout(Some(CONTROL_TIMEOUT))
        .and_then(|_| data.stream.set_write_timeout(Some(WRITE_TIMEOUT)))
        .is_err()
    {
        transfer::close_transport(data.stream, callbacks);
        return CURLE_COULDNT_CONNECT;
    }

    let type_cmd = if metadata.transfer_text {
        "TYPE A\r\n"
    } else {
        "TYPE I\r\n"
    };
    let set_type = match send_command(control, pending, type_cmd) {
        Ok(response) => response,
        Err(code) => return code,
    };
    if !set_type.is_positive() {
        crate::easy::perform::set_error_buffer(handle, "FTP TYPE command failed");
        return CURLE_REMOTE_ACCESS_DENIED;
    }

    let content_length = if metadata.upload || metadata.transfer_text || metadata.nobody {
        None
    } else {
        match send_command(control, pending, &format!("SIZE {object}\r\n")) {
            Ok(response) if response.code == 213 => response
                .text()
                .split_whitespace()
                .nth(1)
                .and_then(|value| value.parse::<usize>().ok()),
            Ok(response) if response.code == 550 => {
                crate::easy::perform::set_error_buffer(handle, "FTP file does not exist");
                return CURLE_REMOTE_FILE_NOT_FOUND;
            }
            Ok(_) => None,
            Err(code) => return code,
        }
    };

    info.pretransfer_time_us = info.connect_time_us;

    if metadata.nobody {
        info.starttransfer_time_us = info.connect_time_us;
        info.total_time_us = transfer::elapsed_us(started.elapsed());
        crate::easy::perform::record_transfer_info(handle, info.clone());
        transfer::close_transport(data.stream, callbacks);
        let _ = send_command(control, pending, "QUIT\r\n");
        return crate::abi::CURLE_OK;
    }

    let preliminary = if metadata.upload {
        match send_command(control, pending, &format!("STOR {object}\r\n")) {
            Ok(response) => response,
            Err(code) => return code,
        }
    } else {
        match send_command(control, pending, &format!("RETR {object}\r\n")) {
            Ok(response) => response,
            Err(code) => return code,
        }
    };
    if preliminary.code == 550 {
        crate::easy::perform::set_error_buffer(
            handle,
            if metadata.upload {
                "FTP upload target is not accessible"
            } else {
                "FTP file does not exist"
            },
        );
        return if metadata.upload {
            CURLE_REMOTE_ACCESS_DENIED
        } else {
            CURLE_REMOTE_FILE_NOT_FOUND
        };
    }
    if !preliminary.is_preliminary() {
        crate::easy::perform::set_error_buffer(
            handle,
            if metadata.upload {
                "FTP STOR command failed"
            } else {
                "FTP RETR command failed"
            },
        );
        return CURLE_REMOTE_ACCESS_DENIED;
    }

    info.starttransfer_time_us = transfer::elapsed_us(started.elapsed());
    let mut low_speed = LowSpeedGuard::new(plan.low_speed);
    if let Err(code) = transfer::invoke_progress_callback(callbacks, 0, content_length) {
        transfer::close_transport(data.stream, callbacks);
        return code;
    }
    let body_result = if metadata.upload {
        transfer_upload_body(
            &mut data.stream,
            handle,
            callbacks,
            metadata.upload_size.and_then(|size| usize::try_from(size).ok()),
            &mut low_speed,
        )
    } else {
        transfer::transfer_body(
            &mut data.stream,
            handle,
            callbacks,
            Vec::new(),
            content_length,
            &mut low_speed,
        )
    };
    transfer::close_transport(data.stream, callbacks);
    if let Err(code) = body_result {
        return code;
    }

    let _ = control.write_all(b"QUIT\r\n");
    let _ = control.flush();

    let completion = match read_response(control, pending) {
        Ok(response) => response,
        Err(code) => return code,
    };
    if !completion.is_positive() && completion.code != 221 {
        crate::easy::perform::set_error_buffer(handle, "FTP transfer did not complete cleanly");
        return CURLE_RECV_ERROR;
    }
    info.response_code = completion.code as i64;
    info.total_time_us = transfer::elapsed_us(started.elapsed());
    crate::easy::perform::record_transfer_info(handle, info.clone());
    crate::abi::CURLE_OK
}

fn transfer_upload_body(
    stream: &mut TransportStream,
    handle: *mut CURL,
    callbacks: EasyCallbacks,
    content_length: Option<usize>,
    low_speed: &mut LowSpeedGuard,
) -> Result<(), CURLcode> {
    let mut sent = 0usize;
    let mut buffer = vec![0u8; 16 * 1024];
    loop {
        let read = transfer::read_request_body_chunk(handle, callbacks, &mut buffer)?;
        if read == 0 {
            stream.flush().map_err(map_io_write_error)?;
            break;
        }
        stream
            .write_all(&buffer[..read])
            .and_then(|_| stream.flush())
            .map_err(map_io_write_error)?;
        sent = sent.saturating_add(read);
        low_speed.observe_progress(read)?;
        transfer::invoke_progress_callback(callbacks, sent, content_length)?;
    }
    Ok(())
}

fn ftp_object_path(parsed: &ParsedProtocolUrl) -> Result<String, CURLcode> {
    let path = parsed.decoded_path()?;
    let object = path.trim_start_matches('/');
    if object.is_empty() {
        Err(CURLE_URL_MALFORMAT)
    } else {
        Ok(object.to_string())
    }
}

fn ftp_credentials(parsed: &ParsedProtocolUrl, metadata: &EasyMetadata) -> (String, String) {
    if let Some(explicit) = auth::explicit_basic_credentials(metadata) {
        return (explicit.username, explicit.password);
    }
    (
        parsed
            .username
            .clone()
            .unwrap_or_else(|| "anonymous".to_string()),
        parsed
            .password
            .clone()
            .unwrap_or_else(|| "ftp@example.com".to_string()),
    )
}

fn send_command(
    stream: &mut TransportStream,
    pending: &mut Vec<u8>,
    command: &str,
) -> Result<FtpResponse, CURLcode> {
    stream
        .write_all(command.as_bytes())
        .map_err(map_io_write_error)?;
    stream.flush().map_err(map_io_write_error)?;
    read_response(stream, pending)
}

fn read_response(
    stream: &mut TransportStream,
    pending: &mut Vec<u8>,
) -> Result<FtpResponse, CURLcode> {
    let first = read_line(stream, pending)?;
    let code = parse_code(&first)?;
    let mut lines = vec![first];
    let multiline = lines[0].as_bytes().get(3) == Some(&b'-');
    if !multiline {
        return Ok(FtpResponse { code, lines });
    }

    loop {
        let line = read_line(stream, pending)?;
        let done = line.as_bytes().len() >= 4
            && parse_code(&line).ok() == Some(code)
            && line.as_bytes().get(3) == Some(&b' ');
        lines.push(line);
        if done {
            return Ok(FtpResponse { code, lines });
        }
    }
}

fn read_line(stream: &mut TransportStream, pending: &mut Vec<u8>) -> Result<String, CURLcode> {
    loop {
        if let Some(index) = pending.iter().position(|byte| *byte == b'\n') {
            let line = pending.drain(..=index).collect::<Vec<_>>();
            return Ok(String::from_utf8_lossy(&line)
                .trim_end_matches(['\r', '\n'])
                .to_string());
        }

        let mut buf = [0u8; 512];
        match stream.read(&mut buf) {
            Ok(0) => {
                if pending.is_empty() {
                    return Err(CURLE_RECV_ERROR);
                }
                let line = std::mem::take(pending);
                return Ok(String::from_utf8_lossy(&line)
                    .trim_end_matches('\r')
                    .to_string());
            }
            Ok(read) => pending.extend_from_slice(&buf[..read]),
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                return Err(CURLE_OPERATION_TIMEDOUT);
            }
            Err(_) => return Err(CURLE_RECV_ERROR),
        }
    }
}

fn parse_code(line: &str) -> Result<u16, CURLcode> {
    line.get(..3)
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or(CURLE_RECV_ERROR)
}

fn parse_epsv_port(response: &FtpResponse) -> Option<u16> {
    let text = response.text();
    let start = text.find("|||")?;
    let rest = &text[start + 3..];
    let end = rest.find('|')?;
    rest[..end].parse().ok()
}

fn parse_pasv_address(response: &FtpResponse) -> Option<PassiveAddress> {
    let text = response.text();
    let start = text.find('(')?;
    let end = text[start + 1..].find(')')? + start + 1;
    let parts = text[start + 1..end]
        .split(',')
        .map(str::trim)
        .collect::<Vec<_>>();
    if parts.len() != 6 {
        return None;
    }
    let host = format!("{}.{}.{}.{}", parts[0], parts[1], parts[2], parts[3]);
    let hi = parts[4].parse::<u16>().ok()?;
    let lo = parts[5].parse::<u16>().ok()?;
    Some(PassiveAddress {
        host,
        port: hi * 256 + lo,
    })
}

fn map_io_write_error(error: std::io::Error) -> CURLcode {
    match error.kind() {
        ErrorKind::WouldBlock | ErrorKind::TimedOut => CURLE_OPERATION_TIMEDOUT,
        ErrorKind::BrokenPipe => CURLE_SEND_ERROR,
        _ => CURLE_WRITE_ERROR,
    }
}

struct PassiveAddress {
    host: String,
    port: u16,
}

struct FtpResponse {
    code: u16,
    lines: Vec<String>,
}

impl FtpResponse {
    fn is_positive(&self) -> bool {
        (200..300).contains(&self.code)
    }

    fn is_preliminary(&self) -> bool {
        (100..200).contains(&self.code)
    }

    fn text(&self) -> &str {
        self.lines.last().map(String::as_str).unwrap_or_default()
    }
}
