#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --flavor <openssl|gnutls> [--build-state <path>] [--target <target-id>]..." >&2
}

flavor=""
build_state=""
declare -a targets=()
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
    --target)
      targets+=("${2:-}")
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

if ((${#targets[@]} == 0)); then
  targets=(src:curl libtest:lib1301)
fi

for target_id in "${targets[@]}"; do
  record="$(jq -c --arg id "${target_id}" '.targets[] | select(.target_id==$id)' "${build_state}")"
  [[ -n "${record}" ]] || { echo "unknown target in build state: ${target_id}" >&2; exit 1; }
  runnable="$(jq -r '.runnable.adapter // empty' <<<"${record}")"
  [[ -n "${runnable}" ]] || { echo "target ${target_id} has no declared runnable path" >&2; exit 1; }
  exe_path="$(jq -r '.executable_path' <<<"${record}")"
  backup_path="${exe_path}.pre-relink"
  if [[ ! -f "${backup_path}" ]]; then
    cp "${exe_path}" "${backup_path}"
  fi
  mapfile -t objects < <(jq -r '.object_paths[]' <<<"${record}")
  mapfile -t link_args < <(jq -r '.link_args[]' <<<"${record}")
  cc "${objects[@]}" -o "${exe_path}" "${link_args[@]}"
  case "${runnable}" in
    runtests)
      testcase="$(jq -r '.runnable.testcase' <<<"${record}")"
      "${script_dir}/run-upstream-tests.sh" --flavor "${flavor}" --build-state "${build_state}" --test "${testcase}"
      ;;
    curl-tool)
      "${script_dir}/run-curl-tool-smoke.sh" --implementation compat --flavor "${flavor}" --build-state "${build_state}" --binary "${exe_path}"
      ;;
    http-client)
      program="$(jq -r '.runnable.program' <<<"${record}")"
      "${script_dir}/run-http-client-tests.sh" --flavor "${flavor}" --build-state "${build_state}" --program "${program}" --binary "${exe_path}"
      ;;
    ldap-devpkg)
      "${script_dir}/run-ldap-devpkg-test.sh" --flavor "${flavor}" --build-state "${build_state}" --binary "${exe_path}"
      ;;
    *)
      echo "unsupported runnable adapter: ${runnable}" >&2
      exit 1
      ;;
  esac
done
