"""A local mock HTTP server for comparing ShareX's and ShareXrs's uploaders.

It listens on 127.0.0.1 only, records every request exactly as it arrived
(request line, headers in order with their original case, body bytes) and
answers each with the next canned response from a queue.

Both harnesses rewrite every destination URL to
``http://127.0.0.1:<port>/__/<scheme>/<authority><path>?<query>`` just
before sending it, so nothing ever leaves the machine.

Control endpoints (not recorded):
  POST /__mock/reset  {"responses": [{"status": 200, "reason": "OK",
                        "headers": [["Content-Type", "application/json"]],
                        "body": "...", "body_base64": "..."}]}
                      clears the log and queues the responses; "{base}" in a
                      header value or body becomes this server's base URL.
  GET  /__mock/log    the requests recorded since the last reset.

Usage: python mock_server.py [port]   (default 18765)
"""

import base64
import json
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

LOCK = threading.Lock()
STATE = {"responses": [], "log": []}


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    server_version = "mock"
    sys_version = ""

    def log_message(self, *args):  # quiet
        pass

    def _read_body(self):
        te = self.headers.get("Transfer-Encoding", "")
        if te.lower() == "chunked":
            data = b""
            while True:
                line = self.rfile.readline().strip()
                size = int(line.split(b";")[0], 16)
                if size == 0:
                    self.rfile.readline()
                    break
                data += self.rfile.read(size)
                self.rfile.readline()
            return data, True
        length = int(self.headers.get("Content-Length", "0") or 0)
        return (self.rfile.read(length) if length else b""), False

    def _send(self, status, reason, headers, body):
        self.send_response_only(status, reason)
        for k, v in headers:
            self.send_header(k, v)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _handle(self):
        body, chunked = self._read_body()
        base = f"http://127.0.0.1:{self.server.server_address[1]}"
        if self.path.startswith("/__mock/"):
            if self.path == "/__mock/reset":
                spec = json.loads(body or b"{}")
                with LOCK:
                    STATE["responses"] = list(spec.get("responses", []))
                    STATE["log"] = []
                self._send(200, "OK", [("Content-Type", "application/json")], b"{}")
            elif self.path == "/__mock/log":
                with LOCK:
                    data = json.dumps(STATE["log"]).encode()
                self._send(200, "OK", [("Content-Type", "application/json")], data)
            else:
                self._send(404, "Not Found", [], b"")
            return
        record = {
            "method": self.command,
            "target": self.path,
            "version": self.request_version,
            "headers": [[k, v] for k, v in self.headers.raw_items()],
            "chunked": chunked,
            "body_base64": base64.b64encode(body).decode(),
        }
        with LOCK:
            STATE["log"].append(record)
            resp = STATE["responses"].pop(0) if STATE["responses"] else None
        if resp is None:
            self._send(404, "Not Found", [("Content-Type", "text/plain")], b"no canned response")
            return
        if "body_base64" in resp:
            out = base64.b64decode(resp["body_base64"])
        else:
            out = resp.get("body", "").replace("{base}", base).encode("utf-8")
        headers = [(k, v.replace("{base}", base)) for k, v in resp.get("headers", [])]
        self._send(resp.get("status", 200), resp.get("reason"), headers, out)

    do_GET = do_POST = do_PUT = do_PATCH = do_DELETE = do_HEAD = _handle


class Server(ThreadingHTTPServer):
    daemon_threads = True

    def handle_error(self, request, client_address):
        # Clients closing idle keep-alive connections isn't worth a traceback.
        if not isinstance(sys.exc_info()[1], (ConnectionResetError, ConnectionAbortedError, BrokenPipeError)):
            super().handle_error(request, client_address)


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 18765
    server = Server(("127.0.0.1", port), Handler)
    print(f"mock server on http://127.0.0.1:{port}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
