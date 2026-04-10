use crate::conn::cache::parse_url_authority;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Origin {
    pub scheme: String,
    pub host: String,
    pub port: u16,
}

impl Origin {
    pub(crate) fn from_url(url: &str) -> Option<Self> {
        let parsed = parse_url_authority(url)?;
        Some(Self {
            scheme: parsed.scheme,
            host: parsed.host,
            port: parsed.port,
        })
    }

    pub(crate) fn same_origin(&self, other: &Self) -> bool {
        self == other
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RedirectPolicy {
    pub enabled: bool,
    pub max_redirs: usize,
    pub unrestricted_auth: bool,
    pub auto_referer: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RedirectDecision {
    pub next_url: String,
    pub allow_cross_origin_auth: bool,
    pub referer: Option<String>,
}

pub(crate) fn resolve_redirect_target(current_url: &str, location: &str) -> Option<String> {
    if location.contains("://") {
        return Some(location.to_string());
    }

    let (scheme, rest) = current_url.split_once("://")?;
    let trimmed = rest.split('#').next().unwrap_or(rest);
    let path_start = trimmed.find(['/', '?']).unwrap_or(trimmed.len());
    let authority = &trimmed[..path_start];
    if location.starts_with('/') {
        return Some(format!("{scheme}://{authority}{location}"));
    }

    let base = current_url
        .rsplit_once('/')
        .map(|(base, _)| base)
        .unwrap_or(current_url);
    Some(format!("{base}/{location}"))
}

pub(crate) fn strip_credentials_and_fragment(url: &str) -> String {
    let fragmentless = url.split('#').next().unwrap_or(url);
    let Some((scheme, rest)) = fragmentless.split_once("://") else {
        return fragmentless.to_string();
    };
    let without_userinfo = rest.rsplit_once('@').map(|(_, tail)| tail).unwrap_or(rest);
    format!("{scheme}://{without_userinfo}")
}

pub(crate) fn decide_redirect(
    current_url: &str,
    status_code: u16,
    location: Option<&str>,
    redirect_count: usize,
    policy: RedirectPolicy,
    initial_origin: Option<&Origin>,
) -> Option<RedirectDecision> {
    if !policy.enabled || redirect_count >= policy.max_redirs {
        return None;
    }
    if !matches!(status_code, 301 | 302 | 303 | 307 | 308) {
        return None;
    }

    let next_url = resolve_redirect_target(current_url, location?)?;
    let allow_cross_origin_auth = if policy.unrestricted_auth {
        true
    } else if let Some(initial_origin) = initial_origin {
        let next_origin = Origin::from_url(&next_url)?;
        initial_origin.same_origin(&next_origin)
    } else {
        false
    };
    let referer = policy
        .auto_referer
        .then(|| strip_credentials_and_fragment(current_url));
    Some(RedirectDecision {
        next_url,
        allow_cross_origin_auth,
        referer,
    })
}

#[cfg(test)]
mod tests {
    use super::{decide_redirect, strip_credentials_and_fragment, Origin, RedirectPolicy};

    #[test]
    fn strips_credentials_from_referer() {
        assert_eq!(
            strip_credentials_and_fragment("http://user:pass@example.test/path?a=1#frag"),
            "http://example.test/path?a=1"
        );
    }

    #[test]
    fn cross_origin_redirect_blocks_auth_by_default() {
        let initial = Origin::from_url("http://a.test/path").expect("origin");
        let decision = decide_redirect(
            "http://a.test/path",
            302,
            Some("http://b.test/next"),
            0,
            RedirectPolicy {
                enabled: true,
                max_redirs: 8,
                unrestricted_auth: false,
                auto_referer: true,
            },
            Some(&initial),
        )
        .expect("redirect");
        assert!(!decision.allow_cross_origin_auth);
        assert_eq!(decision.referer.as_deref(), Some("http://a.test/path"));
    }
}
