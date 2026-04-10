pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/smtp.c"];

pub(crate) fn matches(scheme: &str) -> bool {
    matches!(scheme, "smtp" | "smtps")
}
