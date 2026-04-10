#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --flavor <openssl|gnutls>" >&2
}

ensure_apt_package() {
  local package="$1"
  shift

  if "$@"; then
    return 0
  fi

  if ! command -v apt-get >/dev/null 2>&1; then
    echo "missing required package ${package}, and apt-get is unavailable" >&2
    exit 1
  fi

  if ! command -v sudo >/dev/null 2>&1 || ! sudo -n true >/dev/null 2>&1; then
    echo "missing required package ${package}, and passwordless sudo is unavailable" >&2
    exit 1
  fi

  export DEBIAN_FRONTEND=noninteractive
  if ! dpkg-query -W -f='${Status}' "${package}" 2>/dev/null | grep -q "install ok installed"; then
    if ! sudo -n apt-get install -y --no-install-recommends "${package}"; then
      sudo -n apt-get update
      sudo -n apt-get install -y --no-install-recommends "${package}"
    fi
  fi

  if ! "$@"; then
    echo "required package ${package} is still unavailable after installation" >&2
    exit 1
  fi
}

ensure_gnutls_deps() {
  if pkg-config --exists gnutls && [[ -d /usr/include/gnutls ]]; then
    return 0
  fi

  if ! command -v apt-get >/dev/null 2>&1; then
    echo "gnutls flavor requires libgnutls28-dev, and apt-get is unavailable" >&2
    exit 1
  fi

  if ! command -v sudo >/dev/null 2>&1 || ! sudo -n true >/dev/null 2>&1; then
    echo "gnutls flavor requires libgnutls28-dev, and passwordless sudo is unavailable" >&2
    exit 1
  fi

  export DEBIAN_FRONTEND=noninteractive
  if ! dpkg-query -W -f='${Status}' libgnutls28-dev 2>/dev/null | grep -q "install ok installed"; then
    if ! sudo -n apt-get install -y --no-install-recommends libgnutls28-dev; then
      sudo -n apt-get update
      sudo -n apt-get install -y --no-install-recommends libgnutls28-dev
    fi
  fi

  if ! pkg-config --exists gnutls || [[ ! -d /usr/include/gnutls ]]; then
    echo "gnutls flavor requires a real GnuTLS development toolchain" >&2
    exit 1
  fi
}

have_nghttp2_support() {
  pkg-config --exists libnghttp2 && [[ -f /usr/include/nghttp2/nghttp2.h ]]
}

ensure_nghttp2_deps() {
  ensure_apt_package libnghttp2-dev have_nghttp2_support
  ensure_apt_package nghttp2-proxy command -v nghttpx
}

have_libssh2_support() {
  pkg-config --exists libssh2 && [[ -f /usr/include/libssh2.h ]]
}

ensure_libssh2_deps() {
  ensure_apt_package libssh2-1-dev have_libssh2_support
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

if [[ -z "$flavor" ]]; then
  usage
  exit 2
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
safe_dir="$(cd "${script_dir}/.." && pwd)"
repo_root="$(cd "${safe_dir}/.." && pwd)"
build_root="${safe_dir}/.reference/${flavor}"
source_root="${build_root}/source"
worktree="${source_root}/upstream"
dist_dir="${build_root}/dist"
metadata_file="${build_root}/metadata.json"
vendor_root="${safe_dir}/vendor/upstream"

mkdir -p "${build_root}"

if [[ ! -d "${vendor_root}" ]]; then
  echo "missing vendored upstream tree: ${vendor_root}" >&2
  exit 1
fi

if git_rev="$(git -C "${repo_root}" rev-parse HEAD 2>/dev/null)"; then
  :
else
  git_rev="$(python3 - "${vendor_root}/manifest.json" <<'PY'
import hashlib
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
digest = hashlib.sha256(path.read_bytes()).hexdigest()
print(f"vendor:{digest}")
PY
)"
fi

ensure_nghttp2_deps
ensure_libssh2_deps

expected_nghttp2=enabled
expected_libssh2=enabled
expected_nghttpx=present
if [[ -f "${metadata_file}" ]] && [[ -f "${dist_dir}/libcurl-reference-${flavor}.so.4" ]]; then
  if python3 - "${metadata_file}" "${git_rev}" "${flavor}" "${expected_nghttp2}" "${expected_libssh2}" "${expected_nghttpx}" <<'PY'
import json
import pathlib
import sys

metadata = json.loads(pathlib.Path(sys.argv[1]).read_text())
expected_rev = sys.argv[2]
expected_flavor = sys.argv[3]
expected_nghttp2 = sys.argv[4]
expected_libssh2 = sys.argv[5]
expected_nghttpx = sys.argv[6]
ok = (
    metadata.get("git_rev") == expected_rev
    and metadata.get("requested_flavor") == expected_flavor
    and metadata.get("actual_backend") == expected_flavor
    and metadata.get("source_tree") == "safe/vendor/upstream"
    and metadata.get("dist", {}).get("shared") == f"libcurl-reference-{expected_flavor}.so.4"
    and metadata.get("optional_features", {}).get("nghttp2") == expected_nghttp2
    and metadata.get("optional_features", {}).get("libssh2") == expected_libssh2
    and metadata.get("runtime_tools", {}).get("nghttpx") == expected_nghttpx
)
raise SystemExit(0 if ok else 1)
PY
  then
    exit 0
  fi
fi

rm -rf "${source_root}" "${dist_dir}"
mkdir -p "${worktree}" "${dist_dir}"

cp -a "${vendor_root}/." "${worktree}/"

if [[ "${flavor}" == "openssl" ]]; then
  while IFS= read -r backup_file; do
    rel="${backup_file#${worktree}/.pc/90_gnutls.patch/}"
    mkdir -p "$(dirname "${worktree}/${rel}")"
    cp "${backup_file}" "${worktree}/${rel}"
  done < <(find "${worktree}/.pc/90_gnutls.patch" -type f | sort)
fi

actual_backend="${flavor}"
ssl_args=()
if [[ "${flavor}" == "openssl" ]]; then
  ssl_args=(--with-openssl --without-gnutls)
else
  ensure_gnutls_deps
  ssl_args=(--with-gnutls --without-openssl)
fi

common_configure_args=(
  --disable-dependency-tracking
  --disable-symbol-hiding
  --enable-versioned-symbols
  --disable-manual
  --without-libidn2
  --with-nghttp2
  --without-libpsl
  --without-librtmp
  --without-libssh
  --with-libssh2
  --without-zstd
  --without-brotli
  --disable-ldap
  --disable-ldaps
)

(
  cd "${worktree}"
  chmod +x configure
  ./configure "${common_configure_args[@]}" "${ssl_args[@]}"
  make -j"$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)" -C lib
)

shared_basename="libcurl-reference-${flavor}.so.4"
shared_input=""
for candidate in \
  "${worktree}/lib/.libs/libcurl-${flavor}.so.4" \
  "${worktree}/lib/.libs/libcurl.so.4"
do
  if [[ -f "${candidate}" ]]; then
    shared_input="${candidate}"
    break
  fi
done
if [[ -z "${shared_input}" ]]; then
  echo "could not locate built shared library for ${flavor}" >&2
  exit 1
fi
cp "${shared_input}" "${dist_dir}/${shared_basename}"

static_input=""
for candidate in \
  "${worktree}/lib/.libs/libcurl-${flavor}.a" \
  "${worktree}/lib/.libs/libcurl.a"
do
  if [[ -f "${candidate}" ]]; then
    static_input="${candidate}"
    break
  fi
done
if [[ -n "${static_input}" ]]; then
  cp "${static_input}" "${dist_dir}/libcurl-reference-${flavor}.a"
fi

libtool_input=""
for candidate in \
  "${worktree}/lib/libcurl-${flavor}.la" \
  "${worktree}/lib/libcurl.la"
do
  if [[ -f "${candidate}" ]]; then
    libtool_input="${candidate}"
    break
  fi
done
if [[ -n "${libtool_input}" ]]; then
  cp "${libtool_input}" "${dist_dir}/libcurl-reference-${flavor}.la"
fi

python3 - "${metadata_file}" "${git_rev}" "${flavor}" "${actual_backend}" "${expected_nghttp2}" "${expected_libssh2}" "${expected_nghttpx}" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
path.write_text(
    json.dumps(
        {
            "git_rev": sys.argv[2],
            "requested_flavor": sys.argv[3],
            "actual_backend": sys.argv[4],
            "source_tree": "safe/vendor/upstream",
            "optional_features": {
                "nghttp2": sys.argv[5],
                "libssh2": sys.argv[6],
            },
            "runtime_tools": {
                "nghttpx": sys.argv[7],
            },
            "dist": {
                "shared": f"libcurl-reference-{sys.argv[3]}.so.4",
                "static": f"libcurl-reference-{sys.argv[3]}.a",
                "libtool": f"libcurl-reference-{sys.argv[3]}.la",
            },
        },
        indent=2,
        sort_keys=True,
    )
    + "\n"
)
PY
