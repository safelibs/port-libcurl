#!/usr/bin/env python3
from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
SAFE_DIR = SCRIPT_DIR.parent
REPO_ROOT = SAFE_DIR.parent
MANIFEST_PATH = SAFE_DIR / "metadata" / "test-manifest.json"
VENDOR_ROOT = SAFE_DIR / "vendor" / "upstream"
COMPAT_ROOT = SAFE_DIR / ".compat"
INCLUDE_RE = re.compile(r'^\s*#\s*include\s+"([^"]+)"', re.M)


class HarnessError(RuntimeError):
    pass


@dataclass(frozen=True)
class FlavorConfig:
    name: str
    cargo_feature: str
    soname: str
    cargo_target_dir: Path
    stage_dir: Path
    worktree_dir: Path
    objects_dir: Path
    executables_dir: Path
    build_state_path: Path
    reference_root: Path


def flavor_config(flavor: str) -> FlavorConfig:
    if flavor == "openssl":
        feature = "openssl-flavor"
        soname = "libcurl.so.4"
    elif flavor == "gnutls":
        feature = "gnutls-flavor"
        soname = "libcurl-gnutls.so.4"
    else:
        raise HarnessError(f"unsupported flavor: {flavor}")

    root = COMPAT_ROOT / flavor
    return FlavorConfig(
        name=flavor,
        cargo_feature=feature,
        soname=soname,
        cargo_target_dir=SAFE_DIR / "target" / "compat-consumers" / flavor,
        stage_dir=root / "stage",
        worktree_dir=root / "worktree",
        objects_dir=root / "objects",
        executables_dir=root / "executables",
        build_state_path=root / "build-state.json",
        reference_root=SAFE_DIR / ".reference" / flavor / "source" / "original",
    )


def run(
    *args: str,
    cwd: Path | None = None,
    capture: bool = False,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        list(args),
        cwd=str(cwd) if cwd else None,
        env=env,
        text=True,
        check=False,
        capture_output=capture,
    )
    if completed.returncode != 0:
        if capture:
            raise HarnessError(
                f"command failed ({completed.returncode}): {' '.join(args)}\n"
                f"stdout:\n{completed.stdout}\n"
                f"stderr:\n{completed.stderr}"
            )
        raise HarnessError(f"command failed ({completed.returncode}): {' '.join(args)}")
    return completed


def load_manifest() -> dict:
    return json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))


def sha256_path(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_tracked(paths: list[Path]) -> set[Path]:
    if not paths:
        return set()
    rels = [str(path.relative_to(REPO_ROOT)) for path in paths]
    completed = run("git", "ls-files", "-z", "--", *rels, cwd=REPO_ROOT, capture=True)
    tracked = {
        (REPO_ROOT / rel.decode("utf-8")).resolve()
        for rel in completed.stdout.encode("utf-8").split(b"\x00")
        if rel
    }
    return tracked


def git_ls_files(prefixes: list[str]) -> list[Path]:
    completed = run("git", "ls-files", "-z", "--", *prefixes, cwd=REPO_ROOT, capture=True)
    return [
        (REPO_ROOT / rel.decode("utf-8")).resolve()
        for rel in completed.stdout.encode("utf-8").split(b"\x00")
        if rel
    ]


def component_root(component: str, worktree: Path | None = None) -> Path:
    base = worktree if worktree is not None else REPO_ROOT / "original"
    mapping = {
        "src": base / "src",
        "libtest": base / "tests" / "libtest",
        "server": base / "tests" / "server",
        "http-client": base / "tests" / "http" / "clients",
        "debian": base / "debian" / "tests",
    }
    try:
        return mapping[component]
    except KeyError as exc:
        raise HarnessError(f"unsupported component: {component}") from exc


def tokenize(raw: str) -> list[str]:
    if not raw:
        return []
    return shlex.split(raw, posix=True)


def canonicalize_token(root: Path, token: str) -> Path | None:
    if not token.endswith((".c", ".h", ".pl", ".pm", ".md", ".in", ".ini", ".rc")):
        return None
    candidate = (root / token).resolve()
    try:
        candidate.relative_to(REPO_ROOT.resolve())
    except ValueError:
        return None
    return candidate


def all_build_targets(manifest: dict) -> list[dict]:
    targets = []
    for target in manifest["compatibility_build"]["targets"]:
        copied = dict(target)
        if copied["target_id"] == "libtest:lib1521" and not copied["sources"]:
            copied["sources"] = [
                "lib1521.c",
                "../../lib/timediff.c",
                "../../lib/timediff.h",
                "first.c",
                "test.h",
            ]
        targets.append(copied)
    targets.append(
        {
            "target_id": "debian:ldap-bindata",
            "component": "debian",
            "name": "ldap-bindata",
            "role": "libcurl-consumer",
            "common": {
                "AM_CPPFLAGS": {
                    "raw": "-I$(top_srcdir)/include",
                    "resolved": "-I$(top_srcdir)/include",
                }
            },
            "fields": {
                "sources": {
                    "raw": "LDAP-bindata.c",
                    "resolved": "LDAP-bindata.c",
                },
                "cflags": {
                    "raw": "",
                    "resolved": "",
                },
                "ldadd": {
                    "raw": "pkg-config:ldap",
                    "resolved": "pkg-config:ldap",
                },
            },
            "sources": ["LDAP-bindata.c"],
        }
    )
    return targets


def vendor_required_lib_paths(manifest: dict) -> set[Path]:
    initial: set[Path] = set()
    component_roots = {
        "src": REPO_ROOT / "original" / "src",
        "libtest": REPO_ROOT / "original" / "tests" / "libtest",
        "server": REPO_ROOT / "original" / "tests" / "server",
        "http-client": REPO_ROOT / "original" / "tests" / "http" / "clients",
        "debian": REPO_ROOT / "original" / "debian" / "tests",
    }
    for key in ("curlx_cfiles",):
        for token in manifest["curl_tool_sources"].get(key, []):
            candidate = canonicalize_token(component_roots["src"], token)
            if candidate:
                initial.add(candidate)
    for token in manifest["curl_tool_sources"].get("curl_cfiles", []):
        candidate = canonicalize_token(component_roots["src"], token)
        if candidate and candidate.parts[-2] == "lib":
            initial.add(candidate)

    for target in all_build_targets(manifest):
        root = component_roots[target["component"]]
        for token in target["sources"]:
            candidate = canonicalize_token(root, token)
            if candidate and "original/lib/" in str(candidate):
                initial.add(candidate)

    tracked_under_lib = {
        path.resolve()
        for path in REPO_ROOT.joinpath("original", "lib").rglob("*")
        if path.is_file()
    }
    tracked_lib = git_tracked(sorted(tracked_under_lib))
    resolved = set(path for path in initial if path in tracked_lib)
    queue = list(resolved)
    while queue:
        current = queue.pop()
        text = current.read_text(encoding="utf-8", errors="ignore")
        for include_name in INCLUDE_RE.findall(text):
            include_path = (current.parent / include_name).resolve()
            if include_path in tracked_lib and include_path not in resolved:
                resolved.add(include_path)
                queue.append(include_path)
    return resolved


def compute_vendor_entries(manifest: dict) -> list[dict]:
    tracked_paths = set(
        git_ls_files(
            [
                "original/src",
                "original/tests",
                "original/.pc/90_gnutls.patch",
                "original/debian/tests/LDAP-bindata.c",
            ]
        )
    )
    tracked_paths.update(vendor_required_lib_paths(manifest))

    entries = []
    for path in sorted(tracked_paths):
        if not path.exists():
            raise HarnessError(f"required vendored file is missing: {path}")
        tracked = path in git_tracked([path])
        if not tracked:
            raise HarnessError(f"required vendored file is not git-tracked: {path}")
        relative = path.relative_to(REPO_ROOT)
        if relative.parts[0] != "original":
            raise HarnessError(f"vendored source must stay under original/: {relative}")
        destination = Path("safe/vendor/upstream") / Path(*relative.parts[1:])
        entries.append(
            {
                "source": relative.as_posix(),
                "destination": destination.as_posix(),
                "tracked": True,
                "sha256": sha256_path(path),
            }
        )
    return entries


def vendor_assets(manifest: dict) -> None:
    entries = compute_vendor_entries(manifest)
    if VENDOR_ROOT.exists():
        shutil.rmtree(VENDOR_ROOT)
    VENDOR_ROOT.mkdir(parents=True, exist_ok=True)

    for entry in entries:
        src = REPO_ROOT / entry["source"]
        dst = REPO_ROOT / entry["destination"]
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dst)

    manifest_path = VENDOR_ROOT / "manifest.json"
    manifest_path.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "root": "safe/vendor/upstream",
                "entries": entries,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )


def export_tracked_tree(mode: str, dest: Path) -> None:
    if dest.exists():
        if any(dest.iterdir()):
            raise HarnessError(f"destination must be empty: {dest}")
    else:
        dest.mkdir(parents=True)

    if mode == "safe-only":
        tracked = git_ls_files(["safe"])
        destinations = [(path, dest / path.relative_to(REPO_ROOT / "safe")) for path in tracked]
    elif mode == "with-root-harness":
        tracked = git_ls_files(["safe", "dependents.json"])
        destinations = []
        for path in tracked:
            relative = path.relative_to(REPO_ROOT)
            if relative.as_posix() == "dependents.json":
                destinations.append((path, dest / "dependents.json"))
            elif relative.parts[0] == "safe":
                destinations.append((path, dest / "safe" / Path(*relative.parts[1:])))
            else:
                raise HarnessError(f"unexpected tracked export path: {relative}")
    else:
        raise HarnessError(f"unsupported export mode: {mode}")

    for src, dst in destinations:
        if not src.exists():
            raise HarnessError(f"required tracked export input is missing: {src}")
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dst)


def ensure_reference(flavor: FlavorConfig) -> None:
    run("bash", str(SCRIPT_DIR / "build-reference-curl.sh"), "--flavor", flavor.name, cwd=REPO_ROOT)


def stage_safe_library(flavor: FlavorConfig) -> dict:
    ensure_reference(flavor)
    flavor.stage_dir.mkdir(parents=True, exist_ok=True)
    (flavor.stage_dir / "include").mkdir(parents=True, exist_ok=True)
    (flavor.stage_dir / "lib").mkdir(parents=True, exist_ok=True)

    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(flavor.cargo_target_dir)
    run(
        "cargo",
        "build",
        "--manifest-path",
        str(SAFE_DIR / "Cargo.toml"),
        "--no-default-features",
        "--features",
        flavor.cargo_feature,
        cwd=REPO_ROOT,
        env=env,
    )

    built_safe = flavor.cargo_target_dir / "debug" / "libport_libcurl_safe.so"
    reference_lib = SAFE_DIR / ".reference" / flavor.name / "dist" / f"libcurl-reference-{flavor.name}.so.4"
    if not built_safe.exists():
        raise HarnessError(f"missing built safe library: {built_safe}")
    if not reference_lib.exists():
        raise HarnessError(f"missing reference sidecar library: {reference_lib}")

    include_root = flavor.stage_dir / "include"
    lib_root = flavor.stage_dir / "lib"
    if include_root.exists():
        shutil.rmtree(include_root)
    include_root.mkdir(parents=True, exist_ok=True)
    shutil.copytree(SAFE_DIR / "include", include_root, dirs_exist_ok=True)

    stage_soname = lib_root / flavor.soname
    shutil.copy2(built_safe, stage_soname)
    shutil.copy2(reference_lib, lib_root / reference_lib.name)
    libcurl_link = lib_root / "libcurl.so"
    if libcurl_link.exists() or libcurl_link.is_symlink():
        libcurl_link.unlink()
    libcurl_link.symlink_to(flavor.soname)
    if flavor.name == "gnutls":
        gnutls_link = lib_root / "libcurl-gnutls.so"
        if gnutls_link.exists() or gnutls_link.is_symlink():
            gnutls_link.unlink()
        gnutls_link.symlink_to(flavor.soname)
    return {
        "include_dir": include_root,
        "lib_dir": lib_root,
        "library_path": stage_soname,
        "reference_library_path": lib_root / reference_lib.name,
    }


def sync_worktree(flavor: FlavorConfig) -> None:
    if not VENDOR_ROOT.exists():
        raise HarnessError("vendored tree is missing, run vendor-compat-assets first")
    if flavor.worktree_dir.exists():
        shutil.rmtree(flavor.worktree_dir)
    shutil.copytree(VENDOR_ROOT, flavor.worktree_dir)

    if flavor.name == "openssl":
        patch_root = flavor.worktree_dir / ".pc" / "90_gnutls.patch"
        for backup in sorted(patch_root.rglob("*")):
            if backup.is_file():
                relative = backup.relative_to(patch_root)
                target = flavor.worktree_dir / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(backup, target)

    include_root = flavor.worktree_dir / "include"
    if include_root.exists():
        shutil.rmtree(include_root)
    shutil.copytree(SAFE_DIR / "include", include_root)

    reference_config = flavor.reference_root / "lib" / "curl_config.h"
    reference_tests_config = flavor.reference_root / "tests" / "config"
    reference_curl_config = flavor.reference_root / "curl-config"
    if not reference_config.exists():
        raise HarnessError(f"missing reference curl_config.h: {reference_config}")
    if not reference_tests_config.exists():
        raise HarnessError(f"missing reference tests/config: {reference_tests_config}")
    if not reference_curl_config.exists():
        raise HarnessError(f"missing reference curl-config: {reference_curl_config}")

    shutil.copy2(reference_config, flavor.worktree_dir / "lib" / "curl_config.h")
    shutil.copy2(reference_tests_config, flavor.worktree_dir / "tests" / "config")
    shutil.copy2(reference_curl_config, flavor.worktree_dir / "curl-config")
    os.chmod(flavor.worktree_dir / "curl-config", 0o755)

    generated_lib1521 = flavor.worktree_dir / "tests" / "libtest" / "lib1521.c"
    curl_header = flavor.worktree_dir / "include" / "curl" / "curl.h"
    completed = subprocess.run(
        ["perl", str(flavor.worktree_dir / "tests" / "libtest" / "mk-lib1521.pl")],
        cwd=str(generated_lib1521.parent),
        text=True,
        input=curl_header.read_text(encoding="utf-8"),
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        raise HarnessError(
            "failed to generate lib1521.c\n"
            f"stdout:\n{completed.stdout}\n"
            f"stderr:\n{completed.stderr}"
        )
    lib1521_text = completed.stdout
    generated_lib1521.write_text(lib1521_text, encoding="utf-8")

    for dotlibs in (
        flavor.worktree_dir / "tests" / "libtest" / ".libs",
        flavor.worktree_dir / "tests" / "server" / ".libs",
        flavor.worktree_dir / "src" / ".libs",
        flavor.worktree_dir / "tests" / "http" / "clients" / ".libs",
    ):
        dotlibs.mkdir(parents=True, exist_ok=True)


def manifest_common_value(target: dict, key: str) -> str:
    entry = target.get("common", {}).get(key)
    return entry.get("resolved", "") if entry else ""


def target_field_value(target: dict, key: str) -> str:
    entry = target.get("fields", {}).get(key)
    return entry.get("resolved", "") if entry else ""


def substitute_make_vars(expr: str, *, top_srcdir: Path, top_builddir: Path, srcdir: Path, target: dict) -> str:
    values = {
        "top_srcdir": top_srcdir.as_posix(),
        "top_builddir": top_builddir.as_posix(),
        "srcdir": srcdir.as_posix(),
        "AM_CPPFLAGS": manifest_common_value(target, "AM_CPPFLAGS"),
        "AM_CFLAGS": manifest_common_value(target, "AM_CFLAGS"),
        "AM_LDFLAGS": manifest_common_value(target, "AM_LDFLAGS"),
        "BLANK_AT_MAKETIME": "",
        "CURL_LDFLAGS_BIN": "",
        "LIBDIR": top_builddir.joinpath("lib").as_posix(),
        "SUPPORTFILES_LIBS": manifest_common_value(target, "SUPPORTFILES_LIBS"),
        "TESTUTIL_LIBS": manifest_common_value(target, "TESTUTIL_LIBS"),
        "LDADD": manifest_common_value(target, "LDADD"),
        "LIBS": manifest_common_value(target, "LIBS"),
    }

    current = expr
    for _ in range(8):
        updated = current
        for name, value in values.items():
            updated = updated.replace(f"$({name})", value)
        if updated == current:
            break
        current = updated
    return current


def pkg_config_tokens(*packages: str, mode: str) -> list[str]:
    if not packages:
        return []
    flag = "--cflags" if mode == "cflags" else "--libs"
    completed = run("pkg-config", flag, *packages, capture=True, cwd=REPO_ROOT)
    return tokenize(completed.stdout.strip())


def pkg_config_exists(*packages: str) -> bool:
    completed = subprocess.run(
        ["pkg-config", "--exists", *packages],
        cwd=str(REPO_ROOT),
        check=False,
    )
    return completed.returncode == 0


def link_placeholder_tokens(flavor: FlavorConfig, placeholder: str, mode: str) -> list[str]:
    if placeholder in {"@CURL_NETWORK_LIBS@", "@CURL_NETWORK_AND_TIME_LIBS@", "@ZLIB_LIBS@", "@SSL_LIBS@"}:
        return []
    if placeholder == "@LDAP_LIBS@" and mode == "libs":
        return pkg_config_tokens("ldap", mode="libs")
    if placeholder == "@LDAP_CFLAGS@" and mode == "cflags":
        return pkg_config_tokens("ldap", mode="cflags")
    raise HarnessError(f"unsupported placeholder token: {placeholder}")


def expand_flags(
    flavor: FlavorConfig,
    target: dict,
    expr: str,
    *,
    mode: str,
    inject_safe_lib: bool,
) -> list[str]:
    srcdir = component_root(target["component"], flavor.worktree_dir)
    substituted = substitute_make_vars(
        expr,
        top_srcdir=flavor.worktree_dir,
        top_builddir=flavor.worktree_dir,
        srcdir=srcdir,
        target=target,
    )
    tokens = tokenize(substituted)
    expanded: list[str] = []
    for token in tokens:
        if token in {"@CURL_NETWORK_LIBS@", "@CURL_NETWORK_AND_TIME_LIBS@", "@SSL_LIBS@", "@ZLIB_LIBS@"}:
            expanded.extend(link_placeholder_tokens(flavor, token, mode))
            continue
        if token == "pkg-config:ldap":
            expanded.extend(pkg_config_tokens("ldap", mode="libs"))
            continue
        if token.endswith("libcurl.la") or token.endswith("libcurl-gnutls.la"):
            if inject_safe_lib:
                expanded.extend(
                    [
                        f"-L{(flavor.stage_dir / 'lib').as_posix()}",
                        "-Wl,-rpath",
                        f"-Wl,{(flavor.stage_dir / 'lib').as_posix()}",
                        "-lcurl",
                    ]
                )
            continue
        expanded.append(token)
    return expanded


def target_preprocessor_flags(flavor: FlavorConfig, target: dict) -> list[str]:
    expr = target_field_value(target, "cppflags") or manifest_common_value(target, "AM_CPPFLAGS")
    tokens = expand_flags(flavor, target, expr, mode="cflags", inject_safe_lib=False)
    if target["target_id"] == "debian:ldap-bindata":
        tokens.extend(pkg_config_tokens("ldap", mode="cflags"))
    return tokens


def target_cflags(flavor: FlavorConfig, target: dict) -> list[str]:
    expr = target_field_value(target, "cflags") or manifest_common_value(target, "AM_CFLAGS")
    return expand_flags(flavor, target, expr, mode="cflags", inject_safe_lib=False)


def target_ldflags(flavor: FlavorConfig, target: dict) -> list[str]:
    expr = target_field_value(target, "ldflags") or manifest_common_value(target, "AM_LDFLAGS")
    return expand_flags(flavor, target, expr, mode="libs", inject_safe_lib=False)


def target_link_args(flavor: FlavorConfig, target: dict) -> list[str]:
    expr = target_field_value(target, "ldadd") or manifest_common_value(target, "LDADD")
    inject_safe_lib = target["role"] in {"libcurl-consumer", "curl-tool"}
    return expand_flags(flavor, target, expr, mode="libs", inject_safe_lib=inject_safe_lib)


def source_path(flavor: FlavorConfig, target: dict, token: str) -> Path:
    root = component_root(target["component"], flavor.worktree_dir)
    path = (root / token).resolve()
    if not path.exists():
        raise HarnessError(f"missing source for {target['target_id']}: {path}")
    return path


def executable_path(flavor: FlavorConfig, target: dict) -> Path:
    if target["component"] == "src":
        return flavor.worktree_dir / "src" / target["name"]
    if target["component"] == "libtest":
        return flavor.worktree_dir / "tests" / "libtest" / target["name"]
    if target["component"] == "server":
        return flavor.worktree_dir / "tests" / "server" / target["name"]
    if target["component"] == "http-client":
        return flavor.worktree_dir / "tests" / "http" / "clients" / target["name"]
    if target["component"] == "debian":
        return flavor.worktree_dir / "debian" / "tests" / target["name"]
    raise HarnessError(f"unsupported component for executable path: {target['component']}")


def runnable_metadata(target: dict) -> dict | None:
    if target["component"] == "libtest" and target["role"] == "libcurl-consumer":
        match = re.fullmatch(r"lib(\d+)", target["name"])
        if match:
            return {"adapter": "runtests", "testcase": match.group(1)}
    if target["component"] == "src":
        return {"adapter": "curl-tool", "name": target["name"]}
    if target["component"] == "http-client":
        return {"adapter": "http-client", "program": target["name"]}
    if target["target_id"] == "debian:ldap-bindata":
        return {"adapter": "ldap-devpkg", "name": target["name"]}
    return None


def build_targets(flavor: FlavorConfig, targets: list[dict], jobs: int) -> dict:
    stage_info = stage_safe_library(flavor)
    sync_worktree(flavor)
    flavor.objects_dir.mkdir(parents=True, exist_ok=True)
    flavor.executables_dir.mkdir(parents=True, exist_ok=True)

    object_cache: dict[tuple[str, tuple[str, ...]], Path] = {}
    compile_tasks: dict[tuple[str, tuple[str, ...]], tuple[Path, list[str], Path]] = {}
    target_records: list[dict] = []

    for target in targets:
        if target["target_id"] == "debian:ldap-bindata" and not pkg_config_exists("ldap"):
            target_records.append(
                {
                    "target_id": target["target_id"],
                    "component": target["component"],
                    "name": target["name"],
                    "role": target["role"],
                    "source_dir": component_root(target["component"], flavor.worktree_dir).as_posix(),
                    "generated_outputs": [],
                    "sources": list(target["sources"]),
                    "compile_args": [],
                    "link_args": [],
                    "object_paths": [],
                    "object_path": None,
                    "executable_path": None,
                    "runnable": None,
                    "skipped_reason": "pkg-config package ldap is unavailable",
                }
            )
            continue

        srcdir = component_root(target["component"], flavor.worktree_dir)
        cppflags = target_preprocessor_flags(flavor, target)
        cflags = target_cflags(flavor, target)
        compile_args = [
            os.environ.get("CC", "cc"),
            "-std=gnu11",
            "-DHAVE_CONFIG_H",
            "-Wall",
            "-Wextra",
            "-Wno-deprecated-declarations",
            "-Wno-unused-parameter",
            "-Wno-sign-compare",
            "-Wno-pointer-sign",
            *cppflags,
            *cflags,
        ]

        generated_outputs: list[str] = []
        if target["target_id"] == "debian:ldap-bindata":
            generated_outputs = []
        elif target["name"] == "lib1521":
            generated_outputs = [(flavor.worktree_dir / "tests" / "libtest" / "lib1521.c").as_posix()]

        object_paths: list[Path] = []
        for token in target["sources"]:
            if not token.endswith(".c"):
                continue
            src = source_path(flavor, target, token)
            key = (src.as_posix(), tuple(compile_args))
            if key not in object_cache:
                obj_hash = hashlib.sha256(
                    ("\0".join([src.as_posix(), *compile_args])).encode("utf-8")
                ).hexdigest()[:20]
                obj_path = flavor.objects_dir / f"{obj_hash}-{src.name}.o"
                object_cache[key] = obj_path
                compile_tasks[key] = (src, compile_args, obj_path)
            object_paths.append(object_cache[key])

        link_args = target_link_args(flavor, target)
        ldflags = target_ldflags(flavor, target)
        exe_path = executable_path(flavor, target)
        exe_path.parent.mkdir(parents=True, exist_ok=True)
        target_records.append(
            {
                "target_id": target["target_id"],
                "component": target["component"],
                "name": target["name"],
                "role": target["role"],
                "source_dir": srcdir.as_posix(),
                "generated_outputs": generated_outputs,
                "sources": list(target["sources"]),
                "compile_args": compile_args,
                "link_args": ldflags + link_args,
                "object_paths": [path.as_posix() for path in object_paths],
                "object_path": object_paths[0].as_posix() if object_paths else None,
                "executable_path": exe_path.as_posix(),
                "runnable": runnable_metadata(target),
            }
        )

    def compile_one(task: tuple[Path, list[str], Path]) -> None:
        src, compile_args, obj_path = task
        cmd = [*compile_args, "-c", src.as_posix(), "-o", obj_path.as_posix()]
        run(*cmd, cwd=src.parent)

    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, jobs)) as executor:
        futures = [executor.submit(compile_one, task) for task in compile_tasks.values()]
        for future in concurrent.futures.as_completed(futures):
            future.result()

    for record in target_records:
        if record.get("skipped_reason"):
            continue
        exe_path = Path(record["executable_path"])
        cmd = [
            os.environ.get("CC", "cc"),
            *record["object_paths"],
            "-o",
            exe_path.as_posix(),
            *record["link_args"],
        ]
        run(*cmd, cwd=exe_path.parent)
        if record["component"] in {"libtest", "server", "src", "http-client"}:
            dotlibs = exe_path.parent / ".libs" / exe_path.name
            if dotlibs.exists() or dotlibs.is_symlink():
                dotlibs.unlink()
            dotlibs.symlink_to(Path("..") / exe_path.name)

    state = {
        "schema_version": 1,
        "flavor": flavor.name,
        "worktree": flavor.worktree_dir.as_posix(),
        "stage": {
            "include_dir": Path(stage_info["include_dir"]).as_posix(),
            "lib_dir": Path(stage_info["lib_dir"]).as_posix(),
            "library_path": Path(stage_info["library_path"]).as_posix(),
            "reference_library_path": Path(stage_info["reference_library_path"]).as_posix(),
            "soname": flavor.soname,
        },
        "targets": target_records,
    }
    flavor.build_state_path.parent.mkdir(parents=True, exist_ok=True)
    flavor.build_state_path.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return state


def selected_targets(manifest: dict, selectors: list[str]) -> list[dict]:
    all_targets = all_build_targets(manifest)
    if not selectors:
        return all_targets
    target_map = {target["target_id"]: target for target in all_targets}
    selected = []
    for selector in selectors:
        if selector not in target_map:
            raise HarnessError(f"unknown target selector: {selector}")
        selected.append(target_map[selector])
    return selected


def cmd_vendor(_args: argparse.Namespace) -> None:
    vendor_assets(load_manifest())


def cmd_export(args: argparse.Namespace) -> None:
    export_tracked_tree(args.mode, args.dest.resolve())


def cmd_build(args: argparse.Namespace) -> None:
    manifest = load_manifest()
    flavor = flavor_config(args.flavor)
    if not VENDOR_ROOT.exists():
        vendor_assets(manifest)
    targets = selected_targets(manifest, [] if args.all else args.target)
    build_targets(flavor, targets, jobs=args.jobs or os.cpu_count() or 1)


def main() -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    vendor_parser = subparsers.add_parser("vendor")
    vendor_parser.set_defaults(func=cmd_vendor)

    export_parser = subparsers.add_parser("export")
    export_parser.add_argument("--mode", choices=["safe-only", "with-root-harness"], required=True)
    export_parser.add_argument("--dest", type=Path, required=True)
    export_parser.set_defaults(func=cmd_export)

    build_parser = subparsers.add_parser("build")
    build_parser.add_argument("--flavor", choices=["openssl", "gnutls"], required=True)
    build_parser.add_argument("--all", action="store_true")
    build_parser.add_argument("--target", action="append", default=[])
    build_parser.add_argument("--jobs", type=int, default=0)
    build_parser.set_defaults(func=cmd_build)

    args = parser.parse_args()
    try:
        args.func(args)
    except HarnessError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
