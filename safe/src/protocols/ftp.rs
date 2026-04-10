pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/ftp.c"];

pub(crate) fn matches(scheme: &str) -> bool {
    matches!(scheme, "ftp" | "ftps")
}
