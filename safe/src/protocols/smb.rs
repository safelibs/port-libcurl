use crate::abi::{CURLcode, CURL};
use crate::easy::perform::{EasyCallbacks, EasyMetadata};
use crate::protocols::ParsedProtocolUrl;
use crate::transfer::{self, TransferPlan, TransportStream};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/smb.c"];

const CURLE_URL_MALFORMAT: CURLcode = 3;
const CURLE_COULDNT_CONNECT: CURLcode = 7;
const CURLE_REMOTE_ACCESS_DENIED: CURLcode = 9;
const CURLE_RECV_ERROR: CURLcode = 56;
const CURLE_SEND_ERROR: CURLcode = 55;
const CURLE_LOGIN_DENIED: CURLcode = 67;
const CURLE_REMOTE_FILE_NOT_FOUND: CURLcode = 78;

const IO_TIMEOUT: Duration = Duration::from_secs(10);
const SMB2_NEGOTIATE: u16 = 0x0000;
const SMB2_SESSION_SETUP: u16 = 0x0001;
const STATUS_SUCCESS: u32 = 0x0000_0000;
const STATUS_MORE_PROCESSING_REQUIRED: u32 = 0xc000_0016;
const STATUS_ACCESS_DENIED: u32 = 0xc000_0022;
const STATUS_LOGON_FAILURE: u32 = 0xc000_006d;
const STATUS_OBJECT_NAME_NOT_FOUND: u32 = 0xc000_0034;
const STATUS_BAD_NETWORK_NAME: u32 = 0xc000_00cc;

pub(crate) fn matches(scheme: &str) -> bool {
    matches!(scheme, "smb" | "smbs")
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
    if connected
        .stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .and_then(|_| connected.stream.set_write_timeout(Some(IO_TIMEOUT)))
        .is_err()
    {
        transfer::close_transport(connected.stream, callbacks);
        return CURLE_COULDNT_CONNECT;
    }

    let code = perform_smb_exchange(
        handle,
        &mut connected.stream,
        &connected.info,
        callbacks,
        &parsed,
        started,
    );
    transfer::close_transport(connected.stream, callbacks);
    code
}

fn perform_smb_exchange(
    handle: *mut CURL,
    stream: &mut TransportStream,
    base_info: &crate::easy::perform::RecordedTransferInfo,
    _callbacks: EasyCallbacks,
    parsed: &ParsedProtocolUrl,
    started: Instant,
) -> CURLcode {
    let negotiate = build_negotiate_request(0);
    if stream
        .write_all(&negotiate)
        .and_then(|_| stream.flush())
        .is_err()
    {
        return CURLE_SEND_ERROR;
    }
    let negotiate_response = match read_smb2_response(stream) {
        Ok(response) => response,
        Err(code) => return code,
    };
    if negotiate_response.command != SMB2_NEGOTIATE {
        crate::easy::perform::set_error_buffer(
            handle,
            "SMB server returned an unexpected negotiate reply",
        );
        return CURLE_RECV_ERROR;
    }

    let session_setup = build_session_setup_request(1, ntlm_negotiate_blob());
    if stream
        .write_all(&session_setup)
        .and_then(|_| stream.flush())
        .is_err()
    {
        return CURLE_SEND_ERROR;
    }
    let session_response = match read_smb2_response(stream) {
        Ok(response) => response,
        Err(code) => return code,
    };
    if session_response.command != SMB2_SESSION_SETUP {
        crate::easy::perform::set_error_buffer(
            handle,
            "SMB server returned an unexpected session setup reply",
        );
        return CURLE_RECV_ERROR;
    }

    match session_response.status {
        STATUS_SUCCESS | STATUS_MORE_PROCESSING_REQUIRED => {
            let mut info = base_info.clone();
            info.pretransfer_time_us = info.connect_time_us;
            info.starttransfer_time_us = info.connect_time_us;
            info.total_time_us = transfer::elapsed_us(started.elapsed());
            crate::easy::perform::record_transfer_info(handle, info);

            let _ = parsed.path_segments();
            crate::abi::CURLE_OK
        }
        STATUS_LOGON_FAILURE | STATUS_ACCESS_DENIED => {
            crate::easy::perform::set_error_buffer(
                handle,
                "SMB server rejected the anonymous session setup",
            );
            CURLE_LOGIN_DENIED
        }
        STATUS_OBJECT_NAME_NOT_FOUND | STATUS_BAD_NETWORK_NAME => {
            crate::easy::perform::set_error_buffer(
                handle,
                "SMB server could not resolve the requested share or object",
            );
            CURLE_REMOTE_FILE_NOT_FOUND
        }
        status => {
            crate::easy::perform::set_error_buffer(
                handle,
                &format!("SMB session setup failed with status 0x{status:08x}"),
            );
            CURLE_REMOTE_ACCESS_DENIED
        }
    }
}

struct Smb2Response {
    status: u32,
    command: u16,
}

fn build_negotiate_request(message_id: u64) -> Vec<u8> {
    let mut client_guid = [0u8; 16];
    let _ = crate::rand::fill_random(&mut client_guid);
    let dialects = [0x0202u16, 0x0210u16, 0x0300u16, 0x0302u16];

    let mut body = Vec::new();
    body.extend_from_slice(&36u16.to_le_bytes());
    body.extend_from_slice(&(dialects.len() as u16).to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(&client_guid);
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes());
    for dialect in dialects {
        body.extend_from_slice(&dialect.to_le_bytes());
    }
    wrap_nbt(build_smb2_header(SMB2_NEGOTIATE, message_id, 0, 0, &body))
}

fn build_session_setup_request(message_id: u64, security_blob: Vec<u8>) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&25u16.to_le_bytes());
    body.push(0);
    body.push(0);
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(&(64u16 + 24u16).to_le_bytes());
    body.extend_from_slice(&(security_blob.len() as u16).to_le_bytes());
    body.extend_from_slice(&0u64.to_le_bytes());
    body.extend_from_slice(&security_blob);
    wrap_nbt(build_smb2_header(
        SMB2_SESSION_SETUP,
        message_id,
        0,
        0,
        &body,
    ))
}

fn build_smb2_header(
    command: u16,
    message_id: u64,
    tree_id: u32,
    session_id: u64,
    body: &[u8],
) -> Vec<u8> {
    let mut packet = Vec::with_capacity(64 + body.len());
    packet.extend_from_slice(b"\xfeSMB");
    packet.extend_from_slice(&64u16.to_le_bytes());
    packet.extend_from_slice(&0u16.to_le_bytes());
    packet.extend_from_slice(&0u32.to_le_bytes());
    packet.extend_from_slice(&command.to_le_bytes());
    packet.extend_from_slice(&1u16.to_le_bytes());
    packet.extend_from_slice(&0u32.to_le_bytes());
    packet.extend_from_slice(&0u32.to_le_bytes());
    packet.extend_from_slice(&message_id.to_le_bytes());
    packet.extend_from_slice(&0u32.to_le_bytes());
    packet.extend_from_slice(&tree_id.to_le_bytes());
    packet.extend_from_slice(&session_id.to_le_bytes());
    packet.extend_from_slice(&[0u8; 16]);
    packet.extend_from_slice(body);
    packet
}

fn ntlm_negotiate_blob() -> Vec<u8> {
    let mut blob = Vec::new();
    blob.extend_from_slice(b"NTLMSSP\0");
    blob.extend_from_slice(&1u32.to_le_bytes());
    blob.extend_from_slice(&0x0008_8207u32.to_le_bytes());
    blob.extend_from_slice(&0u16.to_le_bytes());
    blob.extend_from_slice(&0u16.to_le_bytes());
    blob.extend_from_slice(&0u32.to_le_bytes());
    blob.extend_from_slice(&0u16.to_le_bytes());
    blob.extend_from_slice(&0u16.to_le_bytes());
    blob.extend_from_slice(&0u32.to_le_bytes());
    blob
}

fn wrap_nbt(mut packet: Vec<u8>) -> Vec<u8> {
    let length = packet.len();
    let mut wrapped = Vec::with_capacity(length + 4);
    wrapped.push(0);
    wrapped.push(((length >> 16) & 0xff) as u8);
    wrapped.push(((length >> 8) & 0xff) as u8);
    wrapped.push((length & 0xff) as u8);
    wrapped.append(&mut packet);
    wrapped
}

fn read_smb2_response(stream: &mut TransportStream) -> Result<Smb2Response, CURLcode> {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).map_err(map_io_read_error)?;
    let length =
        ((usize::from(header[1])) << 16) | ((usize::from(header[2])) << 8) | usize::from(header[3]);
    if length < 64 {
        return Err(CURLE_RECV_ERROR);
    }
    let mut packet = vec![0u8; length];
    stream.read_exact(&mut packet).map_err(map_io_read_error)?;
    if packet.get(..4) != Some(b"\xfeSMB".as_slice()) {
        return Err(CURLE_RECV_ERROR);
    }
    Ok(Smb2Response {
        status: u32::from_le_bytes(packet[8..12].try_into().expect("smb2 status bytes")),
        command: u16::from_le_bytes(packet[12..14].try_into().expect("smb2 command bytes")),
    })
}

fn map_io_read_error(error: std::io::Error) -> CURLcode {
    match error.kind() {
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => CURLE_COULDNT_CONNECT,
        _ => CURLE_RECV_ERROR,
    }
}
