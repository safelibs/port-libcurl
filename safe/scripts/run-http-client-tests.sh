#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --flavor <openssl|gnutls> [--build-state <path>] [--program <name> | --clients <name>...] [--binary <path>]" >&2
}

flavor=""
build_state=""
program=""
binary_override=""
declare -a clients=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --flavor)
      flavor="${2:-}"
      shift 2
      ;;
    --build-state)
      build_state="${2:-}"
      shift 2
      ;;
    --program)
      program="${2:-}"
      shift 2
      ;;
    --clients)
      shift
      added=0
      while [[ $# -gt 0 && "$1" != -* ]]; do
        clients+=("$1")
        shift
        added=1
      done
      (( added )) || { usage; exit 2; }
      ;;
    --binary)
      binary_override="${2:-}"
      shift 2
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
if [[ -z "${build_state}" ]]; then
  build_state="${safe_dir}/.compat/${flavor}/build-state.json"
fi

if [[ -n "${program}" ]]; then
  clients=("${program}")
fi

if ((${#clients[@]} == 0)); then
  echo "no HTTP client program selected; use --program or --clients" >&2
  exit 2
fi

if [[ -n "${binary_override}" && ${#clients[@]} -ne 1 ]]; then
  echo "--binary can only be used with a single --program/--clients entry" >&2
  exit 2
fi

declare -a build_targets=()
needs_websocket_fixture=0
needs_http_fixture=0
for client in "${clients[@]}"; do
  case "${client}" in
    h2-download|h2-pausing|h2-serverpush|h2-upgrade-extreme|tls-session-reuse)
      needs_http_fixture=1
      build_targets+=(--target "http-client:${client}")
      ;;
    ws-data|ws-pingpong)
      needs_websocket_fixture=1
      build_targets+=(--target "http-client:${client}")
      ;;
    *)
      echo "unknown HTTP client program: ${client}" >&2
      exit 2
      ;;
  esac
done

if (( needs_http_fixture )) && ! command -v nghttpx >/dev/null 2>&1; then
  echo "HTTP/2 client programs require nghttpx to provide the tracked local fixture" >&2
  exit 1
fi

if [[ -z "${binary_override}" ]]; then
  "${script_dir}/build-compat-consumers.sh" --flavor "${flavor}" "${build_targets[@]}"
  [[ -f "${build_state}" ]] || { echo "missing build state: ${build_state}" >&2; exit 1; }
fi

fixture_dir="$(mktemp -d)"
http_root="${fixture_dir}/http-root"
http_pid_file="${fixture_dir}/http.pid"
http_port_file="${fixture_dir}/http.port"
http_log_file="${fixture_dir}/http.log"
http_port=""
websocket_port=""
websocket_pid=""
websocket_log_file="${fixture_dir}/websocket.log"
tls_proxy_pid=""
tls_proxy_port=""
tls_proxy_log_file="${fixture_dir}/nghttpx-tls.log"
h2c_proxy_pid=""
h2c_proxy_port=""
h2c_proxy_log_file="${fixture_dir}/nghttpx-h2c.log"

cleanup() {
  if [[ -n "${websocket_pid}" ]]; then
    kill "${websocket_pid}" 2>/dev/null || true
    wait "${websocket_pid}" 2>/dev/null || true
  fi
  if [[ -n "${tls_proxy_pid}" ]]; then
    kill "${tls_proxy_pid}" 2>/dev/null || true
    wait "${tls_proxy_pid}" 2>/dev/null || true
  fi
  if [[ -n "${h2c_proxy_pid}" ]]; then
    kill "${h2c_proxy_pid}" 2>/dev/null || true
    wait "${h2c_proxy_pid}" 2>/dev/null || true
  fi
  if [[ -f "${http_pid_file}" ]]; then
    "${script_dir}/http-fixtures.sh" stop --pid-file "${http_pid_file}" >/dev/null 2>&1 || true
  fi
  rm -rf "${fixture_dir}"
}
trap cleanup EXIT

pick_port() {
  python3 - <<'PY'
import socket

sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.bind(("127.0.0.1", 0))
print(sock.getsockname()[1])
sock.close()
PY
}

wait_for_tcp() {
  local port="$1"
  local pid="$2"
  local log_file="$3"
  for _ in $(seq 1 100); do
    if python3 - "$port" <<'PY' >/dev/null 2>&1
import socket
import sys

sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.settimeout(0.2)
try:
    sock.connect(("127.0.0.1", int(sys.argv[1])))
except OSError:
    raise SystemExit(1)
finally:
    sock.close()
PY
    then
      return 0
    fi
    if ! kill -0 "${pid}" 2>/dev/null; then
      cat "${log_file}" >&2 || true
      echo "fixture on port ${port} exited before becoming ready" >&2
      exit 1
    fi
    sleep 0.1
  done
  cat "${log_file}" >&2 || true
  echo "fixture on port ${port} did not become ready" >&2
  exit 1
}

start_http_fixture() {
  "${script_dir}/http-fixtures.sh" prepare --root "${http_root}"
  "${script_dir}/http-fixtures.sh" start \
    --root "${http_root}" \
    --pid-file "${http_pid_file}" \
    --port-file "${http_port_file}" \
    --log "${http_log_file}"
  http_port="$(cat "${http_port_file}")"
}

start_nghttpx() {
  local mode="$1"
  local port="$2"
  local backend_port="$3"
  local log_file="$4"
  local cert="${safe_dir}/vendor/upstream/tests/certs/Server-localhost-sv.pem"
  local key="${safe_dir}/vendor/upstream/tests/certs/Server-localhost-sv.key"
  local -a cmd=(
    nghttpx
    --conf=/dev/null
    --single-thread
    -n1
    --frontend-http2-max-concurrent-streams=100
    --backend-connections-per-frontend=16
    -b"127.0.0.1,${backend_port}"
  )

  if [[ "${mode}" == "tls" ]]; then
    cmd+=(-f"127.0.0.1,${port}" "${key}" "${cert}")
  else
    cmd+=(-f"127.0.0.1,${port};no-tls")
  fi

  "${cmd[@]}" >"${log_file}" 2>&1 &
  local pid=$!
  wait_for_tcp "${port}" "${pid}" "${log_file}"
  printf '%s\n' "${pid}"
}

start_websocket_fixture() {
  local port_file="${fixture_dir}/websocket.port"
  python3 -u - "${port_file}" >"${websocket_log_file}" 2>&1 <<'PY' &
import base64
import hashlib
import socket
import struct
import sys
import threading

GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
port_file = sys.argv[1]

def recv_exact(conn, amount):
    data = bytearray()
    while len(data) < amount:
        chunk = conn.recv(amount - len(data))
        if not chunk:
            raise ConnectionError("short read")
        data.extend(chunk)
    return bytes(data)

def send_frame(conn, opcode, payload=b""):
    header = bytearray([0x80 | opcode])
    length = len(payload)
    if length < 126:
        header.append(length)
    elif length < 65536:
        header.append(126)
        header.extend(struct.pack("!H", length))
    else:
        header.append(127)
        header.extend(struct.pack("!Q", length))
    conn.sendall(header + payload)

def handle(conn):
    with conn:
        request = bytearray()
        while b"\r\n\r\n" not in request:
            chunk = conn.recv(4096)
            if not chunk:
                return
            request.extend(chunk)
        lines = request.decode("latin1").split("\r\n")
        headers = {}
        for line in lines[1:]:
            if not line:
                break
            name, value = line.split(":", 1)
            headers[name.strip().lower()] = value.strip()
        key = headers.get("sec-websocket-key")
        if not key:
            return
        accept = base64.b64encode(
            hashlib.sha1((key + GUID).encode("ascii")).digest()
        ).decode("ascii")
        conn.sendall(
            (
                "HTTP/1.1 101 Switching Protocols\r\n"
                "Upgrade: websocket\r\n"
                "Connection: Upgrade\r\n"
                f"Sec-WebSocket-Accept: {accept}\r\n\r\n"
            ).encode("ascii")
        )
        while True:
            try:
                header = recv_exact(conn, 2)
            except ConnectionError:
                return
            first, second = header
            opcode = first & 0x0F
            length = second & 0x7F
            masked = (second & 0x80) != 0
            if length == 126:
                length = struct.unpack("!H", recv_exact(conn, 2))[0]
            elif length == 127:
                length = struct.unpack("!Q", recv_exact(conn, 8))[0]
            mask = recv_exact(conn, 4) if masked else b""
            payload = bytearray(recv_exact(conn, length)) if length else bytearray()
            if masked:
                for i in range(length):
                    payload[i] ^= mask[i % 4]
            payload = bytes(payload)
            if opcode == 0x8:
                send_frame(conn, 0x8, payload)
                return
            if opcode == 0x9:
                send_frame(conn, 0xA, payload)
                continue
            if opcode in (0x1, 0x2):
                send_frame(conn, opcode, payload)
                continue
            if opcode == 0xA:
                continue
            send_frame(conn, 0x8, b"")
            return

server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
server.bind(("127.0.0.1", 0))
server.listen()
with open(port_file, "w", encoding="utf-8") as fh:
    fh.write(str(server.getsockname()[1]))
    fh.flush()
while True:
    conn, _ = server.accept()
    threading.Thread(target=handle, args=(conn,), daemon=True).start()
PY
  websocket_pid=$!
  for _ in $(seq 1 100); do
    if [[ -s "${port_file}" ]]; then
      websocket_port="$(cat "${port_file}")"
      return 0
    fi
    if ! kill -0 "${websocket_pid}" 2>/dev/null; then
      cat "${websocket_log_file}" >&2 || true
      echo "websocket fixture failed to start" >&2
      exit 1
    fi
    sleep 0.1
  done
  cat "${websocket_log_file}" >&2 || true
  echo "websocket fixture failed to start" >&2
  exit 1
}

if (( needs_http_fixture )); then
  start_http_fixture
  tls_proxy_port="$(pick_port)"
  h2c_proxy_port="$(pick_port)"
  tls_proxy_pid="$(start_nghttpx tls "${tls_proxy_port}" "${http_port}" "${tls_proxy_log_file}")"
  h2c_proxy_pid="$(start_nghttpx h2c "${h2c_proxy_port}" "${http_port}" "${h2c_proxy_log_file}")"
fi

if (( needs_websocket_fixture )); then
  start_websocket_fixture
fi

resolve_binary() {
  local client="$1"
  if [[ -n "${binary_override}" ]]; then
    printf '%s\n' "${binary_override}"
    return 0
  fi

  local binary
  binary="$(jq -r --arg id "http-client:${client}" '.targets[] | select(.target_id==$id) | .executable_path' "${build_state}")"
  if [[ -z "${binary}" || "${binary}" == "null" || ! -x "${binary}" ]]; then
    local fallback="${safe_dir}/.compat/${flavor}/worktree/tests/http/clients/${client}"
    if [[ -x "${fallback}" ]]; then
      binary="${fallback}"
    else
      echo "missing executable path for ${client} in ${build_state}" >&2
      exit 1
    fi
  fi
  printf '%s\n' "${binary}"
}

run_in_workdir() {
  local workdir="$1"
  local log_file="$2"
  shift 2
  mkdir -p "${workdir}"
  if ! (
    cd "${workdir}"
    "$@" >"${log_file}" 2>&1
  ); then
    cat "${log_file}" >&2 || true
    return 1
  fi
}

assert_file() {
  local path="$1"
  local log_file="$2"
  if [[ ! -s "${path}" ]]; then
    cat "${log_file}" >&2 || true
    echo "expected output file was not produced: ${path}" >&2
    exit 1
  fi
}

assert_exists() {
  local path="$1"
  local log_file="$2"
  if [[ ! -e "${path}" ]]; then
    cat "${log_file}" >&2 || true
    echo "expected output path was not produced: ${path}" >&2
    exit 1
  fi
}

run_client() {
  local client="$1"
  local binary="$2"
  local workdir="${fixture_dir}/runs/${client}"
  local log_file="${workdir}/client.log"

  case "${client}" in
    h2-download)
      run_in_workdir "${workdir}" "${log_file}" \
        "${binary}" -m 3 -n 6 "https://localhost:${tls_proxy_port}/large.bin"
      local download_count
      download_count="$(find "${workdir}" -maxdepth 1 -name 'download_*.data' -type f | wc -l | tr -d ' ')"
      if [[ "${download_count}" -lt 6 ]]; then
        cat "${log_file}" >&2 || true
        echo "expected multiplexed downloads, found ${download_count} output files" >&2
        exit 1
      fi
      ;;
    h2-serverpush)
      mkdir -p "${workdir}"
      (
        cd "${workdir}"
        "${binary}" "https://localhost:${tls_proxy_port}/push" >"${log_file}" 2>&1
      ) &
      local push_pid=$!
      local observed_push=0
      for _ in $(seq 1 200); do
        if grep -q "push callback approves" "${log_file}" 2>/dev/null &&
           grep -q "The PATH is /push/asset.txt" "${log_file}" 2>/dev/null &&
           [[ -e "${workdir}/download_0.data" ]] &&
           find "${workdir}" -maxdepth 1 -name 'push*' -type f | grep -q .
        then
          observed_push=1
          break
        fi
        if ! kill -0 "${push_pid}" 2>/dev/null; then
          if ! wait "${push_pid}"; then
            cat "${log_file}" >&2 || true
            exit 1
          fi
          break
        fi
        sleep 0.1
      done
      if kill -0 "${push_pid}" 2>/dev/null; then
        kill "${push_pid}" 2>/dev/null || true
        wait "${push_pid}" 2>/dev/null || true
      fi
      if (( ! observed_push )); then
        cat "${log_file}" >&2 || true
        echo "did not observe the tracked HTTP/2 push callback surface" >&2
        exit 1
      fi
      assert_exists "${workdir}/download_0.data" "${log_file}"
      if ! find "${workdir}" -maxdepth 1 -name 'push*' -type f | grep -q .; then
        cat "${log_file}" >&2 || true
        echo "expected at least one pushed response file" >&2
        exit 1
      fi
      ;;
    h2-pausing)
      run_in_workdir "${workdir}" "${log_file}" \
        "${binary}" "https://localhost:${tls_proxy_port}/large.bin"
      ;;
    h2-upgrade-extreme)
      mkdir -p "${workdir}"
      (
        cd "${workdir}"
        "${binary}" "http://127.0.0.1:${h2c_proxy_port}/large.bin" >"${log_file}" 2>&1
      ) &
      local upgrade_pid=$!
      local observed_upgrade=0
      for _ in $(seq 1 300); do
        local range_hits
        range_hits="$(grep -c '"GET /large.bin HTTP/1.1" 206' "${http_log_file}" 2>/dev/null || true)"
        if [[ "${range_hits}" -ge 5 ]] &&
           grep -q "Connection #0 to host 127.0.0.1 left intact" "${log_file}" 2>/dev/null
        then
          observed_upgrade=1
          break
        fi
        if ! kill -0 "${upgrade_pid}" 2>/dev/null; then
          wait "${upgrade_pid}" || true
          break
        fi
        sleep 0.1
      done
      if kill -0 "${upgrade_pid}" 2>/dev/null; then
        kill "${upgrade_pid}" 2>/dev/null || true
        wait "${upgrade_pid}" 2>/dev/null || true
      fi
      if (( ! observed_upgrade )); then
        cat "${log_file}" >&2 || true
        cat "${http_log_file}" >&2 || true
        echo "did not observe the tracked HTTP/2 upgrade/range surface" >&2
        exit 1
      fi
      ;;
    tls-session-reuse)
      mkdir -p "${workdir}"
      (
        cd "${workdir}"
        "${binary}" "https://localhost:${tls_proxy_port}/plain.txt" >"${log_file}" 2>&1
      ) &
      local reuse_pid=$!
      local observed_reuse=0
      for _ in $(seq 1 200); do
        local plain_hits
        plain_hits="$(grep -c '"GET /plain.txt HTTP/1.1" 200' "${http_log_file}" 2>/dev/null || true)"
        if [[ "${plain_hits}" -ge 1 ]] &&
           grep -q "yet_to_start=0" "${log_file}" 2>/dev/null &&
           grep -q "Connection #0 to host localhost left intact" "${log_file}" 2>/dev/null
        then
          observed_reuse=1
          break
        fi
        if ! kill -0 "${reuse_pid}" 2>/dev/null; then
          wait "${reuse_pid}" || true
          break
        fi
        sleep 0.1
      done
      if kill -0 "${reuse_pid}" 2>/dev/null; then
        kill "${reuse_pid}" 2>/dev/null || true
        wait "${reuse_pid}" 2>/dev/null || true
      fi
      if (( ! observed_reuse )); then
        cat "${log_file}" >&2 || true
        cat "${http_log_file}" >&2 || true
        echo "did not observe the tracked TLS reuse surface" >&2
        exit 1
      fi
      ;;
    ws-data)
      run_in_workdir "${workdir}" "${log_file}" \
        "${binary}" "ws://127.0.0.1:${websocket_port}/echo" 1 300
      ;;
    ws-pingpong)
      run_in_workdir "${workdir}" "${log_file}" \
        "${binary}" "ws://127.0.0.1:${websocket_port}/echo" "compat-ping"
      ;;
  esac
}

for client in "${clients[@]}"; do
  binary="$(resolve_binary "${client}")"
  [[ -x "${binary}" ]] || { echo "missing HTTP client binary: ${binary}" >&2; exit 1; }
  run_client "${client}" "${binary}"
done
