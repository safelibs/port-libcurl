pub(crate) const HEADER_ORIGIN_HEADER: u32 = 1 << 0;
pub(crate) const HEADER_ORIGIN_TRAILER: u32 = 1 << 1;
pub(crate) const HEADER_ORIGIN_CONNECT: u32 = 1 << 2;
pub(crate) const HEADER_ORIGIN_1XX: u32 = 1 << 3;
pub(crate) const HEADER_ORIGIN_PSEUDO: u32 = 1 << 4;
pub(crate) const HEADER_ORIGIN_RESERVED_BIT: u32 = 1 << 27;
pub(crate) const MAX_RESPONSE_HEADERS_BYTES: usize = 300 * 1024;

pub(crate) fn split_header_line(line: &str) -> Option<(&str, &str)> {
    let (name, value) = line.split_once(':')?;
    Some((name.trim(), value.trim()))
}
