use crate::abi::curl_slist;
use std::ffi::CStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResolverOwner {
    Easy,
    Multi,
    Share,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolverLease {
    pub owner: ResolverOwner,
    pub shared: bool,
    pub share_scope: Option<String>,
}

impl ResolverLease {
    pub(crate) fn for_share(share_handle: Option<usize>, default_owner: ResolverOwner) -> Self {
        if let Some(handle) = share_handle {
            Self {
                owner: ResolverOwner::Share,
                shared: true,
                share_scope: Some(format!("share:{handle:016x}")),
            }
        } else {
            Self {
                owner: default_owner,
                shared: false,
                share_scope: None,
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolveOverride {
    pub host: String,
    pub port: u16,
    pub addresses: Vec<String>,
    pub remove: bool,
    pub transient: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConnectOverride {
    pub source_host: Option<String>,
    pub source_port: Option<u16>,
    pub target_host: Option<String>,
    pub target_port: Option<u16>,
}

impl ConnectOverride {
    pub(crate) fn matches(&self, host: &str, port: u16) -> bool {
        let host_match = self
            .source_host
            .as_deref()
            .is_none_or(|candidate| candidate.eq_ignore_ascii_case(host));
        let port_match = self.source_port.is_none_or(|candidate| candidate == port);
        host_match && port_match
    }
}

pub(crate) fn collect_resolve_overrides(list: *mut curl_slist) -> Vec<ResolveOverride> {
    slist_entries(list)
        .into_iter()
        .filter_map(|entry| parse_resolve_override(&entry))
        .collect()
}

pub(crate) fn collect_connect_overrides(list: *mut curl_slist) -> Vec<ConnectOverride> {
    slist_entries(list)
        .into_iter()
        .filter_map(|entry| parse_connect_override(&entry))
        .collect()
}

fn slist_entries(mut list: *mut curl_slist) -> Vec<String> {
    let mut entries = Vec::new();
    while !list.is_null() {
        let data = unsafe { (*list).data };
        if !data.is_null() {
            entries.push(unsafe { CStr::from_ptr(data) }.to_string_lossy().into_owned());
        }
        list = unsafe { (*list).next };
    }
    entries
}

fn parse_resolve_override(entry: &str) -> Option<ResolveOverride> {
    let transient = entry.starts_with('+');
    let remove = entry.starts_with('-');
    let payload = entry.trim_start_matches(['+', '-']);
    let fields = split_colon_fields(payload, 3);
    if fields.len() != 3 {
        return None;
    }

    Some(ResolveOverride {
        host: normalize_host(&fields[0]),
        port: fields[1].parse().ok()?,
        addresses: if remove {
            Vec::new()
        } else {
            fields[2]
                .split(',')
                .filter(|segment| !segment.is_empty())
                .map(normalize_host)
                .collect()
        },
        remove,
        transient,
    })
}

fn parse_connect_override(entry: &str) -> Option<ConnectOverride> {
    let fields = split_colon_fields(entry, 4);
    if fields.len() != 4 {
        return None;
    }

    Some(ConnectOverride {
        source_host: normalize_optional_host(&fields[0]),
        source_port: normalize_optional_port(&fields[1]),
        target_host: normalize_optional_host(&fields[2]),
        target_port: normalize_optional_port(&fields[3]),
    })
}

fn split_colon_fields(input: &str, expected: usize) -> Vec<String> {
    let mut fields = Vec::with_capacity(expected);
    let mut current = String::new();
    let mut bracket_depth = 0usize;

    for ch in input.chars() {
        match ch {
            '[' => {
                bracket_depth += 1;
                current.push(ch);
            }
            ']' => {
                bracket_depth = bracket_depth.saturating_sub(1);
                current.push(ch);
            }
            ':' if bracket_depth == 0 && fields.len() + 1 < expected => {
                fields.push(current);
                current = String::new();
            }
            _ => current.push(ch),
        }
    }

    fields.push(current);
    fields
}

fn normalize_host(host: &str) -> String {
    host.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_string()
}

fn normalize_optional_host(host: &str) -> Option<String> {
    let host = normalize_host(host);
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

fn normalize_optional_port(port: &str) -> Option<u16> {
    if port.is_empty() {
        None
    } else {
        port.parse().ok()
    }
}
