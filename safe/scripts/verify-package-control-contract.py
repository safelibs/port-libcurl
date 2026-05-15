#!/usr/bin/env python3
from __future__ import annotations

import argparse
import glob
import re
import subprocess
import sys
from pathlib import Path


PACKAGES = [
    "curl",
    "libcurl4t64",
    "libcurl3t64-gnutls",
    "libcurl4-openssl-dev",
    "libcurl4-gnutls-dev",
    "libcurl4-doc",
]

CONTRACT_FIELDS = [
    "Architecture",
    "Multi-Arch",
    "Depends",
    "Pre-Depends",
    "Recommends",
    "Suggests",
    "Provides",
    "Conflicts",
    "Breaks",
    "Replaces",
]


def parse_control(path: Path) -> list[dict[str, str]]:
    paragraphs: list[dict[str, str]] = []
    current: dict[str, str] = {}
    last: str | None = None
    for raw in path.read_text(encoding="utf-8").splitlines():
        if not raw.strip():
            if current:
                paragraphs.append(current)
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
        paragraphs.append(current)
    return paragraphs


def by_package(paragraphs: list[dict[str, str]]) -> dict[str, dict[str, str]]:
    return {p["Package"]: p for p in paragraphs if "Package" in p}


def normalize(value: str) -> str:
    return re.sub(r"\s+", " ", value.strip())


def source_build_depends(paragraphs: list[dict[str, str]]) -> str:
    for paragraph in paragraphs:
        if paragraph.get("Source") == "curl":
            return paragraph.get("Build-Depends", "")
    raise SystemExit("missing Source: curl stanza")


def dpkg_field(deb: Path, field: str) -> str:
    completed = subprocess.run(
        ["dpkg-deb", "-f", str(deb), field],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        return ""
    return completed.stdout.strip()


def find_debs(package_root: Path) -> dict[str, Path]:
    candidates = [Path(p) for p in glob.glob(str(package_root / "*.deb"))]
    candidates.extend(Path(p) for p in glob.glob(str(package_root.parent / "*.deb")))
    found: dict[str, Path] = {}
    for deb in candidates:
        package = dpkg_field(deb, "Package")
        if package in PACKAGES:
            found[package] = deb
    return found


def literal_dependency_names(value: str) -> list[str]:
    stripped = re.sub(r"\$\{[^}]+\}", "", value)
    names: list[str] = []
    for item in stripped.split(","):
        item = item.strip()
        if not item:
            continue
        first_alt = item.split("|", 1)[0].strip()
        match = re.match(r"([A-Za-z0-9+.-]+)", first_alt)
        if match:
            names.append(match.group(1))
    return names


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--expected-control", type=Path, required=True)
    parser.add_argument("--actual-control", type=Path, required=True)
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--require-source-build-deps", nargs="*", default=[])
    args = parser.parse_args()

    expected_paragraphs = parse_control(args.expected_control)
    actual_paragraphs = parse_control(args.actual_control)
    expected = by_package(expected_paragraphs)
    actual = by_package(actual_paragraphs)

    if sorted(actual) != sorted(PACKAGES):
        raise SystemExit(f"unexpected binary package set: {sorted(actual)}")

    for package in PACKAGES:
        if package not in expected:
            raise SystemExit(f"expected control is missing {package}")
        for field in CONTRACT_FIELDS:
            expected_value = normalize(expected[package].get(field, ""))
            actual_value = normalize(actual[package].get(field, ""))
            if expected_value != actual_value:
                print(f"{package}: field drift in {field}", file=sys.stderr)
                print(f"expected: {expected_value!r}", file=sys.stderr)
                print(f"actual:   {actual_value!r}", file=sys.stderr)
                return 1

    build_depends = source_build_depends(actual_paragraphs)
    normalized_build_depends = normalize(build_depends)
    for required in args.require_source_build_deps:
        if required not in normalized_build_depends:
            raise SystemExit(f"missing required Build-Depends entry: {required}")

    debs = find_debs(args.package_root.resolve())
    missing = sorted(set(PACKAGES) - set(debs))
    if missing:
        raise SystemExit(f"missing built debs for: {', '.join(missing)}")

    for package, deb in sorted(debs.items()):
        built_package = dpkg_field(deb, "Package")
        if built_package != package:
            raise SystemExit(f"{deb}: expected Package {package}, got {built_package}")
        for field in CONTRACT_FIELDS:
            intended = actual[package].get(field, "")
            built = dpkg_field(deb, field)
            if not intended and built:
                raise SystemExit(f"{package}: unexpected built field {field}: {built}")
            if field == "Architecture":
                if normalize(intended) == "any":
                    if not built:
                        raise SystemExit(f"{package}: built Architecture is empty")
                elif normalize(intended) != normalize(built):
                    raise SystemExit(
                        f"{package}: built {field} drift: expected {intended!r}, got {built!r}"
                    )
                continue
            if field == "Multi-Arch" and normalize(intended) != normalize(built):
                raise SystemExit(
                    f"{package}: built {field} drift: expected {intended!r}, got {built!r}"
                )
            if field != "Architecture":
                for name in literal_dependency_names(intended):
                    if name not in built:
                        raise SystemExit(
                            f"{package}: built {field} is missing literal dependency {name!r}: {built!r}"
                        )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
