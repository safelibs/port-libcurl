#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --implementation <original|safe> --flavor <openssl|gnutls> --matrix <name> --output-dir <path>" >&2
}

implementation=""
flavor=""
matrix=""
output_dir=""
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
    --matrix)
      matrix="${2:-}"
      shift 2
      ;;
    --output-dir)
      output_dir="${2:-}"
      shift 2
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

if [[ -z "${implementation}" || -z "${flavor}" || -z "${matrix}" || -z "${output_dir}" ]]; then
  usage
  exit 2
fi

if [[ "${implementation}" != "original" && "${implementation}" != "safe" ]]; then
  echo "unsupported implementation: ${implementation}" >&2
  exit 2
fi

if [[ "${flavor}" != "openssl" && "${flavor}" != "gnutls" ]]; then
  echo "unsupported flavor: ${flavor}" >&2
  exit 2
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
safe_dir="$(cd "${script_dir}/.." && pwd)"
repo_root="$(cd "${safe_dir}/.." && pwd)"
benchmark_dir="${safe_dir}/benchmarks"
scenarios_file="${benchmark_dir}/scenarios.json"
fixture_dir="$(mktemp -d)"
http_root="${fixture_dir}/http-root"
http_pid_file="${fixture_dir}/http.pid"
http_port_file="${fixture_dir}/http.port"
http_log_file="${fixture_dir}/http.log"
tls_proxy_port_file="${fixture_dir}/tls.port"
tls_proxy_log_file="${fixture_dir}/nghttpx-tls.log"
tls_proxy_pid=""

cleanup() {
  if [[ -n "${tls_proxy_pid}" ]]; then
    kill "${tls_proxy_pid}" 2>/dev/null || true
    wait "${tls_proxy_pid}" 2>/dev/null || true
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
  dd if=/dev/zero of="${http_root}/bench-http1.bin" bs=1M count=1 status=none
  dd if=/dev/zero of="${http_root}/bench-h2.bin" bs=1M count=64 status=none
  "${script_dir}/http-fixtures.sh" start \
    --root "${http_root}" \
    --pid-file "${http_pid_file}" \
    --port-file "${http_port_file}" \
    --log "${http_log_file}"
}

start_tls_proxy() {
  local backend_port="$1"
  local tls_port
  local cert="${safe_dir}/vendor/upstream/tests/certs/Server-localhost-sv.pem"
  local key="${safe_dir}/vendor/upstream/tests/certs/Server-localhost-sv.key"

  if ! command -v nghttpx >/dev/null 2>&1; then
    echo "nghttpx is required for HTTPS benchmark scenarios" >&2
    exit 1
  fi

  tls_port="$(pick_port)"
  printf '%s\n' "${tls_port}" >"${tls_proxy_port_file}"
  nghttpx \
    --conf=/dev/null \
    --single-thread \
    -n1 \
    --frontend-http2-max-concurrent-streams=100 \
    --backend-connections-per-frontend=16 \
    -b"127.0.0.1,${backend_port}" \
    -f"127.0.0.1,${tls_port}" \
    "${key}" "${cert}" >"${tls_proxy_log_file}" 2>&1 &
  tls_proxy_pid=$!
  wait_for_tcp "${tls_port}" "${tls_proxy_pid}" "${tls_proxy_log_file}"
}

build_libraries() {
  bash "${script_dir}/build-reference-curl.sh" --flavor "${flavor}"
  if [[ "${implementation}" == "safe" ]]; then
    bash "${script_dir}/build-compat-consumers.sh" --flavor "${flavor}" --target "http-client:h2-download"
  fi
}

compile_harness() {
  local source_path="$1"
  local output_path="$2"
  local lib_dir="$3"
  shift 3

  cc \
    -O2 \
    -DNDEBUG \
    -std=gnu11 \
    -Wall \
    -Wextra \
    -I"${safe_dir}/include" \
    "${source_path}" \
    -o "${output_path}" \
    -L"${lib_dir}" \
    -Wl,-rpath,"${lib_dir}" \
    "$@"
}

mkdir -p "${output_dir}"
build_libraries
start_http_fixture
http_port="$(cat "${http_port_file}")"
start_tls_proxy "${http_port}"
tls_port="$(cat "${tls_proxy_port_file}")"

build_dir="${fixture_dir}/build"
mkdir -p "${build_dir}"

if [[ "${implementation}" == "original" ]]; then
  reference_dist="${safe_dir}/.reference/${flavor}/dist"
  lib_dir="${build_dir}/reference-lib"
  mkdir -p "${lib_dir}"
  ln -sf "${reference_dist}/libcurl-reference-${flavor}.so.4" "${lib_dir}/libcurl.so.4"
  ln -sf "libcurl.so.4" "${lib_dir}/libcurl.so"
  link_args=("-lcurl")
else
  lib_dir="${safe_dir}/.compat/${flavor}/stage/lib"
  link_args=("-lcurl")
fi

compile_harness "${benchmark_dir}/harness/easy_loop.c" "${build_dir}/easy_loop" "${lib_dir}" "${link_args[@]}"
compile_harness "${benchmark_dir}/harness/multi_parallel.c" "${build_dir}/multi_parallel" "${lib_dir}" "${link_args[@]}"

python3 - "${scenarios_file}" "${matrix}" "${output_dir}" "${implementation}" "${flavor}" "${build_dir}/easy_loop" "${build_dir}/multi_parallel" "${http_port}" "${tls_port}" <<'PY'
import json
import pathlib
import subprocess
import sys

scenarios_path = pathlib.Path(sys.argv[1])
matrix = sys.argv[2]
output_dir = pathlib.Path(sys.argv[3])
implementation = sys.argv[4]
flavor = sys.argv[5]
easy_bin = pathlib.Path(sys.argv[6])
multi_bin = pathlib.Path(sys.argv[7])
http_port = int(sys.argv[8])
tls_port = int(sys.argv[9])

doc = json.loads(scenarios_path.read_text(encoding="utf-8"))
try:
    selected_ids = doc["matrices"][matrix]
except KeyError as exc:
    raise SystemExit(f"unknown benchmark matrix: {matrix}") from exc

output_dir.mkdir(parents=True, exist_ok=True)
summary = {
    "schema_version": 1,
    "matrix": matrix,
    "implementation": implementation,
    "flavor": flavor,
    "scenarios": [],
}

for scenario_id in selected_ids:
    scenario = dict(doc["scenarios"][scenario_id])
    scheme = scenario["scheme"]
    host = scenario["host"]
    port = tls_port if scheme == "https" else http_port
    url = f"{scheme}://{host}:{port}{scenario['path']}"
    output_path = output_dir / f"{scenario_id}.json"
    harness = scenario["harness"]
    cmd = []

    if harness == "easy_loop":
        cmd = [
            str(easy_bin),
            "--scenario", scenario_id,
            "--implementation", implementation,
            "--flavor", flavor,
            "--url", url,
            "--requests", str(scenario["requests"]),
            "--samples", str(scenario["samples"]),
            "--warmups", str(scenario["warmups"]),
            "--output", str(output_path),
            "--http-version", scenario["http_version"],
        ]
        if scenario.get("share_ssl_session"):
            cmd.append("--share-ssl-session")
        if scenario.get("fresh_connect"):
            cmd.append("--fresh-connect")
        if scenario.get("forbid_reuse"):
            cmd.append("--forbid-reuse")
    elif harness == "multi_parallel":
        cmd = [
            str(multi_bin),
            "--scenario", scenario_id,
            "--implementation", implementation,
            "--flavor", flavor,
            "--url", url,
            "--transfers", str(scenario["transfers"]),
            "--parallel", str(scenario["parallel"]),
            "--samples", str(scenario["samples"]),
            "--warmups", str(scenario["warmups"]),
            "--output", str(output_path),
            "--http-version", scenario["http_version"],
        ]
        if scenario.get("max_host_connections") is not None:
            cmd.extend(["--max-host-connections", str(scenario["max_host_connections"])])
        if scenario.get("pipewait"):
            cmd.append("--pipewait")
        if scenario.get("share_ssl_session"):
            cmd.append("--share-ssl-session")
        if scenario.get("fresh_connect"):
            cmd.append("--fresh-connect")
        if scenario.get("forbid_reuse"):
            cmd.append("--forbid-reuse")
    else:
        raise SystemExit(f"unsupported harness: {harness}")

    if scenario.get("insecure"):
        cmd.append("--insecure")
    if scenario.get("resolve_loopback"):
        cmd.extend(["--resolve-host", f"{host}:{port}:127.0.0.1"])

    subprocess.run(cmd, check=True)
    summary["scenarios"].append(
        {
            "scenario_id": scenario_id,
            "output": output_path.name,
            "url": url,
            "harness": harness,
        }
    )

(output_dir / "index.json").write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY
