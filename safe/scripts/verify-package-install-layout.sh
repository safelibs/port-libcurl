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

declare -A roots

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

extract_pkg() {
  local package="$1"
  local deb root
  deb="$(find_deb "${package}")"
  root="${tmp_root}/${package}"
  mkdir -p "${root}"
  dpkg-deb -x "${deb}" "${root}"
  roots["${package}"]="${root}"
}

require_path() {
  local root="$1"
  local path="$2"
  [[ -e "${root}/${path}" || -L "${root}/${path}" ]] || {
    echo "missing ${path} under ${root}" >&2
    exit 1
  }
}

require_path_or_gz() {
  local root="$1"
  local path="$2"
  [[ -e "${root}/${path}" || -L "${root}/${path}" || -e "${root}/${path}.gz" || -L "${root}/${path}.gz" ]] || {
    echo "missing ${path} or ${path}.gz under ${root}" >&2
    exit 1
  }
}

require_glob() {
  local root="$1"
  local pattern="$2"
  compgen -G "${root}/${pattern}" >/dev/null || {
    echo "missing match for ${pattern} under ${root}" >&2
    exit 1
  }
}

for package in curl libcurl4t64 libcurl3t64-gnutls libcurl4-openssl-dev libcurl4-gnutls-dev libcurl4-doc; do
  extract_pkg "${package}"
done

require_path "${roots[curl]}" "usr/bin/curl"
require_path "${roots[curl]}" "usr/share/man/man1/curl.1.gz"

require_glob "${roots[libcurl4t64]}" "usr/lib/${multiarch}/libcurl.so.4*"
require_path "${roots[libcurl4t64]}" "usr/lib/${multiarch}/libcurl-reference-openssl.so.4"

require_glob "${roots[libcurl3t64-gnutls]}" "usr/lib/${multiarch}/libcurl-gnutls.so.4*"
require_path "${roots[libcurl3t64-gnutls]}" "usr/lib/${multiarch}/libcurl-gnutls.so.3"
require_path "${roots[libcurl3t64-gnutls]}" "usr/lib/${multiarch}/libcurl-reference-gnutls.so.4"

for devpkg in libcurl4-openssl-dev libcurl4-gnutls-dev; do
  root="${roots[${devpkg}]}"
  require_path "${root}" "usr/bin/curl-config"
  require_path "${root}" "usr/lib/${multiarch}/pkgconfig/libcurl.pc"
  require_path "${root}" "usr/share/aclocal/libcurl.m4"
  for header in "${package_root}"/include/curl/*.h; do
    require_path "${root}" "usr/include/${multiarch}/curl/$(basename "${header}")"
  done
done

require_path "${roots[libcurl4-openssl-dev]}" "usr/lib/${multiarch}/libcurl.so"
require_path "${roots[libcurl4-openssl-dev]}" "usr/lib/${multiarch}/libcurl.a"
require_path "${roots[libcurl4-gnutls-dev]}" "usr/lib/${multiarch}/libcurl-gnutls.so"
require_path "${roots[libcurl4-gnutls-dev]}" "usr/lib/${multiarch}/libcurl-gnutls.a"
require_path "${roots[libcurl4-gnutls-dev]}" "usr/lib/${multiarch}/libcurl.so"
require_path "${roots[libcurl4-gnutls-dev]}" "usr/lib/${multiarch}/libcurl.a"

doc_root="${roots[libcurl4-doc]}"
while IFS= read -r pattern; do
  [[ -n "${pattern}" ]] || continue
  for source in ${package_root}/${pattern}; do
    [[ -e "${source}" ]] || continue
    require_path_or_gz "${doc_root}" "usr/share/doc/libcurl4-doc/$(basename "${source}")"
  done
done <"${package_root}/debian/libcurl4-doc.docs"

while IFS= read -r pattern; do
  [[ -n "${pattern}" ]] || continue
  for source in ${package_root}/${pattern}; do
    [[ -e "${source}" ]] || continue
    require_path_or_gz "${doc_root}" "usr/share/doc/libcurl4-doc/examples/$(basename "${source}")"
  done
done <"${package_root}/debian/libcurl4-doc.examples"

while IFS= read -r pattern; do
  [[ -n "${pattern}" ]] || continue
  for source in ${package_root}/${pattern}; do
    [[ -e "${source}" ]] || continue
    base="$(basename "${source}")"
    require_path "${doc_root}" "usr/share/man/man3/${base}.gz"
  done
done <"${package_root}/debian/libcurl4-doc.manpages"

while read -r source target; do
  [[ -n "${source:-}" && -n "${target:-}" ]] || continue
  compressed_target="${target#/}.gz"
  require_path "${doc_root}" "${compressed_target}"
done <"${package_root}/debian/libcurl4-doc.links"
