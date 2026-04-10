use std::env;
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
    let forwarders = manifest_dir.join("c_shim/forwarders.c");
    let reference_script = manifest_dir.join("scripts/build-reference-curl.sh");

    for path in [&symbols_path, &checked_in_map, &forwarders, &reference_script] {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rerun-if-env-changed=CC");

    let symbol_manifest = parse_symbols(&symbols_path);
    let generated_map = render_version_script(&symbol_manifest);
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let out_map = out_dir.join(format!("libcurl-{}.map", flavor));
    fs::write(&out_map, generated_map.as_bytes()).expect("write version script");

    let committed_map = fs::read_to_string(&checked_in_map).unwrap_or_default();
    if committed_map != generated_map {
        println!(
            "cargo:warning=checked-in version script {} differs from build-time generation",
            checked_in_map.display()
        );
    }

    run_reference_build(&manifest_dir, &reference_script, flavor);
    compile_bridge(&manifest_dir, &out_map, &symbol_manifest.soname, flavor);
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
    assert!(!namespace.is_empty(), "missing symbol namespace in {}", path.display());
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

fn compile_bridge(manifest_dir: &Path, version_script: &Path, soname: &str, flavor: &str) {
    let cc = env::var("CC").unwrap_or_else(|_| "cc".to_string());
    let artifact_dir = manifest_dir.join("target/foundation").join(flavor);
    fs::create_dir_all(&artifact_dir).expect("create artifact directory");

    let output = artifact_dir.join(match flavor {
        "openssl" => "libcurl-safe-openssl-bridge.so",
        "gnutls" => "libcurl-safe-gnutls-bridge.so",
        _ => unreachable!(),
    });
    let reference_source = manifest_dir
        .join(".reference")
        .join(flavor)
        .join("dist")
        .join(format!("libcurl-reference-{}.so.4", flavor));
    let reference_target = artifact_dir.join(format!("libcurl-reference-{}.so.4", flavor));
    fs::copy(&reference_source, &reference_target).unwrap_or_else(|err| {
        panic!(
            "copy {} -> {} failed: {}",
            reference_source.display(),
            reference_target.display(),
            err
        )
    });

    let forwarders = manifest_dir.join("c_shim/forwarders.c");
    let status = Command::new(&cc)
        .current_dir(manifest_dir)
        .arg("-fPIC")
        .arg("-shared")
        .arg("-O2")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-std=c11")
        .arg(forwarders)
        .arg("-o")
        .arg(&output)
        .arg("-ldl")
        .arg("-pthread")
        .arg("-Wl,--no-undefined")
        .arg(format!("-Wl,-soname,{}", soname))
        .arg(format!("-Wl,--version-script={}", version_script.display()))
        .arg(format!(
            "-DREFERENCE_LIBRARY_FILE=\"{}\"",
            reference_target.file_name().unwrap().to_string_lossy()
        ))
        .arg(format!("-DBRIDGE_FLAVOR=\"{}\"", flavor))
        .status()
        .expect("compile transitional bridge");
    if !status.success() {
        panic!("bridge compilation failed for {}", flavor);
    }

    fs::copy(version_script, artifact_dir.join(format!("libcurl-{}.map", flavor))).unwrap_or_else(
        |err| panic!("copy version script into artifact directory failed: {}", err),
    );
}

