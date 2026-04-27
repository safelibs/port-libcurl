#!/usr/bin/env bash
set -euo pipefail

expected=""
flavor=""
artifact=""
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
    --artifact)
      artifact="${2:-}"
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

case "${flavor}" in
  openssl)
    soname="libcurl.so.4"
    ;;
  gnutls)
    soname="libcurl-gnutls.so.4"
    ;;
  *)
    echo "unknown flavor: ${flavor}" >&2
    exit 2
    ;;
esac

resolve_artifact() {
  local candidates=(
    "${safe_dir}/target/public-abi/${flavor}/stage/lib/${soname}"
    "${safe_dir}/target/public-abi/${flavor}/package/${soname}"
    "${safe_dir}/target/public-abi/${flavor}/debug/libport_libcurl_safe.so"
    "${safe_dir}/target/check-public-abi-${flavor}/debug/deps/libport_libcurl_safe.so"
    "${safe_dir}/target/impl-public-abi-${flavor}/debug/deps/libport_libcurl_safe.so"
    "${safe_dir}/target/check-foundation-${flavor}/debug/deps/libport_libcurl_safe.so"
  )
  local candidate
  for candidate in "${candidates[@]}"; do
    if [[ -f "${candidate}" ]]; then
      printf '%s\n' "${candidate}"
      return 0
    fi
  done
  candidate="$(find "${safe_dir}/target/public-abi/${flavor}/package" -type f -name "${soname}" 2>/dev/null | sort | head -n 1 || true)"
  if [[ -n "${candidate}" && -f "${candidate}" ]]; then
    printf '%s\n' "${candidate}"
    return 0
  fi
  return 1
}

if [[ -z "${artifact}" ]]; then
  artifact="$(resolve_artifact || true)"
fi

if [[ ! -f "${artifact}" ]]; then
  echo "missing ABI artifact for ${flavor}: ${artifact:-<unresolved>}" >&2
  exit 1
fi

expected_soname="$(awk 'NR==1 { print $1 }' "${expected}")"
expected_namespace="$(awk '$1 ~ /^CURL_/ && $1 !~ /^curl_/ { split($1, a, "@"); print a[1]; exit }' "${expected}")"

actual_soname="$(readelf -Wd "${artifact}" | awk '/SONAME/ { gsub(/\[|\]/, "", $NF); print $NF; exit }')"
if [[ "${actual_soname}" != "${expected_soname}" ]]; then
  echo "SONAME mismatch: expected ${expected_soname}, got ${actual_soname}" >&2
  exit 1
fi

version_info="$(readelf --version-info "${artifact}")"
if ! grep -Fq "${expected_namespace}" <<<"${version_info}"; then
  echo "missing expected namespace ${expected_namespace} in ${artifact}" >&2
  exit 1
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "${tmpdir}"' EXIT

nm -D --defined-only --with-symbol-versions "${artifact}" \
  | awk -v expected_namespace="${expected_namespace}" '
      $3 ~ /^curl_/ {
        symbol = $3;
        split(symbol, parts, /@+/);
        if (length(parts) > 1 && parts[2] != expected_namespace) {
          print symbol;
        }
      }
    ' > "${tmpdir}/unexpected.txt"

if [[ -s "${tmpdir}/unexpected.txt" ]]; then
  echo "unexpected public symbol versions in ${artifact}:" >&2
  cat "${tmpdir}/unexpected.txt" >&2
  exit 1
fi
