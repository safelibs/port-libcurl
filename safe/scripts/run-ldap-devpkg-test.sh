#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --flavor <openssl|gnutls> [--implementation <compat|packaged>] [--build-state <path>] [--binary <path>] [--compile-only]" >&2
}

flavor=""
implementation="compat"
build_state=""
binary=""
compile_only=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --flavor)
      flavor="${2:-}"
      shift 2
      ;;
    --implementation)
      implementation="${2:-}"
      shift 2
      ;;
    --build-state)
      build_state="${2:-}"
      shift 2
      ;;
    --binary)
      binary="${2:-}"
      shift 2
      ;;
    --compile-only)
      compile_only=1
      shift
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

[[ -z "${flavor}" ]] && usage && exit 2
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
safe_dir="$(cd "${script_dir}/.." && pwd)"
src="${safe_dir}/vendor/upstream/debian/tests/LDAP-bindata.c"
[[ -f "${src}" ]] || "${script_dir}/vendor-compat-assets.sh"

tmp_root="$(mktemp -d)"
trap 'rm -rf "${tmp_root}"' EXIT
out_bin="${binary:-${tmp_root}/ldap-bindata}"

if [[ "${implementation}" == "compat" ]]; then
  if [[ -z "${build_state}" ]]; then
    build_state="${safe_dir}/.compat/${flavor}/build-state.json"
  fi
  [[ -f "${build_state}" ]] || "${script_dir}/build-compat-consumers.sh" --flavor "${flavor}"
  lib_dir="$(jq -r '.stage.lib_dir' "${build_state}")"
  include_dir="$(jq -r '.stage.include_dir' "${build_state}")"
  gcc "${src}" -I"${include_dir}" -L"${lib_dir}" -Wl,-rpath,"${lib_dir}" -lcurl $(pkgconf --cflags --libs ldap) -o "${out_bin}"
else
  gcc "${src}" $(pkgconf --cflags --libs ldap libcurl) -o "${out_bin}"
fi

if (( compile_only )); then
  exit 0
fi

if ! command -v slapd >/dev/null 2>&1; then
  echo "slapd is unavailable; only the compile step could be verified" >&2
  exit 1
fi

if [[ "${implementation}" == "compat" ]]; then
  LD_LIBRARY_PATH="${lib_dir}:${LD_LIBRARY_PATH:-}" "${out_bin}"
else
  "${out_bin}"
fi
