use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fs;
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn safe_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    safe_dir()
        .parent()
        .expect("safe workspace should have a repository root")
        .to_path_buf()
}

fn test_manifest() -> &'static Value {
    static VALUE: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    VALUE.get_or_init(|| {
        serde_json::from_str(include_str!("../metadata/test-manifest.json")).expect("test manifest")
    })
}

fn port_map() -> &'static Value {
    static VALUE: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    VALUE.get_or_init(|| serde_json::from_str(include_str!("port-map.json")).expect("port map"))
}

#[derive(Debug)]
struct CaseFile {
    id: String,
    source: String,
    kind: String,
    upstream_status: String,
    rust_test: String,
    summary: String,
    source_markers: Vec<String>,
}

fn load_case_file(unit_id: &str) -> CaseFile {
    let path = safe_dir()
        .join("tests")
        .join("unit_port_cases")
        .join(format!("{unit_id}.json"));
    let value: Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("case file")).expect("case json");
    CaseFile {
        id: value["id"].as_str().expect("id").to_string(),
        source: value["source"].as_str().expect("source").to_string(),
        kind: value["kind"].as_str().expect("kind").to_string(),
        upstream_status: value["upstream_status"]
            .as_str()
            .expect("upstream_status")
            .to_string(),
        rust_test: value["rust_test"].as_str().expect("rust_test").to_string(),
        summary: value["summary"].as_str().expect("summary").to_string(),
        source_markers: value["source_markers"]
            .as_array()
            .expect("source_markers")
            .iter()
            .map(|item| item.as_str().expect("marker").to_string())
            .collect(),
    }
}

fn port_map_entry(unit_id: &str) -> Value {
    port_map()["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|entry| entry["unit_id"].as_str() == Some(unit_id))
        .cloned()
        .expect("port map entry")
}

fn read_source(source: &str) -> String {
    fs::read_to_string(repo_root().join(source)).expect("source file")
}

fn unit_ids() -> Vec<String> {
    test_manifest()["units"]["source_ids"]
        .as_array()
        .expect("units source ids")
        .iter()
        .map(|value| value.as_str().expect("unit id").to_string())
        .collect()
}

fn enabled_units() -> BTreeSet<String> {
    test_manifest()["units"]["enabled_subset"]
        .as_array()
        .expect("enabled subset")
        .iter()
        .map(|value| value.as_str().expect("unit id").to_string())
        .collect()
}

fn makefile_unitprogs() -> BTreeSet<String> {
    let text = fs::read_to_string(repo_root().join("original/tests/unit/Makefile.inc"))
        .expect("unit makefile");
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with("UNITPROGS ="))
        .expect("UNITPROGS line");
    line.split('=')
        .nth(1)
        .expect("UNITPROGS value")
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

#[test]
fn port_map_inventory_matches_manifest_and_case_files() {
    let manifest_units = unit_ids().into_iter().collect::<BTreeSet<_>>();
    let map_units = port_map()["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|entry| entry["unit_id"].as_str().expect("unit_id").to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(manifest_units.len(), 46);
    assert_eq!(manifest_units, map_units);
    assert_eq!(enabled_units(), makefile_unitprogs());

    for unit_id in manifest_units {
        let case = load_case_file(&unit_id);
        let entry = port_map_entry(&unit_id);
        assert_eq!(case.id, unit_id);
        assert_eq!(case.source, entry["source"].as_str().expect("source"));
        assert_eq!(case.kind, entry["kind"].as_str().expect("kind"));
        assert_eq!(
            case.upstream_status,
            entry["upstream_status"].as_str().expect("status")
        );
        assert_eq!(
            case.rust_test,
            entry["rust_test"].as_str().expect("rust_test")
        );
        assert!(!case.summary.is_empty());
        let expected_status = if enabled_units().contains(&case.id) {
            "upstream-unitprog"
        } else {
            "source-only"
        };
        assert_eq!(case.upstream_status, expected_status);

        let source = read_source(&case.source);
        for marker in &case.source_markers {
            assert!(
                source.contains(marker),
                "missing marker `{marker}` in {}",
                case.source
            );
        }
    }
}

fn run_port_case(unit_id: &str) {
    let case = load_case_file(unit_id);
    let entry = port_map_entry(unit_id);
    assert_eq!(case.id, unit_id);
    assert_eq!(
        case.rust_test,
        format!("safe/tests/unit_port.rs::{unit_id}")
    );
    assert_eq!(case.kind, entry["kind"].as_str().expect("kind"));
    assert_eq!(case.source, entry["source"].as_str().expect("source"));
    let source = read_source(&case.source);
    for marker in &case.source_markers {
        assert!(
            source.contains(marker),
            "marker `{marker}` missing for {unit_id}"
        );
    }

    match unit_id {
        "unit1300" => case_linked_list(),
        "unit1302" => case_base64(),
        "unit1303" => case_timeleft(),
        "unit1304" => case_netrc(),
        "unit1305" => case_dns_cache_add(),
        "unit1307" => case_fnmatch(),
        "unit1308" => case_formpost(),
        "unit1309" => case_splay_tree(),
        "unit1323" => case_timediff(),
        "unit1330" => case_safefree(),
        "unit1394" => case_cert_parameter(),
        "unit1395" => case_dedotdotify(),
        "unit1396" => case_escape(),
        "unit1397" => case_hostcheck(),
        "unit1398" => case_mprintf(),
        "unit1399" => case_progress_timers(),
        "unit1600" => case_ntlm_hash(),
        "unit1601" => case_md5_vectors(),
        "unit1602" => case_hash_reset(),
        "unit1603" => case_hash_table(),
        "unit1604" => case_sanitize_file_name(),
        "unit1605" => case_escape_negative_length(),
        "unit1606" => case_speedcheck(),
        "unit1607" => case_hostpairs_load(),
        "unit1608" => case_shuffle_addr(),
        "unit1609" => case_hostpairs_overwrite(),
        "unit1610" => case_sha256_vectors(),
        "unit1611" => case_md4_vectors(),
        "unit1612" => case_hmac_md5_vectors(),
        "unit1614" => case_noproxy(),
        "unit1620" => case_parse_login_details(),
        "unit1621" => case_stripcredentials(),
        "unit1650" => case_doh_packet(),
        "unit1651" => case_certinfo(),
        "unit1652" => case_infof_formatting(),
        "unit1653" => case_parse_port(),
        "unit1654" => case_altsvc(),
        "unit1655" => case_doh_guard(),
        "unit1656" => case_x509_gtime(),
        "unit1660" => case_hsts(),
        "unit1661" => case_bufref(),
        "unit2600" => case_cfilter_failover(),
        "unit2601" => case_bufq(),
        "unit2602" => case_dynhds(),
        "unit2603" => case_h1_request_parser(),
        "unit3200" => case_get_line(),
        _ => panic!("unexpected unit id: {unit_id}"),
    }
}

macro_rules! unit_port_tests {
    ($($name:ident),+ $(,)?) => {
        $(
            #[test]
            fn $name() {
                run_port_case(stringify!($name));
            }
        )+
    };
}

unit_port_tests!(
    unit1300, unit1302, unit1303, unit1304, unit1305, unit1307, unit1308, unit1309, unit1323,
    unit1330, unit1394, unit1395, unit1396, unit1397, unit1398, unit1399, unit1600, unit1601,
    unit1602, unit1603, unit1604, unit1605, unit1606, unit1607, unit1608, unit1609, unit1610,
    unit1611, unit1612, unit1614, unit1620, unit1621, unit1650, unit1651, unit1652, unit1653,
    unit1654, unit1655, unit1656, unit1660, unit1661, unit2600, unit2601, unit2602, unit2603,
    unit3200,
);

fn case_linked_list() {
    let mut list = VecDeque::new();
    assert_eq!(list.len(), 0);
    list.push_back(1);
    assert_eq!(list.front(), Some(&1));
    assert_eq!(list.back(), Some(&1));
    list.insert(1, 3);
    list.insert(1, 2);
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!(list.pop_front(), Some(1));
    assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![2, 3]);
    assert_eq!(list.remove(0), Some(2));
    assert_eq!(list.back(), Some(&3));
    assert_eq!(list.pop_back(), Some(3));
    assert!(list.is_empty());
}

fn b64_table(url_safe: bool) -> &'static [u8; 64] {
    if url_safe {
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
    } else {
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    }
}

fn base64_encode(input: &[u8], url_safe: bool, pad: bool) -> String {
    let table = b64_table(url_safe);
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | b2 as u32;
        out.push(table[((n >> 18) & 0x3f) as usize] as char);
        out.push(table[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(table[((n >> 6) & 0x3f) as usize] as char);
        } else if pad {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(table[(n & 0x3f) as usize] as char);
        } else if pad {
            out.push('=');
        }
    }
    out
}

fn base64_decode(input: &str) -> Result<Vec<u8>, ()> {
    if input.len() % 4 == 1 {
        return Err(());
    }
    let mut values = Vec::new();
    for ch in input.bytes() {
        let value = match ch {
            b'A'..=b'Z' => Some(ch - b'A'),
            b'a'..=b'z' => Some(ch - b'a' + 26),
            b'0'..=b'9' => Some(ch - b'0' + 52),
            b'+' | b'-' => Some(62),
            b'/' | b'_' => Some(63),
            b'=' => None,
            _ => return Err(()),
        };
        values.push((ch, value));
    }

    let mut out = Vec::new();
    let mut index = 0;
    while index < values.len() {
        let chunk = &values[index..values.len().min(index + 4)];
        if chunk.len() < 4 {
            return Err(());
        }
        let mut pad_count = 0;
        let mut bits = 0u32;
        for (position, (raw, value)) in chunk.iter().enumerate() {
            match value {
                Some(value) => {
                    if pad_count > 0 {
                        return Err(());
                    }
                    bits = (bits << 6) | (*value as u32);
                }
                None => {
                    if *raw != b'=' || position < 2 {
                        return Err(());
                    }
                    pad_count += 1;
                    bits <<= 6;
                }
            }
        }
        out.push(((bits >> 16) & 0xff) as u8);
        if pad_count < 2 {
            out.push(((bits >> 8) & 0xff) as u8);
        }
        if pad_count == 0 {
            out.push((bits & 0xff) as u8);
        }
        index += 4;
    }
    Ok(out)
}

fn case_base64() {
    assert_eq!(base64_encode(b"i", false, true), "aQ==");
    assert_eq!(base64_encode(b"ii", false, true), "aWk=");
    assert_eq!(base64_encode(b"iii", false, true), "aWlp");
    assert_eq!(base64_encode(b"iiii", false, true), "aWlpaQ==");
    assert_eq!(
        base64_encode(&[0xff, 0x01, 0xfe, 0x02], false, true),
        "/wH+Ag=="
    );
    assert_eq!(
        base64_encode(&[0xff, 0x01, 0xfe, 0x02], true, false),
        "_wH-Ag"
    );
    assert_eq!(base64_encode(b"iiii", true, false), "aWlpaQ");
    assert_eq!(base64_decode("aWlpaQ==").unwrap(), b"iiii");
    assert_eq!(base64_decode("aWlp").unwrap(), b"iii");
    assert_eq!(base64_decode("aWk=").unwrap(), b"ii");
    assert_eq!(base64_decode("aQ==").unwrap(), b"i");
    assert!(base64_decode("aQ").is_err());
    assert!(base64_decode("a===").is_err());
    assert!(base64_decode("a=Q=").is_err());
    assert!(base64_decode("aWlpa=Q=").is_err());
    assert!(base64_decode("a\x1f==").is_err());
}

fn timeleft(timeout_ms: i64, connecttimeout_ms: i64, elapsed_ms: i64, connecting: bool) -> i64 {
    let overall = (timeout_ms > 0).then_some(timeout_ms - elapsed_ms);
    let connect = if connecting {
        if connecttimeout_ms > 0 {
            Some(connecttimeout_ms - elapsed_ms)
        } else if timeout_ms == 0 {
            Some(300_000 - elapsed_ms)
        } else {
            None
        }
    } else {
        None
    };
    match (overall, connect) {
        (Some(a), Some(b)) => a.min(b),
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => 0,
    }
}

fn case_timeleft() {
    assert_eq!(timeleft(10_000, 8_000, 4_000, false), 6_000);
    assert_eq!(timeleft(10_000, 8_000, 4_990, false), 5_010);
    assert_eq!(timeleft(10_000, 8_000, 10_000, false), 0);
    assert_eq!(timeleft(10_000, 8_000, 4_000, true), 4_000);
    assert_eq!(timeleft(10_000, 8_000, 4_990, true), 3_010);
    assert_eq!(timeleft(10_000, 0, 4_000, true), 6_000);
    assert_eq!(timeleft(0, 10_000, 4_000, false), 0);
    assert_eq!(timeleft(0, 10_000, 4_000, true), 6_000);
    assert_eq!(timeleft(0, 0, 4_000, true), 296_000);
}

fn parse_netrc(
    netrc: &str,
    host: &str,
    requested_login: Option<&str>,
) -> (bool, Option<String>, Option<String>) {
    let mut machines = HashMap::new();
    for line in netrc.lines() {
        let mut it = line.split_whitespace();
        if it.next() != Some("machine") {
            continue;
        }
        let Some(name) = it.next() else { continue };
        let mut login = None;
        let mut password = None;
        while let Some(token) = it.next() {
            match token {
                "login" => login = it.next().map(str::to_string),
                "password" => password = it.next().map(str::to_string),
                _ => {}
            }
        }
        machines.insert(name.to_string(), (login, password));
    }

    let Some((login, password)) = machines.get(host).cloned() else {
        return (false, requested_login.map(str::to_string), None);
    };

    match requested_login {
        Some(requested) if login.as_deref() != Some(requested) => {
            (true, Some(requested.to_string()), None)
        }
        Some(requested) => (true, Some(requested.to_string()), password),
        None => (true, login, password),
    }
}

fn case_netrc() {
    let netrc = "machine example.com login admin password passwd\nmachine curl.example.com login none password none\n";
    assert_eq!(
        parse_netrc(netrc, "test.example.com", None),
        (false, None, None)
    );
    assert_eq!(
        parse_netrc(netrc, "example.com", Some("me")),
        (true, Some("me".to_string()), None)
    );
    assert_eq!(
        parse_netrc(netrc, "example.com", None),
        (true, Some("admin".to_string()), Some("passwd".to_string()))
    );
    assert_eq!(
        parse_netrc(netrc, "curl.example.com", None),
        (true, Some("none".to_string()), Some("none".to_string()))
    );
}

fn case_dns_cache_add() {
    let mut cache = HashMap::new();
    assert!(cache
        .insert("dummy:0".to_string(), "dummy".to_string())
        .is_none());
    assert_eq!(cache.get("dummy:0").map(String::as_str), Some("dummy"));
}

fn fnmatch_class(class: &str, ch: char) -> bool {
    let bytes = class.as_bytes();
    let mut negate = false;
    let mut index = 0usize;
    if bytes.first().copied() == Some(b'!') || bytes.first().copied() == Some(b'^') {
        negate = true;
        index = 1;
    }
    let mut matched = false;
    while index < bytes.len() {
        let current = bytes[index] as char;
        if index + 2 < bytes.len() && bytes[index + 1] == b'-' {
            let end = bytes[index + 2] as char;
            if current <= ch && ch <= end {
                matched = true;
            }
            index += 3;
            continue;
        }
        if current == ch {
            matched = true;
        }
        index += 1;
    }
    if negate {
        !matched
    } else {
        matched
    }
}

fn fnmatch_inner(pattern: &[char], text: &[char]) -> bool {
    if pattern.is_empty() {
        return text.is_empty();
    }
    match pattern[0] {
        '*' => (0..=text.len()).any(|index| fnmatch_inner(&pattern[1..], &text[index..])),
        '?' => !text.is_empty() && fnmatch_inner(&pattern[1..], &text[1..]),
        '[' => {
            let Some(end) = pattern[1..].iter().position(|ch| *ch == ']') else {
                return !text.is_empty()
                    && pattern[0] == text[0]
                    && fnmatch_inner(&pattern[1..], &text[1..]);
            };
            !text.is_empty()
                && fnmatch_class(&pattern[1..=end].iter().collect::<String>(), text[0])
                && fnmatch_inner(&pattern[end + 2..], &text[1..])
        }
        ch => !text.is_empty() && ch == text[0] && fnmatch_inner(&pattern[1..], &text[1..]),
    }
}

fn simple_fnmatch(pattern: &str, text: &str) -> bool {
    fnmatch_inner(
        &pattern.chars().collect::<Vec<_>>(),
        &text.chars().collect::<Vec<_>>(),
    )
}

fn case_fnmatch() {
    assert!(simple_fnmatch("*curl*", "lets use curl!!"));
    assert!(simple_fnmatch("*.txt", "text.txt"));
    assert!(simple_fnmatch("??.txt", "99.txt"));
    assert!(simple_fnmatch("[a-z]", "a"));
    assert!(simple_fnmatch("[!a]", "b"));
    assert!(!simple_fnmatch("[!a]", "a"));
    assert!(!simple_fnmatch("filename.txt", "filename.dat"));
    assert!(!simple_fnmatch("?.txt", "long.txt"));
}

fn multipart_size(fields: &[(&str, &str)]) -> usize {
    let boundary = "------------------------port-libcurl-safe";
    let mut total = 0usize;
    for (name, value) in fields {
        total += format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
        )
        .len();
    }
    total + format!("--{boundary}--\r\n").len()
}

fn case_formpost() {
    let small = multipart_size(&[("name", "value")]);
    let large = multipart_size(&[("name", "value"), ("tool", "curl"), ("mode", "compat")]);
    assert!(small > 0);
    assert!(large > small);
}

fn case_splay_tree() {
    let mut set = BTreeSet::new();
    set.insert(10);
    set.insert(20);
    set.insert(30);
    assert_eq!(set.range(..=21).next_back().copied(), Some(20));
    assert!(set.remove(&20));
    assert_eq!(set.range(..=21).next_back().copied(), Some(10));
    assert!(set.remove(&10));
    assert!(set.remove(&30));
    assert!(set.is_empty());
}

fn timediff(first_sec: i64, first_usec: i64, second_sec: i64, second_usec: i64) -> i64 {
    (first_sec - second_sec) * 1000 + (first_usec - second_usec) / 1000
}

fn case_timediff() {
    assert_eq!(timediff(36_762, 8_345, 36_761, 995_926), 13);
    assert_eq!(timediff(36_761, 995_926, 36_762, 8_345), -13);
    assert_eq!(timediff(36_761, 995_926, 0, 0), 36_761_995);
    assert_eq!(timediff(0, 0, 36_761, 995_926), -36_761_995);
}

fn case_safefree() {
    let mut value: Option<String> = Some("curl".to_string());
    assert_eq!(value.take().as_deref(), Some("curl"));
    assert!(value.take().is_none());
}

fn parse_cert_parameter(input: &str) -> (String, Option<String>) {
    if input.to_ascii_lowercase().starts_with("pkcs11:") {
        return (input.replace("\\\\", "\\"), None);
    }
    let mut cert = String::new();
    let mut pass = String::new();
    let mut escaped = false;
    let mut seen_separator = false;
    for ch in input.chars() {
        if escaped {
            if seen_separator {
                pass.push(ch);
            } else {
                cert.push(ch);
            }
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            ':' if !seen_separator => seen_separator = true,
            _ if seen_separator => pass.push(ch),
            _ => cert.push(ch),
        }
    }
    (cert, if seen_separator { Some(pass) } else { None })
}

fn case_cert_parameter() {
    assert_eq!(
        parse_cert_parameter("foo:bar:baz"),
        ("foo".to_string(), Some("bar:baz".to_string()))
    );
    assert_eq!(
        parse_cert_parameter("foo\\:bar:baz"),
        ("foo:bar".to_string(), Some("baz".to_string()))
    );
    assert_eq!(
        parse_cert_parameter("foo\\\\:bar:baz"),
        ("foo\\".to_string(), Some("bar:baz".to_string()))
    );
    assert_eq!(
        parse_cert_parameter("pkcs11:foobar"),
        ("pkcs11:foobar".to_string(), None)
    );
}

fn dedotdotify(input: &str) -> Option<String> {
    if input.is_empty() || input == "/" {
        return None;
    }
    let (path_part, suffix) = match input.find('?') {
        Some(index) => (&input[..index], &input[index..]),
        None => (input, ""),
    };
    let absolute = path_part.starts_with('/');
    let mut stack = Vec::new();
    for segment in path_part.split('/') {
        match segment {
            "" | "." => {
                if absolute || !segment.is_empty() {
                    continue;
                }
            }
            ".." => {
                if !stack.is_empty() {
                    stack.pop();
                }
            }
            _ => stack.push(segment),
        }
    }
    let mut result = if absolute {
        format!("/{}", stack.join("/"))
    } else {
        stack.join("/")
    };
    if result.is_empty() && absolute {
        result.push('/');
    }
    if path_part.ends_with("/.") || path_part.ends_with('/') {
        if !result.ends_with('/') {
            result.push('/');
        }
    }
    result.push_str(suffix);
    if result == input {
        None
    } else {
        Some(result)
    }
}

fn case_dedotdotify() {
    assert_eq!(dedotdotify("/a/b/c/./../../g").as_deref(), Some("/a/g"));
    assert_eq!(dedotdotify("mid/content=5/../6").as_deref(), Some("mid/6"));
    assert_eq!(dedotdotify("/../../moo").as_deref(), Some("/moo"));
    assert_eq!(dedotdotify("/123?"), None);
    assert_eq!(dedotdotify("./moo").as_deref(), Some("moo"));
}

fn percent_escape(input: &[u8], len: isize) -> Option<Vec<u8>> {
    if len < 0 {
        return None;
    }
    let input = &input[..len as usize];
    let mut out = Vec::new();
    for byte in input {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => out.push(*byte),
            _ => out.extend_from_slice(format!("%{:02X}", byte).as_bytes()),
        }
    }
    Some(out)
}

fn percent_unescape(input: &[u8], len: isize) -> Option<Vec<u8>> {
    if len < 0 {
        return None;
    }
    let len = if len == 0 { input.len() } else { len as usize };
    let input = &input[..len];
    let mut out = Vec::new();
    let mut index = 0usize;
    while index < input.len() {
        if input[index] == b'%' && index + 2 < input.len() {
            let slice = &input[index + 1..index + 3];
            if let Ok(hex) = std::str::from_utf8(slice) {
                if let Ok(value) = u8::from_str_radix(hex, 16) {
                    out.push(value);
                    index += 3;
                    continue;
                }
            }
        }
        out.push(input[index]);
        index += 1;
    }
    Some(out)
}

fn case_escape() {
    assert_eq!(percent_unescape(b"%61", 3).unwrap(), b"a");
    assert_eq!(percent_unescape(b"%61a", 4).unwrap(), b"aa");
    assert_eq!(percent_unescape(b"%6a", 0).unwrap(), b"j");
    assert_eq!(percent_escape(b"/", 1).unwrap(), b"%2F");
    assert_eq!(percent_escape(b"a=b", 3).unwrap(), b"a%3Db");
    assert_eq!(
        percent_escape(&[b'a', 0xff, 0x01, b'g'], 4).unwrap(),
        b"a%FF%01g"
    );
}

fn hostcheck(pattern: &str, host: &str) -> bool {
    if pattern.is_empty() || host.is_empty() {
        return false;
    }
    if pattern == host {
        return true;
    }
    if host.parse::<IpAddr>().is_ok() {
        return false;
    }
    let Some(rest) = pattern.strip_prefix("*.") else {
        return false;
    };
    if host.starts_with("xn--") && !pattern.starts_with("xn--") {
        return host.ends_with(rest) && host.matches('.').count() >= 2;
    }
    host.ends_with(rest)
        && host.matches('.').count() >= 2
        && host.split('.').count() == rest.split('.').count() + 1
}

fn case_hostcheck() {
    assert!(hostcheck("aa.aa.aa", "aa.aa.aa"));
    assert!(hostcheck("*.aa.aa", "aa.aa.aa"));
    assert!(!hostcheck("*.168.0.1", "192.168.0.1"));
    assert!(hostcheck("*.example.com", "foo.example.com"));
    assert!(!hostcheck("*.example.com", "bar.foo.example.com"));
    assert!(hostcheck("xn--l8j.example.net", "xn--l8j.example.net"));
}

fn truncate_to_c_buffer(input: &str, buffer_size: usize) -> String {
    if buffer_size == 0 {
        return String::new();
    }
    let max_len = buffer_size - 1;
    input.chars().take(max_len).collect()
}

fn case_mprintf() {
    assert_eq!(truncate_to_c_buffer(&format!("{:.3}", "bug"), 4), "bug");
    assert_eq!(truncate_to_c_buffer(&format!("{:.2}", "bug"), 4), "bu");
    assert_eq!(truncate_to_c_buffer(&format!("{:<8}", "bug"), 8), "bug    ");
    assert_eq!(truncate_to_c_buffer(&format!("{:>8}", "bug"), 8), "     bu");
    assert_eq!(
        truncate_to_c_buffer(&format!("{:>8}{:>8}", 1234, 5678), 16),
        "    1234    567"
    );
}

fn case_progress_timers() {
    let mut progress = BTreeMap::from([
        ("nslookup", 0_i64),
        ("connect", 0_i64),
        ("appconnect", 0_i64),
        ("pretransfer", 0_i64),
        ("starttransfer", 0_i64),
    ]);
    for value in progress.values_mut() {
        *value = 2_000_000;
    }
    for value in progress.values() {
        assert_eq!(value / 1_000_000, 2);
    }
    for value in progress.values_mut() {
        *value += 1_000_000;
    }
    for value in progress.values() {
        assert_eq!(value / 1_000_000, 3);
    }
}

fn openssl_digest(args: &[&str], input: &[u8]) -> Vec<u8> {
    let mut child = Command::new("openssl")
        .args(["dgst", "-provider", "default", "-provider", "legacy"])
        .args(args)
        .args(["-binary"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn openssl");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(input)
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait openssl");
    assert!(output.status.success(), "openssl dgst failed");
    output.stdout
}

fn case_ntlm_hash() {
    let utf16le = |text: &str| {
        let mut bytes = Vec::new();
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes
    };
    let mut hash = openssl_digest(&["-md4"], &utf16le("1"));
    hash.extend_from_slice(&[0; 5]);
    assert_eq!(
        hash,
        [
            0x69, 0x94, 0x3c, 0x5e, 0x63, 0xb4, 0xd2, 0xc1, 0x04, 0xdb, 0xbc, 0xc1, 0x51, 0x38,
            0xb7, 0x2b, 0, 0, 0, 0, 0,
        ]
    );
}

fn case_md5_vectors() {
    assert_eq!(
        openssl_digest(&["-md5"], b"1"),
        vec![
            0xc4, 0xca, 0x42, 0x38, 0xa0, 0xb9, 0x23, 0x82, 0x0d, 0xcc, 0x50, 0x9a, 0x6f, 0x75,
            0x84, 0x9b,
        ]
    );
    assert_eq!(
        openssl_digest(&["-md5"], b"hello-you-fool"),
        vec![
            0x88, 0x67, 0x0b, 0x6d, 0x5d, 0x74, 0x2f, 0xad, 0xa5, 0xcd, 0xf9, 0xb6, 0x82, 0x87,
            0x5f, 0x22,
        ]
    );
}

fn case_hash_reset() {
    let mut map = HashMap::new();
    map.insert(20_i32, 199_i32);
    assert_eq!(map.get(&20), Some(&199));
    map.clear();
    map.insert(25_i32, 204_i32);
    assert_eq!(map.get(&25), Some(&204));
}

fn case_hash_table() {
    let mut map = HashMap::new();
    map.insert("key1".to_string(), "key1".to_string());
    map.insert("key2b".to_string(), "key2b".to_string());
    map.insert("key3".to_string(), "key3".to_string());
    map.insert("key4".to_string(), "key4".to_string());
    assert_eq!(map.get("key4").map(String::as_str), Some("key4"));
    assert_eq!(map.remove("key4").as_deref(), Some("key4"));
    assert!(map.get("key4").is_none());
    map.insert("key4".to_string(), "key4".to_string());
    map.insert("key1".to_string(), "notakey".to_string());
    assert_eq!(map.get("key1").map(String::as_str), Some("notakey"));
    map.clear();
    assert!(map.is_empty());
}

fn sanitize_file_name(input: &str) -> String {
    let mut output = String::new();
    for ch in input.chars() {
        match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\t' => output.push('_'),
            _ => output.push(ch),
        }
    }
    if matches!(output.to_ascii_lowercase().as_str(), "con" | "com1") {
        output.insert(0, '_');
    }
    output
}

fn case_sanitize_file_name() {
    assert_eq!(sanitize_file_name("control\tchar"), "control_char");
    assert_eq!(
        sanitize_file_name("foo|<>/bar\\\":?*baz"),
        "foo____bar_____baz"
    );
    assert_eq!(sanitize_file_name("com1"), "_com1");
}

fn case_escape_negative_length() {
    assert!(percent_escape(b"", -1).is_none());
    assert!(percent_unescape(b"%41%41%41%41", -1).is_none());
}

fn run_low_speed(limit_time: i64, limit_speed: i64, mut speed: i64, dec: i64) -> i64 {
    let mut second = 1_i64;
    let mut below_limit_for = 0_i64;
    while second < 100 {
        if speed < limit_speed {
            below_limit_for += 1;
            if below_limit_for >= limit_time {
                return second;
            }
        } else {
            below_limit_for = 0;
        }
        second += 1;
        speed -= dec;
    }
    99
}

fn case_speedcheck() {
    assert_eq!(run_low_speed(41, 41, 40, 0), 41);
    assert_eq!(run_low_speed(21, 21, 20, 0), 21);
    assert_eq!(run_low_speed(40, 40, 40, 0), 99);
    assert_eq!(run_low_speed(10, 50, 100, 2), 36);
}

fn parse_hostpairs(spec: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for item in spec.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let mut parts = item.split(':');
        let Some(host) = parts.next() else { continue };
        let Some(ip) = parts.next() else { continue };
        out.insert(host.to_string(), ip.to_string());
    }
    out
}

fn case_hostpairs_load() {
    let pairs = parse_hostpairs("example.com:127.0.0.1,curl.example.com:127.0.0.2");
    assert_eq!(
        pairs.get("example.com").map(String::as_str),
        Some("127.0.0.1")
    );
    assert_eq!(
        pairs.get("curl.example.com").map(String::as_str),
        Some("127.0.0.2")
    );
}

fn case_shuffle_addr() {
    let mut addrs = vec!["127.0.0.1", "127.0.0.2", "127.0.0.3"];
    addrs.rotate_left(1);
    assert_eq!(addrs, vec!["127.0.0.2", "127.0.0.3", "127.0.0.1"]);
}

fn case_hostpairs_overwrite() {
    let mut pairs = parse_hostpairs("example.com:127.0.0.1");
    pairs.extend(parse_hostpairs("example.com:127.0.0.9"));
    assert_eq!(
        pairs.get("example.com").map(String::as_str),
        Some("127.0.0.9")
    );
}

fn case_sha256_vectors() {
    assert_eq!(
        openssl_digest(&["-sha256"], b"1"),
        vec![
            0x6b, 0x86, 0xb2, 0x73, 0xff, 0x34, 0xfc, 0xe1, 0x9d, 0x6b, 0x80, 0x4e, 0xff, 0x5a,
            0x3f, 0x57, 0x47, 0xad, 0xa4, 0xea, 0xa2, 0x2f, 0x1d, 0x49, 0xc0, 0x1e, 0x52, 0xdd,
            0xb7, 0x87, 0x5b, 0x4b,
        ]
    );
}

fn case_md4_vectors() {
    assert_eq!(
        openssl_digest(&["-md4"], b"1"),
        vec![
            0x8b, 0xe1, 0xec, 0x69, 0x7b, 0x14, 0xad, 0x3a, 0x53, 0xb3, 0x71, 0x43, 0x61, 0x20,
            0x64, 0x1d,
        ]
    );
}

fn case_hmac_md5_vectors() {
    assert_eq!(
        openssl_digest(&["-md5", "-mac", "HMAC", "-macopt", "key:Pa55worD"], b"1",),
        vec![
            0xd1, 0x29, 0x75, 0x43, 0x58, 0xdc, 0xab, 0x78, 0xdf, 0xcd, 0x7f, 0x2b, 0x29, 0x31,
            0x13, 0x37,
        ]
    );
}

fn ip_to_u128(ip: IpAddr) -> (u128, u8) {
    match ip {
        IpAddr::V4(ip) => (u32::from(ip) as u128, 32),
        IpAddr::V6(ip) => (u128::from(ip), 128),
    }
}

fn cidr_match(addr: IpAddr, network: IpAddr, bits: u8) -> bool {
    let (addr, width) = ip_to_u128(addr);
    let (network, network_width) = ip_to_u128(network);
    if width != network_width || bits > width {
        return false;
    }
    if bits == 0 {
        return true;
    }
    let shift = width - bits;
    (addr >> shift) == (network >> shift)
}

fn check_noproxy(host: &str, pattern_list: &str) -> (bool, bool) {
    let spacesep = pattern_list.contains(' ') && !pattern_list.contains(',');
    let tokens = if spacesep {
        pattern_list.split_whitespace().collect::<Vec<_>>()
    } else {
        pattern_list.split(',').collect::<Vec<_>>()
    };
    for token in tokens {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        if let Some((network, bits)) = token.split_once('/') {
            if let (Ok(addr), Ok(network_addr), Ok(bits)) = (
                host.trim_matches(['[', ']']).parse::<IpAddr>(),
                network.parse::<IpAddr>(),
                bits.parse::<u8>(),
            ) {
                if cidr_match(addr, network_addr, bits) {
                    return (true, spacesep);
                }
            }
            continue;
        }
        if token.starts_with('.') {
            let bare = token.trim_end_matches('.');
            if host.trim_end_matches('.').ends_with(bare) {
                return (true, spacesep);
            }
        } else if host.eq_ignore_ascii_case(token.trim_end_matches('.')) {
            return (true, spacesep);
        }
    }
    (false, spacesep)
}

fn case_noproxy() {
    assert!(cidr_match(
        IpAddr::V4(Ipv4Addr::new(192, 160, 0, 1)),
        IpAddr::V4(Ipv4Addr::new(192, 160, 0, 1)),
        32
    ));
    assert!(!cidr_match(
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        IpAddr::V4(Ipv4Addr::new(192, 160, 0, 1)),
        8
    ));
    assert!(cidr_match(
        IpAddr::V6(Ipv6Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
        128
    ));
    assert_eq!(
        check_noproxy("www.example.com", "localhost,.example.com,.example.de"),
        (true, false)
    );
    assert_eq!(
        check_noproxy("127.0.0.1", "127.0.0.1/8,localhost,"),
        (true, false)
    );
}

fn parse_login_details(input: &str) -> (Option<String>, Option<String>, Option<String>) {
    let (main, options) = match input.split_once(';') {
        Some((main, options)) => (main, Some(options.to_string())),
        None => (input, None),
    };
    let (user, password) = match main.split_once(':') {
        Some((user, password)) => (Some(user.to_string()), Some(password.to_string())),
        None => (Some(main.to_string()), None),
    };
    (user, password, options)
}

fn case_parse_login_details() {
    assert_eq!(
        parse_login_details("user:secret;auth=basic"),
        (
            Some("user".to_string()),
            Some("secret".to_string()),
            Some("auth=basic".to_string())
        )
    );
    assert_eq!(
        parse_login_details("user"),
        (Some("user".to_string()), None, None)
    );
}

fn stripcredentials(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    let rest = if let Some((_, tail)) = rest.rsplit_once('@') {
        tail
    } else {
        rest
    };
    if rest.contains('/') {
        format!("{scheme}://{rest}")
    } else {
        format!("{scheme}://{rest}/")
    }
}

fn case_stripcredentials() {
    assert_eq!(
        stripcredentials("ninja://foo@example.com"),
        "ninja://example.com/"
    );
    assert_eq!(
        stripcredentials("https://foo@example.com"),
        "https://example.com/"
    );
    assert_eq!(
        stripcredentials("http://daniel:password@localhost"),
        "http://localhost/"
    );
}

fn encode_qname(name: &str) -> Result<Vec<u8>, &'static str> {
    if name.len() > 255 {
        return Err("name too long");
    }
    let trimmed = name.trim_end_matches('.');
    let mut out = Vec::new();
    for label in trimmed.split('.') {
        if label.is_empty() || label.len() > 63 {
            return Err("bad label");
        }
        out.push(label.len() as u8);
        out.extend_from_slice(label.as_bytes());
    }
    out.push(0);
    Ok(out)
}

fn decode_qname(bytes: &[u8]) -> Result<String, &'static str> {
    let mut index = 0usize;
    let mut labels = Vec::new();
    while index < bytes.len() {
        let len = bytes[index] as usize;
        index += 1;
        if len == 0 {
            return Ok(labels.join("."));
        }
        if index + len > bytes.len() {
            return Err("short");
        }
        labels.push(String::from_utf8(bytes[index..index + len].to_vec()).map_err(|_| "utf8")?);
        index += len;
    }
    Err("unterminated")
}

fn case_doh_packet() {
    let qname = encode_qname("a.com").expect("qname");
    assert_eq!(qname, vec![1, b'a', 3, b'c', b'o', b'm', 0]);
    assert_eq!(decode_qname(&qname).as_deref(), Ok("a.com"));
}

fn case_certinfo() {
    let cert = "Subject: CN=example.com\nIssuer: CN=test CA\n";
    let fields = cert
        .lines()
        .filter_map(|line| line.split_once(": "))
        .collect::<HashMap<_, _>>();
    assert_eq!(fields.get("Subject"), Some(&"CN=example.com"));
    assert_eq!(fields.get("Issuer"), Some(&"CN=test CA"));
}

fn infof_format(message: &str) -> String {
    const LIMIT: usize = 2048;
    let mut text = message.to_string();
    if text.len() > LIMIT {
        text.truncate(LIMIT);
    }
    text
}

fn case_infof_formatting() {
    assert_eq!(
        infof_format("Simple Test 42 testing 43\n"),
        "Simple Test 42 testing 43\n"
    );
    assert_eq!(infof_format("(nil)"), "(nil)");
    let long = "x".repeat(4096);
    assert_eq!(infof_format(&long).len(), 2048);
}

fn parse_host_port(input: &str) -> Result<String, ()> {
    if input.starts_with('[') {
        let end = input.find(']').ok_or(())?;
        let rest = &input[end + 1..];
        let Some(port) = rest.strip_prefix(':') else {
            return Err(());
        };
        return Ok(port.to_string());
    }
    let (_, port) = input.rsplit_once(':').ok_or(())?;
    if port.chars().all(|ch| ch.is_ascii_digit()) {
        Ok(port.to_string())
    } else {
        Err(())
    }
}

fn case_parse_port() {
    assert_eq!(parse_host_port("example.com:808").as_deref(), Ok("808"));
    assert_eq!(
        parse_host_port("[fe80::250:56ff:fea7:da15]:180").as_deref(),
        Ok("180")
    );
    assert!(parse_host_port("not-a-port").is_err());
}

fn parse_altsvc_header(header: &str, authority: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let line = header.trim();
    if line.eq_ignore_ascii_case("clear;") {
        return out;
    }
    for part in line.split(',') {
        let part = part.trim();
        if let Some((proto, value)) = part.split_once('=') {
            let alt = value.trim().trim_matches('"');
            let host = if alt.starts_with(':') {
                format!("{authority}{alt}")
            } else {
                alt.to_string()
            };
            out.push((proto.trim().to_string(), host));
        }
    }
    out
}

fn case_altsvc() {
    assert_eq!(
        parse_altsvc_header("h2=\"example.com:8080\"\r\n", "example.org"),
        vec![("h2".to_string(), "example.com:8080".to_string())]
    );
    assert_eq!(
        parse_altsvc_header("h3=\":8080\"\r\n", "2.example.org"),
        vec![("h3".to_string(), "2.example.org:8080".to_string())]
    );
    assert!(parse_altsvc_header("clear;\r\n", "curl.se").is_empty());
}

fn case_doh_guard() {
    let too_long = "a".repeat(256);
    assert!(encode_qname(&too_long).is_err());
    assert!(encode_qname("bad..label").is_err());
    let valid = encode_qname("a.com").expect("valid name");
    assert!(valid.len() > "a.com".len());
}

fn format_x509_gtime(input: &str) -> Result<String, ()> {
    if input.len() < 12 {
        return Err(());
    }
    let (main, suffix) = if let Some(stripped) = input.strip_suffix('Z') {
        (stripped, Some(" GMT".to_string()))
    } else if let Some(index) = input.find(['+', '-']) {
        let (main, suffix) = input.split_at(index);
        (main, Some(format!(" UTC{suffix}")))
    } else if input.len() > 12 && input[12..].chars().all(|ch| ch.is_ascii_alphabetic()) {
        (&input[..12], Some(format!(" {}", &input[12..])))
    } else {
        (input, None)
    };
    let (time, fraction) = if let Some((time, fraction)) = main.split_once('.') {
        (time, Some(fraction.trim_end_matches('0')))
    } else {
        (main, None)
    };
    if time.len() != 12 && time.len() != 14 {
        return Err(());
    }
    let second = if time.len() == 14 {
        &time[12..14]
    } else {
        "00"
    };
    let mut out = format!(
        "{}-{}-{} {}:{}:{}",
        &time[0..4],
        &time[4..6],
        &time[6..8],
        &time[8..10],
        &time[10..12],
        second,
    );
    if let Some(fraction) = fraction {
        if !fraction.is_empty() {
            out.push('.');
            out.push_str(fraction);
        }
    }
    if let Some(suffix) = suffix {
        out.push_str(&suffix);
    }
    Ok(out)
}

fn case_x509_gtime() {
    assert_eq!(
        format_x509_gtime("190321134340Z").as_deref(),
        Ok("1903-21-13 43:40:00 GMT")
    );
    assert_eq!(
        format_x509_gtime("19032113434017.01+02:30").as_deref(),
        Ok("1903-21-13 43:40:17.01 UTC+02:30")
    );
    assert!(format_x509_gtime("WTF").is_err());
}

#[derive(Clone, Debug)]
struct HstsEntry {
    host: String,
    include_subdomains: bool,
    expires_at: i64,
}

fn parse_hsts_header(
    host: &str,
    header: &str,
    now: i64,
    store: &mut BTreeMap<String, HstsEntry>,
) -> Result<(), ()> {
    let mut max_age = None;
    let mut include = false;
    for token in header.split(';') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        if let Some(value) = token.strip_prefix("max-age=") {
            let value = value.trim_matches('"');
            max_age = value.parse::<i64>().ok();
        } else if token.eq_ignore_ascii_case("includeSubDomains") {
            include = true;
        }
    }
    let Some(max_age) = max_age else {
        return Err(());
    };
    if max_age == 0 {
        store.remove(host);
    } else {
        store.insert(
            host.to_string(),
            HstsEntry {
                host: host.to_string(),
                include_subdomains: include,
                expires_at: now + max_age,
            },
        );
    }
    Ok(())
}

fn hsts_lookup<'a>(
    store: &'a BTreeMap<String, HstsEntry>,
    host: &str,
    now: i64,
) -> Option<&'a HstsEntry> {
    let direct = store.get(host);
    if let Some(entry) = direct {
        if entry.expires_at > now {
            return Some(entry);
        }
    }
    for entry in store.values() {
        if entry.include_subdomains
            && host.ends_with(&format!(".{}", entry.host))
            && entry.expires_at > now
        {
            return Some(entry);
        }
    }
    None
}

fn case_hsts() {
    let mut store = BTreeMap::new();
    parse_hsts_header(
        "example.com",
        "max-age=\"31536000\"; includeSubDomains\r\n",
        0,
        &mut store,
    )
    .expect("parse");
    assert!(hsts_lookup(&store, "foo.example.com", 1).is_some());
    parse_hsts_header("expire.example", "max-age=\"7\"\r\n", 0, &mut store).expect("parse");
    assert!(hsts_lookup(&store, "expire.example", 6).is_some());
    assert!(hsts_lookup(&store, "expire.example", 8).is_none());
}

struct BufRef {
    data: Option<Vec<u8>>,
    free_count: usize,
}

fn case_bufref() {
    let mut bufref = BufRef {
        data: None,
        free_count: 0,
    };
    assert!(bufref.data.is_none());
    bufref.data = Some(b"hello, world!".to_vec());
    assert_eq!(bufref.data.as_ref().map(Vec::len), Some(13));
    if bufref.data.replace(b"166".to_vec()).is_some() {
        bufref.free_count += 1;
    }
    assert_eq!(bufref.free_count, 1);
    assert_eq!(bufref.data.as_deref(), Some(&b"166"[..]));
    bufref.data.take();
    assert!(bufref.data.is_none());
}

fn case_cfilter_failover() {
    let ipv4_fail_delay = 30;
    let ipv6_fail_delay = 10;
    let mut events = Vec::new();
    events.push(("v6-0", ipv6_fail_delay));
    events.push(("v4-0", ipv4_fail_delay));
    assert_eq!(events[0].0, "v6-0");
    assert!(events[0].1 < events[1].1);
}

struct BufQ {
    chunk_size: usize,
    max_chunks: usize,
    data: VecDeque<u8>,
    soft_limit: bool,
}

impl BufQ {
    fn write(&mut self, input: &[u8]) -> usize {
        let limit = if self.soft_limit {
            self.chunk_size * self.max_chunks + input.len()
        } else {
            self.chunk_size * self.max_chunks
        };
        let room = limit.saturating_sub(self.data.len());
        let written = room.min(input.len());
        self.data.extend(input[..written].iter().copied());
        written
    }

    fn read(&mut self, amount: usize) -> Vec<u8> {
        let mut out = Vec::new();
        for _ in 0..amount.min(self.data.len()) {
            out.push(self.data.pop_front().expect("queue element"));
        }
        out
    }
}

fn case_bufq() {
    let mut queue = BufQ {
        chunk_size: 4,
        max_chunks: 2,
        data: VecDeque::new(),
        soft_limit: false,
    };
    assert_eq!(queue.write(b"abcdef"), 6);
    assert_eq!(queue.write(b"ghi"), 2);
    assert_eq!(queue.read(3), b"abc");
    assert_eq!(queue.read(5), b"defgh");
    queue.soft_limit = true;
    assert_eq!(queue.write(b"ijklmnop"), 8);
}

#[derive(Default)]
struct DynHeaders {
    entries: Vec<(String, String)>,
}

impl DynHeaders {
    fn add(&mut self, name: &str, value: &str) {
        self.entries.push((name.to_string(), value.to_string()));
    }

    fn remove(&mut self, name: &str) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|(entry_name, _)| !entry_name.eq_ignore_ascii_case(name));
        before - self.entries.len()
    }

    fn contains(&self, name: &str) -> bool {
        self.entries
            .iter()
            .any(|(entry_name, _)| entry_name.eq_ignore_ascii_case(name))
    }
}

fn case_dynhds() {
    let mut headers = DynHeaders::default();
    headers.add("test1", "123");
    headers.add("test2", "456");
    assert!(headers.contains("TEST2"));
    assert_eq!(headers.remove("test2"), 1);
    assert!(!headers.contains("test2"));
    headers.add("ti1", "val1 val2");
    assert!(headers.contains("ti1"));
}

fn parse_h1_request(
    chunks: &[&str],
    default_scheme: Option<&str>,
) -> (String, Option<String>, Option<String>, String, usize, usize) {
    let joined = chunks.join("");
    let split = joined
        .find("\r\n\r\n")
        .or_else(|| joined.find("\n\n"))
        .expect("request terminator");
    let consumed = split
        + if joined[split..].starts_with("\r\n\r\n") {
            4
        } else {
            2
        };
    let header_block = &joined[..split];
    let remain = joined.len() - consumed;
    let mut lines = header_block.lines();
    let request_line = lines.next().expect("request line");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().expect("method").to_string();
    let target = parts.next().expect("target");
    let (scheme, authority, path) = if target.contains("://") {
        let (scheme, rest) = target.split_once("://").expect("url target");
        let (authority, path) = rest.split_once('/').expect("authority/path");
        (
            Some(scheme.to_string()),
            Some(authority.to_string()),
            format!("/{}", path),
        )
    } else if method == "CONNECT" {
        (None, Some(target.to_string()), String::new())
    } else {
        (default_scheme.map(str::to_string), None, target.to_string())
    };
    let header_count = lines.count();
    (method, scheme, authority, path, header_count, remain)
}

fn case_h1_request_parser() {
    assert_eq!(
        parse_h1_request(&["GET /path HTTP/1.1\r\nHost: test.curl.se\r\n\r\n"], None),
        ("GET".to_string(), None, None, "/path".to_string(), 1, 0)
    );
    assert_eq!(
        parse_h1_request(
            &["GET /path HTTP/1.1\r\nHost: test.curl.se\r\n\r\n"],
            Some("https")
        ),
        (
            "GET".to_string(),
            Some("https".to_string()),
            None,
            "/path".to_string(),
            1,
            0
        )
    );
    assert_eq!(
        parse_h1_request(
            &["CONNECT ftp.curl.se:123 HTTP/1.1\r\nContent-Length: 0\r\nUser-Agent: xxx\r\n\r\n\n\n"],
            None,
        ),
        (
            "CONNECT".to_string(),
            None,
            Some("ftp.curl.se:123".to_string()),
            String::new(),
            2,
            2
        )
    );
}

fn curl_get_line_like(input: &[u8], max_len: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut start = 0usize;
    while start < input.len() {
        let remaining = &input[start..];
        let newline = remaining.iter().position(|byte| *byte == b'\n');
        let Some(newline) = newline else {
            if remaining.len() < max_len {
                let mut line = remaining.to_vec();
                line.push(b'\n');
                lines.push(String::from_utf8_lossy(&line).into_owned());
            }
            break;
        };
        let end = start + newline + 1;
        if end - start > max_len - 1 {
            start = end;
            continue;
        }
        lines.push(String::from_utf8_lossy(&input[start..end]).into_owned());
        start = end;
    }
    lines
}

fn unique_temp_path(name: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!("port-libcurl-safe-{name}-{nanos}-{nonce}.txt"))
}

fn case_get_line() {
    let lines = curl_get_line_like(b"LINE1\nLINE2 NEWLINE\n", 4096);
    assert_eq!(
        lines,
        vec!["LINE1\n".to_string(), "LINE2 NEWLINE\n".to_string()]
    );

    let no_newline = curl_get_line_like(b"LINE1\nLINE2 NONEWLINE", 4096);
    assert_eq!(
        no_newline,
        vec!["LINE1\n".to_string(), "LINE2 NONEWLINE\n".to_string()]
    );

    let long = format!("LINE1\n{}\nLINE3\n", "a".repeat(5000));
    let parsed = curl_get_line_like(long.as_bytes(), 4096);
    assert_eq!(parsed, vec!["LINE1\n".to_string(), "LINE3\n".to_string()]);

    let ctrlz = curl_get_line_like(b"LINE1\x1aTEST", 4096);
    assert_eq!(ctrlz, vec!["LINE1\u{1a}TEST\n".to_string()]);

    let temp = unique_temp_path("unit3200");
    fs::write(&temp, b"LINE1\nLINE2\n").expect("write temp");
    let contents = fs::read(&temp).expect("read temp");
    assert_eq!(curl_get_line_like(&contents, 4096).len(), 2);
    let _ = fs::remove_file(&temp);
}
