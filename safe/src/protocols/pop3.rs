pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/pop3.c"];

pub(crate) fn matches(scheme: &str) -> bool {
    matches!(scheme, "pop3" | "pop3s")
}
