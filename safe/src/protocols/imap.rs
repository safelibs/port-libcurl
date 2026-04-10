pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/imap.c"];

pub(crate) fn matches(scheme: &str) -> bool {
    matches!(scheme, "imap" | "imaps")
}
