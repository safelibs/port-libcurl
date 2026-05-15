#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --implementation <compat|packaged> [--flavor <openssl|gnutls>] [--build-state <path>] [--binary <path>] [--package-root <path>]" >&2
}

implementation=""
flavor="openssl"
build_state=""
binary_override=""
package_root=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --implementation)
      implementation="${2:-}"
      shift 2
      ;;
    --flavor)
      flavor="${2:-}"
      shift 2
      ;;
    --build-state)
      build_state="${2:-}"
      shift 2
      ;;
    --binary)
      binary_override="${2:-}"
      shift 2
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

[[ -z "${implementation}" ]] && usage && exit 2

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
safe_dir="$(cd "${script_dir}/.." && pwd)"
tmp_root="$(mktemp -d)"
cleanup() {
  "${script_dir}/http-fixtures.sh" stop --pid-file "${tmp_root}/fixture.pid" >/dev/null 2>&1 || true
  rm -rf "${tmp_root}"
}
trap cleanup EXIT

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

if [[ "${implementation}" == "compat" ]]; then
  if [[ -z "${build_state}" ]]; then
    build_state="${safe_dir}/.compat/${flavor}/build-state.json"
  fi
  [[ -f "${build_state}" ]] || "${script_dir}/build-compat-consumers.sh" --flavor "${flavor}"
  [[ -f "${build_state}" ]] || { echo "missing build state: ${build_state}" >&2; exit 1; }
  binary="${binary_override:-$(jq -r '.targets[] | select(.target_id=="src:curl") | .executable_path' "${build_state}")}"
  lib_dir="$(jq -r '.stage.lib_dir' "${build_state}")"
  [[ -x "${binary}" ]] || { echo "missing curl binary: ${binary}" >&2; exit 1; }
else
  if [[ -n "${package_root}" ]]; then
    package_root="$(cd "${package_root}" && pwd)"
    package_extract="${tmp_root}/packages"
    mkdir -p "${package_extract}"
    dpkg-deb -x "$(find_deb "${package_root}" curl)" "${package_extract}"
    dpkg-deb -x "$(find_deb "${package_root}" libcurl4t64)" "${package_extract}"
    multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
    binary="${binary_override:-${package_extract}/usr/bin/curl}"
    lib_dir="${package_extract}/usr/lib/${multiarch}"
  else
    binary="${binary_override:-${PACKAGED_CURL_BIN:-$(command -v curl || true)}}"
    lib_dir="${PACKAGED_LIBRARY_PATH:-}"
  fi
  [[ -n "${binary}" ]] || { echo "missing packaged curl binary" >&2; exit 1; }
  [[ -x "${binary}" ]] || { echo "missing packaged curl binary: ${binary}" >&2; exit 1; }
fi

fixture_root="${tmp_root}/fixture-root"
"${script_dir}/http-fixtures.sh" prepare --root "${fixture_root}"
"${script_dir}/http-fixtures.sh" start --root "${fixture_root}" --pid-file "${tmp_root}/fixture.pid" --port-file "${tmp_root}/fixture.port" --log "${tmp_root}/fixture.log"
base_url="http://127.0.0.1:$(cat "${tmp_root}/fixture.port")"

run_curl() {
  if [[ -n "${lib_dir}" ]]; then
    LD_LIBRARY_PATH="${lib_dir}:${LD_LIBRARY_PATH:-}" "${binary}" "$@"
  else
    "${binary}" "$@"
  fi
}

download_out="${tmp_root}/download.txt"
redirect_out="${tmp_root}/redirect.txt"
headers_out="${tmp_root}/headers.txt"
upload_in="${tmp_root}/upload.txt"

printf 'upload payload through compat curl\n' >"${upload_in}"
run_curl -fsS "${base_url}/plain.txt" -o "${download_out}"
cmp -s "${download_out}" "${fixture_root}/plain.txt"

run_curl -fsS -T "${upload_in}" "${base_url}/upload/curl-tool.txt" >/dev/null
cmp -s "${upload_in}" "${fixture_root}/uploaded/curl-tool.txt"

run_curl -fsSL "${base_url}/redirect" -o "${redirect_out}"
cmp -s "${redirect_out}" "${fixture_root}/redirects/target.txt"

run_curl -fsSI "${base_url}/headers" >"${headers_out}"
grep -qi '^x-compat-fixture: yes' "${headers_out}"
