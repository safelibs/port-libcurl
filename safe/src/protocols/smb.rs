pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/smb.c"];

pub(crate) fn matches(scheme: &str) -> bool {
    matches!(scheme, "smb" | "smbs")
}
