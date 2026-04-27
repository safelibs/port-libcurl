use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct SymbolManifest {
    soname: String,
    namespace: String,
    symbols: Vec<String>,
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let source_files = collect_rust_source_files(&manifest_dir.join("src"));
    let flavor = detect_flavor();
    let symbols_path = match flavor {
        "openssl" => manifest_dir.join("debian/libcurl4t64.symbols"),
        "gnutls" => manifest_dir.join("debian/libcurl3t64-gnutls.symbols"),
        _ => unreachable!(),
    };
    let checked_in_map = match flavor {
        "openssl" => manifest_dir.join("abi/libcurl-openssl.map"),
        "gnutls" => manifest_dir.join("abi/libcurl-gnutls.map"),
        _ => unreachable!(),
    };
    let abi_manifest_path = manifest_dir.join("metadata/abi-manifest.json");
    let forwarders = manifest_dir.join("c_shim/forwarders.c");
    let variadic = manifest_dir.join("c_shim/variadic.c");
    let mprintf = manifest_dir.join("c_shim/mprintf.c");
    let tls_backend = manifest_dir.join("c_shim/tls_backend.c");
    let ssh_backend = manifest_dir.join("c_shim/ssh_backend.c");
    let reference_script = manifest_dir.join("scripts/build-reference-curl.sh");

    for path in [
        &symbols_path,
        &checked_in_map,
        &abi_manifest_path,
        &forwarders,
        &variadic,
        &mprintf,
        &tls_backend,
        &ssh_backend,
        &reference_script,
        &manifest_dir.join("vendor/upstream/manifest.json"),
        &manifest_dir.join("include/curl/curl.h"),
        &manifest_dir.join("include/curl/options.h"),
    ] {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    for path in &source_files {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rerun-if-env-changed=CC");

    let symbol_manifest = parse_symbols(&symbols_path);
    let generated_map = render_version_script(&symbol_manifest);
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let out_map = out_dir.join(format!("libcurl-{}.map", flavor));
    let public_exports = collect_rust_public_exports(&source_files);
    let public_export_shim = out_dir.join(format!("public-exports-{}.S", flavor));
    fs::write(&out_map, generated_map.as_bytes()).expect("write version script");
    fs::write(
        &public_export_shim,
        render_public_export_shim(
            &symbol_manifest.namespace,
            &public_exports.public_exports,
            &public_exports.hidden_helpers,
        ),
    )
    .expect("write public export shim");

    let committed_map = fs::read_to_string(&checked_in_map).unwrap_or_default();
    if committed_map != generated_map {
        println!(
            "cargo:warning=checked-in version script {} differs from build-time generation",
            checked_in_map.display()
        );
    }

    run_reference_build(&manifest_dir, &reference_script, flavor);
    generate_easy_option_table(&manifest_dir, &abi_manifest_path, &out_dir);
    compile_c_shims(&manifest_dir, flavor, &public_export_shim);

    println!("cargo:rustc-link-lib=dl");
    link_psl();
    println!("cargo:rustc-link-lib=ssh2");
    match flavor {
        "openssl" => {
            println!("cargo:rustc-link-lib=ssl");
            println!("cargo:rustc-link-lib=crypto");
        }
        "gnutls" => {
            println!("cargo:rustc-link-lib=gnutls");
        }
        _ => unreachable!(),
    }
    println!(
        "cargo:rustc-cdylib-link-arg=-Wl,-soname,{}",
        symbol_manifest.soname
    );
    println!(
        "cargo:rustc-cdylib-link-arg=-Wl,--version-script={}",
        out_map.display()
    );
}

fn link_psl() {
    let candidates = [
        Path::new("/lib/x86_64-linux-gnu/libpsl.so.5"),
        Path::new("/usr/lib/x86_64-linux-gnu/libpsl.so.5"),
        Path::new("/lib/i386-linux-gnu/libpsl.so.5"),
        Path::new("/usr/lib/i386-linux-gnu/libpsl.so.5"),
    ];
    if let Some(found) = candidates.iter().find(|path| path.is_file()) {
        if let Some(parent) = found.parent() {
            println!("cargo:rustc-link-search=native={}", parent.display());
        }
        println!("cargo:rustc-link-arg=-l:{}", found.file_name().unwrap().to_string_lossy());
    } else {
        println!("cargo:rustc-link-lib=psl");
    }
}

#[derive(Default)]
struct RustPublicExports {
    public_exports: Vec<String>,
    hidden_helpers: Vec<String>,
}

fn detect_flavor() -> &'static str {
    let openssl = env::var_os("CARGO_FEATURE_OPENSSL_FLAVOR").is_some();
    let gnutls = env::var_os("CARGO_FEATURE_GNUTLS_FLAVOR").is_some();
    match (openssl, gnutls) {
        (true, false) => "openssl",
        (false, true) => "gnutls",
        (true, true) => panic!("enable exactly one flavor feature"),
        (false, false) => panic!("enable one of `openssl-flavor` or `gnutls-flavor`"),
    }
}

fn collect_rust_source_files(src_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_source_files_recursive(src_dir, &mut files);
    files.sort();
    files
}

fn collect_rust_source_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries =
        fs::read_dir(dir).unwrap_or_else(|err| panic!("read_dir {}: {}", dir.display(), err));
    for entry in entries {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let file_type = entry.file_type().expect("entry file type");
        if file_type.is_dir() {
            collect_rust_source_files_recursive(&path, files);
        } else if path.extension() == Some(OsStr::new("rs")) {
            files.push(path);
        }
    }
}

fn collect_rust_public_exports(source_files: &[PathBuf]) -> RustPublicExports {
    let mut public_exports = BTreeSet::new();
    let mut hidden_helpers = BTreeSet::new();

    for path in source_files {
        let contents = fs::read_to_string(path)
            .unwrap_or_else(|err| panic!("read {}: {}", path.display(), err));
        for line in contents.lines() {
            if let Some(name) = parse_rust_abi_fn_name(line, "port_safe_export_curl_") {
                public_exports.insert(name.to_string());
            }
            if let Some(name) = parse_rust_abi_fn_name(line, "port_safe_") {
                hidden_helpers.insert(name.to_string());
            }
        }
    }

    RustPublicExports {
        public_exports: public_exports.into_iter().collect(),
        hidden_helpers: hidden_helpers.into_iter().collect(),
    }
}

fn parse_rust_abi_fn_name<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    const PREFIXES: [&str; 2] = ["pub unsafe extern \"C\" fn ", "pub extern \"C\" fn "];

    let trimmed = line.trim();
    for marker in PREFIXES {
        let Some(rest) = trimmed.strip_prefix(marker) else {
            continue;
        };
        let end = rest
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .unwrap_or(rest.len());
        let name = &rest[..end];
        if name.starts_with(prefix) {
            return Some(name);
        }
    }

    None
}

fn parse_symbols(path: &Path) -> SymbolManifest {
    let contents = fs::read_to_string(path).expect("read symbols file");
    let mut lines = contents.lines().filter(|line| !line.trim().is_empty());
    let header = lines.next().expect("symbols header");
    let soname = header
        .split_whitespace()
        .next()
        .expect("soname")
        .to_string();

    let mut namespace = String::new();
    let mut symbols = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with('*') {
            continue;
        }
        let Some((token, _minver)) = trimmed.split_once(' ') else {
            continue;
        };
        let Some((name, version)) = token.split_once('@') else {
            continue;
        };
        if name == "HIDDEN" {
            continue;
        }
        if name.starts_with("CURL_") && name == version {
            namespace = name.to_string();
            continue;
        }
        if name.starts_with("curl_") {
            symbols.push(name.to_string());
        }
    }
    assert!(
        !namespace.is_empty(),
        "missing symbol namespace in {}",
        path.display()
    );
    SymbolManifest {
        soname,
        namespace,
        symbols,
    }
}

fn render_version_script(manifest: &SymbolManifest) -> String {
    let mut body = String::new();
    body.push_str(&format!("{} {{\n", manifest.namespace));
    body.push_str("  global:\n");
    for symbol in &manifest.symbols {
        body.push_str("    ");
        body.push_str(symbol);
        body.push_str(";\n");
    }
    body.push_str("  local:\n");
    body.push_str("    *;\n");
    body.push_str("};\n");
    body
}

fn render_public_export_shim(
    namespace: &str,
    public_exports: &[String],
    hidden_helpers: &[String],
) -> String {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH");
    let branch_template = match target_arch.as_str() {
        "x86_64" => "  jmp {target}\n",
        "aarch64" => "  b {target}\n",
        other => panic!("unsupported target arch for ABI export shim: {}", other),
    };

    let mut source = String::new();
    source.push_str(".text\n");
    for helper in hidden_helpers {
        source.push_str(".hidden ");
        source.push_str(helper);
        source.push('\n');
    }
    for symbol in public_exports {
        let public_symbol = symbol
            .strip_prefix("port_safe_export_")
            .unwrap_or_else(|| panic!("unexpected export impl symbol {}", symbol));
        let wrapper = format!("{}_shim", public_symbol);
        source.push_str(".hidden ");
        source.push_str(symbol);
        source.push('\n');
        source.push_str(".globl ");
        source.push_str(&wrapper);
        source.push('\n');
        source.push_str(".type ");
        source.push_str(&wrapper);
        source.push_str(", @function\n");
        source.push_str(&wrapper);
        source.push_str(":\n");
        source.push_str(&branch_template.replace("{target}", symbol));
        source.push_str(".size ");
        source.push_str(&wrapper);
        source.push_str(", .-");
        source.push_str(&wrapper);
        source.push('\n');
        source.push_str(".symver ");
        source.push_str(&wrapper);
        source.push_str(", ");
        source.push_str(public_symbol);
        source.push_str("@@");
        source.push_str(namespace);
        source.push('\n');
    }
    source.push_str(".section .note.GNU-stack,\"\",@progbits\n");
    source
}

fn run_reference_build(manifest_dir: &Path, script: &Path, flavor: &str) {
    let status = Command::new("bash")
        .arg(script)
        .arg("--flavor")
        .arg(flavor)
        .current_dir(manifest_dir)
        .status()
        .expect("spawn build-reference-curl.sh");
    if !status.success() {
        panic!("reference build failed for {}", flavor);
    }
}

fn compile_c_shims(manifest_dir: &Path, flavor: &str, public_export_shim: &Path) {
    let reference_path = manifest_dir
        .join(".reference")
        .join(flavor)
        .join("dist")
        .join(format!("libcurl-reference-{}.so.4", flavor));
    let reference_file = reference_path
        .file_name()
        .and_then(OsStr::to_str)
        .expect("reference library filename");
    let reference_abs = reference_path
        .to_str()
        .expect("reference library absolute path");
    let reference_file_define = format!("\"{}\"", reference_file);
    let reference_abs_define = format!("\"{}\"", reference_abs);
    let flavor_define = format!("\"{}\"", flavor);

    cc::Build::new()
        .include(manifest_dir.join("include"))
        .file(manifest_dir.join("c_shim/forwarders.c"))
        .file(manifest_dir.join("c_shim/variadic.c"))
        .file(manifest_dir.join("c_shim/mprintf.c"))
        .file(manifest_dir.join("c_shim/tls_backend.c"))
        .file(manifest_dir.join("c_shim/ssh_backend.c"))
        .file(public_export_shim)
        .flag_if_supported("-std=c11")
        .flag_if_supported("-fPIC")
        .flag_if_supported("-Wall")
        .flag_if_supported("-Wextra")
        .warnings(true)
        .define(
            "REFERENCE_LIBRARY_FILE",
            Some(reference_file_define.as_str()),
        )
        .define(
            "REFERENCE_LIBRARY_ABSPATH",
            Some(reference_abs_define.as_str()),
        )
        .define("BRIDGE_FLAVOR", Some(flavor_define.as_str()))
        .define(
            if flavor == "openssl" {
                "SAFE_TLS_OPENSSL"
            } else {
                "SAFE_TLS_GNUTLS"
            },
            Some("1"),
        )
        .compile("port_libcurl_safe_shims");
}

fn generate_easy_option_table(manifest_dir: &Path, abi_manifest_path: &Path, out_dir: &Path) {
    let contents = fs::read_to_string(abi_manifest_path).expect("read abi manifest");
    let manifest: Value = serde_json::from_str(&contents).expect("parse abi manifest json");
    let entries = manifest["option_metadata"]["entries"]
        .as_array()
        .expect("option_metadata.entries");
    let ids: Vec<String> = entries
        .iter()
        .map(|entry| entry["id"].as_str().expect("option id").to_string())
        .collect();
    let values = resolve_curl_option_values(manifest_dir, out_dir, &ids);

    let mut output = String::new();
    output.push_str("// Generated by build.rs from safe/metadata/abi-manifest.json.\n\n");
    output.push_str(&format!(
        "pub(crate) const EASY_OPTION_COUNT: usize = {};\n\n",
        entries.len()
    ));
    output.push_str(
        "pub(crate) const EASY_OPTIONS: [crate::abi::curl_easyoption; EASY_OPTION_COUNT + 1] = [\n",
    );
    for entry in entries {
        let id_name = entry["id"].as_str().expect("option id");
        let id_value = values
            .get(id_name)
            .unwrap_or_else(|| panic!("missing generated value for {}", id_name));
        let name = entry["name"].as_str().expect("option name");
        let ty = match entry["type"].as_str().expect("option type") {
            "CURLOT_LONG" => "crate::abi::CURLOT_LONG",
            "CURLOT_VALUES" => "crate::abi::CURLOT_VALUES",
            "CURLOT_OFF_T" => "crate::abi::CURLOT_OFF_T",
            "CURLOT_OBJECT" => "crate::abi::CURLOT_OBJECT",
            "CURLOT_STRING" => "crate::abi::CURLOT_STRING",
            "CURLOT_SLIST" => "crate::abi::CURLOT_SLIST",
            "CURLOT_CBPTR" => "crate::abi::CURLOT_CBPTR",
            "CURLOT_BLOB" => "crate::abi::CURLOT_BLOB",
            "CURLOT_FUNCTION" => "crate::abi::CURLOT_FUNCTION",
            other => panic!("unsupported easy option type {}", other),
        };
        let flags = match entry["flags"].as_str().expect("option flags") {
            "0" => "0",
            "CURLOT_FLAG_ALIAS" => "crate::abi::CURLOT_FLAG_ALIAS",
            other => panic!("unsupported easy option flags {}", other),
        };
        output.push_str("    crate::abi::curl_easyoption {\n");
        output.push_str(&format!(
            "        name: b\"{}\\0\".as_ptr().cast::<core::ffi::c_char>(),\n",
            name
        ));
        output.push_str(&format!("        id: {}u32,\n", id_value));
        output.push_str(&format!("        type_: {},\n", ty));
        output.push_str(&format!("        flags: {},\n", flags));
        output.push_str("    },\n");
    }
    output.push_str("    crate::abi::curl_easyoption {\n");
    output.push_str("        name: core::ptr::null(),\n");
    output.push_str("        id: 0,\n");
    output.push_str("        type_: 0,\n");
    output.push_str("        flags: 0,\n");
    output.push_str("    },\n");
    output.push_str("];\n");

    fs::write(out_dir.join("easy_options.rs"), output).expect("write easy options table");
}

fn resolve_curl_option_values(
    manifest_dir: &Path,
    out_dir: &Path,
    ids: &[String],
) -> BTreeMap<String, String> {
    let helper_c = out_dir.join("print_easy_options.c");
    let helper_bin = out_dir.join("print_easy_options");
    let cc = env::var("CC").unwrap_or_else(|_| "cc".to_string());

    let mut source = String::new();
    source.push_str("#include <stdio.h>\n#include <curl/curl.h>\n\nint main(void) {\n");
    for id in ids {
        source.push_str(&format!("  printf(\"{}=%ld\\n\", (long){});\n", id, id));
    }
    source.push_str("  return 0;\n}\n");
    fs::write(&helper_c, source).expect("write easy option helper source");

    let status = Command::new(&cc)
        .current_dir(manifest_dir)
        .arg("-std=c11")
        .arg("-I")
        .arg(manifest_dir.join("include"))
        .arg(&helper_c)
        .arg("-o")
        .arg(&helper_bin)
        .status()
        .expect("compile easy option helper");
    if !status.success() {
        panic!("easy option helper compilation failed");
    }

    let output = Command::new(&helper_bin)
        .current_dir(manifest_dir)
        .output()
        .expect("run easy option helper");
    if !output.status.success() {
        panic!("easy option helper execution failed");
    }

    let stdout = String::from_utf8(output.stdout).expect("easy option helper output");
    let mut values = BTreeMap::new();
    for line in stdout.lines() {
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        values.insert(name.to_string(), value.trim().to_string());
    }
    values
}
