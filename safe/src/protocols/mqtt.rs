use crate::abi::{CURLcode, CURL};
use crate::easy::perform::{EasyCallbacks, EasyMetadata};
use crate::protocols::ParsedProtocolUrl;
use crate::transfer::{self, TransferPlan};
use std::time::{Duration, Instant};

pub(crate) const UPSTREAM_SOURCES: &[&str] = &["original/lib/mqtt.c"];

const CURLE_URL_MALFORMAT: CURLcode = 3;
const CURLE_COULDNT_CONNECT: CURLcode = 7;
const CURLE_SEND_ERROR: CURLcode = 55;

const IO_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) fn matches(scheme: &str) -> bool {
    scheme == "mqtt"
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
    let connect_packet = [
        0x10, 0x17, 0x00, 0x04, b'M', b'Q', b'T', b'T', 0x04, 0x02, 0x00, 0x0a, 0x00, 0x0b, b'p',
        b'o', b'r', b't', b'-', b'l', b'i', b'b', b'c', b'u', b'r', b'l',
    ];
    let disconnect_packet = [0xe0, 0x00];
    let code = match std::io::Write::write_all(&mut stream.stream, &connect_packet)
        .and_then(|_| std::io::Write::write_all(&mut stream.stream, &disconnect_packet))
        .and_then(|_| std::io::Write::flush(&mut stream.stream))
    {
        Ok(()) => {
            let mut info = stream.info;
            info.pretransfer_time_us = info.connect_time_us;
            info.starttransfer_time_us = info.connect_time_us;
            info.total_time_us = transfer::elapsed_us(started.elapsed());
            crate::easy::perform::record_transfer_info(handle, info);
            crate::abi::CURLE_OK
        }
        Err(_) => CURLE_SEND_ERROR,
    };
    transfer::close_transport(stream.stream, callbacks);
    code
}
