#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 (--safe-only|--with-root-harness) --dest <dir>" >&2
}

mode=""
dest=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --safe-only)
      [[ -n "${mode}" ]] && usage && exit 2
      mode="safe-only"
      shift
      ;;
    --with-root-harness)
      [[ -n "${mode}" ]] && usage && exit 2
      mode="with-root-harness"
      shift
      ;;
    --dest)
      dest="${2:-}"
      shift 2
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

if [[ -z "${mode}" || -z "${dest}" ]]; then
  usage
  exit 2
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
python3 "${script_dir}/compat_harness.py" export --mode "${mode}" --dest "${dest}"
