#!/usr/bin/env bash
set -euo pipefail

expected=""
flavor=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --expected)
      expected="${2:-}"
      shift 2
      ;;
    --flavor)
      flavor="${2:-}"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ -z "${expected}" || -z "${flavor}" ]]; then
  echo "usage: $0 --expected <symbols-file> --flavor <openssl|gnutls>" >&2
  exit 2
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
safe_dir="$(cd "${script_dir}/.." && pwd)"
artifact="${safe_dir}/target/foundation/${flavor}/libcurl-safe-${flavor}-bridge.so"

if [[ ! -f "${artifact}" ]]; then
  echo "missing bridge artifact: ${artifact}" >&2
  exit 1
fi

expected_soname="$(awk 'NR==1 { print $1 }' "${expected}")"
expected_namespace="$(awk '$1 ~ /^CURL_/ && $1 !~ /^curl_/ { split($1, a, "@"); print a[1]; exit }' "${expected}")"

actual_soname="$(readelf -Wd "${artifact}" | awk '/SONAME/ { gsub(/\[|\]/, "", $NF); print $NF; exit }')"
if [[ "${actual_soname}" != "${expected_soname}" ]]; then
  echo "SONAME mismatch: expected ${expected_soname}, got ${actual_soname}" >&2
  exit 1
fi

if ! readelf --version-info "${artifact}" | grep -q "${expected_namespace}"; then
  echo "missing expected namespace ${expected_namespace} in ${artifact}" >&2
  exit 1
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "${tmpdir}"' EXIT

awk '$1 ~ /^curl_/ { split($1, a, "@"); print a[1] "@" a[2] }' "${expected}" \
  | sort -u > "${tmpdir}/expected.txt"

readelf -Ws "${artifact}" \
  | awk '$4 ~ /FUNC|IFUNC/ && $7 != "UND" && $8 ~ /^curl_.*@/ { print $8 }' \
  | sed 's/@@/@/' \
  | sort -u > "${tmpdir}/actual.txt"

diff -u "${tmpdir}/expected.txt" "${tmpdir}/actual.txt"
