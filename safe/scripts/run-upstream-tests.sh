#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --flavor <openssl|gnutls> [--build-state <path>] [--test <n>]... [--tests <n>...] [--require-all-runtests]" >&2
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
    --tests)
      shift
      added=0
      while [[ $# -gt 0 && "$1" != -* ]]; do
        tests+=("$1")
        shift
        added=1
      done
      (( added )) || { usage; exit 2; }
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
current_source_fingerprint="$(
  python3 - "${safe_dir}" <<'PY'
import hashlib
import pathlib
import subprocess
import sys

safe_dir = pathlib.Path(sys.argv[1]).resolve()
repo_root = safe_dir.parent
completed = subprocess.run(
    ["git", "ls-files", "-z", "--", "safe"],
    cwd=repo_root,
    check=True,
    capture_output=True,
)
digest = hashlib.sha256()
for raw in completed.stdout.split(b"\0"):
    if not raw:
        continue
    rel = raw.decode("utf-8")
    path = repo_root / rel
    if not path.is_file():
        continue
    digest.update(rel.encode("utf-8"))
    digest.update(b"\0")
    digest.update(path.read_bytes())
    digest.update(b"\0")
print(digest.hexdigest())
PY
)"
state_source_fingerprint=""
if [[ -f "${build_state}" ]]; then
  state_source_fingerprint="$(jq -r '.source_fingerprint // ""' "${build_state}")"
fi
if [[ ! -f "${build_state}" || "${state_source_fingerprint}" != "${current_source_fingerprint}" ]]; then
  "${script_dir}/build-compat-consumers.sh" --flavor "${flavor}"
fi
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
