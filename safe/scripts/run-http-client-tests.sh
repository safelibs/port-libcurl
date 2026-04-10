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
for client in "${clients[@]}"; do
  case "${client}" in
    h2-download|h2-pausing|h2-serverpush|h2-upgrade-extreme|tls-session-reuse)
      if ! command -v nghttpd >/dev/null 2>&1; then
        echo "program ${client} requires nghttpd; tests/http/README.md and tests/http/config.ini.in both assume external HTTP server tooling" >&2
        exit 1
      fi
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

if [[ ! -f "${build_state}" || ${#build_targets[@]} -gt 0 ]]; then
  "${script_dir}/build-compat-consumers.sh" --flavor "${flavor}" "${build_targets[@]}"
fi
[[ -f "${build_state}" ]] || { echo "missing build state: ${build_state}" >&2; exit 1; }

fixture_dir=""
fixture_pid=""
fixture_port=""
cleanup() {
  if [[ -n "${fixture_pid}" ]]; then
    kill "${fixture_pid}" 2>/dev/null || true
    wait "${fixture_pid}" 2>/dev/null || true
  fi
  if [[ -n "${fixture_dir}" ]]; then
    rm -rf "${fixture_dir}"
  fi
}
trap cleanup EXIT

start_websocket_fixture() {
  fixture_dir="$(mktemp -d)"
  local port_file="${fixture_dir}/port"
  local log_file="${fixture_dir}/fixture.log"
  python3 -u - "${port_file}" >"${log_file}" 2>&1 <<'PY' &
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
  fixture_pid=$!
  for _ in $(seq 1 100); do
    if [[ -s "${port_file}" ]]; then
      fixture_port="$(cat "${port_file}")"
      return 0
    fi
    sleep 0.1
  done
  cat "${log_file}" >&2 || true
  echo "websocket fixture failed to start" >&2
  exit 1
}

if (( needs_websocket_fixture )); then
  start_websocket_fixture
fi

resolve_binary() {
  local client="$1"
  if [[ -n "${binary_override}" ]]; then
    printf '%s\n' "${binary_override}"
  else
    jq -r --arg id "http-client:${client}" '.targets[] | select(.target_id==$id) | .executable_path' "${build_state}"
  fi
}

run_client() {
  local client="$1"
  local binary="$2"
  case "${client}" in
    ws-data)
      "${binary}" "ws://127.0.0.1:${fixture_port}/echo" 1 300
      ;;
    ws-pingpong)
      "${binary}" "ws://127.0.0.1:${fixture_port}/echo" "compat-ping"
      ;;
    *)
      echo "HTTP client runner is dependency-gated and intentionally does not fabricate the absent pytest fixture tree; provide the required external service and execute ${binary} manually." >&2
      exit 1
      ;;
  esac
}

for client in "${clients[@]}"; do
  binary="$(resolve_binary "${client}")"
  [[ -x "${binary}" ]] || { echo "missing HTTP client binary: ${binary}" >&2; exit 1; }
  run_client "${client}" "${binary}"
done
