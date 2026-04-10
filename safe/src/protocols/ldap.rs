use crate::abi::{CURLcode, CURL};
use crate::easy::perform::{EasyCallbacks, EasyMetadata, RecordedTransferInfo};
use crate::http::auth;
use crate::protocols::{percent_decode, ParsedProtocolUrl};
use crate::transfer::{self, LowSpeedGuard, TransferPlan, TransportStream};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/ldap.c", "original/lib/openldap.c"];

const CURLE_URL_MALFORMAT: CURLcode = 3;
const CURLE_COULDNT_CONNECT: CURLcode = 7;
const CURLE_REMOTE_ACCESS_DENIED: CURLcode = 9;
const CURLE_RECV_ERROR: CURLcode = 56;
const CURLE_SEND_ERROR: CURLcode = 55;
const CURLE_LOGIN_DENIED: CURLcode = 67;
const CURLE_REMOTE_FILE_NOT_FOUND: CURLcode = 78;

const IO_TIMEOUT: Duration = Duration::from_secs(15);

pub(crate) fn matches(scheme: &str) -> bool {
    matches!(scheme, "ldap" | "ldaps")
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

    let query = match LdapQuery::from_url(&parsed) {
        Ok(query) => query,
        Err(code) => return code,
    };
    let credentials = ldap_credentials(&parsed, metadata);
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

    let code = perform_ldap_exchange(
        handle,
        &mut connected.stream,
        &connected.info,
        callbacks,
        plan,
        &credentials,
        &parsed,
        &query,
        started,
    );
    transfer::close_transport(connected.stream, callbacks);
    code
}

fn perform_ldap_exchange(
    handle: *mut CURL,
    stream: &mut TransportStream,
    base_info: &RecordedTransferInfo,
    callbacks: EasyCallbacks,
    plan: &TransferPlan,
    credentials: &Credentials,
    _parsed: &ParsedProtocolUrl,
    query: &LdapQuery,
    started: Instant,
) -> CURLcode {
    if send_message(stream, &encode_bind_request(1, &credentials)).is_err() {
        return CURLE_SEND_ERROR;
    }
    let bind = match read_message(stream) {
        Ok(message) => message,
        Err(code) => return code,
    };
    match parse_bind_response(&bind) {
        Ok(()) => {}
        Err((code, message)) => {
            crate::easy::perform::set_error_buffer(handle, &message);
            return code;
        }
    }

    if send_message(stream, &encode_search_request(2, query)).is_err() {
        return CURLE_SEND_ERROR;
    }

    let mut low_speed = LowSpeedGuard::new(plan.low_speed);
    if let Err(code) = transfer::invoke_progress_callback(callbacks, 0, None) {
        return code;
    }
    let mut delivered = 0usize;
    loop {
        let message = match read_message(stream) {
            Ok(message) => message,
            Err(code) => return code,
        };
        match parse_message_kind(&message) {
            Ok(MessageKind::SearchEntry(entry)) => {
                let mut rendered = render_entry(&entry);
                if let Err(code) = transfer::deliver_write(handle, callbacks, &mut rendered) {
                    return code;
                }
                delivered = delivered.saturating_add(rendered.len());
                if let Err(code) = low_speed.observe_progress(rendered.len()) {
                    return code;
                }
                if let Err(code) = transfer::invoke_progress_callback(callbacks, delivered, None) {
                    return code;
                }
            }
            Ok(MessageKind::SearchReference(urls)) => {
                let mut rendered = render_references(&urls);
                if !rendered.is_empty() {
                    if let Err(code) = transfer::deliver_write(handle, callbacks, &mut rendered) {
                        return code;
                    }
                    delivered = delivered.saturating_add(rendered.len());
                    if let Err(code) = low_speed.observe_progress(rendered.len()) {
                        return code;
                    }
                    if let Err(code) =
                        transfer::invoke_progress_callback(callbacks, delivered, None)
                    {
                        return code;
                    }
                }
            }
            Ok(MessageKind::SearchDone(result)) => {
                if result.code != 0 {
                    if !result.message.is_empty() {
                        crate::easy::perform::set_error_buffer(handle, &result.message);
                    }
                    return map_result_code(result.code);
                }
                break;
            }
            Ok(MessageKind::Other) => {}
            Err(code) => return code,
        }
    }

    let _ = send_message(stream, &encode_unbind_request(3));

    let mut info = base_info.clone();
    info.pretransfer_time_us = info.connect_time_us;
    info.starttransfer_time_us = info.connect_time_us;
    info.total_time_us = transfer::elapsed_us(started.elapsed());
    crate::easy::perform::record_transfer_info(handle, info);
    crate::abi::CURLE_OK
}

#[derive(Clone, Debug)]
struct LdapQuery {
    base_dn: String,
    attributes: Vec<String>,
    scope: u8,
    filter: Filter,
}

impl LdapQuery {
    fn from_url(parsed: &ParsedProtocolUrl) -> Result<Self, CURLcode> {
        let base_dn = parsed.decoded_path()?.trim_start_matches('/').to_string();
        let mut attributes = Vec::new();
        let mut scope = 0u8;
        let mut filter = Filter::Presence("objectClass".to_string());

        if let Some(query) = parsed.query.as_deref() {
            let mut parts = query.splitn(4, '?');
            if let Some(attr_part) = parts.next() {
                for attr in attr_part.split(',') {
                    let decoded = percent_decode(attr.as_bytes())?;
                    if !decoded.is_empty() {
                        attributes.push(decoded);
                    }
                }
            }
            if let Some(scope_part) = parts.next() {
                scope = match scope_part.to_ascii_lowercase().as_str() {
                    "" | "base" => 0,
                    "one" => 1,
                    "sub" => 2,
                    _ => 0,
                };
            }
            if let Some(filter_part) = parts.next() {
                let decoded = percent_decode(filter_part.as_bytes())?;
                if !decoded.is_empty() {
                    filter = Filter::parse(&decoded);
                }
            }
        }

        Ok(Self {
            base_dn,
            attributes,
            scope,
            filter,
        })
    }
}

#[derive(Clone, Debug)]
enum Filter {
    Presence(String),
    Equality { attribute: String, value: Vec<u8> },
}

impl Filter {
    fn parse(input: &str) -> Self {
        let trimmed = input.trim().trim_start_matches('(').trim_end_matches(')');
        if let Some((attribute, value)) = trimmed.split_once('=') {
            let attribute = attribute.trim().to_string();
            if !attribute.is_empty() {
                if value == "*" {
                    return Self::Presence(attribute);
                }
                return Self::Equality {
                    attribute,
                    value: value.as_bytes().to_vec(),
                };
            }
        }
        Self::Presence("objectClass".to_string())
    }
}

#[derive(Clone, Debug)]
struct Credentials {
    bind_dn: String,
    password: String,
}

fn ldap_credentials(parsed: &ParsedProtocolUrl, metadata: &EasyMetadata) -> Credentials {
    if let Some(explicit) = auth::explicit_basic_credentials(metadata) {
        return Credentials {
            bind_dn: explicit.username,
            password: explicit.password,
        };
    }
    Credentials {
        bind_dn: parsed.username.clone().unwrap_or_default(),
        password: parsed.password.clone().unwrap_or_default(),
    }
}

#[derive(Clone, Debug)]
struct LdapEntry {
    dn: String,
    attributes: Vec<(String, Vec<Vec<u8>>)>,
}

#[derive(Clone, Debug)]
struct SearchResult {
    code: u8,
    message: String,
}

#[derive(Clone, Debug)]
enum MessageKind {
    SearchEntry(LdapEntry),
    SearchReference(Vec<String>),
    SearchDone(SearchResult),
    Other,
}

fn send_message(stream: &mut TransportStream, message: &[u8]) -> std::io::Result<()> {
    stream.write_all(message)?;
    stream.flush()
}

fn read_message(stream: &mut TransportStream) -> Result<Vec<u8>, CURLcode> {
    let mut first = [0u8; 2];
    stream.read_exact(&mut first).map_err(map_io_read_error)?;
    if first[0] != 0x30 {
        return Err(CURLE_RECV_ERROR);
    }

    let length = if first[1] & 0x80 == 0 {
        usize::from(first[1])
    } else {
        let count = usize::from(first[1] & 0x7f);
        if count == 0 || count > 4 {
            return Err(CURLE_RECV_ERROR);
        }
        let mut length_bytes = vec![0u8; count];
        stream
            .read_exact(&mut length_bytes)
            .map_err(map_io_read_error)?;
        let length = length_bytes
            .into_iter()
            .fold(0usize, |acc, byte| (acc << 8) | usize::from(byte));
        length
    };
    let mut body = vec![0u8; length];
    stream.read_exact(&mut body).map_err(map_io_read_error)?;
    let mut header = vec![0x30];
    header.extend_from_slice(&encode_length(length));
    header.extend_from_slice(&body);
    Ok(header)
}

fn encode_bind_request(message_id: i64, credentials: &Credentials) -> Vec<u8> {
    let mut bind = Vec::new();
    bind.extend_from_slice(&encode_integer(message_id));

    let mut request = Vec::new();
    request.extend_from_slice(&encode_integer(3));
    request.extend_from_slice(&encode_octet_string(credentials.bind_dn.as_bytes()));
    request.extend_from_slice(&encode_context_primitive(
        0,
        credentials.password.as_bytes(),
    ));
    bind.extend_from_slice(&encode_constructed(0x60, &request));

    encode_sequence(&bind)
}

fn encode_search_request(message_id: i64, query: &LdapQuery) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&encode_integer(message_id));

    let mut request = Vec::new();
    request.extend_from_slice(&encode_octet_string(query.base_dn.as_bytes()));
    request.extend_from_slice(&encode_enumerated(query.scope));
    request.extend_from_slice(&encode_enumerated(0));
    request.extend_from_slice(&encode_integer(0));
    request.extend_from_slice(&encode_integer(0));
    request.extend_from_slice(&encode_boolean(false));
    request.extend_from_slice(&encode_filter(&query.filter));

    let mut attributes = Vec::new();
    for attribute in &query.attributes {
        attributes.extend_from_slice(&encode_octet_string(attribute.as_bytes()));
    }
    request.extend_from_slice(&encode_constructed(0x30, &attributes));

    body.extend_from_slice(&encode_constructed(0x63, &request));
    encode_sequence(&body)
}

fn encode_unbind_request(message_id: i64) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&encode_integer(message_id));
    body.extend_from_slice(&encode_context_primitive(2, &[]));
    encode_sequence(&body)
}

fn encode_sequence(body: &[u8]) -> Vec<u8> {
    encode_constructed(0x30, body)
}

fn encode_constructed(tag: u8, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 8);
    out.push(tag);
    out.extend_from_slice(&encode_length(body.len()));
    out.extend_from_slice(body);
    out
}

fn encode_context_primitive(index: u8, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 8);
    out.push(0x80 | (index & 0x1f));
    out.extend_from_slice(&encode_length(body.len()));
    out.extend_from_slice(body);
    out
}

fn encode_integer(value: i64) -> Vec<u8> {
    let mut bytes = value.to_be_bytes().to_vec();
    while bytes.len() > 1 {
        let remove = (bytes[0] == 0x00 && bytes[1] & 0x80 == 0)
            || (bytes[0] == 0xff && bytes[1] & 0x80 != 0);
        if !remove {
            break;
        }
        bytes.remove(0);
    }
    let mut out = Vec::with_capacity(bytes.len() + 4);
    out.push(0x02);
    out.extend_from_slice(&encode_length(bytes.len()));
    out.extend_from_slice(&bytes);
    out
}

fn encode_enumerated(value: u8) -> Vec<u8> {
    vec![0x0a, 0x01, value]
}

fn encode_boolean(value: bool) -> Vec<u8> {
    vec![0x01, 0x01, if value { 0xff } else { 0x00 }]
}

fn encode_octet_string(value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len() + 4);
    out.push(0x04);
    out.extend_from_slice(&encode_length(value.len()));
    out.extend_from_slice(value);
    out
}

fn encode_filter(filter: &Filter) -> Vec<u8> {
    match filter {
        Filter::Presence(attribute) => {
            let mut out = Vec::with_capacity(attribute.len() + 4);
            out.push(0x87);
            out.extend_from_slice(&encode_length(attribute.len()));
            out.extend_from_slice(attribute.as_bytes());
            out
        }
        Filter::Equality { attribute, value } => {
            let mut body = Vec::new();
            body.extend_from_slice(&encode_octet_string(attribute.as_bytes()));
            body.extend_from_slice(&encode_octet_string(value));
            encode_constructed(0xa3, &body)
        }
    }
}

fn encode_length(len: usize) -> Vec<u8> {
    if len < 0x80 {
        return vec![len as u8];
    }
    let mut buf = len.to_be_bytes().to_vec();
    while buf.first() == Some(&0) {
        buf.remove(0);
    }
    let mut out = Vec::with_capacity(buf.len() + 1);
    out.push(0x80 | (buf.len() as u8));
    out.extend_from_slice(&buf);
    out
}

fn parse_bind_response(message: &[u8]) -> Result<(), (CURLcode, String)> {
    let (_, op_tag, op_value) =
        parse_envelope(message).map_err(|_| (CURLE_RECV_ERROR, String::new()))?;
    if op_tag != 0x61 {
        return Err((
            CURLE_RECV_ERROR,
            "LDAP bind reply was malformed".to_string(),
        ));
    }
    let result = parse_result(op_value).map_err(|_| {
        (
            CURLE_RECV_ERROR,
            "LDAP bind reply was malformed".to_string(),
        )
    })?;
    if result.code == 0 {
        Ok(())
    } else {
        Err((map_result_code(result.code), result.message))
    }
}

fn parse_message_kind(message: &[u8]) -> Result<MessageKind, CURLcode> {
    let (_, op_tag, op_value) = parse_envelope(message)?;
    match op_tag {
        0x64 => Ok(MessageKind::SearchEntry(parse_search_entry(op_value)?)),
        0x65 => Ok(MessageKind::SearchDone(parse_result(op_value)?)),
        0x73 => Ok(MessageKind::SearchReference(parse_search_reference(
            op_value,
        )?)),
        _ => Ok(MessageKind::Other),
    }
}

fn parse_envelope(message: &[u8]) -> Result<(i64, u8, &[u8]), CURLcode> {
    let mut cursor = 0usize;
    let (tag, value) = next_tlv(message, &mut cursor)?;
    if tag != 0x30 {
        return Err(CURLE_RECV_ERROR);
    }
    let mut inner = 0usize;
    let (id_tag, id_value) = next_tlv(value, &mut inner)?;
    if id_tag != 0x02 {
        return Err(CURLE_RECV_ERROR);
    }
    let message_id = decode_integer(id_value)?;
    let (op_tag, op_value) = next_tlv(value, &mut inner)?;
    Ok((message_id, op_tag, op_value))
}

fn parse_result(value: &[u8]) -> Result<SearchResult, CURLcode> {
    let mut cursor = 0usize;
    let (code_tag, code_value) = next_tlv(value, &mut cursor)?;
    if code_tag != 0x0a || code_value.len() != 1 {
        return Err(CURLE_RECV_ERROR);
    }
    let (_, _) = next_tlv(value, &mut cursor)?;
    let (_, message) = next_tlv(value, &mut cursor)?;
    Ok(SearchResult {
        code: code_value[0],
        message: String::from_utf8_lossy(message).into_owned(),
    })
}

fn parse_search_entry(value: &[u8]) -> Result<LdapEntry, CURLcode> {
    let mut cursor = 0usize;
    let (dn_tag, dn) = next_tlv(value, &mut cursor)?;
    if dn_tag != 0x04 {
        return Err(CURLE_RECV_ERROR);
    }
    let (attrs_tag, attrs_value) = next_tlv(value, &mut cursor)?;
    if attrs_tag != 0x30 {
        return Err(CURLE_RECV_ERROR);
    }

    let mut attributes = Vec::new();
    let mut attr_cursor = 0usize;
    while attr_cursor < attrs_value.len() {
        let (attr_tag, attr_value) = next_tlv(attrs_value, &mut attr_cursor)?;
        if attr_tag != 0x30 {
            return Err(CURLE_RECV_ERROR);
        }
        let mut field_cursor = 0usize;
        let (name_tag, name) = next_tlv(attr_value, &mut field_cursor)?;
        if name_tag != 0x04 {
            return Err(CURLE_RECV_ERROR);
        }
        let (set_tag, set_value) = next_tlv(attr_value, &mut field_cursor)?;
        if set_tag != 0x31 {
            return Err(CURLE_RECV_ERROR);
        }
        let mut values = Vec::new();
        let mut set_cursor = 0usize;
        while set_cursor < set_value.len() {
            let (_, raw) = next_tlv(set_value, &mut set_cursor)?;
            values.push(raw.to_vec());
        }
        attributes.push((String::from_utf8_lossy(name).into_owned(), values));
    }

    Ok(LdapEntry {
        dn: String::from_utf8_lossy(dn).into_owned(),
        attributes,
    })
}

fn parse_search_reference(value: &[u8]) -> Result<Vec<String>, CURLcode> {
    let mut cursor = 0usize;
    let mut urls = Vec::new();
    while cursor < value.len() {
        let (tag, raw) = next_tlv(value, &mut cursor)?;
        if tag != 0x04 {
            return Err(CURLE_RECV_ERROR);
        }
        urls.push(String::from_utf8_lossy(raw).into_owned());
    }
    Ok(urls)
}

fn render_entry(entry: &LdapEntry) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"dn: ");
    out.extend_from_slice(entry.dn.as_bytes());
    out.push(b'\n');
    for (name, values) in &entry.attributes {
        for value in values {
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(b": ");
            if value
                .iter()
                .all(|byte| !byte.is_ascii_control() || *byte == b'\t')
            {
                out.extend_from_slice(value);
            } else {
                out.extend_from_slice(render_binary(value).as_bytes());
            }
            out.push(b'\n');
        }
    }
    out.push(b'\n');
    out
}

fn render_references(urls: &[String]) -> Vec<u8> {
    let mut out = Vec::new();
    for url in urls {
        out.extend_from_slice(b"ref: ");
        out.extend_from_slice(url.as_bytes());
        out.push(b'\n');
    }
    if !out.is_empty() {
        out.push(b'\n');
    }
    out
}

fn render_binary(value: &[u8]) -> String {
    let mut out = String::with_capacity(value.len() * 2);
    for byte in value {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn next_tlv<'a>(input: &'a [u8], cursor: &mut usize) -> Result<(u8, &'a [u8]), CURLcode> {
    if *cursor >= input.len() {
        return Err(CURLE_RECV_ERROR);
    }
    let tag = input[*cursor];
    *cursor += 1;
    if *cursor >= input.len() {
        return Err(CURLE_RECV_ERROR);
    }
    let first = input[*cursor];
    *cursor += 1;
    let length = if first & 0x80 == 0 {
        usize::from(first)
    } else {
        let count = usize::from(first & 0x7f);
        if count == 0 || *cursor + count > input.len() {
            return Err(CURLE_RECV_ERROR);
        }
        let mut length = 0usize;
        for _ in 0..count {
            length = (length << 8) | usize::from(input[*cursor]);
            *cursor += 1;
        }
        length
    };
    if *cursor + length > input.len() {
        return Err(CURLE_RECV_ERROR);
    }
    let value = &input[*cursor..*cursor + length];
    *cursor += length;
    Ok((tag, value))
}

fn decode_integer(value: &[u8]) -> Result<i64, CURLcode> {
    if value.is_empty() || value.len() > 8 {
        return Err(CURLE_RECV_ERROR);
    }
    let negative = value[0] & 0x80 != 0;
    let mut buf = if negative { [0xffu8; 8] } else { [0u8; 8] };
    let offset = 8 - value.len();
    buf[offset..].copy_from_slice(value);
    Ok(i64::from_be_bytes(buf))
}

fn map_result_code(code: u8) -> CURLcode {
    match code {
        0 => crate::abi::CURLE_OK,
        32 => CURLE_REMOTE_FILE_NOT_FOUND,
        49 => CURLE_LOGIN_DENIED,
        50 | 53 => CURLE_REMOTE_ACCESS_DENIED,
        _ => CURLE_RECV_ERROR,
    }
}

fn map_io_read_error(error: std::io::Error) -> CURLcode {
    match error.kind() {
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => CURLE_COULDNT_CONNECT,
        _ => CURLE_RECV_ERROR,
    }
}
