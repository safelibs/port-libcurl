pub type size_t = usize;
pub type time_t = c_long;
pub type curl_socket_t = c_int;

pub type CURLcode = u32;
pub type CURLversion = u32;
pub type CURLoption = u32;
pub type CURLINFO = u32;
pub type CURLMcode = i32;
pub type CURLMoption = u32;
pub type CURLFORMcode = u32;
pub type CURLformoption = u32;
pub type CURLHcode = u32;
pub type CURLUcode = u32;
pub type CURLUPart = u32;
pub type CURLSHcode = u32;
pub type CURLSHoption = u32;
pub type CURLsslset = u32;
pub type CURLMSG = u32;
pub type CURLSTScode = u32;
pub type curl_easytype = u32;
pub type curl_sslbackend = u32;
pub type curlfiletype = u32;
pub type curl_khtype = u32;
pub type curl_lock_data = u32;
pub type curl_lock_access = u32;

pub const CURLE_OK: CURLcode = 0;
pub const CURLE_FAILED_INIT: CURLcode = 2;
pub const CURLE_URL_MALFORMAT: CURLcode = 3;
pub const CURLE_NOT_BUILT_IN: CURLcode = 4;
pub const CURLE_OUT_OF_MEMORY: CURLcode = 27;
pub const CURLE_BAD_FUNCTION_ARGUMENT: CURLcode = 43;
pub const CURLE_UNKNOWN_OPTION: CURLcode = 48;

pub const CURLM_OK: CURLMcode = 0;
pub const CURLM_UNKNOWN_OPTION: CURLMcode = 6;

pub const CURLSHE_OK: CURLSHcode = 0;
pub const CURLSHE_BAD_OPTION: CURLSHcode = 1;

pub const CURLUE_OK: CURLUcode = 0;
pub const CURLUE_OUT_OF_MEMORY: CURLUcode = 7;

pub const CURLSSLSET_OK: CURLsslset = 0;
pub const CURLSSLSET_UNKNOWN_BACKEND: CURLsslset = 1;
pub const CURLSSLSET_TOO_LATE: CURLsslset = 2;
pub const CURLSSLSET_NO_BACKENDS: CURLsslset = 3;

pub const CURLVERSION_NOW: CURLversion = 10;

pub const CURL_GLOBAL_SSL: c_long = 1 << 0;
pub const CURL_GLOBAL_WIN32: c_long = 1 << 1;
pub const CURL_GLOBAL_ALL: c_long = CURL_GLOBAL_SSL | CURL_GLOBAL_WIN32;
pub const CURL_GLOBAL_DEFAULT: c_long = CURL_GLOBAL_ALL;

pub const CURL_LOCK_DATA_NONE: curl_lock_data = 0;
pub const CURL_LOCK_DATA_SHARE: curl_lock_data = 1;
pub const CURL_LOCK_DATA_COOKIE: curl_lock_data = 2;
pub const CURL_LOCK_DATA_DNS: curl_lock_data = 3;
pub const CURL_LOCK_DATA_SSL_SESSION: curl_lock_data = 4;
pub const CURL_LOCK_DATA_CONNECT: curl_lock_data = 5;
pub const CURL_LOCK_DATA_PSL: curl_lock_data = 6;
pub const CURL_LOCK_DATA_HSTS: curl_lock_data = 7;

pub const CURL_LOCK_ACCESS_NONE: curl_lock_access = 0;
pub const CURL_LOCK_ACCESS_SHARED: curl_lock_access = 1;
pub const CURL_LOCK_ACCESS_SINGLE: curl_lock_access = 2;

pub const CURLSHOPT_NONE: CURLSHoption = 0;
pub const CURLSHOPT_SHARE: CURLSHoption = 1;
pub const CURLSHOPT_UNSHARE: CURLSHoption = 2;
pub const CURLSHOPT_LOCKFUNC: CURLSHoption = 3;
pub const CURLSHOPT_UNLOCKFUNC: CURLSHoption = 4;
pub const CURLSHOPT_USERDATA: CURLSHoption = 5;

pub const CURLUPART_URL: CURLUPart = 0;
pub const CURLUPART_SCHEME: CURLUPart = 1;
pub const CURLUPART_USER: CURLUPart = 2;
pub const CURLUPART_PASSWORD: CURLUPart = 3;
pub const CURLUPART_OPTIONS: CURLUPart = 4;
pub const CURLUPART_HOST: CURLUPart = 5;
pub const CURLUPART_PORT: CURLUPart = 6;
pub const CURLUPART_PATH: CURLUPart = 7;
pub const CURLUPART_QUERY: CURLUPart = 8;
pub const CURLUPART_FRAGMENT: CURLUPart = 9;
pub const CURLUPART_ZONEID: CURLUPart = 10;

pub const CURLOT_LONG: curl_easytype = 0;
pub const CURLOT_VALUES: curl_easytype = 1;
pub const CURLOT_OFF_T: curl_easytype = 2;
pub const CURLOT_OBJECT: curl_easytype = 3;
pub const CURLOT_STRING: curl_easytype = 4;
pub const CURLOT_SLIST: curl_easytype = 5;
pub const CURLOT_CBPTR: curl_easytype = 6;
pub const CURLOT_BLOB: curl_easytype = 7;
pub const CURLOT_FUNCTION: curl_easytype = 8;
pub const CURLOT_FLAG_ALIAS: c_uint = 1 << 0;

pub const CURLSSLBACKEND_NONE: curl_sslbackend = 0;
pub const CURLSSLBACKEND_OPENSSL: curl_sslbackend = 1;
pub const CURLSSLBACKEND_GNUTLS: curl_sslbackend = 2;

#[repr(C)]
pub struct curl_opaque_placeholder {
    _private: [u8; 0],
}

#[repr(C)]
pub struct CURL {
    _private: [u8; 0],
}

#[repr(C)]
pub struct CURLM {
    _private: [u8; 0],
}

#[repr(C)]
pub struct CURLSH {
    _private: [u8; 0],
}

#[repr(C)]
pub struct CURLU {
    _private: [u8; 0],
}

#[repr(C)]
pub struct curl_mime {
    _private: [u8; 0],
}

#[repr(C)]
pub struct curl_mimepart {
    _private: [u8; 0],
}

#[repr(C)]
pub struct curl_pushheaders {
    _private: [u8; 0],
}

#[repr(C)]
pub struct sockaddr {
    pub sa_family: u16,
    pub sa_data: [c_char; 14],
}

#[repr(C)]
pub struct curl_blob {
    pub data: *mut c_void,
    pub len: size_t,
    pub flags: c_uint,
}

#[repr(C)]
pub struct curl_slist {
    pub data: *mut c_char,
    pub next: *mut curl_slist,
}

#[repr(C)]
pub struct curl_httppost {
    pub next: *mut curl_httppost,
    pub name: *mut c_char,
    pub namelength: c_long,
    pub contents: *mut c_char,
    pub contentslength: c_long,
    pub buffer: *mut c_char,
    pub bufferlength: c_long,
    pub contenttype: *mut c_char,
    pub contentheader: *mut curl_slist,
    pub more: *mut curl_httppost,
    pub flags: c_long,
    pub showfilename: *mut c_char,
    pub userp: *mut c_void,
    pub contentlen: curl_off_t,
}

#[repr(C)]
pub struct curl_fileinfo_strings {
    pub time: *mut c_char,
    pub perm: *mut c_char,
    pub user: *mut c_char,
    pub group: *mut c_char,
    pub target: *mut c_char,
}

#[repr(C)]
pub struct curl_fileinfo {
    pub filename: *mut c_char,
    pub filetype: curlfiletype,
    pub time: time_t,
    pub perm: c_uint,
    pub uid: c_int,
    pub gid: c_int,
    pub size: curl_off_t,
    pub hardlinks: c_long,
    pub strings: curl_fileinfo_strings,
    pub flags: c_uint,
    pub b_data: *mut c_char,
    pub b_size: size_t,
    pub b_used: size_t,
}

#[repr(C)]
pub struct curl_sockaddr {
    pub family: c_int,
    pub socktype: c_int,
    pub protocol: c_int,
    pub addrlen: c_uint,
    pub addr: sockaddr,
}

#[repr(C)]
pub struct curl_khkey {
    pub key: *const c_char,
    pub len: size_t,
    pub keytype: curl_khtype,
}

#[repr(C)]
pub struct curl_hstsentry {
    pub name: *mut c_char,
    pub namelen: size_t,
    pub includeSubDomains: u8,
    pub expire: [c_char; 18],
}

#[repr(C)]
pub struct curl_index {
    pub index: size_t,
    pub total: size_t,
}

#[repr(C)]
pub struct curl_forms {
    pub option: CURLformoption,
    pub value: *const c_char,
}

#[repr(C)]
pub struct curl_ssl_backend {
    pub id: curl_sslbackend,
    pub name: *const c_char,
}

#[repr(C)]
pub struct curl_certinfo {
    pub num_of_certs: c_int,
    pub certinfo: *mut *mut curl_slist,
}

#[repr(C)]
pub struct curl_tlssessioninfo {
    pub backend: curl_sslbackend,
    pub internals: *mut c_void,
}

#[repr(C)]
pub struct curl_easyoption {
    pub name: *const c_char,
    pub id: CURLoption,
    pub type_: curl_easytype,
    pub flags: c_uint,
}

#[repr(C)]
pub struct curl_header {
    pub name: *mut c_char,
    pub value: *mut c_char,
    pub amount: size_t,
    pub index: size_t,
    pub origin: c_uint,
    pub anchor: *mut c_void,
}

#[repr(C)]
pub union CURLMsgData {
    pub whatever: *mut c_void,
    pub result: CURLcode,
}

#[repr(C)]
pub struct CURLMsg {
    pub msg: CURLMSG,
    pub easy_handle: *mut CURL,
    pub data: CURLMsgData,
}

#[repr(C)]
pub struct curl_waitfd {
    pub fd: curl_socket_t,
    pub events: i16,
    pub revents: i16,
}

#[repr(C)]
pub struct curl_ws_frame {
    pub age: c_int,
    pub flags: c_int,
    pub offset: curl_off_t,
    pub bytesleft: curl_off_t,
    pub len: size_t,
}

#[repr(C)]
pub struct curl_version_info_data {
    pub age: CURLversion,
    pub version: *const c_char,
    pub version_num: c_uint,
    pub host: *const c_char,
    pub features: c_int,
    pub ssl_version: *const c_char,
    pub ssl_version_num: c_long,
    pub libz_version: *const c_char,
    pub protocols: *const *const c_char,
    pub ares: *const c_char,
    pub ares_num: c_int,
    pub libidn: *const c_char,
    pub iconv_ver_num: c_int,
    pub libssh_version: *const c_char,
    pub brotli_ver_num: c_uint,
    pub brotli_version: *const c_char,
    pub nghttp2_ver_num: c_uint,
    pub nghttp2_version: *const c_char,
    pub quic_version: *const c_char,
    pub cainfo: *const c_char,
    pub capath: *const c_char,
    pub zstd_ver_num: c_uint,
    pub zstd_version: *const c_char,
    pub hyper_version: *const c_char,
    pub gsasl_version: *const c_char,
    pub feature_names: *const *const c_char,
}

pub type curl_malloc_callback = Option<unsafe extern "C" fn(size_t) -> *mut c_void>;
pub type curl_free_callback = Option<unsafe extern "C" fn(*mut c_void)>;
pub type curl_realloc_callback = Option<unsafe extern "C" fn(*mut c_void, size_t) -> *mut c_void>;
pub type curl_strdup_callback = Option<unsafe extern "C" fn(*const c_char) -> *mut c_char>;
pub type curl_calloc_callback = Option<unsafe extern "C" fn(size_t, size_t) -> *mut c_void>;
pub type curl_read_callback =
    Option<unsafe extern "C" fn(*mut c_char, size_t, size_t, *mut c_void) -> size_t>;
pub type curl_write_callback =
    Option<unsafe extern "C" fn(*mut c_char, size_t, size_t, *mut c_void) -> size_t>;
pub type curl_seek_callback =
    Option<unsafe extern "C" fn(*mut c_void, curl_off_t, c_int) -> c_int>;
pub type curl_ioctl_callback =
    Option<unsafe extern "C" fn(*mut CURL, c_int, *mut c_void) -> c_int>;
pub type curl_formget_callback =
    Option<unsafe extern "C" fn(*mut c_void, *const c_char, size_t) -> size_t>;
pub type curl_lock_function =
    Option<unsafe extern "C" fn(*mut CURL, curl_lock_data, curl_lock_access, *mut c_void)>;
pub type curl_unlock_function =
    Option<unsafe extern "C" fn(*mut CURL, curl_lock_data, *mut c_void)>;
pub type curl_hstsread_callback =
    Option<unsafe extern "C" fn(*mut CURL, *mut curl_hstsentry, *mut c_void) -> CURLSTScode>;
pub type curl_hstswrite_callback = Option<
    unsafe extern "C" fn(*mut CURL, *mut curl_hstsentry, *mut curl_index, *mut c_void) -> CURLSTScode,
>;
pub type curl_socklen_t_alias = curl_socklen_t;
