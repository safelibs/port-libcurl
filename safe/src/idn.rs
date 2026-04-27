pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/idn.c"];

use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

fn strip_ipv6_zone(host: &str) -> &str {
    host.split_once('%').map(|(addr, _)| addr).unwrap_or(host)
}

pub(crate) fn is_ip_literal(host: &str) -> bool {
    Ipv4Addr::from_str(host).is_ok() || Ipv6Addr::from_str(strip_ipv6_zone(host)).is_ok()
}

pub(crate) fn host_to_ascii(host: &str) -> Result<String, ()> {
    if host.is_empty() || is_ip_literal(host) {
        return Ok(host.to_string());
    }
    idna::domain_to_ascii(host).map_err(|_| ())
}

pub(crate) fn host_to_unicode(host: &str) -> Result<String, ()> {
    if host.is_empty() || is_ip_literal(host) {
        return Ok(host.to_string());
    }
    let (decoded, errors) = idna::domain_to_unicode(host);
    if errors.is_err() {
        Err(())
    } else {
        Ok(decoded)
    }
}

pub(crate) fn normalize_host_for_transfer(host: &str) -> Result<String, ()> {
    host_to_ascii(host)
}
