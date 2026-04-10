use std::collections::{HashMap, VecDeque};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UrlAuthority {
    pub scheme: String,
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ConnectionCacheKey {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub proxy_host: Option<String>,
    pub proxy_port: Option<u16>,
    pub tunnel_proxy: bool,
    pub conn_to_host: Option<String>,
    pub conn_to_port: Option<u16>,
    pub tls_peer_identity: Option<String>,
    pub auth_context: Option<String>,
    pub share_scope: Option<String>,
}

impl ConnectionCacheKey {
    pub(crate) fn authority(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[derive(Default)]
pub(crate) struct ConnectionCache {
    entries: HashMap<ConnectionCacheKey, usize>,
    order: VecDeque<ConnectionCacheKey>,
}

impl ConnectionCache {
    pub(crate) fn remember(
        &mut self,
        key: ConnectionCacheKey,
        limit: usize,
        new_connection_id: usize,
    ) -> (usize, bool) {
        if let Some(existing) = self.entries.get(&key).copied() {
            self.touch(&key);
            return (existing, true);
        }

        if limit > 0 && self.entries.len() >= limit {
            while let Some(evicted) = self.order.pop_front() {
                if self.entries.remove(&evicted).is_some() {
                    break;
                }
            }
        }

        self.entries.insert(key.clone(), new_connection_id);
        self.order.push_back(key);
        (new_connection_id, false)
    }

    pub(crate) fn get(&self, key: &ConnectionCacheKey) -> Option<usize> {
        self.entries.get(key).copied()
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn remove(&mut self, key: &ConnectionCacheKey) {
        self.entries.remove(key);
        self.order.retain(|candidate| candidate != key);
    }

    fn touch(&mut self, key: &ConnectionCacheKey) {
        self.order.retain(|candidate| candidate != key);
        self.order.push_back(key.clone());
    }
}

pub(crate) fn parse_url_authority(url: &str) -> Option<UrlAuthority> {
    let (scheme, rest) = url.split_once("://")?;
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(rest)
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(rest.split(['/', '?', '#']).next().unwrap_or(rest));
    let (host, port) = split_host_port(authority, default_port_for_scheme(scheme))?;
    Some(UrlAuthority {
        scheme: scheme.to_ascii_lowercase(),
        host,
        port,
    })
}

pub(crate) fn parse_proxy_authority(proxy: &str, default_scheme: &str) -> Option<(String, u16)> {
    if let Some(authority) = parse_url_authority(proxy) {
        return Some((authority.host, authority.port));
    }

    let (host, port) = split_host_port(proxy, default_port_for_scheme(default_scheme))?;
    Some((host, port))
}

fn split_host_port(input: &str, default_port: u16) -> Option<(String, u16)> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(rest) = trimmed.strip_prefix('[') {
        let end = rest.find(']')?;
        let host = rest[..end].to_string();
        let port = if let Some(port_text) = rest[end + 1..].strip_prefix(':') {
            port_text.parse().ok()?
        } else {
            default_port
        };
        return Some((host, port));
    }

    if let Some((host, port_text)) = trimmed.rsplit_once(':') {
        if !host.contains(':') && !port_text.is_empty() {
            return Some((host.to_string(), port_text.parse().ok()?));
        }
    }

    Some((trimmed.to_string(), default_port))
}

fn default_port_for_scheme(scheme: &str) -> u16 {
    match scheme.to_ascii_lowercase().as_str() {
        "http" | "ws" => 80,
        "https" | "wss" => 443,
        "ftp" => 21,
        "ftps" => 990,
        "smtp" => 25,
        "smtps" => 465,
        "imap" => 143,
        "imaps" => 993,
        "pop3" => 110,
        "pop3s" => 995,
        _ => 0,
    }
}
