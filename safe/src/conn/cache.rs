use std::collections::HashMap;

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
}

impl ConnectionCache {
    pub(crate) fn insert(&mut self, key: ConnectionCacheKey, connection_id: usize) {
        self.entries.insert(key, connection_id);
    }

    pub(crate) fn get(&self, key: &ConnectionCacheKey) -> Option<usize> {
        self.entries.get(key).copied()
    }

    pub(crate) fn remove(&mut self, key: &ConnectionCacheKey) {
        self.entries.remove(key);
    }
}
