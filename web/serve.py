#!/usr/bin/env python3
"""Serves web/dist for development, with the headers shared memory needs.

`fs/res/...` maps to the prepared resources (default `.local/horizon-client/26.3`): the files the
game fetches on demand. A bundle (web/bundle.py) goes brotli-compressed to a browser that takes it.

    python3 web/serve.py [--host 127.0.0.1] [--port 8026] [--resources PATH] [--access-log PATH]

`--access-log` appends `seconds<TAB>bytes<TAB>/res/path` for every resource file served: the files
a run reads, in order.
"""

import argparse
import functools
import http.server
import os
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class Handler(http.server.SimpleHTTPRequestHandler):
    resources: Path
    access_log = None
    access_lock = threading.Lock()
    started = time.monotonic()

    def end_headers(self):
        # Cross-origin isolation: SharedArrayBuffer (the wasm memory threads share).
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        self.send_header("Cross-Origin-Resource-Policy", "same-origin")
        self.send_header("Cache-Control", "no-cache")
        super().end_headers()

    def translate_path(self, path):
        path = path.split("?", 1)[0].split("#", 1)[0]
        if path.startswith("/fs/res/"):
            relative = Path(http.server.urllib.parse.unquote(path[len("/fs/res/"):]))
            if ".." in relative.parts:
                return str(self.resources / "missing")
            return str(self.resources / relative)
        return super().translate_path(path)

    def do_GET(self):
        path = self.path.split("?", 1)[0]
        if path == "/fs/index.txt":
            # A new page load: times restart.
            Handler.started = time.monotonic()
        packed = Path(self.translate_path(path) + ".br")
        if path.endswith(".bundle") and packed.is_file() and "br" in self.headers.get("Accept-Encoding", ""):
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Encoding", "br")
            self.send_header("Content-Length", str(packed.stat().st_size))
            self.end_headers()
            with packed.open("rb") as f:
                self.copyfile(f, self.wfile)
            return
        super().do_GET()
        if self.access_log is not None and path.startswith("/fs/res/"):
            local = Path(self.translate_path(path))
            size = local.stat().st_size if local.is_file() else -1
            with self.access_lock:
                self.access_log.write(f"{time.monotonic() - Handler.started:.3f}\t{size}\t{path[3:]}\n")
                self.access_log.flush()

    def log_message(self, format, *args):
        if os.environ.get("HORIZON_WEB_LOG"):
            super().log_message(format, *args)


Handler.extensions_map = {**Handler.extensions_map, ".wasm": "application/wasm", ".js": "text/javascript"}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8026)
    parser.add_argument("--resources", type=Path, default=ROOT / ".local/horizon-client/26.3")
    parser.add_argument("--access-log", type=Path)
    args = parser.parse_args()
    if args.access_log:
        Handler.access_log = args.access_log.open("a")
    Handler.resources = args.resources.resolve()
    handler = functools.partial(Handler, directory=str(ROOT / "web/dist"))
    server = http.server.ThreadingHTTPServer((args.host, args.port), handler)
    print(f"Serving web/dist on http://{args.host}:{args.port}/", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
