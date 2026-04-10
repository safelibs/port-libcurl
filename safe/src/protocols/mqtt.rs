pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/mqtt.c"];

pub(crate) fn matches(scheme: &str) -> bool {
    scheme == "mqtt"
}
