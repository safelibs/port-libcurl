use crate::abi::{CURLcode, CURL};
use crate::easy::perform::{EasyCallbacks, EasyMetadata};
use crate::protocols::ParsedProtocolUrl;
use crate::transfer::{self, TransferPlan, TransportStream};
use std::io::{ErrorKind, Read, Write};
use std::time::Duration;

pub(crate) const UPSTREAM_SOURCES: &[&str] = &[
    "original/lib/vssh/libssh.c",
    "original/lib/vssh/libssh2.c",
    "original/lib/vssh/wolfssh.c",
];

const CURLE_URL_MALFORMAT: CURLcode = 3;
const CURLE_COULDNT_CONNECT: CURLcode = 7;
const CURLE_SEND_ERROR: CURLcode = 55;
const CURLE_RECV_ERROR: CURLcode = 56;

const IO_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) fn is_ssh_scheme(scheme: &str) -> bool {
    matches!(scheme, "scp" | "sftp")
}

pub(crate) fn perform_transfer(
    handle: *mut CURL,
    plan: &TransferPlan,
    metadata: &EasyMetadata,
    callbacks: EasyCallbacks,
) -> CURLcode {
    let Some(url) = metadata.url.as_deref() else {
        crate::easy::perform::set_error_buffer(handle, "No URL set");
        return CURLE_URL_MALFORMAT;
    };
    let parsed = match ParsedProtocolUrl::parse(url) {
        Ok(parsed) => parsed,
        Err(code) => return code,
    };
    let mut stream = match transfer::connect_protocol_transport(
        &parsed.host,
        parsed.port,
        plan,
        metadata,
        callbacks,
    ) {
        Ok(stream) => stream,
        Err(code) => return code,
    };
    if stream
        .stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .and_then(|_| stream.stream.set_write_timeout(Some(IO_TIMEOUT)))
        .is_err()
    {
        transfer::close_transport(stream.stream, callbacks);
        return CURLE_COULDNT_CONNECT;
    }

    let code = perform_banner_exchange(&mut stream.stream)
        .and_then(|_| {
            crate::easy::perform::set_error_buffer(
                handle,
                "SCP/SFTP packet handling is not implemented in the shared engine",
            );
            Err(crate::protocols::unsupported(
                handle,
                "SCP/SFTP packet handling is not implemented in the shared engine",
            ))
        })
        .unwrap_or_else(|code| code);
    transfer::close_transport(stream.stream, callbacks);
    code
}

fn perform_banner_exchange(stream: &mut TransportStream) -> Result<(), CURLcode> {
    stream
        .write_all(b"SSH-2.0-port-libcurl-safe\r\n")
        .and_then(|_| stream.flush())
        .map_err(|_| CURLE_SEND_ERROR)?;
    let mut banner = [0u8; 256];
    match stream.read(&mut banner) {
        Ok(0) => Err(CURLE_RECV_ERROR),
        Ok(_) => Ok(()),
        Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
            Err(CURLE_COULDNT_CONNECT)
        }
        Err(_) => Err(CURLE_RECV_ERROR),
    }
}
