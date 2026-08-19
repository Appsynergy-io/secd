#!/usr/bin/env python3
"""A minimal OCI registry, over TLS, for exercising the release image push.

Deliberately strict rather than permissive: a stub that accepts anything would
let a broken pusher pass. It reproduces the parts of GHCR that the hand-rolled
pusher in scripts/release.sh actually depends on, and rejects the mistakes it
could plausibly make:

  * the first request is answered 401 with a Bearer challenge, so
    fetch_bearer()/parse_challenge() are exercised rather than skipped;
  * the token endpoint asserts the requested scope;
  * blob uploads return a *relative* Location, which is what GHCR sends and
    the shape most likely to be mishandled by urljoin;
  * every blob body is re-hashed and rejected on mismatch, which is what makes
    a gzip-determinism regression visible;
  * a manifest referencing an unknown blob, or naming the wrong size, is
    rejected.

TLS is used rather than plain http so release.sh keeps its https-only
invariant; relaxing that for localhost is exactly the kind of thing that later
leaks into production.

Usage: fake-registry.py --cert C --key K --store DIR [--port N]
Prints "listening <port>" on stdout, then serves until killed.
"""

import argparse
import hashlib
import json
import re
import ssl
import sys
import urllib.parse
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

TOKEN = "fake-registry-token"
STATE = {"store": None, "host": ""}
LOCK = threading.Lock()
UPLOADS: dict[str, bytes] = {}


def blob_path(digest: str) -> Path:
    return STATE["store"] / "blobs" / digest.replace(":", "_")


def manifest_path(name: str, ref: str) -> Path:
    safe = name.replace("/", "_") + "@" + ref.replace(":", "_")
    return STATE["store"] / "manifests" / safe


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt, *args):  # noqa: A002 - stdlib signature
        print("fake-registry: " + fmt % args, file=sys.stderr)

    # ---------------------------------------------------------------- helpers

    def send(self, code, body=b"", headers=None):
        self.send_response(code)
        for key, value in (headers or {}).items():
            self.send_header(key, value)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        if body:
            self.wfile.write(body)

    def fail(self, code, message):
        self.send(code, json.dumps({"errors": [{"message": message}]}).encode())

    def authorized(self) -> bool:
        return self.headers.get("Authorization") == "Bearer " + TOKEN

    def challenge(self):
        realm = f"https://{STATE['host']}/token"
        self.send(
            401,
            b"",
            {"WWW-Authenticate": f'Bearer realm="{realm}",service="fake-registry"'},
        )

    def body(self) -> bytes:
        return self.rfile.read(int(self.headers.get("Content-Length", "0")))

    # ---------------------------------------------------------------- routes

    def do_GET(self):
        if self.path.startswith("/token"):
            query = urllib.parse.parse_qs(self.path.partition("?")[2])
            scope = (query.get("scope") or [""])[0]
            if not re.fullmatch(r"repository:[\w./-]+:pull,push", scope):
                return self.fail(400, f"unexpected scope {scope!r}")
            return self.send(200, json.dumps({"token": TOKEN}).encode())

        if not self.authorized():
            return self.challenge()

        m = re.fullmatch(r"/v2/(?P<name>.+)/manifests/(?P<ref>[^/]+)", self.path)
        if m:
            path = manifest_path(m["name"], m["ref"])
            if not path.is_file():
                return self.fail(404, "unknown manifest")
            raw = path.read_bytes()
            return self.send(
                200,
                raw,
                {
                    "Docker-Content-Digest": "sha256:" + hashlib.sha256(raw).hexdigest(),
                    "Content-Type": json.loads(raw).get("mediaType", "application/json"),
                },
            )
        return self.fail(404, "not found")

    def do_POST(self):
        if not self.authorized():
            return self.challenge()
        m = re.fullmatch(r"/v2/(?P<name>.+)/blobs/uploads/", self.path)
        if not m:
            return self.fail(404, "not found")
        with LOCK:
            token = f"u{len(UPLOADS)}"
            UPLOADS[token] = b""
        # Relative Location, as GHCR sends.
        return self.send(
            202, b"", {"Location": f"/v2/{m['name']}/blobs/uploads/{token}"}
        )

    def do_PUT(self):
        if not self.authorized():
            return self.challenge()
        path, _, query = self.path.partition("?")

        m = re.fullmatch(r"/v2/(?P<name>.+)/blobs/uploads/(?P<token>[^/]+)", path)
        if m:
            want = (urllib.parse.parse_qs(query).get("digest") or [""])[0]
            data = self.body()
            got = "sha256:" + hashlib.sha256(data).hexdigest()
            if got != want:
                return self.fail(400, f"digest {want} does not match body {got}")
            blob_path(got).parent.mkdir(parents=True, exist_ok=True)
            blob_path(got).write_bytes(data)
            return self.send(201, b"", {"Docker-Content-Digest": got})

        m = re.fullmatch(r"/v2/(?P<name>.+)/manifests/(?P<ref>[^/]+)", path)
        if m:
            raw = self.body()
            try:
                doc = json.loads(raw)
            except json.JSONDecodeError:
                return self.fail(400, "manifest is not json")
            refs = [doc.get("config", {})] + list(doc.get("layers", []))
            for entry in refs:
                digest = entry.get("digest", "")
                stored = blob_path(digest)
                if not stored.is_file():
                    return self.fail(400, f"manifest references unknown blob {digest}")
                if entry.get("size") != stored.stat().st_size:
                    return self.fail(
                        400,
                        f"blob {digest} is {stored.stat().st_size} bytes, "
                        f"manifest says {entry.get('size')}",
                    )
            out = manifest_path(m["name"], m["ref"])
            out.parent.mkdir(parents=True, exist_ok=True)
            out.write_bytes(raw)
            digest = "sha256:" + hashlib.sha256(raw).hexdigest()
            (STATE["store"] / "last-digest").write_text(digest + "\n")
            return self.send(201, b"", {"Docker-Content-Digest": digest})

        return self.fail(404, "not found")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cert", required=True)
    ap.add_argument("--key", required=True)
    ap.add_argument("--store", required=True)
    ap.add_argument("--port", type=int, default=0)
    args = ap.parse_args()

    STATE["store"] = Path(args.store)
    STATE["store"].mkdir(parents=True, exist_ok=True)

    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    STATE["host"] = f"127.0.0.1:{server.server_address[1]}"
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(args.cert, args.key)
    server.socket = ctx.wrap_socket(server.socket, server_side=True)

    print(f"listening {server.server_address[1]}", flush=True)
    server.serve_forever()
    return 0


if __name__ == "__main__":
    sys.exit(main())
