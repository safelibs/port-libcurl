pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/gopher.c"];

pub(crate) fn matches(scheme: &str) -> bool {
    scheme == "gopher"
}
