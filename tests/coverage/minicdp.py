# /// script
# requires-python = ">=3.9"
# dependencies = []
# ///
"""minicdp.py -- stdlib-only CDP client for E2E gate kits.

HTTP /json discovery plus a minimal RFC6455 websocket: client frames
masked, server frames unmasked, ping answered with pong, continuation
frames joined. CDP responses are matched by id; events are dropped.
Enough surface for Runtime.evaluate, Page.navigate and the
Userscript.*/Browser.* commands the gates drive.
"""

import json
import os
import socket
import struct
import time
import urllib.request


def http_get(url, timeout=5):
    with urllib.request.urlopen(url, timeout=timeout) as r:
        return json.loads(r.read().decode())


class WS:
    def __init__(self, host, port, path):
        self.sock = socket.create_connection((host, port), timeout=10)
        self.buf = b""
        key = "dGhlIHNhbXBsZSBub25jZQ=="
        req = (f"GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\n"
               "Upgrade: websocket\r\nConnection: Upgrade\r\n"
               f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n"
               "\r\n")
        self.sock.sendall(req.encode())
        head = b""
        while b"\r\n\r\n" not in head:
            chunk = self.sock.recv(4096)
            if not chunk:
                raise ConnectionError("ws handshake eof")
            head += chunk
        if b" 101 " not in head.split(b"\r\n", 1)[0]:
            raise ConnectionError("ws handshake rejected: %r" % head[:120])
        self.buf = head.split(b"\r\n\r\n", 1)[1]

    def _read_exact(self, n):
        while len(self.buf) < n:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise ConnectionError("ws eof")
            self.buf += chunk
        out, self.buf = self.buf[:n], self.buf[n:]
        return out

    def recv_text(self):
        frags = []
        while True:
            b1, b2 = self._read_exact(2)
            opcode = b1 & 0x0F
            fin = b1 & 0x80
            length = b2 & 0x7F
            if length == 126:
                length = struct.unpack(">H", self._read_exact(2))[0]
            elif length == 127:
                length = struct.unpack(">Q", self._read_exact(8))[0]
            mask = self._read_exact(4) if b2 & 0x80 else None
            payload = self._read_exact(length) if length else b""
            if mask:  # RFC: servers never mask; stay correct if one does
                payload = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
            if opcode == 8:
                raise ConnectionError("ws close")
            if opcode == 9:  # ping -> pong
                self._send_frame(0xA, payload)
                continue
            if opcode == 0xA:  # pong: never solicited here, drop
                continue
            if opcode in (1, 0):
                frags.append(payload)
            if fin:
                return b"".join(frags).decode("utf-8", "replace")

    def _send_frame(self, opcode, payload):
        mask = os.urandom(4)
        header = bytes([0x80 | opcode])
        n = len(payload)
        if n < 126:
            header += bytes([0x80 | n])
        elif n < 65536:
            header += bytes([0x80 | 126]) + struct.pack(">H", n)
        else:
            header += bytes([0x80 | 127]) + struct.pack(">Q", n)
        masked = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
        self.sock.sendall(header + mask + masked)

    def send_json(self, obj):
        self._send_frame(1, json.dumps(obj).encode())


class CDP:
    """One websocket session against a page target on 127.0.0.1:<port>."""

    def __init__(self, port, timeout=15):
        self.port = port
        deadline = time.time() + timeout
        target = None
        while time.time() < deadline:
            try:
                targets = http_get(f"http://127.0.0.1:{port}/json/list")
                pages = [t for t in targets if t.get("type") == "page"]
                if pages:
                    target = pages[0]["webSocketDebuggerUrl"]
                    break
            except Exception:
                pass
            time.sleep(0.25)
        if not target:
            raise ConnectionError(f"no page target on port {port}")
        path = target.split("127.0.0.1", 1)[1].split("/", 1)[1]
        path = "/" + path
        self.ws = WS("127.0.0.1", port, path)
        self.id = 0

    def send(self, method, params=None):
        self.id += 1
        self.ws.send_json({"id": self.id, "method": method,
                           "params": params or {}})
        while True:
            msg = json.loads(self.ws.recv_text())
            if msg.get("id") == self.id:
                return msg

    def eval(self, expression):
        r = self.send("Runtime.evaluate",
                      {"expression": expression, "returnByValue": True})
        return r.get("result", {}).get("result", {}).get("value")
