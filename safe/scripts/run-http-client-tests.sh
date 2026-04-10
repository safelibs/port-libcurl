#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --flavor <openssl|gnutls> [--build-state <path>] [--program <name>] [--binary <path>]" >&2
}

flavor=""
build_state=""
program=""
binary_override=""
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
[[ -f "${build_state}" ]] || "${script_dir}/build-compat-consumers.sh" --flavor "${flavor}"
[[ -f "${build_state}" ]] || { echo "missing build state: ${build_state}" >&2; exit 1; }

if [[ -z "${program}" ]]; then
  echo "no HTTP client program selected; use --program" >&2
  exit 2
fi

case "${program}" in
  h2-download|h2-pausing|h2-serverpush|h2-upgrade-extreme|tls-session-reuse)
    if ! command -v nghttpd >/dev/null 2>&1; then
      echo "program ${program} requires nghttpd; tests/http/README.md and tests/http/config.ini.in both assume external HTTP server tooling" >&2
      exit 1
    fi
    ;;
  ws-data|ws-pingpong)
    python3 - <<'PY' >/dev/null 2>&1 || {
import websockets  # noqa: F401
PY
      echo "program ${program} requires the Python websockets module and a websocket echo service; the tracked workspace does not include the upstream pytest fixture tree" >&2
      exit 1
    }
    ;;
  *)
    echo "unknown HTTP client program: ${program}" >&2
    exit 2
    ;;
esac

binary="${binary_override:-$(jq -r --arg id "http-client:${program}" '.targets[] | select(.target_id==$id) | .executable_path' "${build_state}")}"
[[ -x "${binary}" ]] || { echo "missing HTTP client binary: ${binary}" >&2; exit 1; }
echo "HTTP client runner is dependency-gated and intentionally does not fabricate the absent pytest fixture tree; provide the required external service and execute ${binary} manually." >&2
exit 1
