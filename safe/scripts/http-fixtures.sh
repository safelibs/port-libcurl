#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 prepare --root <dir> | start --root <dir> --pid-file <file> --port-file <file> --log <file> | stop --pid-file <file>" >&2
}

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

cmd="${1:-}"
shift || true
case "${cmd}" in
  prepare)
    root=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --root)
          root="${2:-}"
          shift 2
          ;;
        *)
          usage
          exit 2
          ;;
      esac
    done
    [[ -z "${root}" ]] && usage && exit 2
    python3 "${script_dir}/http-fixture.py" prepare --root "${root}"
    ;;
  start)
    root=""
    pid_file=""
    port_file=""
    log_file=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --root)
          root="${2:-}"
          shift 2
          ;;
        --pid-file)
          pid_file="${2:-}"
          shift 2
          ;;
        --port-file)
          port_file="${2:-}"
          shift 2
          ;;
        --log)
          log_file="${2:-}"
          shift 2
          ;;
        *)
          usage
          exit 2
          ;;
      esac
    done
    [[ -z "${root}" || -z "${pid_file}" || -z "${port_file}" || -z "${log_file}" ]] && usage && exit 2
    rm -f "${pid_file}" "${port_file}"
    python3 "${script_dir}/http-fixture.py" serve --root "${root}" --port-file "${port_file}" >"${log_file}" 2>&1 &
    fixture_pid=$!
    printf '%s\n' "${fixture_pid}" >"${pid_file}"
    for _ in $(seq 1 100); do
      if [[ -s "${port_file}" ]]; then
        exit 0
      fi
      sleep 0.1
    done
    cat "${log_file}" >&2 || true
    echo "HTTP fixture did not start" >&2
    exit 1
    ;;
  stop)
    pid_file=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --pid-file)
          pid_file="${2:-}"
          shift 2
          ;;
        *)
          usage
          exit 2
          ;;
      esac
    done
    [[ -z "${pid_file}" ]] && usage && exit 2
    if [[ -f "${pid_file}" ]]; then
      kill "$(cat "${pid_file}")" 2>/dev/null || true
      rm -f "${pid_file}"
    fi
    ;;
  *)
    usage
    exit 2
    ;;
esac
