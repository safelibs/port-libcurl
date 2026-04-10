use port_libcurl_safe::abi::{
    self, curl_easyoption, curl_version_info_data, CURLMsg, CURLOT_BLOB, CURLOT_CBPTR,
    CURLOT_FLAG_ALIAS, CURLOT_FUNCTION, CURLOT_LONG, CURLOT_OBJECT, CURLOT_OFF_T,
    CURLOT_SLIST, CURLOT_STRING, CURLOT_VALUES,
};
use port_libcurl_safe::BUILD_FLAVOR;
use serde_json::Value;
use std::collections::BTreeMap;
use std::ffi::{c_char, CString};
use std::fs;
use std::mem::{align_of, offset_of, size_of};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr;

const OPAQUE_STRUCTS: &[&str] = &[
    "CURL",
    "CURLM",
    "CURLSH",
    "CURLU",
    "curl_mime",
    "curl_mimepart",
    "curl_pushheaders",
];

const LAYOUT_STRUCTS: &[&str] = &[
    "CURLMsg",
    "curl_blob",
    "curl_certinfo",
    "curl_easyoption",
    "curl_fileinfo",
    "curl_forms",
    "curl_header",
    "curl_hstsentry",
    "curl_httppost",
    "curl_index",
    "curl_khkey",
    "curl_slist",
    "curl_sockaddr",
    "curl_ssl_backend",
    "curl_tlssessioninfo",
    "curl_version_info_data",
    "curl_waitfd",
    "curl_ws_frame",
];

unsafe extern "C" {
    fn curl_easy_option_by_name(name: *const c_char) -> *const curl_easyoption;
    fn curl_easy_option_by_id(id: u32) -> *const curl_easyoption;
    fn curl_easy_option_next(prev: *const curl_easyoption) -> *const curl_easyoption;
}

#[derive(Debug)]
struct LayoutLine {
    size: usize,
    align: usize,
    offsets: BTreeMap<String, usize>,
}

fn safe_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn compiler() -> String {
    std::env::var("CC").unwrap_or_else(|_| "cc".to_string())
}

fn compile_layout_helper() -> PathBuf {
    let out_dir = safe_dir().join("target/abi-layout").join(BUILD_FLAVOR);
    fs::create_dir_all(&out_dir).expect("create abi-layout output directory");

    let source = out_dir.join("layout_helper.c");
    let binary = out_dir.join("layout_helper");
    fs::write(&source, helper_source()).expect("write layout helper");

    let status = Command::new(compiler())
        .arg("-std=c11")
        .arg("-Wall")
        .arg("-Wextra")
        .arg(format!("-I{}", safe_dir().join("include").display()))
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .status()
        .expect("spawn C compiler for layout helper");
    assert!(status.success(), "layout helper compilation failed");

    binary
}

fn helper_source() -> &'static str {
    r#"#include <stddef.h>
#include <stdio.h>

#include <curl/curl.h>
#include <curl/easy.h>
#include <curl/header.h>
#include <curl/multi.h>
#include <curl/options.h>
#include <curl/websockets.h>

#define BEGIN(name, type) printf("%s|%zu|%zu", name, sizeof(type), _Alignof(type))
#define FIELD(type, field) printf("|%s=%zu", #field, offsetof(type, field))
#define FIELD_VALUE(name, value) printf("|%s=%zu", name, (size_t)(value))
#define END() putchar('\n')

int main(void) {
  BEGIN("CURLMsg", CURLMsg);
  FIELD(CURLMsg, msg);
  FIELD(CURLMsg, easy_handle);
  FIELD(CURLMsg, data);
  END();

  BEGIN("curl_blob", struct curl_blob);
  FIELD(struct curl_blob, data);
  FIELD(struct curl_blob, len);
  FIELD(struct curl_blob, flags);
  END();

  BEGIN("curl_certinfo", struct curl_certinfo);
  FIELD(struct curl_certinfo, num_of_certs);
  FIELD(struct curl_certinfo, certinfo);
  END();

  BEGIN("curl_easyoption", struct curl_easyoption);
  FIELD(struct curl_easyoption, name);
  FIELD(struct curl_easyoption, id);
  FIELD(struct curl_easyoption, type);
  FIELD(struct curl_easyoption, flags);
  END();

  BEGIN("curl_fileinfo", struct curl_fileinfo);
  FIELD(struct curl_fileinfo, filename);
  FIELD(struct curl_fileinfo, filetype);
  FIELD(struct curl_fileinfo, time);
  FIELD(struct curl_fileinfo, perm);
  FIELD(struct curl_fileinfo, uid);
  FIELD(struct curl_fileinfo, gid);
  FIELD(struct curl_fileinfo, size);
  FIELD(struct curl_fileinfo, hardlinks);
  FIELD(struct curl_fileinfo, strings);
  FIELD(struct curl_fileinfo, flags);
  FIELD(struct curl_fileinfo, b_data);
  FIELD(struct curl_fileinfo, b_size);
  FIELD(struct curl_fileinfo, b_used);
  END();

  BEGIN("curl_forms", struct curl_forms);
  FIELD(struct curl_forms, option);
  FIELD(struct curl_forms, value);
  END();

  BEGIN("curl_header", struct curl_header);
  FIELD(struct curl_header, name);
  FIELD(struct curl_header, value);
  FIELD(struct curl_header, amount);
  FIELD(struct curl_header, index);
  FIELD(struct curl_header, origin);
  FIELD(struct curl_header, anchor);
  END();

  BEGIN("curl_hstsentry", struct curl_hstsentry);
  FIELD(struct curl_hstsentry, name);
  FIELD(struct curl_hstsentry, namelen);
  FIELD_VALUE("includeSubDomains",
              offsetof(struct curl_hstsentry, expire) - 1);
  FIELD(struct curl_hstsentry, expire);
  END();

  BEGIN("curl_httppost", struct curl_httppost);
  FIELD(struct curl_httppost, next);
  FIELD(struct curl_httppost, name);
  FIELD(struct curl_httppost, namelength);
  FIELD(struct curl_httppost, contents);
  FIELD(struct curl_httppost, contentslength);
  FIELD(struct curl_httppost, buffer);
  FIELD(struct curl_httppost, bufferlength);
  FIELD(struct curl_httppost, contenttype);
  FIELD(struct curl_httppost, contentheader);
  FIELD(struct curl_httppost, more);
  FIELD(struct curl_httppost, flags);
  FIELD(struct curl_httppost, showfilename);
  FIELD(struct curl_httppost, userp);
  FIELD(struct curl_httppost, contentlen);
  END();

  BEGIN("curl_index", struct curl_index);
  FIELD(struct curl_index, index);
  FIELD(struct curl_index, total);
  END();

  BEGIN("curl_khkey", struct curl_khkey);
  FIELD(struct curl_khkey, key);
  FIELD(struct curl_khkey, len);
  FIELD(struct curl_khkey, keytype);
  END();

  BEGIN("curl_slist", struct curl_slist);
  FIELD(struct curl_slist, data);
  FIELD(struct curl_slist, next);
  END();

  BEGIN("curl_sockaddr", struct curl_sockaddr);
  FIELD(struct curl_sockaddr, family);
  FIELD(struct curl_sockaddr, socktype);
  FIELD(struct curl_sockaddr, protocol);
  FIELD(struct curl_sockaddr, addrlen);
  FIELD(struct curl_sockaddr, addr);
  END();

  BEGIN("curl_ssl_backend", struct curl_ssl_backend);
  FIELD(struct curl_ssl_backend, id);
  FIELD(struct curl_ssl_backend, name);
  END();

  BEGIN("curl_tlssessioninfo", struct curl_tlssessioninfo);
  FIELD(struct curl_tlssessioninfo, backend);
  FIELD(struct curl_tlssessioninfo, internals);
  END();

  BEGIN("curl_version_info_data", curl_version_info_data);
  FIELD(curl_version_info_data, age);
  FIELD(curl_version_info_data, version);
  FIELD(curl_version_info_data, version_num);
  FIELD(curl_version_info_data, host);
  FIELD(curl_version_info_data, features);
  FIELD(curl_version_info_data, ssl_version);
  FIELD(curl_version_info_data, ssl_version_num);
  FIELD(curl_version_info_data, libz_version);
  FIELD(curl_version_info_data, protocols);
  FIELD(curl_version_info_data, ares);
  FIELD(curl_version_info_data, ares_num);
  FIELD(curl_version_info_data, libidn);
  FIELD(curl_version_info_data, iconv_ver_num);
  FIELD(curl_version_info_data, libssh_version);
  FIELD(curl_version_info_data, brotli_ver_num);
  FIELD(curl_version_info_data, brotli_version);
  FIELD(curl_version_info_data, nghttp2_ver_num);
  FIELD(curl_version_info_data, nghttp2_version);
  FIELD(curl_version_info_data, quic_version);
  FIELD(curl_version_info_data, cainfo);
  FIELD(curl_version_info_data, capath);
  FIELD(curl_version_info_data, zstd_ver_num);
  FIELD(curl_version_info_data, zstd_version);
  FIELD(curl_version_info_data, hyper_version);
  FIELD(curl_version_info_data, gsasl_version);
  FIELD(curl_version_info_data, feature_names);
  END();

  BEGIN("curl_waitfd", struct curl_waitfd);
  FIELD(struct curl_waitfd, fd);
  FIELD(struct curl_waitfd, events);
  FIELD(struct curl_waitfd, revents);
  END();

  BEGIN("curl_ws_frame", struct curl_ws_frame);
  FIELD(struct curl_ws_frame, age);
  FIELD(struct curl_ws_frame, flags);
  FIELD(struct curl_ws_frame, offset);
  FIELD(struct curl_ws_frame, bytesleft);
  FIELD(struct curl_ws_frame, len);
  END();

  return 0;
}
"#
}

fn run_layout_helper(binary: &Path) -> BTreeMap<String, LayoutLine> {
    let output = Command::new(binary)
        .output()
        .expect("run layout helper");
    assert!(output.status.success(), "layout helper failed");

    let text = String::from_utf8(output.stdout).expect("layout helper stdout utf8");
    let mut result = BTreeMap::new();
    for line in text.lines() {
        let mut parts = line.split('|');
        let name = parts.next().expect("layout name").to_string();
        let size = parts
            .next()
            .expect("layout size")
            .parse::<usize>()
            .expect("parse size");
        let align = parts
            .next()
            .expect("layout align")
            .parse::<usize>()
            .expect("parse align");
        let mut offsets = BTreeMap::new();
        for field in parts {
            let (field_name, offset) = field.split_once('=').expect("field offset");
            offsets.insert(field_name.to_string(), offset.parse::<usize>().expect("parse offset"));
        }
        result.insert(name, LayoutLine { size, align, offsets });
    }
    result
}

fn manifest() -> Value {
    serde_json::from_str(include_str!("../metadata/abi-manifest.json")).expect("parse abi manifest")
}

fn expected_easy_type(name: &str) -> u32 {
    match name {
        "CURLOT_LONG" => CURLOT_LONG,
        "CURLOT_VALUES" => CURLOT_VALUES,
        "CURLOT_OFF_T" => CURLOT_OFF_T,
        "CURLOT_OBJECT" => CURLOT_OBJECT,
        "CURLOT_STRING" => CURLOT_STRING,
        "CURLOT_SLIST" => CURLOT_SLIST,
        "CURLOT_CBPTR" => CURLOT_CBPTR,
        "CURLOT_BLOB" => CURLOT_BLOB,
        "CURLOT_FUNCTION" => CURLOT_FUNCTION,
        other => panic!("unexpected easy option type {other}"),
    }
}

macro_rules! assert_layout {
    ($layouts:expr, $name:literal, $ty:ty, { $($c_field:literal => $r_field:tt),* $(,)? }) => {{
        let layout = $layouts.get($name).expect(concat!("missing layout for ", $name));
        assert_eq!(layout.size, size_of::<$ty>(), "size mismatch for {}", $name);
        assert_eq!(layout.align, align_of::<$ty>(), "align mismatch for {}", $name);
        $(
            assert_eq!(
                *layout.offsets.get($c_field).expect(concat!("missing field ", $c_field)),
                offset_of!($ty, $r_field),
                "offset mismatch for {}.{}",
                $name,
                $c_field
            );
        )*
    }};
}

#[test]
fn public_layout_and_option_table_match_manifest() {
    let manifest = manifest();
    let manifest_structs: Vec<&str> = manifest["public_struct_names"]
        .as_array()
        .expect("public_struct_names")
        .iter()
        .map(|item| item.as_str().expect("manifest struct name"))
        .collect();

    let mut expected_structs = Vec::new();
    expected_structs.extend_from_slice(OPAQUE_STRUCTS);
    expected_structs.extend_from_slice(LAYOUT_STRUCTS);
    expected_structs.sort_unstable();

    let mut actual_structs = manifest_structs.clone();
    actual_structs.sort_unstable();
    assert_eq!(actual_structs, expected_structs);

    let layouts = run_layout_helper(&compile_layout_helper());

    assert_layout!(layouts, "CURLMsg", CURLMsg, {
        "msg" => msg,
        "easy_handle" => easy_handle,
        "data" => data,
    });
    assert_layout!(layouts, "curl_blob", abi::curl_blob, {
        "data" => data,
        "len" => len,
        "flags" => flags,
    });
    assert_layout!(layouts, "curl_certinfo", abi::curl_certinfo, {
        "num_of_certs" => num_of_certs,
        "certinfo" => certinfo,
    });
    assert_layout!(layouts, "curl_easyoption", abi::curl_easyoption, {
        "name" => name,
        "id" => id,
        "type" => type_,
        "flags" => flags,
    });
    assert_layout!(layouts, "curl_fileinfo", abi::curl_fileinfo, {
        "filename" => filename,
        "filetype" => filetype,
        "time" => time,
        "perm" => perm,
        "uid" => uid,
        "gid" => gid,
        "size" => size,
        "hardlinks" => hardlinks,
        "strings" => strings,
        "flags" => flags,
        "b_data" => b_data,
        "b_size" => b_size,
        "b_used" => b_used,
    });
    assert_layout!(layouts, "curl_forms", abi::curl_forms, {
        "option" => option,
        "value" => value,
    });
    assert_layout!(layouts, "curl_header", abi::curl_header, {
        "name" => name,
        "value" => value,
        "amount" => amount,
        "index" => index,
        "origin" => origin,
        "anchor" => anchor,
    });
    assert_layout!(layouts, "curl_hstsentry", abi::curl_hstsentry, {
        "name" => name,
        "namelen" => namelen,
        "includeSubDomains" => includeSubDomains,
        "expire" => expire,
    });
    assert_layout!(layouts, "curl_httppost", abi::curl_httppost, {
        "next" => next,
        "name" => name,
        "namelength" => namelength,
        "contents" => contents,
        "contentslength" => contentslength,
        "buffer" => buffer,
        "bufferlength" => bufferlength,
        "contenttype" => contenttype,
        "contentheader" => contentheader,
        "more" => more,
        "flags" => flags,
        "showfilename" => showfilename,
        "userp" => userp,
        "contentlen" => contentlen,
    });
    assert_layout!(layouts, "curl_index", abi::curl_index, {
        "index" => index,
        "total" => total,
    });
    assert_layout!(layouts, "curl_khkey", abi::curl_khkey, {
        "key" => key,
        "len" => len,
        "keytype" => keytype,
    });
    assert_layout!(layouts, "curl_slist", abi::curl_slist, {
        "data" => data,
        "next" => next,
    });
    assert_layout!(layouts, "curl_sockaddr", abi::curl_sockaddr, {
        "family" => family,
        "socktype" => socktype,
        "protocol" => protocol,
        "addrlen" => addrlen,
        "addr" => addr,
    });
    assert_layout!(layouts, "curl_ssl_backend", abi::curl_ssl_backend, {
        "id" => id,
        "name" => name,
    });
    assert_layout!(layouts, "curl_tlssessioninfo", abi::curl_tlssessioninfo, {
        "backend" => backend,
        "internals" => internals,
    });
    assert_layout!(layouts, "curl_version_info_data", curl_version_info_data, {
        "age" => age,
        "version" => version,
        "version_num" => version_num,
        "host" => host,
        "features" => features,
        "ssl_version" => ssl_version,
        "ssl_version_num" => ssl_version_num,
        "libz_version" => libz_version,
        "protocols" => protocols,
        "ares" => ares,
        "ares_num" => ares_num,
        "libidn" => libidn,
        "iconv_ver_num" => iconv_ver_num,
        "libssh_version" => libssh_version,
        "brotli_ver_num" => brotli_ver_num,
        "brotli_version" => brotli_version,
        "nghttp2_ver_num" => nghttp2_ver_num,
        "nghttp2_version" => nghttp2_version,
        "quic_version" => quic_version,
        "cainfo" => cainfo,
        "capath" => capath,
        "zstd_ver_num" => zstd_ver_num,
        "zstd_version" => zstd_version,
        "hyper_version" => hyper_version,
        "gsasl_version" => gsasl_version,
        "feature_names" => feature_names,
    });
    assert_layout!(layouts, "curl_waitfd", abi::curl_waitfd, {
        "fd" => fd,
        "events" => events,
        "revents" => revents,
    });
    assert_layout!(layouts, "curl_ws_frame", abi::curl_ws_frame, {
        "age" => age,
        "flags" => flags,
        "offset" => offset,
        "bytesleft" => bytesleft,
        "len" => len,
    });

    let entries = manifest["option_metadata"]["entries"]
        .as_array()
        .expect("option_metadata.entries");
    let mut runtime_entries = Vec::new();
    let mut cursor = ptr::null();
    loop {
        cursor = unsafe { curl_easy_option_next(cursor) };
        if cursor.is_null() {
            break;
        }
        runtime_entries.push(cursor);
    }
    assert_eq!(runtime_entries.len(), entries.len());

    for (runtime_ptr, entry) in runtime_entries.iter().zip(entries.iter()) {
        let runtime_ptr = *runtime_ptr;
        let runtime = unsafe { &*runtime_ptr };
        let runtime_name = unsafe { std::ffi::CStr::from_ptr(runtime.name) }
            .to_str()
            .expect("runtime option name utf8");
        assert_eq!(runtime_name, entry["name"].as_str().expect("manifest option name"));
        assert_eq!(
            runtime.type_,
            expected_easy_type(entry["type"].as_str().expect("manifest option type"))
        );
        let expected_alias = if entry["is_alias"].as_bool().expect("manifest alias flag") {
            CURLOT_FLAG_ALIAS
        } else {
            0
        };
        assert_eq!(runtime.flags & CURLOT_FLAG_ALIAS, expected_alias);

        let lower = CString::new(runtime_name.to_ascii_lowercase()).expect("lowercase option");
        assert_eq!(unsafe { curl_easy_option_by_name(lower.as_ptr()) }, runtime_ptr);

        let by_id = unsafe { curl_easy_option_by_id(runtime.id) };
        if (runtime.flags & CURLOT_FLAG_ALIAS) != 0 {
            assert!(!by_id.is_null());
            assert_ne!(by_id, runtime_ptr);
            assert_eq!(unsafe { (*by_id).id }, runtime.id);
            assert_eq!(unsafe { (*by_id).flags & CURLOT_FLAG_ALIAS }, 0);
        } else {
            assert_eq!(by_id, runtime_ptr);
        }
    }
}
