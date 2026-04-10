pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/idn.c"];

pub(crate) fn requires_reference_backend(url: &str) -> bool {
    !url.is_ascii()
}
