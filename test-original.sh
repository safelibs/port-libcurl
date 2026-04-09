#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
IMAGE_TAG="${LIBCURL_ORIGINAL_TEST_IMAGE:-libcurl-original-test:ubuntu24.04}"
ONLY=""

usage() {
  cat <<'EOF'
usage: test-original.sh [--only <dependent-name>]

Builds the local Ubuntu curl source package from original/, installs the
resulting runtime libraries into a Docker container, and smoke-tests the
libcurl-dependent software listed in dependents.json using only public APIs.

--only runs just one dependent by exact .dependents[].name.
EOF
}

while (($#)); do
  case "$1" in
    --only)
      ONLY="${2:?missing value for --only}"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      printf 'unknown option: %s\n' "$1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

for tool in docker git jq; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'missing required host tool: %s\n' "$tool" >&2
    exit 1
  }
done

[[ -d "$ROOT/original" ]] || {
  echo "missing original source tree" >&2
  exit 1
}

[[ -f "$ROOT/dependents.json" ]] || {
  echo "missing dependents.json" >&2
  exit 1
}

[[ -e /dev/fuse ]] || {
  echo "/dev/fuse is required to exercise HTTPDirFS inside Docker" >&2
  exit 1
}

if [[ -n "$ONLY" ]]; then
  jq -e --arg name "$ONLY" '.dependents[] | select(.name == $name)' \
    "$ROOT/dependents.json" >/dev/null || {
    printf 'unknown dependent in dependents.json: %s\n' "$ONLY" >&2
    exit 1
  }
fi

docker build -t "$IMAGE_TAG" - <<'DOCKERFILE'
FROM ubuntu:24.04

ARG DEBIAN_FRONTEND=noninteractive

RUN rm -f /etc/apt/sources.list.d/ubuntu.sources \
 && cat >/etc/apt/sources.list.d/ubuntu.sources <<'SRC'
Types: deb deb-src
URIs: http://archive.ubuntu.com/ubuntu/
Suites: noble noble-updates noble-backports
Components: main universe restricted multiverse
Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg

Types: deb deb-src
URIs: http://security.ubuntu.com/ubuntu/
Suites: noble-security
Components: main universe restricted multiverse
Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg
SRC

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      build-essential \
      ca-certificates \
      cmake \
      dpkg-dev \
      fakeroot \
      fuse3 \
      fwupd \
      gdal-bin \
      git \
      gzip \
      httpdirfs \
      jcat \
      jq \
      libarchive-tools \
      libhts-dev \
      makepkg \
      mount \
      ostree \
      pacman-package-manager \
      php8.3-cli \
      php8.3-curl \
      pkg-config \
      python3 \
      python3-librepo \
      python3-pycurl \
      r-base-core \
      r-cran-curl \
      xz-utils \
      zstd \
 && apt-get build-dep -y curl \
 && rm -rf /var/lib/apt/lists/*
DOCKERFILE

docker run --rm -i \
  --device /dev/fuse \
  --cap-add SYS_ADMIN \
  --security-opt apparmor:unconfined \
  -e "LIBCURL_TEST_ONLY=$ONLY" \
  -v "$ROOT:/work:ro" \
  "$IMAGE_TAG" \
  bash -s <<'CONTAINER'
set -euo pipefail

export LANG=C.UTF-8
export LC_ALL=C.UTF-8
export DEBIAN_FRONTEND=noninteractive

ROOT=/work
ONLY_FILTER="${LIBCURL_TEST_ONLY:-}"
TEST_ROOT=/tmp/libcurl-dependent-tests
SOURCE_EXPORT_ROOT=/tmp/libcurl-source-export
OPENSSL_BUILD_ROOT=/tmp/libcurl-openssl
GNUTLS_BUILD_ROOT=/tmp/libcurl-gnutls
BUILD_LOG_DIR="$TEST_ROOT/logs"
HTTP_ROOT="$TEST_ROOT/http-root"
HTTP_PORT_FILE="$TEST_ROOT/http-port"
HTTP_SERVER_LOG="$BUILD_LOG_DIR/http-server.log"
HTTP_SERVER_PID=""
HTTP_BASE=""
FWUPD_CERT_ROOT="$TEST_ROOT/fwupd-certificates"
HOME="$TEST_ROOT/home"
XDG_CACHE_HOME="$TEST_ROOT/cache"

mkdir -p "$TEST_ROOT" "$BUILD_LOG_DIR" "$HOME" "$XDG_CACHE_HOME"

log_step() {
  printf '\n==> %s\n' "$1"
}

die() {
  echo "error: $*" >&2
  exit 1
}

require_nonempty_file() {
  local path="$1"

  [[ -s "$path" ]] || die "expected non-empty file: $path"
}

require_file_contains() {
  local path="$1"
  local needle="$2"

  if ! grep -F -- "$needle" "$path" >/dev/null 2>&1; then
    printf 'missing expected text in %s: %s\n' "$path" "$needle" >&2
    printf -- '--- %s ---\n' "$path" >&2
    cat "$path" >&2
    exit 1
  fi
}

run_logged() {
  local log_name="$1"
  shift
  local log_file="$BUILD_LOG_DIR/${log_name}.log"

  if ! "$@" >"$log_file" 2>&1; then
    printf 'command failed for %s; last 200 log lines from %s:\n' "$log_name" "$log_file" >&2
    tail -n 200 "$log_file" >&2 || true
    return 1
  fi
}

cleanup() {
  if [[ -n "$HTTP_SERVER_PID" ]]; then
    kill "$HTTP_SERVER_PID" >/dev/null 2>&1 || true
    wait "$HTTP_SERVER_PID" 2>/dev/null || true
  fi
}

trap cleanup EXIT

selected() {
  local name="$1"

  [[ -z "$ONLY_FILTER" || "$ONLY_FILTER" == "$name" ]]
}

validate_dependents_inventory() {
  python3 <<'PY'
import json
from pathlib import Path

expected = [
    "Git",
    "CMake",
    "PHP cURL extension",
    "PycURL",
    "R curl package",
    "GDAL",
    "OSTree",
    "librepo",
    "HTSlib",
    "pacman/libalpm",
    "HTTPDirFS",
    "fwupd",
]

data = json.loads(Path("/work/dependents.json").read_text(encoding="utf-8"))
actual = [entry["name"] for entry in data["dependents"]]

if actual != expected:
    raise SystemExit(
        "unexpected dependents.json contents: "
        f"expected {expected}, found {actual}"
    )
PY
}

export_tracked_source() {
  log_step "Exporting tracked libcurl source"
  rm -rf "$SOURCE_EXPORT_ROOT"
  mkdir -p "$SOURCE_EXPORT_ROOT"
  git config --global --add safe.directory "$ROOT"
  git -C "$ROOT" ls-files -z -- dependents.json original \
    | (cd "$ROOT" && tar --null -T - -cf -) \
    | tar -xf - -C "$SOURCE_EXPORT_ROOT"
}

restore_pre_gnutls_files() {
  local dest_root="$1"
  local rel

  for rel in \
    docs/examples/Makefile.am \
    lib/Makefile.am \
    lib/libcurl.vers.in \
    src/Makefile.am \
    tests/http/clients/Makefile.am \
    tests/http/clients/Makefile.in \
    tests/libtest/Makefile.am; do
    cp "$SOURCE_EXPORT_ROOT/original/.pc/90_gnutls.patch/$rel" "$dest_root/$rel"
  done
}

prepare_runtime_build_trees() {
  rm -rf "$OPENSSL_BUILD_ROOT" "$GNUTLS_BUILD_ROOT"
  cp -a "$SOURCE_EXPORT_ROOT/original" "$OPENSSL_BUILD_ROOT"
  cp -a "$SOURCE_EXPORT_ROOT/original" "$GNUTLS_BUILD_ROOT"
  restore_pre_gnutls_files "$OPENSSL_BUILD_ROOT"
}

configure_runtime_tree() {
  local name="$1"
  local tree_root="$2"
  shift 2
  local configure_args=("$@")

  run_logged "${name}-buildconf" bash -lc "cd '$tree_root' && ./buildconf"
  run_logged "${name}-configure" bash -lc "
    cd '$tree_root'
    ./configure \
      --prefix=/usr/local \
      --libdir=/usr/local/lib \
      --disable-static \
      --disable-dependency-tracking \
      --disable-symbol-hiding \
      --enable-threaded-resolver \
      --enable-versioned-symbols \
      --with-lber-lib=lber \
      --with-gssapi=/usr \
      --with-libssh \
      --without-libssh2 \
      --with-nghttp2 \
      --with-zsh-functions-dir=/usr/share/zsh/vendor-completions \
      ${configure_args[*]}
  "
}

build_runtime_tree() {
  local name="$1"
  local tree_root="$2"

  run_logged "${name}-make" bash -lc "cd '$tree_root' && make -C lib -j$(nproc)"
  run_logged "${name}-install" bash -lc "cd '$tree_root' && make -C lib install"
}

build_local_curl_runtime() {
  log_step "Building local libcurl runtime libraries"
  prepare_runtime_build_trees
  configure_runtime_tree \
    openssl \
    "$OPENSSL_BUILD_ROOT" \
    --with-openssl \
    --with-ca-path=/etc/ssl/certs \
    --with-ca-bundle=/etc/ssl/certs/ca-certificates.crt
  build_runtime_tree openssl "$OPENSSL_BUILD_ROOT"

  configure_runtime_tree \
    gnutls \
    "$GNUTLS_BUILD_ROOT" \
    --with-gnutls \
    --with-ca-path=/etc/ssl/certs
  build_runtime_tree gnutls "$GNUTLS_BUILD_ROOT"

  ln -sf /usr/local/lib/libcurl-gnutls.so.4 /usr/local/lib/libcurl-gnutls.so.3
  printf '/usr/local/lib\n' >/etc/ld.so.conf.d/zz-local-libcurl.conf
  ldconfig
  export LD_LIBRARY_PATH=/usr/local/lib
}

write_http_server() {
  cat >"$TEST_ROOT/http_server.py" <<'PY'
#!/usr/bin/env python3
import functools
import http.server
import os
import pathlib
import re
import shutil
import sys
import urllib.parse


class Handler(http.server.SimpleHTTPRequestHandler):
    server_version = "libcurl-smoke-http/1.0"

    def __init__(self, *args, directory=None, **kwargs):
        self._range = None
        super().__init__(*args, directory=directory, **kwargs)

    def log_message(self, fmt, *args):
        sys.stderr.write("%s - - [%s] %s\n" % (
            self.client_address[0],
            self.log_date_time_string(),
            fmt % args,
        ))

    def do_PUT(self):
        rel = urllib.parse.urlparse(self.path).path.lstrip("/")
        if not rel.startswith("upload/"):
          self.send_error(405, "PUT only supported under /upload/")
          return

        target = pathlib.Path(self.directory) / "uploaded" / rel[len("upload/"):]
        target.parent.mkdir(parents=True, exist_ok=True)
        length = int(self.headers.get("Content-Length", "0"))
        with target.open("wb") as fh:
            remaining = length
            while remaining > 0:
                chunk = self.rfile.read(min(65536, remaining))
                if not chunk:
                    break
                fh.write(chunk)
                remaining -= len(chunk)

        self.send_response(201)
        self.send_header("Content-Length", "2")
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(b"ok")

    def copyfile(self, source, outputfile):
        if self._range is None:
            super().copyfile(source, outputfile)
            return

        remaining = self._range[1] - self._range[0] + 1
        while remaining > 0:
            chunk = source.read(min(65536, remaining))
            if not chunk:
                break
            outputfile.write(chunk)
            remaining -= len(chunk)

    def send_head(self):
        self._range = None
        path = self.translate_path(self.path)
        if os.path.isdir(path):
            return super().send_head()

        try:
            fh = open(path, "rb")
        except OSError:
            self.send_error(404, "File not found")
            return None

        size = os.fstat(fh.fileno()).st_size
        content_type = self.guess_type(path)
        range_header = self.headers.get("Range")

        if range_header:
            match = re.fullmatch(r"bytes=(\d*)-(\d*)", range_header.strip())
            if match is None:
                fh.close()
                self.send_error(416, "Invalid range")
                return None

            start_s, end_s = match.groups()
            if start_s == "" and end_s == "":
                fh.close()
                self.send_error(416, "Invalid range")
                return None

            if start_s == "":
                length = int(end_s)
                if length <= 0:
                    fh.close()
                    self.send_error(416, "Invalid range")
                    return None
                start = max(size - length, 0)
                end = size - 1
            else:
                start = int(start_s)
                end = int(end_s) if end_s else size - 1

            if start >= size or start > end:
                fh.close()
                self.send_response(416)
                self.send_header("Content-Range", f"bytes */{size}")
                self.end_headers()
                return None

            end = min(end, size - 1)
            self._range = (start, end)
            fh.seek(start)
            self.send_response(206)
            self.send_header("Content-type", content_type)
            self.send_header("Content-Length", str(end - start + 1))
            self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
            self.send_header("Accept-Ranges", "bytes")
            self.end_headers()
            return fh

        self.send_response(200)
        self.send_header("Content-type", content_type)
        self.send_header("Content-Length", str(size))
        self.send_header("Accept-Ranges", "bytes")
        self.end_headers()
        return fh


def main() -> int:
    root = pathlib.Path(sys.argv[1]).resolve()
    port_file = pathlib.Path(sys.argv[2])
    root.mkdir(parents=True, exist_ok=True)
    handler = functools.partial(Handler, directory=str(root))
    with http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler) as server:
        port_file.write_text(str(server.server_address[1]), encoding="utf-8")
        server.serve_forever()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
PY
  chmod +x "$TEST_ROOT/http_server.py"
}

prepare_git_fixture() {
  local bare_repo="$HTTP_ROOT/git/smoke.git"
  local work_repo="$TEST_ROOT/git-work"

  rm -rf "$bare_repo" "$work_repo"
  mkdir -p "$(dirname "$bare_repo")"

  git init --bare --initial-branch=main "$bare_repo" >/dev/null
  git init -b main "$work_repo" >/dev/null
  printf 'git smoke\n' >"$work_repo/README.txt"
  git -C "$work_repo" add README.txt
  git -C "$work_repo" -c user.name='Smoke Test' -c user.email='smoke@example.com' \
    commit -m 'initial' >/dev/null
  git -C "$work_repo" remote add origin "$bare_repo"
  git -C "$work_repo" push origin main >/dev/null
  git -C "$bare_repo" update-server-info
}

prepare_ostree_fixture() {
  local repo_root="$HTTP_ROOT/ostree/repo"
  local tree_root="$TEST_ROOT/ostree-tree"

  rm -rf "$repo_root" "$tree_root"
  mkdir -p "$(dirname "$repo_root")"
  mkdir -p "$tree_root"
  printf 'ostree smoke\n' >"$tree_root/message.txt"
  ostree --repo="$repo_root" init --mode=archive-z2 >/dev/null
  ostree --repo="$repo_root" commit -b main -s 'smoke' -m 'smoke commit' \
    --tree="dir=$tree_root" >/dev/null
  ostree --repo="$repo_root" summary -u >/dev/null
}

prepare_pacman_fixture() {
  local package_root="$TEST_ROOT/pacman-package"
  local repo_root="$HTTP_ROOT/pacman/repo"
  local archive_path="$repo_root/smoke-pkg-1.0-1-any.pkg.tar"
  local package_path="${archive_path}.zst"
  local payload_size

  rm -rf "$package_root" "$repo_root"
  mkdir -p "$package_root" "$repo_root"

  mkdir -p "$package_root/usr/share/smoke-pkg"
  printf 'pacman smoke\n' >"$package_root/usr/share/smoke-pkg/probe.txt"
  payload_size="$(wc -c <"$package_root/usr/share/smoke-pkg/probe.txt" | tr -d '[:space:]')"
  cat >"$package_root/.PKGINFO" <<EOF
pkgname = smoke-pkg
pkgbase = smoke-pkg
pkgver = 1.0-1
pkgdesc = libcurl pacman smoke package
url = https://example.invalid/libcurl-smoke
builddate = $(date +%s)
packager = libcurl smoke <smoke@example.com>
size = ${payload_size}
arch = any
license = MIT
EOF

  run_logged pacman-package \
    bash -lc "cd '$package_root' && bsdtar --format=ustar -cf '$archive_path' .PKGINFO usr && zstd -q -f '$archive_path' -o '$package_path'"
  run_logged pacman-repo-add \
    repo-add "$repo_root/smoke.db.tar.gz" "$package_path"
}

prepare_fwupd_certificates() {
  local activation_date
  local expiration_date

  rm -rf "$FWUPD_CERT_ROOT"
  mkdir -p "$FWUPD_CERT_ROOT"
  activation_date="$(date -u -d '1 day ago' +%Y-%m-%dT%H:%M:%S)"
  expiration_date="$(date -u -d '7 days' +%Y-%m-%dT%H:%M:%S)"

  cat >"$FWUPD_CERT_ROOT/ca.cfg" <<EOF
organization = "libcurl smoke"
cn = "libcurl smoke CA"
uri = "https://example.invalid/libcurl-smoke"
email = "smoke@example.invalid"
crl_dist_points = "https://example.invalid/libcurl-smoke/crl"
serial = 1
crl_number = 1
path_len = 1
activation_date = "$activation_date"
expiration_date = "$expiration_date"
ca
cert_signing_key
crl_signing_key
code_signing_key
EOF
  cat >"$FWUPD_CERT_ROOT/signer.cfg" <<EOF
cn = "libcurl smoke signer"
uri = "https://example.invalid/libcurl-smoke"
email = "smoke@example.invalid"
activation_date = "$activation_date"
expiration_date = "$expiration_date"
signing_key
code_signing_key
EOF

  run_logged fwupd-ca-key \
    certtool --generate-privkey --outfile "$FWUPD_CERT_ROOT/ca.key"
  run_logged fwupd-ca-cert \
    certtool --generate-self-signed \
      --load-privkey "$FWUPD_CERT_ROOT/ca.key" \
      --template "$FWUPD_CERT_ROOT/ca.cfg" \
      --outfile "$FWUPD_CERT_ROOT/ca.pem"
  run_logged fwupd-signer-key \
    certtool --generate-privkey --rsa --bits 2048 \
      --outfile "$FWUPD_CERT_ROOT/signer.key"
  run_logged fwupd-signer-request \
    certtool --generate-request \
      --load-privkey "$FWUPD_CERT_ROOT/signer.key" \
      --template "$FWUPD_CERT_ROOT/signer.cfg" \
      --outfile "$FWUPD_CERT_ROOT/signer.csr"
  run_logged fwupd-signer-cert \
    certtool --generate-certificate --rsa --bits 2048 \
      --load-request "$FWUPD_CERT_ROOT/signer.csr" \
      --load-ca-certificate "$FWUPD_CERT_ROOT/ca.pem" \
      --load-ca-privkey "$FWUPD_CERT_ROOT/ca.key" \
      --template "$FWUPD_CERT_ROOT/signer.cfg" \
      --outfile "$FWUPD_CERT_ROOT/signer.pem"
  require_nonempty_file "$FWUPD_CERT_ROOT/ca.pem"
  require_nonempty_file "$FWUPD_CERT_ROOT/signer.pem"
  require_nonempty_file "$FWUPD_CERT_ROOT/signer.key"
}

prepare_fwupd_fixture() {
  prepare_fwupd_certificates
  mkdir -p "$HTTP_ROOT/fwupd"
  cat >"$HTTP_ROOT/fwupd/metadata.xml" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<components version="0.9" origin="fwupd">
  <component type="firmware">
    <id>org.example.libcurl.smoke.device</id>
    <provides>
      <firmware type="flashed">2d47f29b-83a2-4f31-a2e8-63474f4d4c2e</firmware>
    </provides>
    <releases>
      <release version="1" timestamp="1456743843">
        <description><p>libcurl smoke metadata</p></description>
      </release>
    </releases>
  </component>
</components>
EOF
  gzip -c "$HTTP_ROOT/fwupd/metadata.xml" >"$HTTP_ROOT/fwupd/metadata.xml.gz"
  run_logged fwupd-jcat \
    jcat-tool --basename --appstream-id smoke \
      sign "$HTTP_ROOT/fwupd/metadata.xml.gz.jcat" "$HTTP_ROOT/fwupd/metadata.xml.gz" \
      "$FWUPD_CERT_ROOT/signer.pem" "$FWUPD_CERT_ROOT/signer.key"
  run_logged fwupd-jcat-checksum \
    jcat-tool --basename --kind sha256 \
      self-sign "$HTTP_ROOT/fwupd/metadata.xml.gz.jcat" "$HTTP_ROOT/fwupd/metadata.xml.gz"
}

prepare_http_fixtures() {
  log_step "Preparing local HTTP fixtures"
  rm -rf "$HTTP_ROOT"
  mkdir -p \
    "$HTTP_ROOT/cmake" \
    "$HTTP_ROOT/gdal" \
    "$HTTP_ROOT/htslib" \
    "$HTTP_ROOT/httpdirfs" \
    "$HTTP_ROOT/uploaded"

  printf 'downloaded through libcurl\n' >"$HTTP_ROOT/plain.txt"
  printf 'cmake download payload\n' >"$HTTP_ROOT/cmake/download.txt"
  cat >"$HTTP_ROOT/gdal/sample.geojson" <<'EOF'
{
  "type": "FeatureCollection",
  "name": "sample",
  "features": [
    {
      "type": "Feature",
      "properties": {
        "name": "smoke"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [1.0, 2.0]
      }
    }
  ]
}
EOF
  printf 'htslib smoke\n' >"$HTTP_ROOT/htslib/data.txt"
  printf 'httpdirfs smoke\n' >"$HTTP_ROOT/httpdirfs/hello.txt"

  if selected "Git"; then
    prepare_git_fixture
  fi
  if selected "OSTree"; then
    prepare_ostree_fixture
  fi
  if selected "pacman/libalpm"; then
    prepare_pacman_fixture
  fi
  if selected "fwupd"; then
    prepare_fwupd_fixture
  fi
}

start_http_server() {
  local i

  log_step "Starting local HTTP server"
  rm -f "$HTTP_PORT_FILE"
  write_http_server
  python3 "$TEST_ROOT/http_server.py" "$HTTP_ROOT" "$HTTP_PORT_FILE" >"$HTTP_SERVER_LOG" 2>&1 &
  HTTP_SERVER_PID="$!"

  for i in $(seq 1 100); do
    if [[ -s "$HTTP_PORT_FILE" ]]; then
      HTTP_BASE="http://127.0.0.1:$(cat "$HTTP_PORT_FILE")"
      export HTTP_BASE
      return
    fi
    sleep 0.1
  done

  tail -n 200 "$HTTP_SERVER_LOG" >&2 || true
  die "HTTP server did not start"
}

test_git() {
  local clone_root="$TEST_ROOT/git-clone"

  selected "Git" || return 0
  log_step "Testing Git over HTTP"
  rm -rf "$clone_root"
  run_logged git-clone git clone "$HTTP_BASE/git/smoke.git" "$clone_root"
  require_nonempty_file "$clone_root/README.txt"
  require_file_contains "$clone_root/README.txt" "git smoke"
}

test_cmake() {
  local script_path="$TEST_ROOT/cmake-smoke.cmake"
  local upload_input="$TEST_ROOT/cmake-upload.txt"
  local download_output="$TEST_ROOT/cmake-downloaded.txt"

  selected "CMake" || return 0
  log_step "Testing CMake file(DOWNLOAD) and file(UPLOAD)"
  printf 'cmake upload payload\n' >"$upload_input"
  cat >"$script_path" <<EOF
file(DOWNLOAD "$HTTP_BASE/cmake/download.txt" "$download_output" STATUS download_status LOG download_log)
list(GET download_status 0 download_code)
if(NOT download_code EQUAL 0)
  message(FATAL_ERROR "download failed: \${download_status} :: \${download_log}")
endif()
file(UPLOAD "$upload_input" "$HTTP_BASE/upload/cmake.txt" STATUS upload_status LOG upload_log)
list(GET upload_status 0 upload_code)
if(NOT upload_code EQUAL 0)
  message(FATAL_ERROR "upload failed: \${upload_status} :: \${upload_log}")
endif()
EOF
  run_logged cmake-smoke cmake -P "$script_path"
  require_nonempty_file "$download_output"
  require_file_contains "$download_output" "cmake download payload"
  require_nonempty_file "$HTTP_ROOT/uploaded/cmake.txt"
  require_file_contains "$HTTP_ROOT/uploaded/cmake.txt" "cmake upload payload"
}

test_php_curl() {
  local script_path="$TEST_ROOT/php-curl-smoke.php"

  selected "PHP cURL extension" || return 0
  log_step "Testing PHP cURL extension"
  cat >"$script_path" <<EOF
<?php
\$ch = curl_init("$HTTP_BASE/plain.txt");
curl_setopt(\$ch, CURLOPT_RETURNTRANSFER, true);
\$body = curl_exec(\$ch);
if (\$body === false) {
    fwrite(STDERR, curl_error(\$ch) . PHP_EOL);
    exit(1);
}
if (\$body !== "downloaded through libcurl\\n") {
    fwrite(STDERR, "unexpected body: " . \$body . PHP_EOL);
    exit(1);
}
curl_close(\$ch);
EOF
  run_logged php-curl php "$script_path"
}

test_pycurl() {
  local script_path="$TEST_ROOT/pycurl-smoke.py"

  selected "PycURL" || return 0
  log_step "Testing PycURL"
  cat >"$script_path" <<EOF
import io
import pycurl

buf = io.BytesIO()
curl = pycurl.Curl()
curl.setopt(pycurl.URL, "$HTTP_BASE/plain.txt")
curl.setopt(pycurl.WRITEDATA, buf)
curl.perform()
curl.close()

body = buf.getvalue().decode("utf-8")
if body != "downloaded through libcurl\\n":
    raise SystemExit(f"unexpected body: {body!r}")
EOF
  run_logged pycurl python3 "$script_path"
}

test_r_curl() {
  selected "R curl package" || return 0
  log_step "Testing R curl package"
  run_logged r-curl \
    Rscript -e "library(curl); res <- curl_fetch_memory('${HTTP_BASE}/plain.txt'); stopifnot(rawToChar(res\$content) == 'downloaded through libcurl\\n')"
}

test_gdal() {
  local output_path="$TEST_ROOT/gdal-ogrinfo.txt"

  selected "GDAL" || return 0
  log_step "Testing GDAL /vsicurl/"
  run_logged gdal-ogrinfo \
    ogrinfo -ro -so "/vsicurl/${HTTP_BASE}/gdal/sample.geojson" sample
  cp "$BUILD_LOG_DIR/gdal-ogrinfo.log" "$output_path"
  require_file_contains "$output_path" "Feature Count: 1"
  require_file_contains "$output_path" "name: String"
}

test_ostree() {
  local client_repo="$TEST_ROOT/ostree-client"
  local checkout_dir="$TEST_ROOT/ostree-checkout"

  selected "OSTree" || return 0
  log_step "Testing OSTree HTTP pull"
  rm -rf "$client_repo" "$checkout_dir"
  ostree --repo="$client_repo" init --mode=bare-user >/dev/null
  ostree remote add --repo="$client_repo" --no-gpg-verify origin "$HTTP_BASE/ostree/repo" >/dev/null
  run_logged ostree-pull ostree --repo="$client_repo" pull origin main
  run_logged ostree-checkout ostree --repo="$client_repo" checkout origin:main "$checkout_dir"
  require_nonempty_file "$checkout_dir/message.txt"
  require_file_contains "$checkout_dir/message.txt" "ostree smoke"
}

test_librepo() {
  local script_path="$TEST_ROOT/librepo-smoke.py"
  local output_path="$TEST_ROOT/librepo-downloaded.txt"

  selected "librepo" || return 0
  log_step "Testing librepo download_url()"
  cat >"$script_path" <<EOF
import os
import librepo

fd = os.open("$output_path", os.O_RDWR | os.O_CREAT | os.O_TRUNC, 0o666)
try:
    librepo.download_url("$HTTP_BASE/plain.txt", fd)
finally:
    os.close(fd)
EOF
  run_logged librepo python3 "$script_path"
  require_nonempty_file "$output_path"
  require_file_contains "$output_path" "downloaded through libcurl"
}

test_htslib() {
  local source_path="$TEST_ROOT/htslib-smoke.c"
  local binary_path="$TEST_ROOT/htslib-smoke"

  selected "HTSlib" || return 0
  log_step "Testing HTSlib hopen() over HTTP"
  cat >"$source_path" <<EOF
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <htslib/hfile.h>

int main(void) {
  const char *url = "$HTTP_BASE/htslib/data.txt";
  char buffer[64] = {0};
  ssize_t got;
  hFILE *fp = hopen(url, "r");
  if (fp == NULL) {
    perror("hopen");
    return 1;
  }
  got = hread(fp, buffer, sizeof(buffer) - 1);
  if (got < 0) {
    perror("hread");
    hclose(fp);
    return 1;
  }
  buffer[got] = '\\0';
  if (strcmp(buffer, "htslib smoke\\n") != 0) {
    fprintf(stderr, "unexpected body: %s\\n", buffer);
    hclose(fp);
    return 1;
  }
  if (hclose(fp) != 0) {
    perror("hclose");
    return 1;
  }
  return 0;
}
EOF
  run_logged htslib-build \
    bash -lc "gcc -O2 -Wall -Wextra '$source_path' -o '$binary_path' \$(pkg-config --cflags --libs htslib)"
  run_logged htslib-run "$binary_path"
}

test_pacman() {
  local root_dir="$TEST_ROOT/pacman-root"
  local config_path="$TEST_ROOT/pacman.conf"

  selected "pacman/libalpm" || return 0
  log_step "Testing pacman/libalpm HTTP repo download"
  rm -rf "$root_dir"
  mkdir -p "$root_dir/root" "$root_dir/db" "$root_dir/cache" "$root_dir/gpg" "$root_dir/hooks"
  cat >"$config_path" <<EOF
[options]
Architecture = auto
CacheDir = $root_dir/cache
DBPath = $root_dir/db
GPGDir = $root_dir/gpg
LogFile = $root_dir/pacman.log
HookDir = $root_dir/hooks
SigLevel = Never
LocalFileSigLevel = Never
RemoteFileSigLevel = Never

[smoke]
Server = $HTTP_BASE/pacman/repo
EOF
  run_logged pacman-sync \
    pacman --config "$config_path" --root "$root_dir/root" --cachedir "$root_dir/cache" \
      --dbpath "$root_dir/db" --gpgdir "$root_dir/gpg" --noconfirm -Sy smoke-pkg
  require_nonempty_file "$root_dir/root/usr/share/smoke-pkg/probe.txt"
  require_file_contains "$root_dir/root/usr/share/smoke-pkg/probe.txt" "pacman smoke"
}

test_httpdirfs() {
  local mount_root="$TEST_ROOT/httpdirfs-mount"
  local cache_root="$TEST_ROOT/httpdirfs-cache"
  local log_path="$BUILD_LOG_DIR/httpdirfs.log"
  local pid=""
  local i

  selected "HTTPDirFS" || return 0
  log_step "Testing HTTPDirFS mount and read"
  rm -rf "$mount_root" "$cache_root"
  mkdir -p "$mount_root" "$cache_root"

  cleanup_httpdirfs() {
    if mountpoint -q "$mount_root"; then
      fusermount3 -u "$mount_root" >/dev/null 2>&1 || true
    fi
    if [[ -n "$pid" ]]; then
      kill "$pid" >/dev/null 2>&1 || true
      wait "$pid" 2>/dev/null || true
    fi
  }

  trap cleanup_httpdirfs RETURN

  httpdirfs -f --cache --cache-location "$cache_root" "$HTTP_BASE/httpdirfs/" "$mount_root" >"$log_path" 2>&1 &
  pid="$!"

  for i in $(seq 1 100); do
    if mountpoint -q "$mount_root"; then
      break
    fi
    if ! kill -0 "$pid" >/dev/null 2>&1; then
      tail -n 200 "$log_path" >&2 || true
      die "httpdirfs exited before mounting"
    fi
    sleep 0.1
  done

  mountpoint -q "$mount_root" || {
    tail -n 200 "$log_path" >&2 || true
    die "httpdirfs did not mount"
  }

  require_nonempty_file "$mount_root/hello.txt"
  require_file_contains "$mount_root/hello.txt" "httpdirfs smoke"

  cleanup_httpdirfs
  trap - RETURN
}

test_fwupd() {
  local fwupd_root="$TEST_ROOT/fwupd"
  local etc_root="$fwupd_root/etc"
  local data_root="$fwupd_root/usr/share"
  local metadata_cache=""
  local remotes_json="$TEST_ROOT/fwupd-remotes.json"
  local signature_cache=""
  local state_root="$fwupd_root/var"

  selected "fwupd" || return 0
  log_step "Testing fwupdtool refresh from HTTP remote"
  rm -rf "$fwupd_root"
  mkdir -p \
    "$etc_root/fwupd/remotes.d" \
    "$etc_root/pki/fwupd-metadata" \
    "$data_root/fwupd/remotes.d" \
    "$state_root/lib/fwupd"
  cp "$FWUPD_CERT_ROOT/ca.pem" "$etc_root/pki/fwupd-metadata/smoke-ca.pem"
  cat >"$etc_root/fwupd/remotes.d/smoke.conf" <<EOF
[fwupd Remote]
Enabled=true
Title=Smoke Remote
Keyring=jcat
MetadataURI=$HTTP_BASE/fwupd/metadata.xml.gz
RefreshInterval=0
EOF
  run_logged fwupd-refresh \
    env \
      FWUPD_MACHINE_KIND=container \
      FWUPD_SYSCONFDIR="$etc_root" \
      FWUPD_DATADIR="$data_root" \
      FWUPD_LOCALSTATEDIR="$state_root" \
      fwupdtool refresh --force
  if ! env \
      FWUPD_MACHINE_KIND=container \
      FWUPD_SYSCONFDIR="$etc_root" \
      FWUPD_DATADIR="$data_root" \
      FWUPD_LOCALSTATEDIR="$state_root" \
      fwupdtool get-remotes >"$remotes_json" 2>"$BUILD_LOG_DIR/fwupd-get-remotes.log"; then
    printf 'command failed for fwupd-get-remotes; last 200 log lines from %s:\n' \
      "$BUILD_LOG_DIR/fwupd-get-remotes.log" >&2
    tail -n 200 "$BUILD_LOG_DIR/fwupd-get-remotes.log" >&2 || true
    return 1
  fi
  metadata_cache="$(sed -n 's/^ *Filename: *//p' "$remotes_json" | head -n1)"
  if [[ -z "$metadata_cache" ]]; then
    printf 'failed to extract FilenameCache from %s\n' "$remotes_json" >&2
    printf -- '--- %s ---\n' "$remotes_json" >&2
    cat "$remotes_json" >&2 || true
    return 1
  fi
  signature_cache="$(sed -n 's/^ *Filename Signature: *//p' "$remotes_json" | head -n1)"
  if [[ -z "$signature_cache" ]]; then
    printf 'failed to extract FilenameCacheSig from %s\n' "$remotes_json" >&2
    printf -- '--- %s ---\n' "$remotes_json" >&2
    cat "$remotes_json" >&2 || true
    return 1
  fi
  [[ -n "$metadata_cache" ]] || die "failed to discover fwupd metadata cache path for smoke remote"
  [[ -n "$signature_cache" ]] || die "failed to discover fwupd signature cache path for smoke remote"
  require_nonempty_file "$metadata_cache"
  require_nonempty_file "$signature_cache"
}

validate_dependents_inventory
export_tracked_source
build_local_curl_runtime
prepare_http_fixtures
start_http_server

test_git
test_cmake
test_php_curl
test_pycurl
test_r_curl
test_gdal
test_ostree
test_librepo
test_htslib
test_pacman
test_httpdirfs
test_fwupd

log_step "All selected dependent smoke tests passed"
CONTAINER
