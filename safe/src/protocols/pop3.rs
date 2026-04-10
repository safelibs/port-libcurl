use crate::abi::{CURLcode, CURL};
use crate::easy::perform::{EasyCallbacks, EasyMetadata};
use crate::protocols::ParsedProtocolUrl;
use crate::transfer::{self, LowSpeedGuard, TransferPlan};
use std::io::{ErrorKind, Write};
use std::time::{Duration, Instant};

pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/pop3.c"];

const CURLE_URL_MALFORMAT: CURLcode = 3;
const CURLE_COULDNT_CONNECT: CURLcode = 7;
const CURLE_SEND_ERROR: CURLcode = 55;

const IO_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) fn matches(scheme: &str) -> bool {
    matches!(scheme, "pop3" | "pop3s")
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
    let started = Instant::now();
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
    let mut info = stream.info.clone();
    info.pretransfer_time_us = info.connect_time_us;
    info.starttransfer_time_us = info.connect_time_us;
    let result = stream
        .stream
        .write_all(b"CAPA\r\nQUIT\r\n")
        .and_then(|_| stream.stream.flush())
        .map_err(|error| match error.kind() {
            ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::BrokenPipe => CURLE_SEND_ERROR,
            _ => CURLE_SEND_ERROR,
        })
        .and_then(|_| {
            let mut low_speed = LowSpeedGuard::new(plan.low_speed);
            transfer::invoke_progress_callback(callbacks, 0, None)?;
            transfer::transfer_body(
                &mut stream.stream,
                handle,
                callbacks,
                Vec::new(),
                None,
                &mut low_speed,
            )
        });
    let code = match result {
        Ok(()) => {
            info.total_time_us = transfer::elapsed_us(started.elapsed());
            crate::easy::perform::record_transfer_info(handle, info);
            crate::abi::CURLE_OK
        }
        Err(code) => code,
    };
    transfer::close_transport(stream.stream, callbacks);
    code
}
