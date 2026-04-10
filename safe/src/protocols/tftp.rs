use crate::abi::{CURLcode, CURL};
use crate::easy::perform::{EasyCallbacks, EasyMetadata, RecordedTransferInfo};
use crate::protocols::ParsedProtocolUrl;
use crate::transfer::{self, LowSpeedGuard, TransferPlan};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/tftp.c"];

const CURLE_URL_MALFORMAT: CURLcode = 3;
const CURLE_COULDNT_CONNECT: CURLcode = 7;
const CURLE_REMOTE_ACCESS_DENIED: CURLcode = 9;
const CURLE_WRITE_ERROR: CURLcode = 23;
const CURLE_READ_ERROR: CURLcode = 26;
const CURLE_OPERATION_TIMEDOUT: CURLcode = 28;
const CURLE_SEND_ERROR: CURLcode = 55;
const CURLE_RECV_ERROR: CURLcode = 56;
const CURLE_REMOTE_FILE_NOT_FOUND: CURLcode = 78;

const TFTP_RRQ: u16 = 1;
const TFTP_WRQ: u16 = 2;
const TFTP_DATA: u16 = 3;
const TFTP_ACK: u16 = 4;
const TFTP_ERROR: u16 = 5;
const TFTP_OACK: u16 = 6;
const TFTP_BLOCK_SIZE: usize = 512;
const IO_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) fn matches(scheme: &str) -> bool {
    scheme == "tftp"
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
    let path = match parsed.decoded_path() {
        Ok(path) => path.trim_start_matches('/').to_string(),
        Err(code) => return code,
    };
    if path.is_empty() {
        crate::easy::perform::set_error_buffer(handle, "TFTP URL must include a file path");
        return CURLE_URL_MALFORMAT;
    }

    let started = Instant::now();
    let mut socket = match bind_socket(&parsed.host, parsed.port) {
        Ok(socket) => socket,
        Err(code) => return code,
    };
    let server = match resolve_peer(&parsed.host, parsed.port) {
        Ok(server) => server,
        Err(code) => return code,
    };
    if socket
        .set_read_timeout(Some(IO_TIMEOUT))
        .and_then(|_| socket.set_write_timeout(Some(IO_TIMEOUT)))
        .is_err()
    {
        return CURLE_COULDNT_CONNECT;
    }

    let mut info = socket_transfer_info(&socket);
    info.pretransfer_time_us = 0;
    info.starttransfer_time_us = 0;

    let code = if metadata.upload {
        match upload_file(
            &mut socket,
            server,
            handle,
            callbacks,
            plan,
            &path,
            metadata.transfer_text,
        ) {
            Ok(()) => crate::abi::CURLE_OK,
            Err(code) => code,
        }
    } else {
        match download_file(
            &mut socket,
            server,
            handle,
            callbacks,
            plan,
            &path,
            metadata.transfer_text,
        ) {
            Ok(()) => crate::abi::CURLE_OK,
            Err(code) => code,
        }
    };

    if code == crate::abi::CURLE_OK {
        info.total_time_us = transfer::elapsed_us(started.elapsed());
        if let Ok(peer) = socket.peer_addr() {
            info.primary_ip = Some(peer.ip().to_string());
            info.primary_port = Some(peer.port());
        }
        if let Ok(local) = socket.local_addr() {
            info.local_ip = Some(local.ip().to_string());
            info.local_port = Some(local.port());
        }
        crate::easy::perform::record_transfer_info(handle, info);
    }
    code
}

fn bind_socket(host: &str, port: u16) -> Result<UdpSocket, CURLcode> {
    let server = resolve_peer(host, port)?;
    let bind_addr = match server {
        SocketAddr::V4(_) => "0.0.0.0:0",
        SocketAddr::V6(_) => "[::]:0",
    };
    UdpSocket::bind(bind_addr).map_err(|_| CURLE_COULDNT_CONNECT)
}

fn resolve_peer(host: &str, port: u16) -> Result<SocketAddr, CURLcode> {
    (host, port)
        .to_socket_addrs()
        .map_err(|_| CURLE_COULDNT_CONNECT)?
        .next()
        .ok_or(CURLE_COULDNT_CONNECT)
}

fn socket_transfer_info(socket: &UdpSocket) -> RecordedTransferInfo {
    let local = socket.local_addr().ok();
    RecordedTransferInfo {
        local_ip: local.as_ref().map(|addr| addr.ip().to_string()),
        local_port: local.as_ref().map(|addr| addr.port()),
        ..RecordedTransferInfo::default()
    }
}

fn download_file(
    socket: &mut UdpSocket,
    server: SocketAddr,
    handle: *mut CURL,
    callbacks: EasyCallbacks,
    plan: &TransferPlan,
    path: &str,
    transfer_text: bool,
) -> Result<(), CURLcode> {
    let request = build_request(TFTP_RRQ, path, transfer_text);
    if socket.send_to(&request, server).is_err() {
        return Err(CURLE_SEND_ERROR);
    }

    let mut low_speed = LowSpeedGuard::new(plan.low_speed);
    let mut delivered = 0usize;
    let mut expected_block = 1u16;
    let mut peer = None;
    let mut packet = [0u8; 4 + TFTP_BLOCK_SIZE + 256];
    transfer::invoke_progress_callback(callbacks, 0, None)?;

    loop {
        let (size, addr) = match socket.recv_from(&mut packet) {
            Ok(result) => result,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                return Err(CURLE_OPERATION_TIMEDOUT);
            }
            Err(_) => return Err(CURLE_RECV_ERROR),
        };
        if size < 4 {
            return Err(CURLE_RECV_ERROR);
        }

        if let Some(active) = peer {
            if addr != active {
                continue;
            }
        } else if socket.connect(addr).is_ok() {
            peer = Some(addr);
        }

        match u16::from_be_bytes([packet[0], packet[1]]) {
            TFTP_DATA => {
                let block = u16::from_be_bytes([packet[2], packet[3]]);
                if block != expected_block {
                    if block == expected_block.wrapping_sub(1) {
                        if send_ack(socket, block).is_err() {
                            return Err(CURLE_SEND_ERROR);
                        }
                        continue;
                    }
                    return Err(CURLE_RECV_ERROR);
                }

                let mut body = packet[4..size].to_vec();
                transfer::deliver_write(handle, callbacks, &mut body)?;
                delivered = delivered.saturating_add(body.len());
                low_speed.observe_progress(body.len())?;
                transfer::invoke_progress_callback(callbacks, delivered, None)?;

                if send_ack(socket, block).is_err() {
                    return Err(CURLE_SEND_ERROR);
                }
                if size < 4 + TFTP_BLOCK_SIZE {
                    return Ok(());
                }
                expected_block = expected_block.wrapping_add(1);
            }
            TFTP_ERROR => return Err(map_error_packet(handle, &packet[..size])),
            TFTP_OACK => {
                if send_ack(socket, 0).is_err() {
                    return Err(CURLE_SEND_ERROR);
                }
            }
            _ => return Err(CURLE_RECV_ERROR),
        }
    }
}

fn upload_file(
    socket: &mut UdpSocket,
    server: SocketAddr,
    handle: *mut CURL,
    callbacks: EasyCallbacks,
    plan: &TransferPlan,
    path: &str,
    transfer_text: bool,
) -> Result<(), CURLcode> {
    let request = build_request(TFTP_WRQ, path, transfer_text);
    if socket.send_to(&request, server).is_err() {
        return Err(CURLE_SEND_ERROR);
    }

    let mut low_speed = LowSpeedGuard::new(plan.low_speed);
    let mut peer = None;
    let mut packet = [0u8; 4 + TFTP_BLOCK_SIZE + 256];
    let mut next_block = 1u16;
    let mut awaiting_ack = 0u16;
    let mut final_block_sent = false;
    transfer::invoke_progress_callback(callbacks, 0, None)?;

    loop {
        let (size, addr) = match socket.recv_from(&mut packet) {
            Ok(result) => result,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                return Err(CURLE_OPERATION_TIMEDOUT);
            }
            Err(_) => return Err(CURLE_RECV_ERROR),
        };
        if size < 4 {
            return Err(CURLE_RECV_ERROR);
        }

        if let Some(active) = peer {
            if addr != active {
                continue;
            }
        } else if socket.connect(addr).is_ok() {
            peer = Some(addr);
        }

        match u16::from_be_bytes([packet[0], packet[1]]) {
            TFTP_ACK => {
                let block = u16::from_be_bytes([packet[2], packet[3]]);
                if block != awaiting_ack {
                    continue;
                }
                if final_block_sent {
                    return Ok(());
                }

                let mut chunk = [0u8; TFTP_BLOCK_SIZE];
                let read = transfer::read_request_body_chunk(handle, callbacks, &mut chunk)?;
                let data = build_data_packet(next_block, &chunk[..read]);
                if socket.send(&data).is_err() {
                    return Err(CURLE_SEND_ERROR);
                }
                low_speed.observe_progress(read)?;
                transfer::invoke_progress_callback(callbacks, read, None)?;
                awaiting_ack = next_block;
                next_block = next_block.wrapping_add(1);
                final_block_sent = read < TFTP_BLOCK_SIZE;
            }
            TFTP_ERROR => return Err(map_error_packet(handle, &packet[..size])),
            TFTP_OACK if awaiting_ack == 0 => {
                let mut chunk = [0u8; TFTP_BLOCK_SIZE];
                let read = transfer::read_request_body_chunk(handle, callbacks, &mut chunk)?;
                let data = build_data_packet(next_block, &chunk[..read]);
                if socket.send(&data).is_err() {
                    return Err(CURLE_SEND_ERROR);
                }
                low_speed.observe_progress(read)?;
                transfer::invoke_progress_callback(callbacks, read, None)?;
                awaiting_ack = next_block;
                next_block = next_block.wrapping_add(1);
                final_block_sent = read < TFTP_BLOCK_SIZE;
            }
            _ => return Err(CURLE_RECV_ERROR),
        }
    }
}

fn build_request(opcode: u16, path: &str, transfer_text: bool) -> Vec<u8> {
    let mut packet = Vec::with_capacity(4 + path.len() + 10);
    packet.extend_from_slice(&opcode.to_be_bytes());
    packet.extend_from_slice(path.as_bytes());
    packet.push(0);
    packet.extend_from_slice(if transfer_text { b"netascii" } else { b"octet" });
    packet.push(0);
    packet
}

fn build_data_packet(block: u16, payload: &[u8]) -> Vec<u8> {
    let mut packet = Vec::with_capacity(payload.len() + 4);
    packet.extend_from_slice(&TFTP_DATA.to_be_bytes());
    packet.extend_from_slice(&block.to_be_bytes());
    packet.extend_from_slice(payload);
    packet
}

fn send_ack(socket: &UdpSocket, block: u16) -> std::io::Result<usize> {
    let mut packet = [0u8; 4];
    packet[..2].copy_from_slice(&TFTP_ACK.to_be_bytes());
    packet[2..].copy_from_slice(&block.to_be_bytes());
    socket.send(&packet)
}

fn map_error_packet(handle: *mut CURL, packet: &[u8]) -> CURLcode {
    if packet.len() < 4 {
        return CURLE_RECV_ERROR;
    }
    let code = u16::from_be_bytes([packet[2], packet[3]]);
    if packet.len() > 5 {
        let text = String::from_utf8_lossy(&packet[4..packet.len() - 1]);
        if !text.is_empty() {
            crate::easy::perform::set_error_buffer(handle, &text);
        }
    }
    match code {
        1 => CURLE_REMOTE_FILE_NOT_FOUND,
        2 | 6 => CURLE_REMOTE_ACCESS_DENIED,
        3 => CURLE_WRITE_ERROR,
        4 => CURLE_READ_ERROR,
        _ => CURLE_RECV_ERROR,
    }
}
