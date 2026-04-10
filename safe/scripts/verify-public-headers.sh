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

diff -ru --strip-trailing-cr "${expected}" "${actual}"

