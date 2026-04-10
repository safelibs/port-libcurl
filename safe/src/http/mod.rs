pub(crate) mod altsvc;
pub(crate) mod auth;
pub(crate) mod cookies;
pub(crate) mod headers_api;
pub(crate) mod hsts;
pub(crate) mod proxy;
pub(crate) mod request;
pub(crate) mod response;

#[derive(Clone, Default)]
pub(crate) struct HandleHttpState {
    pub headers: headers_api::HeaderStore,
    pub cookies: cookies::CookieStore,
    pub hsts: hsts::HstsStore,
    pub altsvc: altsvc::AltSvcCache,
}

impl HandleHttpState {
    pub(crate) fn clear_transient(&mut self) {
        self.headers.clear();
        self.altsvc.clear_runtime();
    }
}
