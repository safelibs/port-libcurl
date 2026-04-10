#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --flavor <openssl|gnutls> [--build-state <path>] [--set <name>]... [--entry <id>]... [--target <target-id>]... [--tests <selector>...] [--list-sets]" >&2
}

normalize_target() {
  case "$1" in
    *:*)
      printf '%s\n' "$1"
      ;;
    curl)
      printf 'src:curl\n'
      ;;
    [0-9]*)
      printf 'libtest:lib%s\n' "$1"
      ;;
    lib*|chkhostname|libauthretry|libntlmconnect|libprereq)
      printf 'libtest:%s\n' "$1"
      ;;
    disabled|fake_ntlm|getpart|mqttd|resolve|rtspd|sockfilt|socksd|sws|tftpd)
      printf 'server:%s\n' "$1"
      ;;
    h2-download|h2-pausing|h2-serverpush|h2-upgrade-extreme|tls-session-reuse|ws-data|ws-pingpong)
      printf 'http-client:%s\n' "$1"
      ;;
    ldap-bindata)
      printf 'debian:ldap-bindata\n'
      ;;
    *)
      echo "unsupported target selector: $1" >&2
      return 1
      ;;
  esac
}

flavor=""
build_state=""
list_sets=0
declare -a sets=()
declare -a entries=()
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
    --set)
      sets+=("${2:-}")
      shift 2
      ;;
    --entry)
      entries+=("${2:-}")
      shift 2
      ;;
    --target)
      targets+=("$(normalize_target "${2:-}")")
      shift 2
      ;;
    --tests)
      shift
      added=0
      while [[ $# -gt 0 && "$1" != -* ]]; do
        targets+=("$(normalize_target "$1")")
        shift
        added=1
      done
      (( added )) || { usage; exit 2; }
      ;;
    --list-sets)
      list_sets=1
      shift
      ;;
    *)
      if [[ "$1" == -* ]]; then
        usage
        exit 2
      fi
      targets+=("$(normalize_target "$1")")
      shift
      ;;
  esac
done

[[ -z "${flavor}" ]] && usage && exit 2

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
safe_dir="$(cd "${script_dir}/.." && pwd)"
manifest_path="${safe_dir}/compat/link-manifest.json"
[[ -f "${manifest_path}" ]] || { echo "missing link manifest: ${manifest_path}" >&2; exit 1; }

if (( list_sets )); then
  jq -r '.sets | to_entries[] | "\(.key)\t\(.value.description)"' "${manifest_path}"
  exit 0
fi

if ((${#sets[@]} == 0 && ${#entries[@]} == 0 && ${#targets[@]} == 0)); then
  sets=("curated-broad")
fi

selection_json="$(
  LINK_MANIFEST="${manifest_path}" \
  LINK_FLAVOR="${flavor}" \
  LINK_SETS="$(printf '%s\n' "${sets[@]}")" \
  LINK_ENTRIES="$(printf '%s\n' "${entries[@]}")" \
  LINK_TARGETS="$(printf '%s\n' "${targets[@]}")" \
  python3 <<'PY'
import json
import os
import sys

manifest_path = os.environ["LINK_MANIFEST"]
flavor = os.environ["LINK_FLAVOR"]
set_names = [line for line in os.environ.get("LINK_SETS", "").splitlines() if line]
entry_names = [line for line in os.environ.get("LINK_ENTRIES", "").splitlines() if line]
target_names = [line for line in os.environ.get("LINK_TARGETS", "").splitlines() if line]

manifest = json.load(open(manifest_path, encoding="utf-8"))
entries = {entry["id"]: entry for entry in manifest["entries"]}
selected = []
seen = set()

for set_name in set_names:
    if set_name not in manifest["sets"]:
        print(f"unknown link manifest set: {set_name}", file=sys.stderr)
        sys.exit(1)
    for entry_id in manifest["sets"][set_name]["entries"]:
        if entry_id not in entries:
            print(f"set {set_name} references unknown entry: {entry_id}", file=sys.stderr)
            sys.exit(1)
        if entry_id not in seen:
            selected.append(entries[entry_id])
            seen.add(entry_id)

for entry_id in entry_names:
    if entry_id not in entries:
        print(f"unknown link manifest entry: {entry_id}", file=sys.stderr)
        sys.exit(1)
    if entry_id not in seen:
        selected.append(entries[entry_id])
        seen.add(entry_id)

for target_id in target_names:
    matches = [entry for entry in manifest["entries"] if entry["relink_target_id"] == target_id]
    if not matches:
        print(f"no link manifest entry for target: {target_id}", file=sys.stderr)
        sys.exit(1)
    for entry in matches:
        if entry["id"] not in seen:
            selected.append(entry)
            seen.add(entry["id"])

filtered = []
for entry in selected:
    flavors = entry.get("flavors", [])
    if flavors and flavor not in flavors:
        continue
    filtered.append(entry)

if not filtered:
    print("no runnable link-compat entries selected", file=sys.stderr)
    sys.exit(1)

print(json.dumps(filtered))
PY
)"

default_build_state="${safe_dir}/.compat/${flavor}/build-state.json"
if [[ -z "${build_state}" ]]; then
  build_state="${default_build_state}"
fi

mapfile -t build_targets < <(jq -r '.[].target_id' <<<"${selection_json}" | sort -u)
compat_build_cmd=("${script_dir}/build-compat-consumers.sh" --flavor "${flavor}")
for target_id in "${build_targets[@]}"; do
  compat_build_cmd+=(--target "${target_id}")
done
"${compat_build_cmd[@]}"

[[ -f "${default_build_state}" ]] || { echo "missing build state: ${default_build_state}" >&2; exit 1; }
if [[ "${build_state}" != "${default_build_state}" ]]; then
  mkdir -p "$(dirname "${build_state}")"
  cp "${default_build_state}" "${build_state}"
fi

resolve_entry() {
  local entry_json="$1"
  ENTRY_JSON="${entry_json}" python3 - "${build_state}" <<'PY'
import json
import os
import sys

state_path = sys.argv[1]
state = json.load(open(state_path, encoding="utf-8"))
entry = json.loads(os.environ["ENTRY_JSON"])
records = {record["target_id"]: record for record in state["targets"]}
record = records.get(entry["target_id"])
if record is None:
    print(f"missing build metadata for target {entry['target_id']}", file=sys.stderr)
    sys.exit(1)

runtime = entry.get("runtime") or {}
if not runtime.get("adapter"):
    print(f"missing runtime metadata for entry {entry['id']}", file=sys.stderr)
    sys.exit(1)

build_runtime = record.get("runnable") or {}
if not build_runtime.get("adapter"):
    print(f"target {entry['target_id']} has no runtime metadata in build state", file=sys.stderr)
    sys.exit(1)

source_tokens = [token for token in record["sources"] if token.endswith(".c")]
object_paths = record["object_paths"]
if len(source_tokens) != len(object_paths):
    print(f"object/source mismatch for target {entry['target_id']}", file=sys.stderr)
    sys.exit(1)

object_map = dict(zip(source_tokens, object_paths))
missing = [token for token in entry["object_ids"] if token not in object_map]
if missing:
    print(
        f"target {entry['target_id']} is missing build objects for: {', '.join(missing)}",
        file=sys.stderr,
    )
    sys.exit(1)

selected_objects = [object_map[token] for token in entry["object_ids"]]

print(
    json.dumps(
        {
            "build_runtime": build_runtime,
            "executable_path": record["executable_path"],
            "link_args": record["link_args"],
            "object_paths": selected_objects,
        }
    )
)
PY
}

while IFS= read -r entry_json; do
  [[ -n "${entry_json}" ]] || continue
  runtime_adapter="$(jq -r '.runtime.adapter' <<<"${entry_json}")"
  runtime_test_id="$(jq -r '.runtime.test_id // empty' <<<"${entry_json}")"
  runtime_client_name="$(jq -r '.runtime.client_name // empty' <<<"${entry_json}")"
  relink_target_id="$(jq -r '.relink_target_id' <<<"${entry_json}")"

  resolution_json="$(resolve_entry "${entry_json}")"
  build_runtime_adapter="$(jq -r '.build_runtime.adapter' <<<"${resolution_json}")"
  case "${runtime_adapter}" in
    libtest)
      [[ "${build_runtime_adapter}" == "runtests" ]] || {
        echo "runtime mismatch for ${relink_target_id}: expected runtests build adapter" >&2
        exit 1
      }
      [[ -n "${runtime_test_id}" ]] || {
        echo "missing libtest runtime test id for ${relink_target_id}" >&2
        exit 1
      }
      [[ "$(jq -r '.build_runtime.testcase // empty' <<<"${resolution_json}")" == "${runtime_test_id}" ]] || {
        echo "build-state testcase mismatch for ${relink_target_id}" >&2
        exit 1
      }
      ;;
    curl-tool-smoke)
      [[ "${build_runtime_adapter}" == "curl-tool" ]] || {
        echo "runtime mismatch for ${relink_target_id}: expected curl-tool build adapter" >&2
        exit 1
      }
      ;;
    http-client)
      [[ "${build_runtime_adapter}" == "http-client" ]] || {
        echo "runtime mismatch for ${relink_target_id}: expected http-client build adapter" >&2
        exit 1
      }
      [[ -n "${runtime_client_name}" ]] || {
        echo "missing http-client runtime client name for ${relink_target_id}" >&2
        exit 1
      }
      [[ "$(jq -r '.build_runtime.program // empty' <<<"${resolution_json}")" == "${runtime_client_name}" ]] || {
        echo "build-state client mismatch for ${relink_target_id}" >&2
        exit 1
      }
      ;;
    ldap-devpkg)
      [[ "${build_runtime_adapter}" == "ldap-devpkg" ]] || {
        echo "runtime mismatch for ${relink_target_id}: expected ldap-devpkg build adapter" >&2
        exit 1
      }
      ;;
    *)
      echo "unsupported runtime adapter: ${runtime_adapter}" >&2
      exit 1
      ;;
  esac

  exe_path="$(jq -r '.executable_path' <<<"${resolution_json}")"
  backup_path="${exe_path}.pre-relink"
  if [[ ! -f "${backup_path}" ]]; then
    cp "${exe_path}" "${backup_path}"
  fi
  mapfile -t objects < <(jq -r '.object_paths[]' <<<"${resolution_json}")
  mapfile -t link_args < <(jq -r '.link_args[]' <<<"${resolution_json}")
  cc "${objects[@]}" -o "${exe_path}" "${link_args[@]}"

  case "${runtime_adapter}" in
    libtest)
      "${script_dir}/run-upstream-tests.sh" --flavor "${flavor}" --build-state "${build_state}" --test "${runtime_test_id}"
      ;;
    curl-tool-smoke)
      "${script_dir}/run-curl-tool-smoke.sh" --implementation compat --flavor "${flavor}" --build-state "${build_state}" --binary "${exe_path}"
      ;;
    http-client)
      "${script_dir}/run-http-client-tests.sh" --flavor "${flavor}" --build-state "${build_state}" --program "${runtime_client_name}" --binary "${exe_path}"
      ;;
    ldap-devpkg)
      "${script_dir}/run-ldap-devpkg-test.sh" --flavor "${flavor}" --build-state "${build_state}" --binary "${exe_path}"
      ;;
  esac
done < <(jq -c '.[]' <<<"${selection_json}")
