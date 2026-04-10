pub(crate) const UPSTREAM_SOURCES: &[&str] = &[
    "original/lib/vssh/libssh.c",
    "original/lib/vssh/libssh2.c",
    "original/lib/vssh/wolfssh.c",
];

pub(crate) fn is_ssh_scheme(scheme: &str) -> bool {
    matches!(scheme, "scp" | "sftp")
}
