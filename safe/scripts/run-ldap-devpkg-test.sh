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
[[ -f "${src}" ]] || {
  echo "missing tracked LDAP test source: ${src}" >&2
  echo "refresh vendored inputs with safe/scripts/vendor-compat-assets.sh from a full repo checkout" >&2
  exit 1
}

tmp_root="$(mktemp -d)"
trap 'rm -rf "${tmp_root}"' EXIT
out_bin="${binary:-${tmp_root}/ldap-bindata}"
shim_include_dir="${safe_dir}/compat/ldap-devpkg/include"

setup_local_ldap_devpkg() {
  [[ -d "${shim_include_dir}" ]] || return 1

  local ldap_lib
  local lber_lib
  ldap_lib="$(ldconfig -p 2>/dev/null | awk '/libldap\.so\.2 / { print $NF; exit }')"
  lber_lib="$(ldconfig -p 2>/dev/null | awk '/liblber\.so\.2 / { print $NF; exit }')"
  [[ -n "${ldap_lib}" && -n "${lber_lib}" ]] || return 1

  local pc_dir="${tmp_root}/pkgconfig"
  mkdir -p "${pc_dir}"
  cat > "${pc_dir}/ldap.pc" <<EOF
prefix=${safe_dir}/compat/ldap-devpkg
includedir=${shim_include_dir}
libdir=$(dirname "${ldap_lib}")

Name: ldap
Description: port-libcurl repo-local LDAP devpkg shim
Version: 2.0
Cflags: -I\${includedir}
Libs: -L\${libdir} -l:$(basename "${ldap_lib}") -l:$(basename "${lber_lib}")
EOF
  export PKG_CONFIG_PATH="${pc_dir}${PKG_CONFIG_PATH:+:${PKG_CONFIG_PATH}}"
}

have_ldap_devpkg() {
  local cflags
  if pkgconf --exists ldap >/dev/null 2>&1; then
    cflags="$(pkgconf --cflags ldap 2>/dev/null || true)"
    if printf '#include <ldap.h>\n#include <ldap_utf8.h>\n#include <ldif.h>\n' | gcc -E ${cflags} - >/dev/null 2>&1; then
      return 0
    fi
  fi

  setup_local_ldap_devpkg || return 1
  pkgconf --exists ldap >/dev/null 2>&1 || return 1
  cflags="$(pkgconf --cflags ldap 2>/dev/null || true)"
  printf '#include <ldap.h>\n#include <ldap_utf8.h>\n#include <ldif.h>\n' | gcc -E ${cflags} - >/dev/null 2>&1
}

if ! have_ldap_devpkg; then
  if (( compile_only )); then
    echo "skipping LDAP compile-only coverage: pkg-config ldap and ldap.h are unavailable" >&2
    exit 0
  fi
  echo "missing LDAP development headers; install the ldap dev package to run this check" >&2
  exit 1
fi

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
  echo "slapd is unavailable; the LDAP compile step succeeded and runtime coverage is skipped" >&2
  exit 0
fi

if (( EUID != 0 )); then
  echo "LDAP runtime coverage requires root; the compile step succeeded and runtime coverage is skipped" >&2
  exit 0
fi

if [[ "${implementation}" == "compat" ]]; then
  LD_LIBRARY_PATH="${lib_dir}:${LD_LIBRARY_PATH:-}" "${out_bin}"
else
  "${out_bin}"
fi
