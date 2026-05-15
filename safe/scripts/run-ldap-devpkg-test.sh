#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --flavor <openssl|gnutls> [--implementation <compat|packaged>] [--build-state <path>] [--binary <path>] [--compile-only] [--package-root <path>]" >&2
}

flavor=""
implementation="compat"
build_state=""
binary=""
compile_only=0
package_root=""
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
    --package-root)
      package_root="${2:-}"
      shift 2
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

[[ -z "${flavor}" ]] && usage && exit 2
if [[ -n "${package_root}" && "${implementation}" == "compat" ]]; then
  implementation="packaged"
fi
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
safe_dir="$(cd "${script_dir}/.." && pwd)"
src="${safe_dir}/debian/tests/LDAP-bindata.c"
if [[ ! -f "${src}" ]]; then
  src="${safe_dir}/vendor/upstream/debian/tests/LDAP-bindata.c"
fi
[[ -f "${src}" ]] || {
  echo "missing tracked LDAP test source: ${src}" >&2
  echo "refresh vendored inputs with safe/scripts/vendor-compat-assets.sh from a full repo checkout" >&2
  exit 1
}

tmp_root="$(mktemp -d)"
trap 'rm -rf "${tmp_root}"' EXIT
out_bin="${binary:-${tmp_root}/ldap-bindata}"
shim_include_dir="${safe_dir}/compat/ldap-devpkg/include"
package_extract=""
package_lib_dir=""

find_deb() {
  local root="$1"
  local package="$2"
  local deb
  for deb in "${root}"/*.deb "${root}/../"*.deb; do
    [[ -e "${deb}" ]] || continue
    if [[ "$(dpkg-deb -f "${deb}" Package)" == "${package}" ]]; then
      printf '%s\n' "${deb}"
      return 0
    fi
  done
  echo "missing built deb for ${package} under ${root} or ${root}/.." >&2
  return 1
}

setup_packaged_libcurl() {
  [[ -n "${package_root}" ]] || return 0
  package_root="$(cd "${package_root}" && pwd)"
  package_extract="${tmp_root}/packages"
  mkdir -p "${package_extract}"
  local runtime_pkg dev_pkg
  case "${flavor}" in
    openssl)
      runtime_pkg="libcurl4t64"
      dev_pkg="libcurl4-openssl-dev"
      ;;
    gnutls)
      runtime_pkg="libcurl3t64-gnutls"
      dev_pkg="libcurl4-gnutls-dev"
      ;;
    *)
      echo "unsupported flavor: ${flavor}" >&2
      exit 2
      ;;
  esac
  dpkg-deb -x "$(find_deb "${package_root}" "${runtime_pkg}")" "${package_extract}"
  dpkg-deb -x "$(find_deb "${package_root}" "${dev_pkg}")" "${package_extract}"
  package_lib_dir="${package_extract}/usr/lib/$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
}

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
  setup_packaged_libcurl
  if [[ -n "${package_extract}" ]]; then
    libcurl_flags="$(
      PKG_CONFIG_SYSROOT_DIR="${package_extract}" \
      PKG_CONFIG_LIBDIR="${package_lib_dir}/pkgconfig" \
      pkgconf --cflags --libs libcurl
    )"
    gcc "${src}" $(pkgconf --cflags --libs ldap) ${libcurl_flags} -Wl,-rpath,"${package_lib_dir}" -o "${out_bin}"
  else
    gcc "${src}" $(pkgconf --cflags --libs ldap libcurl) -o "${out_bin}"
  fi
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
elif [[ -n "${package_lib_dir}" ]]; then
  LD_LIBRARY_PATH="${package_lib_dir}:${LD_LIBRARY_PATH:-}" "${out_bin}"
else
  "${out_bin}"
fi
