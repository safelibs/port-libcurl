use crate::http::response::split_header_line;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HstsEntry {
    pub host: String,
    pub include_subdomains: bool,
    pub expires: i64,
    pub expire_text: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct HstsStore {
    entries: Vec<HstsEntry>,
}

impl HstsStore {
    pub(crate) fn remember(&mut self, host: &str, include_subdomains: bool, expires: i64) {
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|entry| entry.host.eq_ignore_ascii_case(host))
        {
            if expires > existing.expires {
                existing.expires = expires;
            }
            existing.include_subdomains |= include_subdomains;
            existing.expire_text = None;
            return;
        }
        self.entries.push(HstsEntry {
            host: host.to_ascii_lowercase(),
            include_subdomains,
            expires,
            expire_text: None,
        });
    }

    pub(crate) fn remember_callback_entry(
        &mut self,
        host: &str,
        include_subdomains: bool,
        expire_text: &str,
    ) {
        self.entries
            .retain(|entry| !entry.host.eq_ignore_ascii_case(host));
        self.entries.push(HstsEntry {
            host: host.to_ascii_lowercase(),
            include_subdomains,
            expires: 0,
            expire_text: (!expire_text.is_empty()).then(|| expire_text.to_string()),
        });
    }

    pub(crate) fn lookup(&self, host: &str) -> Option<&HstsEntry> {
        let host = host.to_ascii_lowercase();
        if let Some(exact) = self
            .entries
            .iter()
            .find(|entry| entry.host.eq_ignore_ascii_case(&host))
        {
            return Some(exact);
        }

        self.entries
            .iter()
            .filter(|entry| {
                entry.include_subdomains
                    && host.len() > entry.host.len()
                    && host.ends_with(&entry.host)
                    && host.as_bytes()[host.len() - entry.host.len() - 1] == b'.'
            })
            .max_by_key(|entry| entry.host.len())
    }

    pub(crate) fn entries(&self) -> &[HstsEntry] {
        &self.entries
    }
}

pub(crate) fn record_from_header(store: &mut HstsStore, url_host: &str, line: &str) {
    let Some((name, value)) = split_header_line(line) else {
        return;
    };
    if !name.eq_ignore_ascii_case("strict-transport-security") {
        return;
    }

    let mut max_age = None;
    let mut include_subdomains = false;
    for segment in value.split(';') {
        let trimmed = segment.trim();
        if trimmed.eq_ignore_ascii_case("includesubdomains") {
            include_subdomains = true;
            continue;
        }
        let Some((attr_name, attr_value)) = trimmed.split_once('=') else {
            continue;
        };
        if attr_name.trim().eq_ignore_ascii_case("max-age") {
            max_age = attr_value.trim().parse::<i64>().ok();
        }
    }

    if let Some(max_age) = max_age {
        store.remember(url_host, include_subdomains, max_age);
    }
}

#[cfg(test)]
mod tests {
    use super::HstsStore;

    #[test]
    fn hsts_prefers_exact_match_before_parent_tailmatch() {
        let mut store = HstsStore::default();
        store.remember("example.com", true, 10);
        store.remember("a.example.com", true, 11);
        let match_entry = store.lookup("a.example.com").expect("hsts");
        assert_eq!(match_entry.host, "a.example.com");
    }

    #[test]
    fn hsts_prefers_longest_tailmatch() {
        let mut store = HstsStore::default();
        store.remember("example.com", true, 10);
        store.remember("b.example.com", true, 10);
        let match_entry = store.lookup("c.b.example.com").expect("hsts");
        assert_eq!(match_entry.host, "b.example.com");
    }
}
