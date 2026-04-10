#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --flavor <openssl|gnutls> [--build-state <path>] [--test <n>]..." >&2
}

flavor=""
build_state=""
declare -a tests=()
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
    --test)
      tests+=("${2:-}")
      shift 2
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

[[ -z "${flavor}" ]] && usage && exit 2
if ((${#tests[@]} == 0)); then
  tests=(1013 1022 1156 1301 1502 1596)
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cmd=("${script_dir}/run-upstream-tests.sh" --flavor "${flavor}")
if [[ -n "${build_state}" ]]; then
  cmd+=(--build-state "${build_state}")
fi
for test_id in "${tests[@]}"; do
  cmd+=(--test "${test_id}")
done
"${cmd[@]}"
