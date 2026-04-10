pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/telnet.c"];

pub(crate) fn matches(scheme: &str) -> bool {
    scheme == "telnet"
}
