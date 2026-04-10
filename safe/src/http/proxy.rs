use crate::conn::cache::parse_proxy_authority;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProxyCredentials {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProxyIdentity {
    pub host: String,
    pub port: u16,
    pub credentials: Option<ProxyCredentials>,
}

impl ProxyIdentity {
    pub(crate) fn key_fragment(&self) -> String {
        let mut key = format!("{}:{}", self.host, self.port);
        if let Some(credentials) = self.credentials.as_ref() {
            key.push_str(";proxy-user=");
            key.push_str(&credentials.username);
            key.push_str(";proxy-pass=");
            key.push_str(&credentials.password);
        }
        key
    }
}

pub(crate) fn build_proxy_identity(
    proxy: Option<&str>,
    scheme: &str,
    proxy_userpwd: Option<&str>,
    proxy_username: Option<&str>,
    proxy_password: Option<&str>,
) -> Option<ProxyIdentity> {
    let (host, port) = parse_proxy_authority(proxy?, scheme)?;
    Some(ProxyIdentity {
        host,
        port,
        credentials: resolve_proxy_credentials(proxy_userpwd, proxy_username, proxy_password),
    })
}

pub(crate) fn resolve_proxy_credentials(
    proxy_userpwd: Option<&str>,
    proxy_username: Option<&str>,
    proxy_password: Option<&str>,
) -> Option<ProxyCredentials> {
    if let Some(pair) = proxy_userpwd {
        let (username, password) = pair.split_once(':').unwrap_or((pair, ""));
        return Some(ProxyCredentials {
            username: username.to_string(),
            password: password.to_string(),
        });
    }
    proxy_username.map(|username| ProxyCredentials {
        username: username.to_string(),
        password: proxy_password.unwrap_or_default().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{build_proxy_identity, resolve_proxy_credentials};

    #[test]
    fn proxy_credentials_remain_case_sensitive() {
        let one = resolve_proxy_credentials(Some("User:Pass"), None, None).expect("creds");
        let two = resolve_proxy_credentials(Some("user:Pass"), None, None).expect("creds");
        assert_ne!(one, two);
    }

    #[test]
    fn proxy_identity_includes_credentials_in_key() {
        let proxy = build_proxy_identity(
            Some("http://proxy.test:8080"),
            "http",
            Some("User:Pass"),
            None,
            None,
        )
        .expect("proxy");
        assert_eq!(
            proxy.key_fragment(),
            "proxy.test:8080;proxy-user=User;proxy-pass=Pass"
        );
    }
}
