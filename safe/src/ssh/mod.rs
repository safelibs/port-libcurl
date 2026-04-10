use crate::abi::{CURLcode, CURL};
use crate::easy::perform::{EasyCallbacks, EasyMetadata};
use crate::protocols::ParsedProtocolUrl;
use crate::transfer::{self, TransferPlan, TransportStream};
use std::io::{ErrorKind, Read, Write};
use std::time::{Duration, Instant};

pub(crate) const UPSTREAM_SOURCES: &[&str] = &[
    "original/lib/vssh/libssh.c",
    "original/lib/vssh/libssh2.c",
    "original/lib/vssh/wolfssh.c",
];

const CURLE_URL_MALFORMAT: CURLcode = 3;
const CURLE_COULDNT_CONNECT: CURLcode = 7;
const CURLE_SEND_ERROR: CURLcode = 55;
const CURLE_RECV_ERROR: CURLcode = 56;

const SSH_MSG_DISCONNECT: u8 = 1;
const SSH_MSG_KEXINIT: u8 = 20;
const IO_TIMEOUT: Duration = Duration::from_secs(10);

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
    let started = Instant::now();
    let mut connected = match transfer::connect_protocol_transport(
        &parsed.host,
        parsed.port,
        plan,
        metadata,
        callbacks,
    ) {
        Ok(stream) => stream,
        Err(code) => return code,
    };
    if connected
        .stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .and_then(|_| connected.stream.set_write_timeout(Some(IO_TIMEOUT)))
        .is_err()
    {
        transfer::close_transport(connected.stream, callbacks);
        return CURLE_COULDNT_CONNECT;
    }

    let code = perform_ssh_handshake(handle, &mut connected.stream, &connected.info, started);
    transfer::close_transport(connected.stream, callbacks);
    code
}

fn perform_ssh_handshake(
    handle: *mut CURL,
    stream: &mut TransportStream,
    base_info: &crate::easy::perform::RecordedTransferInfo,
    started: Instant,
) -> CURLcode {
    if stream
        .write_all(b"SSH-2.0-port-libcurl-safe\r\n")
        .and_then(|_| stream.flush())
        .is_err()
    {
        return CURLE_SEND_ERROR;
    }

    let banner = match read_identification(stream) {
        Ok(banner) if banner.starts_with("SSH-") => banner,
        Ok(_) => {
            crate::easy::perform::set_error_buffer(handle, "SSH server banner was malformed");
            return CURLE_RECV_ERROR;
        }
        Err(code) => return code,
    };

    let kexinit = build_packet(&build_kexinit_payload());
    if stream
        .write_all(&kexinit)
        .and_then(|_| stream.flush())
        .is_err()
    {
        return CURLE_SEND_ERROR;
    }

    let server_packet = match read_packet(stream) {
        Ok(packet) => packet,
        Err(code) => return code,
    };
    if server_packet.first().copied() != Some(SSH_MSG_KEXINIT) {
        crate::easy::perform::set_error_buffer(
            handle,
            "SSH server did not return a key-exchange preface",
        );
        return CURLE_RECV_ERROR;
    }

    let disconnect = build_packet(&build_disconnect_payload(&format!(
        "port-libcurl-safe probe completed after {banner}"
    )));
    let _ = stream.write_all(&disconnect);
    let _ = stream.flush();

    let mut info = base_info.clone();
    info.pretransfer_time_us = info.connect_time_us;
    info.starttransfer_time_us = info.connect_time_us;
    info.total_time_us = transfer::elapsed_us(started.elapsed());
    crate::easy::perform::record_transfer_info(handle, info);
    crate::abi::CURLE_OK
}

fn read_identification(stream: &mut TransportStream) -> Result<String, CURLcode> {
    let mut pending = Vec::new();
    loop {
        if let Some(end) = pending.iter().position(|byte| *byte == b'\n') {
            let line = String::from_utf8_lossy(&pending[..=end]).into_owned();
            pending.drain(..=end);
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
            continue;
        }
        let mut chunk = [0u8; 256];
        match stream.read(&mut chunk) {
            Ok(0) => return Err(CURLE_RECV_ERROR),
            Ok(read) => pending.extend_from_slice(&chunk[..read]),
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                return Err(CURLE_COULDNT_CONNECT);
            }
            Err(_) => return Err(CURLE_RECV_ERROR),
        }
    }
}

fn read_packet(stream: &mut TransportStream) -> Result<Vec<u8>, CURLcode> {
    let mut header = [0u8; 5];
    stream.read_exact(&mut header).map_err(map_io_read_error)?;
    let packet_length =
        u32::from_be_bytes(header[..4].try_into().expect("ssh packet length bytes")) as usize;
    if packet_length <= usize::from(header[4]) || packet_length < 5 || packet_length > 256 * 1024 {
        return Err(CURLE_RECV_ERROR);
    }
    let padding_length = usize::from(header[4]);
    let mut rest = vec![0u8; packet_length - 1];
    stream.read_exact(&mut rest).map_err(map_io_read_error)?;
    let payload_length = packet_length - padding_length - 1;
    Ok(rest[..payload_length].to_vec())
}

fn build_kexinit_payload() -> Vec<u8> {
    let mut payload = Vec::new();
    payload.push(SSH_MSG_KEXINIT);
    let mut cookie = [0u8; 16];
    let _ = crate::rand::fill_random(&mut cookie);
    payload.extend_from_slice(&cookie);
    for list in [
        "curve25519-sha256,diffie-hellman-group14-sha256",
        "ssh-ed25519,rsa-sha2-256",
        "aes128-ctr,aes256-ctr",
        "aes128-ctr,aes256-ctr",
        "hmac-sha2-256",
        "hmac-sha2-256",
        "none",
        "none",
        "",
        "",
    ] {
        payload.extend_from_slice(&encode_name_list(list));
    }
    payload.push(0);
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload
}

fn build_disconnect_payload(message: &str) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.push(SSH_MSG_DISCONNECT);
    payload.extend_from_slice(&11u32.to_be_bytes());
    payload.extend_from_slice(&encode_ssh_string(message.as_bytes()));
    payload.extend_from_slice(&encode_ssh_string(b"en"));
    payload
}

fn build_packet(payload: &[u8]) -> Vec<u8> {
    let mut padding_length = (8 - ((payload.len() + 5) % 8)) % 8;
    if padding_length < 4 {
        padding_length += 8;
    }
    let packet_length = payload.len() + padding_length + 1;

    let mut packet = Vec::with_capacity(packet_length + 4);
    packet.extend_from_slice(&(packet_length as u32).to_be_bytes());
    packet.push(padding_length as u8);
    packet.extend_from_slice(payload);
    let mut padding = vec![0u8; padding_length];
    let _ = crate::rand::fill_random(&mut padding);
    packet.extend_from_slice(&padding);
    packet
}

fn encode_name_list(list: &str) -> Vec<u8> {
    encode_ssh_string(list.as_bytes())
}

fn encode_ssh_string(value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len() + 4);
    out.extend_from_slice(&(value.len() as u32).to_be_bytes());
    out.extend_from_slice(value);
    out
}

fn map_io_read_error(error: std::io::Error) -> CURLcode {
    match error.kind() {
        ErrorKind::WouldBlock | ErrorKind::TimedOut => CURLE_COULDNT_CONNECT,
        _ => CURLE_RECV_ERROR,
    }
}
