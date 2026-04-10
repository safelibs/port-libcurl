use crate::easy::perform::EasyMetadata;
use crate::http::proxy;
use crate::http::request::Origin;
use core::ffi::c_long;
use std::fs;

const CURL_NETRC_IGNORED: c_long = 0;
const CURLAUTH_NEGOTIATE: c_long = 1 << 2;
const CURLAUTH_NTLM: c_long = 1 << 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BasicCredentials {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RequestAuthHeaders {
    pub authorization: Option<String>,
    pub proxy_authorization: Option<String>,
    pub referer: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct NetrcEntry {
    pub machine: Option<String>,
    pub login: Option<String>,
    pub password: Option<String>,
}

pub(crate) fn build_auth_context(metadata: &EasyMetadata) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(credentials) = explicit_basic_credentials(metadata) {
        parts.push(format!(
            "basic={}:{}",
            credentials.username, credentials.password
        ));
    }
    if let Some(credentials) = proxy::resolve_proxy_credentials(
        metadata.proxy_userpwd.as_deref(),
        metadata.proxy_username.as_deref(),
        metadata.proxy_password.as_deref(),
    ) {
        parts.push(format!(
            "proxy-basic={}:{}",
            credentials.username, credentials.password
        ));
    }
    if let Some(token) = metadata.xoauth2_bearer.as_deref() {
        parts.push(format!("bearer={token}"));
    }
    if metadata.netrc_mode != CURL_NETRC_IGNORED {
        parts.push(format!("netrc={}", metadata.netrc_mode));
        if let Some(path) = metadata.netrc_file.as_deref() {
            parts.push(format!("netrc-file={path}"));
        }
    }
    if metadata.httpauth != 0 {
        parts.push(format!("httpauth={}", metadata.httpauth));
    }
    if metadata.proxyauth != 0 {
        parts.push(format!("proxyauth={}", metadata.proxyauth));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(";"))
    }
}

pub(crate) fn request_auth_headers(
    metadata: &EasyMetadata,
    current_url: &str,
    initial_origin: Option<&Origin>,
    allow_cross_origin_auth: bool,
    referer: Option<&str>,
) -> RequestAuthHeaders {
    let current_origin = Origin::from_url(current_url);
    let same_origin = match (initial_origin, current_origin.as_ref()) {
        (Some(initial), Some(current)) => initial.same_origin(current),
        _ => false,
    };
    let allow_server_auth = same_origin || allow_cross_origin_auth;

    let authorization = if allow_server_auth {
        if let Some(token) = metadata.xoauth2_bearer.as_deref() {
            Some(format!("Authorization: Bearer {token}"))
        } else {
            resolve_basic_credentials(metadata, current_url).map(|credentials| {
                let token = base64_encode(
                    format!("{}:{}", credentials.username, credentials.password).as_bytes(),
                );
                format!("Authorization: Basic {token}")
            })
        }
    } else {
        None
    };

    let proxy_authorization = proxy::resolve_proxy_credentials(
        metadata.proxy_userpwd.as_deref(),
        metadata.proxy_username.as_deref(),
        metadata.proxy_password.as_deref(),
    )
    .map(|credentials| {
        let token =
            base64_encode(format!("{}:{}", credentials.username, credentials.password).as_bytes());
        format!("Proxy-Authorization: Basic {token}")
    });

    let referer = referer
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("Referer: {value}"));

    RequestAuthHeaders {
        authorization,
        proxy_authorization,
        referer,
    }
}

pub(crate) fn connection_oriented_auth_enabled(metadata: &EasyMetadata) -> bool {
    let masks = metadata.httpauth | metadata.proxyauth;
    (masks & CURLAUTH_NEGOTIATE) != 0 || (masks & CURLAUTH_NTLM) != 0
}

pub(crate) fn resolve_basic_credentials(
    metadata: &EasyMetadata,
    current_url: &str,
) -> Option<BasicCredentials> {
    explicit_basic_credentials(metadata).or_else(|| netrc_credentials(metadata, current_url))
}

pub(crate) fn explicit_basic_credentials(metadata: &EasyMetadata) -> Option<BasicCredentials> {
    if let Some(pair) = metadata.userpwd.as_deref() {
        let (username, password) = pair.split_once(':').unwrap_or((pair, ""));
        return Some(BasicCredentials {
            username: username.to_string(),
            password: password.to_string(),
        });
    }
    metadata
        .username
        .as_deref()
        .map(|username| BasicCredentials {
            username: username.to_string(),
            password: metadata.password.as_deref().unwrap_or_default().to_string(),
        })
}

pub(crate) fn netrc_credentials(
    metadata: &EasyMetadata,
    current_url: &str,
) -> Option<BasicCredentials> {
    if metadata.netrc_mode == CURL_NETRC_IGNORED {
        return None;
    }
    let host = Origin::from_url(current_url)?.host;
    let path = metadata.netrc_file.clone().or_else(default_netrc_path)?;
    let text = fs::read_to_string(path).ok()?;
    parse_netrc(
        &text,
        &host,
        metadata.username.as_deref(),
        protocol_allows_control_credentials(current_url),
    )
}

fn default_netrc_path() -> Option<String> {
    std::env::var("HOME")
        .ok()
        .map(|home| format!("{home}/.netrc"))
}

fn protocol_allows_control_credentials(url: &str) -> bool {
    let scheme = Origin::from_url(url)
        .map(|origin| origin.scheme)
        .unwrap_or_default();
    !matches!(scheme.as_str(), "http" | "https" | "ws" | "wss")
}

pub(crate) fn parse_netrc(
    text: &str,
    host: &str,
    specific_login: Option<&str>,
    allow_control_credentials: bool,
) -> Option<BasicCredentials> {
    let tokens = tokenize_netrc(text);
    let mut active = NetrcEntry::default();
    let mut candidate = None;
    let mut in_match = false;
    let mut index = 0usize;

    while index < tokens.len() {
        match tokens[index].as_str() {
            "machine" => {
                if let Some(found) =
                    finalize_netrc_candidate(&active, specific_login, allow_control_credentials)
                {
                    candidate = Some(found);
                }
                active = NetrcEntry::default();
                index += 1;
                if let Some(machine) = tokens.get(index) {
                    in_match = machine.eq_ignore_ascii_case(host);
                    if in_match {
                        active.machine = Some(machine.clone());
                    }
                } else {
                    in_match = false;
                }
            }
            "default" => {
                if let Some(found) =
                    finalize_netrc_candidate(&active, specific_login, allow_control_credentials)
                {
                    candidate = Some(found);
                }
                active = NetrcEntry::default();
                in_match = true;
            }
            "login" if in_match => {
                index += 1;
                active.login = tokens.get(index).cloned();
            }
            "password" if in_match => {
                index += 1;
                active.password = tokens.get(index).cloned();
            }
            _ => {}
        }
        index += 1;
    }

    finalize_netrc_candidate(&active, specific_login, allow_control_credentials).or(candidate)
}

fn finalize_netrc_candidate(
    entry: &NetrcEntry,
    specific_login: Option<&str>,
    allow_control_credentials: bool,
) -> Option<BasicCredentials> {
    let login = entry.login.as_deref()?;
    if let Some(expected) = specific_login {
        if login != expected {
            return None;
        }
    }
    let password = entry.password.as_deref().unwrap_or_default();
    if !allow_control_credentials && contains_control(login, password) {
        return None;
    }
    Some(BasicCredentials {
        username: login.to_string(),
        password: password.to_string(),
    })
}

fn tokenize_netrc(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for line in text.lines() {
        let trimmed = line.split('#').next().unwrap_or("");
        tokens.extend(trimmed.split_whitespace().map(str::to_string));
    }
    tokens
}

fn contains_control(username: &str, password: &str) -> bool {
    username.bytes().any(is_control) || password.bytes().any(is_control)
}

fn is_control(byte: u8) -> bool {
    byte < 0x20 || byte == 0x7f
}

pub(crate) fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut chunks = input.chunks_exact(3);
    for chunk in &mut chunks {
        let value = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | (chunk[2] as u32);
        encoded.push(TABLE[((value >> 18) & 0x3f) as usize] as char);
        encoded.push(TABLE[((value >> 12) & 0x3f) as usize] as char);
        encoded.push(TABLE[((value >> 6) & 0x3f) as usize] as char);
        encoded.push(TABLE[(value & 0x3f) as usize] as char);
    }

    let remainder = chunks.remainder();
    if !remainder.is_empty() {
        let mut value = (remainder[0] as u32) << 16;
        if remainder.len() == 2 {
            value |= (remainder[1] as u32) << 8;
        }
        encoded.push(TABLE[((value >> 18) & 0x3f) as usize] as char);
        encoded.push(TABLE[((value >> 12) & 0x3f) as usize] as char);
        if remainder.len() == 2 {
            encoded.push(TABLE[((value >> 6) & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
        encoded.push('=');
    }

    encoded
}

#[cfg(test)]
mod tests {
    use super::{base64_encode, build_auth_context, parse_netrc, BasicCredentials};
    use crate::easy::perform::EasyMetadata;

    #[test]
    fn netrc_default_without_credentials_is_not_a_match() {
        let creds = parse_netrc(
            "machine a.test login alice password secret\ndefault\n",
            "b.test",
            None,
            false,
        );
        assert!(creds.is_none());
    }

    #[test]
    fn netrc_rejects_control_characters_for_http() {
        let creds = parse_netrc(
            "machine a.test login alice password sec\x01ret\n",
            "a.test",
            None,
            false,
        );
        assert!(creds.is_none());
    }

    #[test]
    fn netrc_without_password_yields_blank_password() {
        let creds = parse_netrc("machine a.test login alice\n", "a.test", None, false)
            .expect("netrc creds");
        assert_eq!(
            creds,
            BasicCredentials {
                username: "alice".to_string(),
                password: String::new(),
            }
        );
    }

    #[test]
    fn auth_context_carries_connection_oriented_mask() {
        let metadata = EasyMetadata {
            userpwd: Some("alice:secret".to_string()),
            httpauth: 1 << 2,
            ..EasyMetadata::default()
        };
        let key = build_auth_context(&metadata).expect("key");
        assert!(key.contains("httpauth=4"));
    }

    #[test]
    fn base64_basic_token_matches_expected() {
        assert_eq!(base64_encode(b"alice:secret"), "YWxpY2U6c2VjcmV0");
    }
}
