#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --package-root <dir>" >&2
}

package_root=""
while [[ $# -gt 0 ]]; do
  case "$1" in
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
[[ -n "${package_root}" ]] || { usage; exit 2; }

package_root="$(cd "${package_root}" && pwd)"
multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
tmp_root="$(mktemp -d)"
trap 'rm -rf "${tmp_root}"' EXIT

find_deb() {
  local package="$1"
  local deb
  for deb in "${package_root}"/*.deb "${package_root}/../"*.deb; do
    [[ -e "${deb}" ]] || continue
    if [[ "$(dpkg-deb -f "${deb}" Package)" == "${package}" ]]; then
      printf '%s\n' "${deb}"
      return 0
    fi
  done
  echo "missing built deb for ${package}" >&2
  return 1
}

verify_one() {
  local package="$1"
  local root="${tmp_root}/${package}"
  local deb
  deb="$(find_deb "${package}")"
  mkdir -p "${root}"
  dpkg-deb -x "${deb}" "${root}"

  test -x "${root}/usr/bin/curl-config"
  test -f "${root}/usr/lib/${multiarch}/pkgconfig/libcurl.pc"
  test -f "${root}/usr/share/aclocal/libcurl.m4"

  "${root}/usr/bin/curl-config" --version --cflags --libs --static-libs --configure >/dev/null
  env PKG_CONFIG_LIBDIR="${root}/usr/lib/${multiarch}/pkgconfig" \
      PKG_CONFIG_SYSROOT_DIR="${root}" \
      pkgconf --cflags --libs libcurl >/dev/null

  work="${tmp_root}/autoconf-${package}"
  mkdir -p "${work}/m4"
  cp "${root}/usr/share/aclocal/libcurl.m4" "${work}/m4/libcurl.m4"
  cat >"${work}/configure.ac" <<'EOF'
AC_INIT([libcurl-m4-smoke], [1])
AC_CONFIG_SRCDIR([configure.ac])
AC_PROG_CC
LIBCURL_CHECK_CONFIG([yes], [7.0.0], [:], [AC_MSG_ERROR([libcurl not found])])
AC_OUTPUT
EOF
  (cd "${work}" && ACLOCAL_PATH="${work}/m4" aclocal -I m4 && autoconf)
  test -x "${work}/configure"
}

verify_one libcurl4-openssl-dev
verify_one libcurl4-gnutls-dev
