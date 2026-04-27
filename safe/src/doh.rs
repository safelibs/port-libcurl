pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/doh.c"];

use crate::abi::CURLcode;
use crate::conn::cache::parse_url_authority;
use crate::easy::perform::EasyMetadata;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

const CURLE_URL_MALFORMAT: CURLcode = 3;
const CURLE_COULDNT_RESOLVE_HOST: CURLcode = 6;
const CURLE_COULDNT_CONNECT: CURLcode = 7;
const CURLE_SEND_ERROR: CURLcode = 55;
const CURLE_RECV_ERROR: CURLcode = 56;
const DOH_DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const DNS_TYPE_A: u16 = 1;
const DNS_TYPE_AAAA: u16 = 28;

pub(crate) fn encode_qname(name: &str) -> Result<Vec<u8>, ()> {
    if name.is_empty() {
        return Err(());
    }

    let mut out = Vec::with_capacity(name.len() + 2);
    for label in name.split('.') {
        if label.is_empty() || label.len() > u8::MAX as usize {
            return Err(());
        }
        out.push(label.len() as u8);
        out.extend_from_slice(label.as_bytes());
    }
    out.push(0);
    if out.len() > u8::MAX as usize {
        return Err(());
    }
    Ok(out)
}

pub(crate) fn decode_qname(bytes: &[u8]) -> Result<String, ()> {
    let mut labels = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        let len = bytes[index] as usize;
        index += 1;
        if len == 0 {
            return Ok(labels.join("."));
        }
        if index + len > bytes.len() {
            return Err(());
        }
        let label = std::str::from_utf8(&bytes[index..index + len]).map_err(|_| ())?;
        labels.push(label.to_string());
        index += len;
    }
    Err(())
}

pub(crate) fn validate_doh_url(url: &str) -> bool {
    matches!(
        crate::conn::cache::parse_url_authority(url),
        Some(authority) if matches!(authority.scheme.as_str(), "http" | "https")
    )
}

pub(crate) fn resolve_host(
    host: &str,
    doh_url: &str,
    metadata: &EasyMetadata,
) -> Result<Vec<IpAddr>, CURLcode> {
    let endpoint = DohEndpoint::parse(doh_url)?;
    let mut addresses = Vec::new();
    let mut last_error = CURLE_COULDNT_RESOLVE_HOST;

    for qtype in [DNS_TYPE_A, DNS_TYPE_AAAA] {
        match resolve_question(&endpoint, host, qtype, metadata) {
            Ok(mut found) => addresses.append(&mut found),
            Err(error) => last_error = error,
        }
    }

    if addresses.is_empty() {
        Err(last_error)
    } else {
        addresses.sort_unstable();
        addresses.dedup();
        Ok(addresses)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DohEndpoint {
    scheme: String,
    host: String,
    port: u16,
    request_target: String,
}

impl DohEndpoint {
    fn parse(url: &str) -> Result<Self, CURLcode> {
        if !validate_doh_url(url) {
            return Err(CURLE_URL_MALFORMAT);
        }

        let authority = parse_url_authority(url).ok_or(CURLE_URL_MALFORMAT)?;
        Ok(Self {
            scheme: authority.scheme,
            host: authority.host,
            port: authority.port,
            request_target: extract_request_target(url),
        })
    }

    fn default_port(&self) -> u16 {
        match self.scheme.as_str() {
            "https" => 443,
            _ => 80,
        }
    }

    fn host_header(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        if self.port == self.default_port() {
            host
        } else {
            format!("{host}:{}", self.port)
        }
    }
}

enum DohStream {
    Plain(TcpStream),
    Tls(crate::tls::TlsConnection),
}

impl Read for DohStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.read(buf),
            Self::Tls(stream) => stream.read(buf),
        }
    }
}

impl Write for DohStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.write(buf),
            Self::Tls(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn resolve_question(
    endpoint: &DohEndpoint,
    host: &str,
    qtype: u16,
    metadata: &EasyMetadata,
) -> Result<Vec<IpAddr>, CURLcode> {
    let packet = build_query_packet(host, qtype)?;
    let body = exchange_dns_message(endpoint, &packet, metadata)?;
    parse_dns_response(&body, qtype)
}

fn build_query_packet(host: &str, qtype: u16) -> Result<Vec<u8>, CURLcode> {
    let qname = encode_qname(host).map_err(|_| CURLE_COULDNT_RESOLVE_HOST)?;
    let mut packet = Vec::with_capacity(12 + qname.len() + 4);
    packet.extend_from_slice(&0x1234u16.to_be_bytes());
    packet.extend_from_slice(&0x0100u16.to_be_bytes());
    packet.extend_from_slice(&1u16.to_be_bytes());
    packet.extend_from_slice(&0u16.to_be_bytes());
    packet.extend_from_slice(&0u16.to_be_bytes());
    packet.extend_from_slice(&0u16.to_be_bytes());
    packet.extend_from_slice(&qname);
    packet.extend_from_slice(&qtype.to_be_bytes());
    packet.extend_from_slice(&1u16.to_be_bytes());
    Ok(packet)
}

fn exchange_dns_message(
    endpoint: &DohEndpoint,
    packet: &[u8],
    metadata: &EasyMetadata,
) -> Result<Vec<u8>, CURLcode> {
    let mut stream = connect_endpoint(endpoint, metadata)?;
    let request = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/dns-message\r\nAccept: application/dns-message\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        endpoint.request_target,
        endpoint.host_header(),
        packet.len()
    );
    stream
        .write_all(request.as_bytes())
        .and_then(|_| stream.write_all(packet))
        .map_err(|_| CURLE_SEND_ERROR)?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|_| CURLE_RECV_ERROR)?;
    extract_dns_body(&response)
}

fn connect_endpoint(
    endpoint: &DohEndpoint,
    metadata: &EasyMetadata,
) -> Result<DohStream, CURLcode> {
    let addrs = if let Ok(ip) = endpoint.host.parse::<IpAddr>() {
        vec![SocketAddr::new(ip, endpoint.port)]
    } else {
        let resolved = (endpoint.host.as_str(), endpoint.port)
            .to_socket_addrs()
            .map_err(|_| CURLE_COULDNT_RESOLVE_HOST)?;
        let addrs: Vec<_> = resolved.collect();
        if addrs.is_empty() {
            return Err(CURLE_COULDNT_RESOLVE_HOST);
        }
        addrs
    };

    let timeout = request_timeout(metadata);
    for addr in addrs {
        let stream = match TcpStream::connect_timeout(&addr, timeout) {
            Ok(stream) => stream,
            Err(_) => continue,
        };
        if stream.set_read_timeout(Some(timeout)).is_err()
            || stream.set_write_timeout(Some(timeout)).is_err()
        {
            continue;
        }

        return if endpoint.scheme == "https" {
            let policy = crate::tls::policy_for_scheme("https", metadata);
            crate::tls::connect(stream, &endpoint.host, endpoint.port, metadata, &policy)
                .map(DohStream::Tls)
        } else {
            Ok(DohStream::Plain(stream))
        };
    }

    Err(CURLE_COULDNT_CONNECT)
}

fn request_timeout(metadata: &EasyMetadata) -> Duration {
    if metadata.timeout_ms > 0 {
        Duration::from_millis(metadata.timeout_ms as u64)
    } else {
        DOH_DEFAULT_TIMEOUT
    }
}

fn extract_request_target(url: &str) -> String {
    let Some((_, rest)) = url.split_once("://") else {
        return "/".to_string();
    };
    let Some(index) = rest.find(['/', '?', '#']) else {
        return "/".to_string();
    };
    let tail = &rest[index..];
    let tail = tail.split('#').next().unwrap_or(tail);
    if tail.starts_with('/') {
        tail.to_string()
    } else if tail.starts_with('?') {
        format!("/{tail}")
    } else {
        "/".to_string()
    }
}

fn extract_dns_body(response: &[u8]) -> Result<Vec<u8>, CURLcode> {
    let Some(header_end) = response.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Err(CURLE_RECV_ERROR);
    };
    let header_end = header_end + 4;
    let header_text = std::str::from_utf8(&response[..header_end]).map_err(|_| CURLE_RECV_ERROR)?;
    let status_code = header_text
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or(CURLE_RECV_ERROR)?;
    if status_code != 200 {
        return Err(CURLE_COULDNT_RESOLVE_HOST);
    }

    let body = &response[header_end..];
    let mut content_length = None;
    let mut chunked = false;
    for line in header_text.lines().skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("Content-Length") {
            content_length = value.trim().parse::<usize>().ok();
        } else if name.eq_ignore_ascii_case("Transfer-Encoding")
            && value
                .split(',')
                .any(|entry| entry.trim().eq_ignore_ascii_case("chunked"))
        {
            chunked = true;
        }
    }

    if chunked {
        decode_chunked_body(body)
    } else if let Some(length) = content_length {
        if body.len() < length {
            Err(CURLE_RECV_ERROR)
        } else {
            Ok(body[..length].to_vec())
        }
    } else {
        Ok(body.to_vec())
    }
}

fn decode_chunked_body(body: &[u8]) -> Result<Vec<u8>, CURLcode> {
    let mut decoded = Vec::new();
    let mut cursor = body;
    loop {
        let Some(line_end) = cursor.windows(2).position(|window| window == b"\r\n") else {
            return Err(CURLE_RECV_ERROR);
        };
        let line = std::str::from_utf8(&cursor[..line_end]).map_err(|_| CURLE_RECV_ERROR)?;
        let size = usize::from_str_radix(line.split(';').next().unwrap_or(line).trim(), 16)
            .map_err(|_| CURLE_RECV_ERROR)?;
        cursor = &cursor[line_end + 2..];
        if size == 0 {
            return Ok(decoded);
        }
        if cursor.len() < size + 2 || &cursor[size..size + 2] != b"\r\n" {
            return Err(CURLE_RECV_ERROR);
        }
        decoded.extend_from_slice(&cursor[..size]);
        cursor = &cursor[size + 2..];
    }
}

fn parse_dns_response(body: &[u8], expected_qtype: u16) -> Result<Vec<IpAddr>, CURLcode> {
    if body.len() < 12 {
        return Err(CURLE_RECV_ERROR);
    }
    if u16::from_be_bytes([body[0], body[1]]) != 0x1234 {
        return Err(CURLE_RECV_ERROR);
    }

    let flags = u16::from_be_bytes([body[2], body[3]]);
    if flags & 0x8000 == 0 {
        return Err(CURLE_RECV_ERROR);
    }
    if flags & 0x000f != 0 {
        return Err(CURLE_COULDNT_RESOLVE_HOST);
    }

    let question_count = u16::from_be_bytes([body[4], body[5]]) as usize;
    let answer_count = u16::from_be_bytes([body[6], body[7]]) as usize;
    let mut offset = 12usize;

    for _ in 0..question_count {
        offset = skip_name(body, offset).ok_or(CURLE_RECV_ERROR)?;
        if body.len() < offset + 4 {
            return Err(CURLE_RECV_ERROR);
        }
        offset += 4;
    }

    let mut answers = Vec::new();
    for _ in 0..answer_count {
        offset = skip_name(body, offset).ok_or(CURLE_RECV_ERROR)?;
        if body.len() < offset + 10 {
            return Err(CURLE_RECV_ERROR);
        }

        let record_type = u16::from_be_bytes([body[offset], body[offset + 1]]);
        let record_class = u16::from_be_bytes([body[offset + 2], body[offset + 3]]);
        let rdlength = u16::from_be_bytes([body[offset + 8], body[offset + 9]]) as usize;
        offset += 10;
        if body.len() < offset + rdlength {
            return Err(CURLE_RECV_ERROR);
        }

        if record_class == 1 && record_type == expected_qtype {
            match (record_type, rdlength) {
                (DNS_TYPE_A, 4) => answers.push(IpAddr::from([
                    body[offset],
                    body[offset + 1],
                    body[offset + 2],
                    body[offset + 3],
                ])),
                (DNS_TYPE_AAAA, 16) => {
                    let mut octets = [0u8; 16];
                    octets.copy_from_slice(&body[offset..offset + 16]);
                    answers.push(IpAddr::from(octets));
                }
                _ => {}
            }
        }
        offset += rdlength;
    }

    Ok(answers)
}

fn skip_name(packet: &[u8], mut offset: usize) -> Option<usize> {
    loop {
        let len = *packet.get(offset)?;
        if len & 0xc0 == 0xc0 {
            offset.checked_add(2).filter(|next| *next <= packet.len())?;
            return Some(offset + 2);
        }
        offset += 1;
        if len == 0 {
            return Some(offset);
        }
        offset = offset.checked_add(len as usize)?;
        if offset > packet.len() {
            return None;
        }
    }
}
