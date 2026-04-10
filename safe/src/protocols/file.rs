pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/file.c"];

pub(crate) fn matches(scheme: &str) -> bool {
    scheme == "file"
}
