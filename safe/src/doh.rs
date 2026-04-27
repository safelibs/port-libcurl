pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/doh.c"];

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
