use crate::abi::CURLcode;
use std::path::PathBuf;

pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/file.c"];

const CURLE_URL_MALFORMAT: CURLcode = 3;

pub(crate) fn matches(scheme: &str) -> bool {
    scheme == "file"
}

pub(crate) fn decode_url_path(url: &str) -> Result<PathBuf, CURLcode> {
    let Some(rest) = url.strip_prefix("file://") else {
        return Err(CURLE_URL_MALFORMAT);
    };

    let path = if rest.starts_with('/') {
        rest
    } else {
        let (authority, suffix) = rest.split_once('/').ok_or(CURLE_URL_MALFORMAT)?;
        if !(authority.is_empty() || authority.eq_ignore_ascii_case("localhost")) {
            return Err(CURLE_URL_MALFORMAT);
        }
        suffix
    };

    let decoded = percent_decode(path.as_bytes())?;
    Ok(PathBuf::from(format!(
        "/{}",
        decoded.trim_start_matches('/')
    )))
}

fn percent_decode(input: &[u8]) -> Result<String, CURLcode> {
    let mut out = Vec::with_capacity(input.len());
    let mut idx = 0;
    while idx < input.len() {
        match input[idx] {
            b'%' if idx + 2 < input.len() => {
                let hi = decode_hex(input[idx + 1])?;
                let lo = decode_hex(input[idx + 2])?;
                out.push((hi << 4) | lo);
                idx += 3;
            }
            byte => {
                out.push(byte);
                idx += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| CURLE_URL_MALFORMAT)
}

fn decode_hex(byte: u8) -> Result<u8, CURLcode> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(CURLE_URL_MALFORMAT),
    }
}
