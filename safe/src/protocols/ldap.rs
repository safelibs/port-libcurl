pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/ldap.c", "original/lib/openldap.c"];

pub(crate) fn matches(scheme: &str) -> bool {
    matches!(scheme, "ldap" | "ldaps")
}
