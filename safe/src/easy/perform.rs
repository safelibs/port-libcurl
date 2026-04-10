use crate::abi::{
    CURLcode, CURLoption, CURL, CURLE_BAD_FUNCTION_ARGUMENT, CURLE_FAILED_INIT, CURLM,
};
use crate::dns::{self, ConnectOverride, ResolveOverride};
use crate::multi::state::MultiState;
use crate::transfer::{map_multi_code, LowSpeedWindow, EASY_PERFORM_WAIT_TIMEOUT_MS};
use core::ffi::{c_char, c_int, c_long, c_void};
use std::collections::HashMap;
use std::ffi::CStr;
use std::sync::{Mutex, OnceLock};

const CURLOPT_URL: CURLoption = 10002;
const CURLOPT_PROXY: CURLoption = 10004;
const CURLOPT_USERPWD: CURLoption = 10005;
const CURLOPT_PROXYUSERPWD: CURLoption = 10006;
const CURLOPT_CUSTOMREQUEST: CURLoption = 10036;
const CURLOPT_LOW_SPEED_LIMIT: CURLoption = 19;
const CURLOPT_LOW_SPEED_TIME: CURLoption = 20;
const CURLOPT_HEADER: CURLoption = 42;
const CURLOPT_NOBODY: CURLoption = 44;
const CURLOPT_UPLOAD: CURLoption = 46;
const CURLOPT_FOLLOWLOCATION: CURLoption = 52;
const CURLOPT_PROXYPORT: CURLoption = 59;
const CURLOPT_HTTPPROXYTUNNEL: CURLoption = 61;
const CURLOPT_SSL_VERIFYPEER: CURLoption = 64;
const CURLOPT_MAXCONNECTS: CURLoption = 71;
const CURLOPT_HTTPGET: CURLoption = 80;
const CURLOPT_SSL_VERIFYHOST: CURLoption = 81;
const CURLOPT_SHARE: CURLoption = 10100;
const CURLOPT_CONNECT_ONLY: CURLoption = 141;
const CURLOPT_USERNAME: CURLoption = 10173;
const CURLOPT_PASSWORD: CURLoption = 10174;
const CURLOPT_PROXYUSERNAME: CURLoption = 10175;
const CURLOPT_PROXYPASSWORD: CURLoption = 10176;
const CURLOPT_RESOLVE: CURLoption = 10203;
const CURLOPT_XOAUTH2_BEARER: CURLoption = 10220;
const CURLOPT_PINNEDPUBLICKEY: CURLoption = 10230;
const CURLOPT_CONNECT_TO: CURLoption = 10243;
const CURLOPT_PRE_PROXY: CURLoption = 10262;

#[derive(Clone, Debug)]
pub(crate) struct EasyMetadata {
    pub url: Option<String>,
    pub custom_request: Option<String>,
    pub resolve_overrides: Vec<ResolveOverride>,
    pub connect_overrides: Vec<ConnectOverride>,
    pub proxy: Option<String>,
    pub pre_proxy: Option<String>,
    pub proxy_port: Option<u16>,
    pub tunnel_proxy: bool,
    pub share_handle: Option<usize>,
    pub userpwd: Option<String>,
    pub proxy_userpwd: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub proxy_username: Option<String>,
    pub proxy_password: Option<String>,
    pub xoauth2_bearer: Option<String>,
    pub pinned_public_key: Option<String>,
    pub ssl_verify_peer: bool,
    pub ssl_verify_host: c_long,
    pub connect_only: bool,
    pub follow_location: bool,
    pub header: bool,
    pub nobody: bool,
    pub upload: bool,
    pub http_get: bool,
    pub low_speed: LowSpeedWindow,
    pub maxconnects: Option<c_long>,
}

impl EasyMetadata {
    pub(crate) fn tls_peer_identity(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(pinned_key) = self.pinned_public_key.as_ref() {
            parts.push(format!("pinned={pinned_key}"));
        }
        parts.push(format!("verify_peer={}", self.ssl_verify_peer));
        parts.push(format!("verify_host={}", self.ssl_verify_host));
        Some(parts.join(";"))
    }

    pub(crate) fn auth_context(&self) -> Option<String> {
        let mut parts = Vec::new();
        push_auth_part(&mut parts, "userpwd", self.userpwd.as_deref());
        push_auth_part(&mut parts, "proxy_userpwd", self.proxy_userpwd.as_deref());
        push_auth_part(&mut parts, "username", self.username.as_deref());
        push_auth_part(&mut parts, "password", self.password.as_deref());
        push_auth_part(
            &mut parts,
            "proxy_username",
            self.proxy_username.as_deref(),
        );
        push_auth_part(
            &mut parts,
            "proxy_password",
            self.proxy_password.as_deref(),
        );
        push_auth_part(
            &mut parts,
            "bearer",
            self.xoauth2_bearer.as_deref(),
        );
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(";"))
        }
    }
}

impl Default for EasyMetadata {
    fn default() -> Self {
        Self {
            url: None,
            custom_request: None,
            resolve_overrides: Vec::new(),
            connect_overrides: Vec::new(),
            proxy: None,
            pre_proxy: None,
            proxy_port: None,
            tunnel_proxy: false,
            share_handle: None,
            userpwd: None,
            proxy_userpwd: None,
            username: None,
            password: None,
            proxy_username: None,
            proxy_password: None,
            xoauth2_bearer: None,
            pinned_public_key: None,
            ssl_verify_peer: true,
            ssl_verify_host: 2,
            connect_only: false,
            follow_location: false,
            header: false,
            nobody: false,
            upload: false,
            http_get: false,
            low_speed: LowSpeedWindow::default(),
            maxconnects: None,
        }
    }
}

#[derive(Clone, Debug)]
struct EasyShadow {
    private_multi: Option<usize>,
    attached_multi: Option<usize>,
    metadata: EasyMetadata,
    state: MultiState,
}

impl Default for EasyShadow {
    fn default() -> Self {
        Self {
            private_multi: None,
            attached_multi: None,
            metadata: EasyMetadata::default(),
            state: MultiState::Init,
        }
    }
}

fn registry() -> &'static Mutex<HashMap<usize, EasyShadow>> {
    static REGISTRY: OnceLock<Mutex<HashMap<usize, EasyShadow>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn register_handle(handle: *mut CURL) {
    if handle.is_null() {
        return;
    }
    registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .entry(handle as usize)
        .or_default();
}

pub(crate) fn register_duplicate(source: *mut CURL, duplicate: *mut CURL) {
    if duplicate.is_null() {
        return;
    }

    let shadow = registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get(&(source as usize))
        .cloned()
        .unwrap_or_default();

    registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .insert(
            duplicate as usize,
            EasyShadow {
                private_multi: None,
                attached_multi: None,
                metadata: shadow.metadata,
                state: MultiState::Init,
            },
        );
}

pub(crate) fn reset_handle(handle: *mut CURL) {
    if handle.is_null() {
        return;
    }
    if let Some(shadow) = registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get_mut(&(handle as usize))
    {
        shadow.metadata = EasyMetadata::default();
        shadow.state = MultiState::Init;
    }
}

pub(crate) fn unregister_handle(handle: *mut CURL) -> Option<usize> {
    if handle.is_null() {
        return None;
    }

    registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .remove(&(handle as usize))
        .and_then(|shadow| shadow.private_multi)
}

pub(crate) fn observe_easy_setopt_long(handle: *mut CURL, option: CURLoption, value: c_long) {
    if handle.is_null() {
        return;
    }

    let mut guard = registry().lock().expect("easy registry mutex poisoned");
    let metadata = &mut guard.entry(handle as usize).or_default().metadata;
    match option {
        CURLOPT_MAXCONNECTS => metadata.maxconnects = Some(value),
        CURLOPT_CONNECT_ONLY => metadata.connect_only = value != 0,
        CURLOPT_LOW_SPEED_LIMIT => metadata.low_speed.limit_bytes_per_second = value,
        CURLOPT_LOW_SPEED_TIME => metadata.low_speed.time_window_secs = value,
        CURLOPT_HEADER => metadata.header = value != 0,
        CURLOPT_NOBODY => metadata.nobody = value != 0,
        CURLOPT_UPLOAD => metadata.upload = value != 0,
        CURLOPT_FOLLOWLOCATION => metadata.follow_location = value != 0,
        CURLOPT_HTTPGET => metadata.http_get = value != 0,
        CURLOPT_PROXYPORT => metadata.proxy_port = u16::try_from(value).ok(),
        CURLOPT_HTTPPROXYTUNNEL => metadata.tunnel_proxy = value != 0,
        CURLOPT_SSL_VERIFYPEER => metadata.ssl_verify_peer = value != 0,
        CURLOPT_SSL_VERIFYHOST => metadata.ssl_verify_host = value,
        _ => {}
    }
}

pub(crate) fn observe_easy_setopt_ptr(handle: *mut CURL, option: CURLoption, value: *mut c_void) {
    if handle.is_null() {
        return;
    }

    let mut guard = registry().lock().expect("easy registry mutex poisoned");
    let metadata = &mut guard.entry(handle as usize).or_default().metadata;
    match option {
        CURLOPT_URL => metadata.url = copy_c_string(value.cast()),
        CURLOPT_CUSTOMREQUEST => metadata.custom_request = copy_c_string(value.cast()),
        CURLOPT_PROXY => metadata.proxy = copy_c_string(value.cast()),
        CURLOPT_PRE_PROXY => metadata.pre_proxy = copy_c_string(value.cast()),
        CURLOPT_USERPWD => metadata.userpwd = copy_c_string(value.cast()),
        CURLOPT_PROXYUSERPWD => metadata.proxy_userpwd = copy_c_string(value.cast()),
        CURLOPT_USERNAME => metadata.username = copy_c_string(value.cast()),
        CURLOPT_PASSWORD => metadata.password = copy_c_string(value.cast()),
        CURLOPT_PROXYUSERNAME => metadata.proxy_username = copy_c_string(value.cast()),
        CURLOPT_PROXYPASSWORD => metadata.proxy_password = copy_c_string(value.cast()),
        CURLOPT_XOAUTH2_BEARER => metadata.xoauth2_bearer = copy_c_string(value.cast()),
        CURLOPT_PINNEDPUBLICKEY => metadata.pinned_public_key = copy_c_string(value.cast()),
        CURLOPT_SHARE => metadata.share_handle = (!value.is_null()).then_some(value as usize),
        CURLOPT_RESOLVE => metadata.resolve_overrides = dns::collect_resolve_overrides(value.cast()),
        CURLOPT_CONNECT_TO => {
            metadata.connect_overrides = dns::collect_connect_overrides(value.cast())
        }
        _ => {}
    }
}

pub(crate) fn on_attached(handle: *mut CURL, multi: usize, next_state: MultiState) {
    if handle.is_null() {
        return;
    }

    let mut guard = registry().lock().expect("easy registry mutex poisoned");
    let shadow = guard.entry(handle as usize).or_default();
    shadow.attached_multi = Some(multi);
    shadow.state = MultiState::transition(shadow.state, next_state);
}

pub(crate) fn on_detached(handle: *mut CURL, multi: usize, next_state: MultiState) {
    if handle.is_null() {
        return;
    }

    let mut guard = registry().lock().expect("easy registry mutex poisoned");
    if let Some(shadow) = guard.get_mut(&(handle as usize)) {
        if shadow.attached_multi == Some(multi) {
            shadow.attached_multi = None;
        }
        shadow.state = MultiState::transition(shadow.state, next_state);
    }
}

pub(crate) fn on_transfer_progress(handle: *mut CURL, next_state: MultiState) {
    if handle.is_null() {
        return;
    }

    let mut guard = registry().lock().expect("easy registry mutex poisoned");
    if let Some(shadow) = guard.get_mut(&(handle as usize)) {
        shadow.state = MultiState::transition(shadow.state, next_state);
    }
}

pub(crate) fn mark_message_sent(handle: *mut CURL) {
    on_transfer_progress(handle, MultiState::MsgSent);
}

pub(crate) fn snapshot_metadata(handle: *mut CURL) -> EasyMetadata {
    registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get(&(handle as usize))
        .map(|shadow| shadow.metadata.clone())
        .unwrap_or_default()
}

fn private_multi_for(handle: *mut CURL) -> Option<usize> {
    registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get(&(handle as usize))
        .and_then(|shadow| shadow.private_multi)
}

fn explicit_maxconnects_for(handle: *mut CURL) -> Option<c_long> {
    registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get(&(handle as usize))
        .and_then(|shadow| shadow.metadata.maxconnects)
}

pub(crate) fn attached_multi_for(handle: *mut CURL) -> Option<usize> {
    registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .get(&(handle as usize))
        .and_then(|shadow| shadow.attached_multi)
}

fn set_private_multi(handle: *mut CURL, multi: Option<usize>) {
    registry()
        .lock()
        .expect("easy registry mutex poisoned")
        .entry(handle as usize)
        .or_default()
        .private_multi = multi;
}

pub(crate) unsafe fn easy_perform(handle: *mut CURL) -> CURLcode {
    if handle.is_null() {
        return CURLE_BAD_FUNCTION_ARGUMENT;
    }
    register_handle(handle);

    if attached_multi_for(handle).is_some() {
        return CURLE_FAILED_INIT;
    }

    let mut created_multi = false;
    let multi = if let Some(existing) = private_multi_for(handle) {
        existing as *mut CURLM
    } else {
        let new_multi = unsafe { crate::multi::init_handle() };
        if new_multi.is_null() {
            return crate::abi::CURLE_OUT_OF_MEMORY;
        }
        set_private_multi(handle, Some(new_multi as usize));
        created_multi = true;
        new_multi
    };

    if let Some(maxconnects) = explicit_maxconnects_for(handle) {
        let _ = unsafe {
            crate::multi::dispatch_setopt_long(
                multi,
                crate::multi::CURLMOPT_MAXCONNECTS,
                maxconnects,
            )
        };
    }

    let add_code = unsafe { crate::multi::add_handle(multi, handle) };
    if add_code != crate::abi::CURLM_OK {
        if created_multi {
            let _ = unsafe { crate::multi::cleanup_handle(multi) };
            set_private_multi(handle, None);
        }
        return if add_code == crate::multi::CURLM_OUT_OF_MEMORY {
            crate::abi::CURLE_OUT_OF_MEMORY
        } else {
            CURLE_FAILED_INIT
        };
    }

    let mut result = crate::abi::CURLE_OK;
    loop {
        let poll_code = unsafe {
            crate::multi::poll_handle(
                multi,
                core::ptr::null_mut(),
                0,
                EASY_PERFORM_WAIT_TIMEOUT_MS,
                core::ptr::null_mut(),
            )
        };
        if poll_code != crate::abi::CURLM_OK {
            result = map_multi_code(poll_code);
            break;
        }

        let mut still_running = 0;
        let perform_code = unsafe { crate::multi::perform_handle(multi, &mut still_running) };
        if perform_code != crate::abi::CURLM_OK {
            result = map_multi_code(perform_code);
            break;
        }

        if still_running == 0 {
            let mut queued = 0;
            let msg = unsafe { crate::multi::info_read_handle(multi, &mut queued) };
            if !msg.is_null() && unsafe { (*msg).msg == crate::multi::CURLMSG_DONE } {
                result = unsafe { (*msg).data.result };
            }
            break;
        }
    }

    let _ = unsafe { crate::multi::remove_handle(multi, handle) };
    result
}

pub(crate) unsafe fn easy_pause(handle: *mut CURL, bitmask: c_int) -> CURLcode {
    unsafe { crate::transfer::easy_pause_backend(handle, bitmask) }
}

pub(crate) unsafe fn easy_recv(
    handle: *mut CURL,
    buffer: *mut c_void,
    buflen: usize,
    nread: *mut usize,
) -> CURLcode {
    unsafe { crate::transfer::easy_recv_backend(handle, buffer, buflen, nread) }
}

pub(crate) unsafe fn easy_send(
    handle: *mut CURL,
    buffer: *const c_void,
    buflen: usize,
    nwritten: *mut usize,
) -> CURLcode {
    unsafe { crate::transfer::easy_send_backend(handle, buffer, buflen, nwritten) }
}

pub(crate) unsafe fn easy_upkeep(handle: *mut CURL) -> CURLcode {
    unsafe { crate::transfer::easy_upkeep_backend(handle) }
}

fn copy_c_string(value: *const c_char) -> Option<String> {
    if value.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(value) }.to_string_lossy().into_owned())
    }
}

fn push_auth_part(parts: &mut Vec<String>, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        parts.push(format!("{label}={value}"));
    }
}
