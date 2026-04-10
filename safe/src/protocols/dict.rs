pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/dict.c"];

pub(crate) fn matches(scheme: &str) -> bool {
    scheme == "dict"
}
