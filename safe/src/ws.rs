use crate::abi::{curl_off_t, curl_ws_frame, CURLcode, CURL, CURLE_BAD_FUNCTION_ARGUMENT};
use crate::http::auth::base64_encode;
use crate::rand;
use crate::transfer;
use core::ffi::c_void;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;

const CURLE_OK: CURLcode = 0;
const CURLE_UNSUPPORTED_PROTOCOL: CURLcode = 1;
const CURLE_RECV_ERROR: CURLcode = 56;
const CURLE_SEND_ERROR: CURLcode = 55;
const CURLE_AGAIN: CURLcode = 81;

const CURLWS_TEXT: u32 = 1 << 0;
const CURLWS_BINARY: u32 = 1 << 1;
const CURLWS_CONT: u32 = 1 << 2;
const CURLWS_CLOSE: u32 = 1 << 3;
const CURLWS_PING: u32 = 1 << 4;
const CURLWS_OFFSET: u32 = 1 << 5;
const CURLWS_PONG: u32 = 1 << 6;
const CURLWS_RAW_MODE: i64 = 1 << 0;
const MAX_FRAME_PAYLOAD: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug)]
struct PendingFrame {
    flags: u32,
    payload: Vec<u8>,
    offset: usize,
}

pub(crate) struct WebSocketSession {
    raw_mode: bool,
    recv_buffer: Vec<u8>,
    pending: Option<PendingFrame>,
    frame: curl_ws_frame,
    send_fragment_remaining: Option<u64>,
}

impl WebSocketSession {
    pub(crate) fn handshake(
        stream: &mut TcpStream,
        host_header: &str,
        target: &str,
        extra_headers: &[String],
        raw_mode: bool,
    ) -> Result<Self, CURLcode> {
        let mut nonce = [0u8; 16];
        rand::fill_random(&mut nonce).map_err(|_| CURLE_SEND_ERROR)?;
        let key = base64_encode(&nonce);
        let mut request = String::new();
        request.push_str("GET ");
        request.push_str(target);
        request.push_str(" HTTP/1.1\r\n");
        request.push_str("Host: ");
        request.push_str(host_header);
        request.push_str("\r\n");
        request.push_str("Connection: Upgrade\r\n");
        request.push_str("Upgrade: websocket\r\n");
        request.push_str("Sec-WebSocket-Version: 13\r\n");
        request.push_str("Sec-WebSocket-Key: ");
        request.push_str(&key);
        request.push_str("\r\n");
        for header in extra_headers {
            request.push_str(header);
            request.push_str("\r\n");
        }
        request.push_str("\r\n");
        stream
            .write_all(request.as_bytes())
            .map_err(|_| CURLE_SEND_ERROR)?;
        stream.flush().map_err(|_| CURLE_SEND_ERROR)?;

        let mut response = Vec::new();
        loop {
            if let Some(header_end) = find_header_end(&response) {
                let header_text = String::from_utf8_lossy(&response[..header_end]);
                let mut lines = header_text.lines();
                let status = lines.next().unwrap_or_default();
                if !status.contains(" 101 ") {
                    return Err(CURLE_UNSUPPORTED_PROTOCOL);
                }
                let body_prefix = response[header_end..].to_vec();
                return Ok(Self {
                    raw_mode,
                    recv_buffer: body_prefix,
                    pending: None,
                    frame: curl_ws_frame {
                        age: 0,
                        flags: 0,
                        offset: 0,
                        bytesleft: 0,
                        len: 0,
                    },
                    send_fragment_remaining: None,
                });
            }

            let mut buf = [0u8; 1024];
            match stream.read(&mut buf) {
                Ok(0) => return Err(CURLE_RECV_ERROR),
                Ok(read) => response.extend_from_slice(&buf[..read]),
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                {
                    return Err(CURLE_AGAIN)
                }
                Err(_) => return Err(CURLE_RECV_ERROR),
            }
        }
    }

    fn meta(&self) -> *const curl_ws_frame {
        &self.frame
    }

    fn recv(&mut self, stream: &mut TcpStream, buffer: &mut [u8]) -> Result<usize, CURLcode> {
        if self.raw_mode {
            return match stream.read(buffer) {
                Ok(read) => {
                    self.frame = curl_ws_frame {
                        age: 0,
                        flags: 0,
                        offset: 0,
                        bytesleft: 0,
                        len: read,
                    };
                    Ok(read)
                }
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                {
                    Err(CURLE_AGAIN)
                }
                Err(_) => Err(CURLE_RECV_ERROR),
            };
        }

        loop {
            if let Some(frame) = self.pending.as_mut() {
                let remaining = frame.payload.len().saturating_sub(frame.offset);
                let take = remaining.min(buffer.len());
                buffer[..take].copy_from_slice(&frame.payload[frame.offset..frame.offset + take]);
                self.frame = curl_ws_frame {
                    age: 0,
                    flags: frame.flags as i32,
                    offset: frame.offset as curl_off_t,
                    bytesleft: (remaining.saturating_sub(take)) as curl_off_t,
                    len: take,
                };
                frame.offset += take;
                if frame.offset >= frame.payload.len() {
                    self.pending = None;
                }
                return Ok(take);
            }

            if let Some(frame) = self.try_parse_frame()? {
                if (frame.flags & CURLWS_PING) != 0 {
                    let _ = self.send(stream, &frame.payload, 0, CURLWS_PONG);
                    continue;
                }
                self.pending = Some(frame);
                continue;
            }

            let mut scratch = [0u8; 4096];
            match stream.read(&mut scratch) {
                Ok(0) => return Err(CURLE_RECV_ERROR),
                Ok(read) => self.recv_buffer.extend_from_slice(&scratch[..read]),
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                {
                    return Err(CURLE_AGAIN)
                }
                Err(_) => return Err(CURLE_RECV_ERROR),
            }
        }
    }

    fn try_parse_frame(&mut self) -> Result<Option<PendingFrame>, CURLcode> {
        if self.recv_buffer.len() < 2 {
            return Ok(None);
        }

        let first = self.recv_buffer[0];
        let second = self.recv_buffer[1];
        let masked = (second & 0x80) != 0;
        let mut payload_len = (second & 0x7f) as usize;
        let mut header_len = 2usize;
        if payload_len == 126 {
            if self.recv_buffer.len() < 4 {
                return Ok(None);
            }
            payload_len = u16::from_be_bytes([self.recv_buffer[2], self.recv_buffer[3]]) as usize;
            header_len = 4;
        } else if payload_len == 127 {
            if self.recv_buffer.len() < 10 {
                return Ok(None);
            }
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&self.recv_buffer[2..10]);
            payload_len = u64::from_be_bytes(bytes)
                .try_into()
                .map_err(|_| CURLE_RECV_ERROR)?;
            header_len = 10;
        }
        if payload_len > MAX_FRAME_PAYLOAD {
            return Err(CURLE_RECV_ERROR);
        }

        let mut mask = [0u8; 4];
        if masked {
            if self.recv_buffer.len() < header_len + 4 {
                return Ok(None);
            }
            mask.copy_from_slice(&self.recv_buffer[header_len..header_len + 4]);
            header_len += 4;
        }
        if self.recv_buffer.len() < header_len + payload_len {
            return Ok(None);
        }

        let mut payload = self.recv_buffer[header_len..header_len + payload_len].to_vec();
        if masked {
            for (index, byte) in payload.iter_mut().enumerate() {
                *byte ^= mask[index % 4];
            }
        }
        self.recv_buffer.drain(..header_len + payload_len);
        Ok(Some(PendingFrame {
            flags: flags_from_opcode(first & 0x0f, (first & 0x80) == 0),
            payload,
            offset: 0,
        }))
    }

    fn send(
        &mut self,
        stream: &mut TcpStream,
        buffer: &[u8],
        fragsize: curl_off_t,
        flags: u32,
    ) -> Result<usize, CURLcode> {
        if self.raw_mode {
            if fragsize != 0 || flags != 0 {
                return Err(CURLE_BAD_FUNCTION_ARGUMENT);
            }
            return stream.write(buffer).map_err(|_| CURLE_SEND_ERROR);
        }

        let opcode = opcode_from_flags(flags, self.send_fragment_remaining.is_some())?;
        let mut remaining = self
            .send_fragment_remaining
            .unwrap_or_else(|| fragsize.max(0) as u64);
        if remaining == 0 {
            remaining = buffer.len() as u64;
        }
        let finish = (buffer.len() as u64) >= remaining && (flags & CURLWS_OFFSET) == 0
            || (buffer.len() as u64) >= remaining;
        let frame = encode_frame(buffer, opcode, finish)?;
        stream.write_all(&frame).map_err(|_| CURLE_SEND_ERROR)?;
        stream.flush().map_err(|_| CURLE_SEND_ERROR)?;
        if finish {
            self.send_fragment_remaining = None;
        } else {
            self.send_fragment_remaining = Some(remaining.saturating_sub(buffer.len() as u64));
        }
        Ok(buffer.len())
    }
}

fn flags_from_opcode(opcode: u8, continuation: bool) -> u32 {
    let mut flags = match opcode {
        0x0 => CURLWS_CONT,
        0x1 => CURLWS_TEXT,
        0x2 => CURLWS_BINARY,
        0x8 => CURLWS_CLOSE,
        0x9 => CURLWS_PING,
        0xA => CURLWS_PONG,
        _ => 0,
    };
    if continuation {
        flags |= CURLWS_CONT;
    }
    flags
}

fn opcode_from_flags(flags: u32, continuing: bool) -> Result<u8, CURLcode> {
    if continuing {
        return Ok(0x0);
    }
    if (flags & CURLWS_CLOSE) != 0 {
        return Ok(0x8);
    }
    if (flags & CURLWS_PING) != 0 {
        return Ok(0x9);
    }
    if (flags & CURLWS_PONG) != 0 {
        return Ok(0xA);
    }
    if (flags & CURLWS_TEXT) != 0 {
        return Ok(0x1);
    }
    Ok(0x2)
}

fn encode_frame(payload: &[u8], opcode: u8, finish: bool) -> Result<Vec<u8>, CURLcode> {
    let mut frame = Vec::with_capacity(payload.len() + 14);
    frame.push((if finish { 0x80 } else { 0 }) | opcode);
    if payload.len() < 126 {
        frame.push(0x80 | payload.len() as u8);
    } else if payload.len() <= u16::MAX as usize {
        frame.push(0x80 | 126);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    } else {
        frame.push(0x80 | 127);
        frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    }
    let mut mask = [0u8; 4];
    rand::fill_random(&mut mask).map_err(|_| CURLE_SEND_ERROR)?;
    frame.extend_from_slice(&mask);
    for (index, byte) in payload.iter().enumerate() {
        frame.push(byte ^ mask[index % 4]);
    }
    Ok(frame)
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_ws_meta(curl: *mut CURL) -> *const curl_ws_frame {
    if curl.is_null() {
        return core::ptr::null();
    }
    transfer::with_connect_only_session_mut(curl, |session| {
        session
            .websocket
            .as_ref()
            .map(WebSocketSession::meta)
            .unwrap_or(core::ptr::null())
    })
    .unwrap_or(core::ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_ws_recv(
    curl: *mut CURL,
    buffer: *mut c_void,
    buflen: usize,
    recv: *mut usize,
    metap: *mut *const curl_ws_frame,
) -> CURLcode {
    if curl.is_null() || buffer.is_null() || recv.is_null() || metap.is_null() {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    }
    unsafe {
        *recv = 0;
        *metap = core::ptr::null();
    }
    let Some(result) = transfer::with_connect_only_session_mut(
        curl,
        |session| -> Result<(usize, *const curl_ws_frame), CURLcode> {
            let Some(websocket) = session.websocket.as_mut() else {
                return Err(CURLE_BAD_FUNCTION_ARGUMENT);
            };
            let read = websocket.recv(&mut session.stream, unsafe {
                std::slice::from_raw_parts_mut(buffer.cast::<u8>(), buflen)
            })?;
            Ok((read, websocket.meta()))
        },
    ) else {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    };
    let (read, meta) = match result {
        Ok(values) => values,
        Err(code) => return code,
    };
    unsafe {
        *recv = read;
        *metap = meta;
    }
    CURLE_OK
}

#[no_mangle]
pub unsafe extern "C" fn port_safe_export_curl_ws_send(
    curl: *mut CURL,
    buffer: *const c_void,
    buflen: usize,
    sent: *mut usize,
    fragsize: curl_off_t,
    flags: u32,
) -> CURLcode {
    if curl.is_null() || sent.is_null() || (buflen != 0 && buffer.is_null()) {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    }
    unsafe { *sent = 0 };
    let payload = if buflen == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(buffer.cast::<u8>(), buflen) }
    };
    let Some(result) =
        transfer::with_connect_only_session_mut(curl, |session| -> Result<usize, CURLcode> {
            let Some(websocket) = session.websocket.as_mut() else {
                return Err(CURLE_BAD_FUNCTION_ARGUMENT);
            };
            websocket.send(&mut session.stream, payload, fragsize, flags)
        })
    else {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    };
    let written = match result {
        Ok(written) => written,
        Err(code) => return code,
    };
    unsafe { *sent = written };
    CURLE_OK
}

pub(crate) fn websocket_mode_enabled(connect_mode: i64) -> bool {
    connect_mode >= 2
}

pub(crate) fn raw_mode_enabled(ws_options: i64) -> bool {
    (ws_options & CURLWS_RAW_MODE) != 0
}

#[cfg(test)]
mod tests {
    use super::{encode_frame, WebSocketSession, CURLWS_PING};

    #[test]
    fn encoded_frames_use_fresh_masks() {
        let one = encode_frame(b"hello", 0x1, true).expect("frame");
        let two = encode_frame(b"hello", 0x1, true).expect("frame");
        assert_ne!(&one[2..6], &two[2..6]);
    }

    #[test]
    fn websocket_ping_frame_round_trips_without_busy_loop() {
        let mut session = WebSocketSession {
            raw_mode: false,
            recv_buffer: vec![0x89, 0x00],
            pending: None,
            frame: crate::abi::curl_ws_frame {
                age: 0,
                flags: 0,
                offset: 0,
                bytesleft: 0,
                len: 0,
            },
            send_fragment_remaining: None,
        };
        let frame = session.try_parse_frame().expect("parse").expect("frame");
        assert_eq!(frame.flags & CURLWS_PING, CURLWS_PING);
        assert!(frame.payload.is_empty());
    }
}
