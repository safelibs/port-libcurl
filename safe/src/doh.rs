pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/doh.c"];

pub(crate) fn requires_reference_backend(doh_url: Option<&str>) -> bool {
    doh_url.is_some_and(|value| !value.is_empty())
}
