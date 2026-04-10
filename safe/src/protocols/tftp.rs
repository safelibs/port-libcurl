pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/tftp.c"];

pub(crate) fn matches(scheme: &str) -> bool {
    scheme == "tftp"
}
