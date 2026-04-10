#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <abi-manifest.json>" >&2
  exit 2
fi

manifest_path="$1"
if [[ ! -f "${manifest_path}" ]]; then
  echo "missing manifest: ${manifest_path}" >&2
  exit 1
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
safe_dir="$(cd "${script_dir}/.." && pwd)"
layout_test="${safe_dir}/tests/abi_layout.rs"
public_types="${safe_dir}/src/abi/public_types.rs"

python3 - "${manifest_path}" "${layout_test}" "${public_types}" <<'PY'
import json
import pathlib
import re
import sys

manifest_path = pathlib.Path(sys.argv[1])
layout_test = pathlib.Path(sys.argv[2])
public_types = pathlib.Path(sys.argv[3])

manifest = json.loads(manifest_path.read_text())
layout_text = layout_test.read_text()
public_types_text = public_types.read_text()

def extract_array(name: str) -> set[str]:
    match = re.search(
        rf"const\s+{name}:\s*&\[\s*&str\s*\]\s*=\s*&\[(.*?)\];",
        layout_text,
        re.S,
    )
    if not match:
        raise SystemExit(f"missing {name} in {layout_test}")
    return set(re.findall(r'"([^"]+)"', match.group(1)))

layout_structs = extract_array("LAYOUT_STRUCTS")
opaque_structs = extract_array("OPAQUE_STRUCTS")
manifest_structs = set(manifest["public_struct_names"])

if layout_structs | opaque_structs != manifest_structs:
    missing = sorted(manifest_structs - (layout_structs | opaque_structs))
    extra = sorted((layout_structs | opaque_structs) - manifest_structs)
    raise SystemExit(
        f"layout coverage mismatch: missing={missing or '[]'} extra={extra or '[]'}"
    )

defined_types = set(
    name
    for _, name in re.findall(r"pub\s+(struct|union)\s+([A-Za-z_][A-Za-z0-9_]*)", public_types_text)
)
missing_types = sorted(manifest_structs - defined_types)
if missing_types:
    raise SystemExit(f"manifest structs missing from public_types.rs: {missing_types}")

entries = manifest.get("option_metadata", {}).get("entries", [])
if not entries:
    raise SystemExit("option_metadata.entries is empty")

names = [entry["name"] for entry in entries]
if len(names) != len(set(names)):
    raise SystemExit("option_metadata.entries contains duplicate names")
PY
