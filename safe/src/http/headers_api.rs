use crate::abi::{curl_header, CURLHcode, CURL};
use crate::easy::perform;
use crate::http::response::{
    HEADER_ORIGIN_1XX, HEADER_ORIGIN_CONNECT, HEADER_ORIGIN_HEADER, HEADER_ORIGIN_PSEUDO,
    HEADER_ORIGIN_RESERVED_BIT, HEADER_ORIGIN_TRAILER,
};
use core::ffi::{c_char, c_void};
use core::mem;
use std::ffi::CString;

const CURLHE_OK: CURLHcode = 0;
const CURLHE_BADINDEX: CURLHcode = 1;
const CURLHE_MISSING: CURLHcode = 2;
const CURLHE_NOHEADERS: CURLHcode = 3;
const CURLHE_NOREQUEST: CURLHcode = 4;
const CURLHE_OUT_OF_MEMORY: CURLHcode = 5;
const CURLHE_BAD_ARGUMENT: CURLHcode = 6;

const VALID_HEADER_MASK: u32 = HEADER_ORIGIN_HEADER
    | HEADER_ORIGIN_TRAILER
    | HEADER_ORIGIN_CONNECT
    | HEADER_ORIGIN_1XX
    | HEADER_ORIGIN_PSEUDO;

#[derive(Clone, Debug)]
pub(crate) struct HeaderEntry {
    pub name_c: CString,
    pub value_c: CString,
    pub origin: u32,
    pub request: usize,
}

pub(crate) struct HeaderStore {
    entries: Vec<HeaderEntry>,
    latest_request: usize,
    scratch: [curl_header; 2],
}

impl Default for HeaderStore {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            latest_request: 0,
            scratch: [unsafe { mem::zeroed() }, unsafe { mem::zeroed() }],
        }
    }
}

impl Clone for HeaderStore {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            latest_request: self.latest_request,
            scratch: [unsafe { mem::zeroed() }, unsafe { mem::zeroed() }],
        }
    }
}

unsafe impl Send for HeaderStore {}

fn bytes_eq_ignore_ascii_case(lhs: &[u8], rhs: &[u8]) -> bool {
    lhs.len() == rhs.len()
        && lhs
            .iter()
            .zip(rhs.iter())
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
}

impl HeaderStore {
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.latest_request = 0;
        self.scratch = [unsafe { mem::zeroed() }, unsafe { mem::zeroed() }];
    }

    pub(crate) fn set_latest_request(&mut self, request: usize) {
        self.latest_request = request;
    }

    pub(crate) fn record(&mut self, request: usize, origin: u32, name: &str, value: &str) {
        let Ok(name_c) = CString::new(name) else {
            return;
        };
        let Ok(value_c) = CString::new(value) else {
            return;
        };
        self.latest_request = request;
        self.entries.push(HeaderEntry {
            name_c,
            value_c,
            origin,
            request,
        });
    }

    pub(crate) fn latest_values(&self, name: &str) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| {
                entry.request == self.latest_request
                    && bytes_eq_ignore_ascii_case(entry.name_c.as_bytes(), name.as_bytes())
            })
            .map(|entry| entry.value_c.to_string_lossy().into_owned())
            .collect()
    }

    fn request_index(&self, request: i32) -> Result<usize, CURLHcode> {
        if request < -1 {
            return Err(CURLHE_BAD_ARGUMENT);
        }
        if self.entries.is_empty() {
            return Err(CURLHE_NOHEADERS);
        }
        let request = if request == -1 {
            self.latest_request
        } else {
            request as usize
        };
        if request > self.latest_request {
            return Err(CURLHE_NOREQUEST);
        }
        Ok(request)
    }

    fn fill_slot(
        &mut self,
        slot: usize,
        index: usize,
        amount: usize,
        entry_index: usize,
    ) -> *mut curl_header {
        let entry = &self.entries[entry_index];
        self.scratch[slot] = curl_header {
            name: entry.name_c.as_ptr().cast::<c_char>().cast_mut(),
            value: entry.value_c.as_ptr().cast::<c_char>().cast_mut(),
            amount,
            index,
            origin: entry.origin | HEADER_ORIGIN_RESERVED_BIT,
            anchor: (entry_index + 1) as *mut c_void,
        };
        &mut self.scratch[slot]
    }

    pub(crate) fn header(
        &mut self,
        name: &str,
        index: usize,
        origin: u32,
        request: i32,
    ) -> Result<*mut curl_header, CURLHcode> {
        if origin == 0 || origin & !VALID_HEADER_MASK != 0 {
            return Err(CURLHE_BAD_ARGUMENT);
        }
        let request = self.request_index(request)?;
        let matches = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry.request == request
                    && (entry.origin & origin) != 0
                    && bytes_eq_ignore_ascii_case(entry.name_c.as_bytes(), name.as_bytes())
            })
            .collect::<Vec<_>>();
        if matches.is_empty() {
            return Err(CURLHE_MISSING);
        }
        if index >= matches.len() {
            return Err(CURLHE_BADINDEX);
        }
        let (entry_index, _) = matches[index];
        Ok(self.fill_slot(0, index, matches.len(), entry_index))
    }

    pub(crate) fn next_header(
        &mut self,
        origin: u32,
        request: i32,
        prev_anchor: Option<usize>,
    ) -> Option<*mut curl_header> {
        if origin == 0 || origin & !VALID_HEADER_MASK != 0 {
            return None;
        }
        let request = self.request_index(request).ok()?;
        let start = prev_anchor.unwrap_or(0);
        let entry_index = self
            .entries
            .iter()
            .enumerate()
            .skip(start)
            .find(|(_, entry)| entry.request == request && (entry.origin & origin) != 0)
            .map(|(index, _)| index)?;
        let amount = self
            .entries
            .iter()
            .filter(|entry| {
                entry.request == request
                    && (entry.origin & origin) != 0
                    && entry.name_c.as_bytes() == self.entries[entry_index].name_c.as_bytes()
            })
            .count();
        let index = self
            .entries
            .iter()
            .take(entry_index + 1)
            .filter(|entry| {
                entry.request == request
                    && (entry.origin & origin) != 0
                    && entry.name_c.as_bytes() == self.entries[entry_index].name_c.as_bytes()
            })
            .count()
            - 1;
        Some(self.fill_slot(1, index, amount, entry_index))
    }
}

#[no_mangle]
pub unsafe extern "C" fn curl_easy_header(
    easy: *mut CURL,
    name: *const c_char,
    index: usize,
    origin: u32,
    request: i32,
    hout: *mut *mut curl_header,
) -> CURLHcode {
    if easy.is_null() || name.is_null() || hout.is_null() {
        return CURLHE_BAD_ARGUMENT;
    }
    match perform::with_http_state_mut(easy, |state| {
        let Ok(name) = unsafe { std::ffi::CStr::from_ptr(name) }.to_str() else {
            return Err(CURLHE_BAD_ARGUMENT);
        };
        state.headers.header(name, index, origin, request)
    }) {
        Some(Ok(header)) => {
            unsafe { *hout = header };
            CURLHE_OK
        }
        Some(Err(code)) => code,
        None => CURLHE_OUT_OF_MEMORY,
    }
}

#[no_mangle]
pub unsafe extern "C" fn curl_easy_nextheader(
    easy: *mut CURL,
    origin: u32,
    request: i32,
    prev: *mut curl_header,
) -> *mut curl_header {
    if easy.is_null() {
        return core::ptr::null_mut();
    }
    let prev_anchor = (!prev.is_null()).then(|| unsafe { (*prev).anchor as usize });
    let start = prev_anchor
        .and_then(|value| value.checked_sub(1))
        .map(|value| value + 1);
    perform::with_http_state_mut(easy, |state| {
        state.headers.next_header(origin, request, start)
    })
    .flatten()
    .unwrap_or(core::ptr::null_mut())
}

#[cfg(test)]
mod tests {
    use super::{HeaderStore, HEADER_ORIGIN_HEADER};

    #[test]
    fn header_lookup_preserves_amount_and_index() {
        let mut store = HeaderStore::default();
        store.record(0, HEADER_ORIGIN_HEADER, "Set-Cookie", "a=1");
        store.record(0, HEADER_ORIGIN_HEADER, "Set-Cookie", "b=2");
        let header = store
            .header("set-cookie", 1, HEADER_ORIGIN_HEADER, -1)
            .expect("header");
        assert_eq!(unsafe { (*header).amount }, 2);
        assert_eq!(unsafe { (*header).index }, 1);
    }
}
