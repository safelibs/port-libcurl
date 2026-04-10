#!/usr/bin/env python3
from __future__ import annotations

import argparse
import importlib.util
import json
import pathlib
import tempfile


SCRIPT_DIR = pathlib.Path(__file__).resolve().parent
SAFE_DIR = SCRIPT_DIR.parent


def load_generate_module():
    path = SCRIPT_DIR / "generate-manifests.py"
    spec = importlib.util.spec_from_file_location("generate_manifests", path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def load_json(path: pathlib.Path) -> dict:
    return json.loads(path.read_text())


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--abi", required=True)
    parser.add_argument("--tests", required=True)
    parser.add_argument("--cves", required=True)
    args = parser.parse_args()

    generate_module = load_generate_module()
    abi_path = pathlib.Path(args.abi).resolve()
    tests_path = pathlib.Path(args.tests).resolve()
    cves_path = pathlib.Path(args.cves).resolve()

    with tempfile.TemporaryDirectory() as tmp:
        tmp_root = pathlib.Path(tmp)
        generate_module.write_outputs(SAFE_DIR, tmp_root)

        expected_abi = load_json(tmp_root / "metadata/abi-manifest.json")
        expected_tests = load_json(tmp_root / "metadata/test-manifest.json")
        expected_cves = load_json(tmp_root / "metadata/cve-manifest.json")

        actual_abi = load_json(abi_path)
        actual_tests = load_json(tests_path)
        actual_cves = load_json(cves_path)

        if actual_abi != expected_abi:
            raise SystemExit("abi manifest is out of date")
        if actual_tests != expected_tests:
            raise SystemExit("test manifest is out of date")
        if actual_cves != expected_cves:
            raise SystemExit("cve manifest is out of date")

        if actual_tests["raw_ordered_testcases"].count("test1190") != 2:
            raise SystemExit("test manifest lost the duplicate test1190 token")
        if len(actual_tests["libtests"]["programs"]) != 256:
            raise SystemExit("unexpected libtest program count")
        if len(actual_tests["units"]["source_ids"]) != 46:
            raise SystemExit("unexpected unit source count")
        if len(actual_tests["http_clients"]["programs"]) != 7:
            raise SystemExit("unexpected HTTP client count")
        if len(actual_tests["server_helpers"]["programs"]) != 10:
            raise SystemExit("unexpected server helper count")


if __name__ == "__main__":
    main()
