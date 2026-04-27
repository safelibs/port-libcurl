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
  echo "usage: $0 --expected <symbols-file|libcurl.def> --flavor <openssl|gnutls>" >&2
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
    "${safe_dir}/target/public-abi/${flavor}/debug/deps/libport_libcurl_safe.so"
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

tmpdir="$(mktemp -d)"
trap 'rm -rf "${tmpdir}"' EXIT

first_token="$(awk 'NF { print $1; exit }' "${expected}")"
if [[ "${first_token}" == "EXPORTS" ]]; then
  awk 'NF && $1 != "EXPORTS" { print $1 }' "${expected}" | sort -u > "${tmpdir}/expected.txt"
else
  awk '$1 ~ /^curl_/ { split($1, parts, "@"); print parts[1] }' "${expected}" | sort -u > "${tmpdir}/expected.txt"
fi

nm -D --defined-only --with-symbol-versions "${artifact}" \
  | awk '{ print $NF }' \
  | sed 's/@.*$//' \
  | awk '/^[a-z_]/ { print }' \
  | sort -u > "${tmpdir}/actual-lower.txt"

grep '^curl_' "${tmpdir}/actual-lower.txt" > "${tmpdir}/actual-curl.txt" || true

if ! diff -u "${tmpdir}/expected.txt" "${tmpdir}/actual-curl.txt"; then
  exit 1
fi

comm -23 "${tmpdir}/actual-lower.txt" "${tmpdir}/expected.txt" > "${tmpdir}/unexpected.txt"
if [[ -s "${tmpdir}/unexpected.txt" ]]; then
  echo "unexpected public exports in ${artifact}:" >&2
  cat "${tmpdir}/unexpected.txt" >&2
  exit 1
fi
