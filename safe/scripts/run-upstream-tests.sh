#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --flavor <openssl|gnutls> [--build-state <path>] [--test <n>]... [--require-all-runtests]" >&2
}

flavor=""
build_state=""
require_all=0
declare -a tests=()
declare -a passthrough=()
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
    --require-all-runtests)
      require_all=1
      shift
      ;;
    *)
      passthrough+=("$1")
      shift
      ;;
  esac
done

[[ -z "${flavor}" ]] && usage && exit 2
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
safe_dir="$(cd "${script_dir}/.." && pwd)"
detached_root="$(mktemp -d)"
cleanup() {
  rm -rf "${detached_root}"
}
trap cleanup EXIT
"${script_dir}/export-tracked-tree.sh" --safe-only --dest "${detached_root}"

if [[ -z "${build_state}" ]]; then
  build_state="${safe_dir}/.compat/${flavor}/build-state.json"
fi
[[ -f "${build_state}" ]] || "${script_dir}/build-compat-consumers.sh" --flavor "${flavor}"
[[ -f "${build_state}" ]] || { echo "missing build state: ${build_state}" >&2; exit 1; }

has_required_runtime_targets="$(
  jq -r '
    [
      any(.targets[]; .target_id == "src:curl"),
      any(.targets[]; .target_id == "server:disabled" and (.executable_path // "") != ""),
      any(.targets[]; .target_id == "server:sws" and (.executable_path // "") != ""),
      any(.targets[]; .target_id == "server:sockfilt" and (.executable_path // "") != "")
    ] | all
  ' "${build_state}"
)"
if [[ "${has_required_runtime_targets}" != "true" ]]; then
  "${script_dir}/build-compat-consumers.sh" --flavor "${flavor}"
fi
[[ -f "${build_state}" ]] || { echo "missing build state after rebuild: ${build_state}" >&2; exit 1; }

if (( require_all )); then
  for arg in "${passthrough[@]}"; do
    if [[ "${arg}" == "-f" || "${arg}" == "-k" ]]; then
      echo "--require-all-runtests mode rejects ${arg}" >&2
      exit 2
    fi
  done
fi

worktree="$(jq -r '.worktree' "${build_state}")"
lib_dir="$(jq -r '.stage.lib_dir' "${build_state}")"
curl_bin="$(jq -r '.targets[] | select(.target_id=="src:curl") | .executable_path' "${build_state}")"
[[ -x "${curl_bin}" ]] || { echo "missing curl binary: ${curl_bin}" >&2; exit 1; }

if (( require_all )); then
  mapfile -t tests < <(
    python3 - "${safe_dir}/metadata/test-manifest.json" "${flavor}" <<'PY'
import json
import pathlib
import sys

manifest = json.loads(pathlib.Path(sys.argv[1]).read_text())
flavor = sys.argv[2]
disabled = []
enabled = []
for token in manifest["raw_ordered_testcases"]:
    test_id = token.removeprefix("test")
    disabled_entry = manifest["disabled"]["per_test"].get(test_id)
    if disabled_entry and disabled_entry["selected_flavor_skip"][flavor]:
        disabled.append(test_id)
    else:
        enabled.append(test_id)
path = pathlib.Path(sys.argv[1]).resolve().parent.parent / ".compat" / flavor / "disabled-runtests.json"
path.parent.mkdir(parents=True, exist_ok=True)
path.write_text(json.dumps({"flavor": flavor, "disabled_tokens": disabled}, indent=2, sort_keys=True) + "\n")
for test_id in enabled:
    print(test_id)
PY
  )
fi

declare -a runtests_cmd=(perl "${worktree}/tests/runtests.pl" -a -c "${curl_bin}" -vc "${curl_bin}")
runtests_cmd+=("${passthrough[@]}")
if ((${#tests[@]} > 0)); then
  runtests_cmd+=("${tests[@]}")
fi

(
  cd "${worktree}/tests"
  export srcdir="${worktree}/tests"
  export LD_LIBRARY_PATH="${lib_dir}:${LD_LIBRARY_PATH:-}"
  "${runtests_cmd[@]}"
)
