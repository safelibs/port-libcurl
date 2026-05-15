#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --package-root <dir> --test <name>" >&2
}

package_root=""
test_name=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --package-root)
      package_root="${2:-}"
      shift 2
      ;;
    --test)
      test_name="${2:-}"
      shift 2
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done
[[ -n "${package_root}" && -n "${test_name}" ]] || { usage; exit 2; }

package_root="$(cd "${package_root}" && pwd)"
test_script="${package_root}/debian/tests/${test_name}"
[[ -x "${test_script}" || -f "${test_script}" ]] || {
  echo "missing autopkgtest entrypoint: ${test_script}" >&2
  exit 1
}

if (( EUID != 0 )); then
  echo "run-packaged-autopkgtests.sh must run as root so it can install local debs" >&2
  exit 1
fi

export DEBIAN_FRONTEND=noninteractive
apt-get update
mk-build-deps -ir -t 'apt-get -y --no-install-recommends' "${package_root}/debian/control"

mapfile -t debs < <(
  for deb in "${package_root}"/*.deb "${package_root}/../"*.deb; do
    [[ -e "${deb}" ]] && printf '%s\n' "${deb}"
  done | sort -u
)
((${#debs[@]} > 0)) || { echo "no built debs found for package install" >&2; exit 1; }
apt-get install -y --no-install-recommends "${debs[@]}"

depends_line="$(
  python3 - "${package_root}/debian/tests/control" "${test_name}" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
test_name = sys.argv[2]
paragraphs = path.read_text(encoding="utf-8").split("\n\n")
for paragraph in paragraphs:
    fields = {}
    last = None
    for raw in paragraph.splitlines():
        if not raw.strip():
            continue
        if raw[0].isspace() and last:
            fields[last] += " " + raw.strip()
        else:
            key, value = raw.split(":", 1)
            last = key
            fields[key] = value.strip()
    if fields.get("Tests") == test_name:
        print(fields.get("Depends", ""))
        break
else:
    raise SystemExit(f"unknown autopkgtest: {test_name}")
PY
)"

install_deps=()
IFS=',' read -ra raw_deps <<<"${depends_line}"
for dep in "${raw_deps[@]}"; do
  dep="${dep## }"
  dep="${dep%% }"
  [[ -n "${dep}" && "${dep}" != "@builddeps@" ]] || continue
  dep="${dep%%|*}"
  dep="${dep%% (*}"
  dep="${dep%% [*}"
  dep="${dep## }"
  dep="${dep%% }"
  [[ -n "${dep}" ]] && install_deps+=("${dep}")
done
if ((${#install_deps[@]} > 0)); then
  apt-get install -y --no-install-recommends "${install_deps[@]}"
fi

tmp_root="$(mktemp -d)"
trap 'rm -rf "${tmp_root}"' EXIT
export AUTOPKGTEST_TMP="${tmp_root}"
cd "${package_root}"
sh "debian/tests/${test_name}"
