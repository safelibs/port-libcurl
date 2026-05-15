#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 --expected-control <path> --actual-control <path>" >&2
}

expected=""
actual=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --expected-control)
      expected="${2:-}"
      shift 2
      ;;
    --actual-control)
      actual="${2:-}"
      shift 2
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

[[ -n "${expected}" && -n "${actual}" ]] || { usage; exit 2; }

python3 - "$expected" "$actual" <<'PY'
from pathlib import Path
import sys

FIELDS = ("Tests", "Depends", "Restrictions")


def paragraphs(path: Path) -> list[dict[str, str]]:
    result: list[dict[str, str]] = []
    current: dict[str, str] = {}
    last: str | None = None
    for raw in path.read_text(encoding="utf-8").splitlines():
        if not raw.strip():
            if current:
                result.append(current)
                current = {}
                last = None
            continue
        if raw[0].isspace() and last:
            current[last] += "\n" + raw
            continue
        key, value = raw.split(":", 1)
        last = key
        current[key] = value.strip()
    if current:
        result.append(current)
    return result


def contract(path: Path) -> dict[str, dict[str, str]]:
    parsed = {}
    for paragraph in paragraphs(path):
        name = paragraph.get("Tests")
        if name:
            parsed[name] = {field: paragraph.get(field, "") for field in FIELDS}
    return parsed


expected = contract(Path(sys.argv[1]))
actual = contract(Path(sys.argv[2]))
required_order = ["upstream-tests-openssl", "upstream-tests-gnutls", "curl-ldapi-test"]
if list(actual) != required_order:
    raise SystemExit(f"unexpected safe autopkgtest order/names: {list(actual)}")
if expected != actual:
    for name in required_order:
        if expected.get(name) != actual.get(name):
            print(f"autopkgtest contract drift for {name}", file=sys.stderr)
            print(f"expected: {expected.get(name)}", file=sys.stderr)
            print(f"actual:   {actual.get(name)}", file=sys.stderr)
    raise SystemExit(1)
PY
