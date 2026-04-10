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

if [[ -z "$flavor" ]]; then
  usage
  exit 2
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
safe_dir="$(cd "${script_dir}/.." && pwd)"
repo_root="$(cd "${safe_dir}/.." && pwd)"
build_root="${safe_dir}/.reference/${flavor}"
source_root="${build_root}/source"
worktree="${source_root}/original"
dist_dir="${build_root}/dist"
metadata_file="${build_root}/metadata.json"

mkdir -p "${build_root}"

git_rev="$(git -C "${repo_root}" rev-parse HEAD)"
if [[ -f "${metadata_file}" ]] && [[ -f "${dist_dir}/libcurl-reference-${flavor}.so.4" ]]; then
  if python3 - "${metadata_file}" "${git_rev}" "${flavor}" <<'PY'
import json
import pathlib
import sys

metadata = json.loads(pathlib.Path(sys.argv[1]).read_text())
expected_rev = sys.argv[2]
expected_flavor = sys.argv[3]
ok = (
    metadata.get("git_rev") == expected_rev
    and metadata.get("requested_flavor") == expected_flavor
    and metadata.get("dist", {}).get("shared") == f"libcurl-reference-{expected_flavor}.so.4"
)
raise SystemExit(0 if ok else 1)
PY
  then
    exit 0
  fi
fi

rm -rf "${source_root}" "${dist_dir}"
mkdir -p "${source_root}" "${dist_dir}"

git -C "${repo_root}" archive --format=tar HEAD original | tar -xf - -C "${source_root}"

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
elif pkg-config --exists gnutls && [[ -d /usr/include/gnutls ]]; then
  ssl_args=(--with-gnutls --without-openssl)
else
  actual_backend="openssl-fallback"
  ssl_args=(--with-openssl --without-gnutls)
fi

common_configure_args=(
  --disable-dependency-tracking
  --disable-symbol-hiding
  --enable-versioned-symbols
  --disable-manual
  --without-libidn2
  --without-nghttp2
  --without-libpsl
  --without-librtmp
  --without-libssh
  --without-libssh2
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

python3 - "${metadata_file}" "${git_rev}" "${flavor}" "${actual_backend}" <<'PY'
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
