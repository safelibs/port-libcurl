#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --flavor <openssl|gnutls> [--target <target-id>]... [--jobs <n>]" >&2
}

flavor=""
jobs=""
declare -a targets=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --flavor)
      flavor="${2:-}"
      shift 2
      ;;
    --target)
      targets+=("${2:-}")
      shift 2
      ;;
    --jobs)
      jobs="${2:-}"
      shift 2
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

if [[ -z "${flavor}" ]]; then
  usage
  exit 2
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cmd=(python3 "${script_dir}/compat_harness.py" build --flavor "${flavor}")
if [[ -n "${jobs}" ]]; then
  cmd+=(--jobs "${jobs}")
fi
for target in "${targets[@]}"; do
  cmd+=(--target "${target}")
done
"${cmd[@]}"
