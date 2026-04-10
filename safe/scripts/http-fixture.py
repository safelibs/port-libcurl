#!/usr/bin/env python3
from __future__ import annotations

import argparse
import functools
import http.server
import os
import pathlib
import re
import sys
import urllib.parse


RANGE_RE = re.compile(r"bytes=(\d*)-(\d*)")


def prepare_tree(root: pathlib.Path) -> None:
    if root.exists():
        for child in root.iterdir():
            if child.is_dir():
                for nested in sorted(child.rglob("*"), reverse=True):
                    if nested.is_file() or nested.is_symlink():
                        nested.unlink()
                    elif nested.is_dir():
                        nested.rmdir()
                child.rmdir()
            else:
                child.unlink()
    root.mkdir(parents=True, exist_ok=True)
    (root / "uploaded").mkdir(parents=True, exist_ok=True)
    (root / "redirects").mkdir(parents=True, exist_ok=True)
    (root / "headers").mkdir(parents=True, exist_ok=True)
    (root / "push").mkdir(parents=True, exist_ok=True)

    (root / "plain.txt").write_text("downloaded through compat curl\n", encoding="utf-8")
    (root / "large.bin").write_bytes(bytes((index % 251 for index in range(65536))))
    (root / "redirects" / "target.txt").write_text("redirect landed here\n", encoding="utf-8")
    (root / "headers" / "payload.txt").write_text("header endpoint body\n", encoding="utf-8")
    (root / "push" / "asset.txt").write_text("pushed asset body\n", encoding="utf-8")


class Handler(http.server.SimpleHTTPRequestHandler):
    server_version = "compat-http-fixture/1.0"

    def __init__(self, *args, directory: str, **kwargs):
        self._range: tuple[int, int] | None = None
        super().__init__(*args, directory=directory, **kwargs)

    def log_message(self, fmt: str, *args) -> None:
        sys.stderr.write(
            "%s - - [%s] %s\n"
            % (self.client_address[0], self.log_date_time_string(), fmt % args)
        )

    def do_PUT(self) -> None:
        rel = urllib.parse.urlparse(self.path).path.lstrip("/")
        if not rel.startswith("upload/"):
            self.send_error(405, "PUT only supported under /upload/")
            return

        target = pathlib.Path(self.directory) / "uploaded" / rel[len("upload/") :]
        target.parent.mkdir(parents=True, exist_ok=True)
        remaining = int(self.headers.get("Content-Length", "0"))
        with target.open("wb") as fh:
            while remaining > 0:
                chunk = self.rfile.read(min(65536, remaining))
                if not chunk:
                    break
                fh.write(chunk)
                remaining -= len(chunk)

        self.send_response(201)
        self.send_header("Content-Length", "2")
        self.end_headers()
        self.wfile.write(b"ok")

    def do_GET(self) -> None:
        parsed = urllib.parse.urlparse(self.path)
        if parsed.path == "/redirect":
            self.send_response(302)
            self.send_header("Location", "/redirects/target.txt")
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        if parsed.path == "/push":
            payload = b"primary push response\n"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Link", "</push/asset.txt>; rel=preload")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
            return
        if parsed.path == "/headers":
            payload = (pathlib.Path(self.directory) / "headers" / "payload.txt").read_bytes()
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("X-Compat-Fixture", "yes")
            self.send_header("X-Compat-Path", parsed.path)
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
            return
        super().do_GET()

    def do_HEAD(self) -> None:
        parsed = urllib.parse.urlparse(self.path)
        if parsed.path == "/push":
            payload = b"primary push response\n"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Link", "</push/asset.txt>; rel=preload")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            return
        if parsed.path == "/headers":
            payload = (pathlib.Path(self.directory) / "headers" / "payload.txt").read_bytes()
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("X-Compat-Fixture", "yes")
            self.send_header("X-Compat-Path", parsed.path)
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            return
        super().do_HEAD()

    def copyfile(self, source, outputfile) -> None:
        try:
            if self._range is None:
                super().copyfile(source, outputfile)
                return

            remaining = self._range[1] - self._range[0] + 1
            while remaining > 0:
                chunk = source.read(min(65536, remaining))
                if not chunk:
                    break
                outputfile.write(chunk)
                remaining -= len(chunk)
        except BrokenPipeError:
            return

    def send_head(self):  # noqa: D401
        self._range = None
        path = self.translate_path(self.path)
        if os.path.isdir(path):
            return super().send_head()

        try:
            fh = open(path, "rb")
        except OSError:
            self.send_error(404, "File not found")
            return None

        size = os.fstat(fh.fileno()).st_size
        range_header = self.headers.get("Range")
        if range_header:
            match = RANGE_RE.fullmatch(range_header.strip())
            if not match:
                fh.close()
                self.send_error(416, "Invalid range")
                return None
            start_s, end_s = match.groups()
            if not start_s and not end_s:
                fh.close()
                self.send_error(416, "Invalid range")
                return None
            if not start_s:
                length = int(end_s)
                if length <= 0:
                    fh.close()
                    self.send_error(416, "Invalid range")
                    return None
                start = max(size - length, 0)
                end = size - 1
            else:
                start = int(start_s)
                end = int(end_s) if end_s else size - 1
            if start >= size or start > end:
                fh.close()
                self.send_response(416)
                self.send_header("Content-Range", f"bytes */{size}")
                self.end_headers()
                return None
            end = min(end, size - 1)
            self._range = (start, end)
            fh.seek(start)
            self.send_response(206)
            self.send_header("Content-Type", self.guess_type(path))
            self.send_header("Content-Length", str(end - start + 1))
            self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
            self.send_header("Accept-Ranges", "bytes")
            self.end_headers()
            return fh

        self.send_response(200)
        self.send_header("Content-Type", self.guess_type(path))
        self.send_header("Content-Length", str(size))
        self.send_header("Accept-Ranges", "bytes")
        self.end_headers()
        return fh


def cmd_prepare(args: argparse.Namespace) -> int:
    prepare_tree(args.root.resolve())
    return 0


def cmd_serve(args: argparse.Namespace) -> int:
    root = args.root.resolve()
    root.mkdir(parents=True, exist_ok=True)
    handler = functools.partial(Handler, directory=str(root))
    with http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler) as server:
        args.port_file.write_text(str(server.server_address[1]), encoding="utf-8")
        server.serve_forever()
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    prepare_parser = subparsers.add_parser("prepare")
    prepare_parser.add_argument("--root", type=pathlib.Path, required=True)
    prepare_parser.set_defaults(func=cmd_prepare)

    serve_parser = subparsers.add_parser("serve")
    serve_parser.add_argument("--root", type=pathlib.Path, required=True)
    serve_parser.add_argument("--port-file", type=pathlib.Path, required=True)
    serve_parser.set_defaults(func=cmd_serve)

    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
