#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --flavor <openssl|gnutls>" >&2
}

flavor=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --flavor)
      flavor="${2:-}"
      shift 2
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

if [[ -z "${flavor}" ]]; then
  usage
  exit 2
fi

case "${flavor}" in
  openssl)
    feature="openssl-flavor"
    soname="libcurl.so.4"
    ;;
  gnutls)
    feature="gnutls-flavor"
    soname="libcurl-gnutls.so.4"
    ;;
  *)
    echo "unknown flavor: ${flavor}" >&2
    exit 2
    ;;
esac

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
safe_dir="$(cd "${script_dir}/.." && pwd)"
target_dir="${safe_dir}/target/public-abi/${flavor}"
stage_dir="${target_dir}/stage"
include_dir="${stage_dir}/include"
lib_dir="${stage_dir}/lib"
bin_dir="${stage_dir}/bin"
smoke_src="${safe_dir}/tests/smoke/public_api_smoke.c"
smoke_bin="${bin_dir}/public_api_smoke"
safe_lib_src="${target_dir}/debug/libport_libcurl_safe.so"
ref_lib_src="${safe_dir}/.reference/${flavor}/dist/libcurl-reference-${flavor}.so.4"

rm -rf "${stage_dir}"
mkdir -p "${include_dir}" "${lib_dir}" "${bin_dir}"

CARGO_TARGET_DIR="${target_dir}" \
  cargo build \
    --manifest-path "${safe_dir}/Cargo.toml" \
    --no-default-features \
    --features "${feature}"

if [[ ! -f "${safe_lib_src}" ]]; then
  echo "missing safe shared library: ${safe_lib_src}" >&2
  exit 1
fi

if [[ ! -f "${ref_lib_src}" ]]; then
  echo "missing reference sidecar: ${ref_lib_src}" >&2
  exit 1
fi

cp -a "${safe_dir}/include/." "${include_dir}/"
cp "${safe_lib_src}" "${lib_dir}/${soname}"
cp "${ref_lib_src}" "${lib_dir}/libcurl-reference-${flavor}.so.4"
ln -sf "${soname}" "${lib_dir}/libcurl.so"

"${CC:-cc}" \
  -std=c11 \
  -Wall \
  -Wextra \
  -Wno-deprecated-declarations \
  -I"${include_dir}" \
  "${smoke_src}" \
  -L"${lib_dir}" \
  -Wl,-rpath-link,"${lib_dir}" \
  -lcurl \
  -o "${smoke_bin}"

LD_LIBRARY_PATH="${lib_dir}" "${smoke_bin}"
