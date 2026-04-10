use crate::abi::{curl_certinfo, curl_slist, CURL};
use core::ptr;
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::{Mutex, OnceLock};

pub(crate) const UPSTREAM_SOURCES: &[&str] = &[
    "original/lib/vtls/openssl.c",
    "original/lib/vtls/gtls.c",
    "original/lib/vtls/x509asn1.c",
];

pub(crate) type OwnedCertInfo = Vec<Vec<String>>;

struct StoredCertInfo {
    raw: Box<curl_certinfo>,
    lists: Box<[*mut curl_slist]>,
}

unsafe impl Send for StoredCertInfo {}

impl StoredCertInfo {
    fn new(certs: OwnedCertInfo) -> Option<Self> {
        let mut lists = Vec::with_capacity(certs.len());
        for cert in certs {
            let mut list = ptr::null_mut();
            for entry in cert {
                let entry = match CString::new(entry) {
                    Ok(entry) => entry,
                    Err(_) => {
                        cleanup_lists(&mut lists);
                        unsafe { crate::slist::curl_slist_free_all(list) };
                        return None;
                    }
                };
                let next = unsafe { crate::slist::curl_slist_append(list, entry.as_ptr()) };
                if next.is_null() {
                    cleanup_lists(&mut lists);
                    unsafe { crate::slist::curl_slist_free_all(list) };
                    return None;
                }
                list = next;
            }
            lists.push(list);
        }

        let mut lists = lists.into_boxed_slice();
        let mut raw = Box::new(curl_certinfo {
            num_of_certs: lists.len() as i32,
            certinfo: ptr::null_mut(),
        });
        raw.certinfo = lists.as_mut_ptr();
        Some(Self { raw, lists })
    }
}

impl Drop for StoredCertInfo {
    fn drop(&mut self) {
        for list in self.lists.iter_mut() {
            unsafe { crate::slist::curl_slist_free_all(*list) };
            *list = ptr::null_mut();
        }
    }
}

unsafe extern "C" {
    fn curl_safe_tls_certinfo(conn: *mut super::SafeTlsConnection, out_len: *mut usize) -> *mut u8;
}

fn registry() -> &'static Mutex<HashMap<usize, StoredCertInfo>> {
    static REGISTRY: OnceLock<Mutex<HashMap<usize, StoredCertInfo>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) const fn requested(enabled: bool) -> bool {
    enabled
}

pub(crate) fn capture(connection: &super::TlsConnection) -> Option<OwnedCertInfo> {
    let mut len = 0usize;
    let bytes = unsafe { curl_safe_tls_certinfo(connection.raw, &mut len) };
    if bytes.is_null() || len == 0 {
        if !bytes.is_null() {
            unsafe { super::curl_safe_tls_free_bytes(bytes) };
        }
        return None;
    }

    let serialized = unsafe { std::slice::from_raw_parts(bytes, len) };
    let parsed = parse(serialized);
    unsafe { super::curl_safe_tls_free_bytes(bytes) };
    parsed
}

pub(crate) fn store(handle: *mut CURL, certs: OwnedCertInfo) {
    if handle.is_null() {
        return;
    }

    let Some(stored) = StoredCertInfo::new(certs) else {
        return;
    };
    registry()
        .lock()
        .expect("certinfo registry mutex poisoned")
        .insert(handle as usize, stored);
}

pub(crate) fn lookup(handle: *mut CURL) -> Option<*mut curl_certinfo> {
    if handle.is_null() {
        return None;
    }

    registry()
        .lock()
        .expect("certinfo registry mutex poisoned")
        .get(&(handle as usize))
        .map(|stored| stored.raw.as_ref() as *const curl_certinfo as *mut curl_certinfo)
}

pub(crate) fn clear(handle: *mut CURL) {
    if handle.is_null() {
        return;
    }
    registry()
        .lock()
        .expect("certinfo registry mutex poisoned")
        .remove(&(handle as usize));
}

pub(crate) fn clear_all() {
    let mut guard = registry().lock().expect("certinfo registry mutex poisoned");
    *guard = HashMap::new();
}

fn cleanup_lists(lists: &mut Vec<*mut curl_slist>) {
    for list in lists.drain(..) {
        unsafe { crate::slist::curl_slist_free_all(list) };
    }
}

fn parse(serialized: &[u8]) -> Option<OwnedCertInfo> {
    let mut certs = Vec::<Vec<String>>::new();
    for line in String::from_utf8_lossy(serialized).lines() {
        if line.is_empty() {
            continue;
        }
        let Some((index, entry)) = line.split_once('\t') else {
            continue;
        };
        let Ok(index) = index.parse::<usize>() else {
            continue;
        };
        if certs.len() <= index {
            certs.resize_with(index + 1, Vec::new);
        }
        certs[index].push(entry.to_string());
    }
    (!certs.is_empty()).then_some(certs)
}
