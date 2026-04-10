pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/rtsp.c"];

pub(crate) fn matches(scheme: &str) -> bool {
    scheme == "rtsp"
}
