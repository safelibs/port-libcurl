#!/usr/bin/env bash
set -euo pipefail

expected=""
actual=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --expected)
      expected="${2:-}"
      shift 2
      ;;
    --actual)
      actual="${2:-}"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ -z "${expected}" || -z "${actual}" ]]; then
  echo "usage: $0 --expected <dir> --actual <dir>" >&2
  exit 2
fi

if [[ ! -d "${expected}" ]]; then
  echo "expected directory does not exist: ${expected}" >&2
  exit 2
fi

if [[ ! -d "${actual}" ]]; then
  echo "actual directory does not exist: ${actual}" >&2
  exit 2
fi

expected_list="$(mktemp)"
actual_list="$(mktemp)"
trap 'rm -f "${expected_list}" "${actual_list}"' EXIT

(
  cd "${expected}"
  find . -type f -name '*.h' -printf '%P\n' | LC_ALL=C sort
) > "${expected_list}"

(
  cd "${actual}"
  find . -type f -name '*.h' -printf '%P\n' | LC_ALL=C sort
) > "${actual_list}"

diff -u "${expected_list}" "${actual_list}"

while IFS= read -r header; do
  [[ -n "${header}" ]] || continue
  diff -u --strip-trailing-cr "${expected}/${header}" "${actual}/${header}"
done < "${expected_list}"
