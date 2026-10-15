# /// script
# requires-python = ">=3.9"
# dependencies = []
# ///
"""common.py -- shared harness for the clean-chrome smoke/regression gates.

Everything a gate needs to drive a clean-chrome binary lives here:
platform-resolving binary discovery (linux ``chrome`` / mac ``.app``
bundle / windows ``chrome.exe``), boot/kill lifecycle with fresh
throwaway profiles, an http origin fixture, and the JSON receipt every
gate emits (machine-readable verdict; stdout lines are for humans).

Gate contract (extreme_gate form): exit 0 = all legs green, 1 = any
leg red, 2 = usage error. A leg may return ``ok=None`` to skip (not
counted red; recorded in the receipt with skipped=true).
"""

import glob
import http.server
import json
import os
import platform
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from minicdp import CDP, http_get  # noqa: E402

IS_WIN = os.name == "nt"
IS_MAC = platform.system() == "Darwin"

# CDP attach deadline after boot; 30s of silence means the binary is
# not coming up healthy even on a slow cold-start box.
ATTACH_TIMEOUT = 30


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def port_listening(port, host="127.0.0.1", timeout=2):
    s = socket.socket()
    s.settimeout(timeout)
    try:
        s.connect((host, port))
        return True
    except OSError:
        return False
    finally:
        s.close()


def parse_chrome_arg(arg):
    """Resolve --chrome into the concrete executable or exit(2).

    Accepts a platform binary, a mac .app bundle (bundle dir or the
    inner executable), or a build/deploy directory containing the
    platform binary.
    """
    p = os.path.abspath(arg)
    if os.path.isfile(p):
        return p
    if IS_MAC:
        for cand in (os.path.join(p, "Contents", "MacOS", "Chromium"),
                     os.path.join(p, "Chromium.app", "Contents", "MacOS",
                                  "Chromium")):
            if os.path.isfile(cand):
                return cand
    direct = os.path.join(p, "chrome.exe" if IS_WIN else "chrome")
    if os.path.isfile(direct):
        return direct
    print(f"chrome not found: {arg}", file=sys.stderr)
    sys.exit(2)


def boot(chrome, profile, port=None, extra_args=(), env_extra=None,
         log=None, url="about:blank", headless=True, debug_port=True):
    """Start the binary on a throwaway profile; returns (proc, port).

    Mirrors extreme_gate.boot: session leader on POSIX so kill_group
    takes the whole tree, ``--no-sandbox`` off-Windows (WSL userns
    口径), headless ozone only when headless. ``debug_port=False``
    omits ``--remote-debugging-port`` entirely (the default-9222 leg:
    the contract says a bare argv must listen on 9222).
    """
    os.makedirs(profile, exist_ok=True)
    if port is None:
        port = free_port()
    log = log or os.path.join(os.path.dirname(profile), "chrome.log")
    f = open(log, "ab")
    args = [chrome]
    if headless:
        args += ["--headless=new", "--disable-gpu"]
    args += [f"--user-data-dir={profile}"]
    if debug_port:
        args += [f"--remote-debugging-port={port}"]
    args += ["--no-first-run", url]
    args += list(extra_args)
    if not IS_WIN:
        args += ["--no-sandbox"]
        if headless:
            args += ["--ozone-platform=headless"]
    env = dict(os.environ)
    if env_extra:
        env.update(env_extra)
    proc = subprocess.Popen(
        args, stdin=subprocess.DEVNULL, stdout=f, stderr=f, env=env,
        start_new_session=(not IS_WIN))
    return proc, port


def kill_group(proc):
    """Terminate the browser and its process tree."""
    if proc is None or proc.poll() is not None:
        return
    if IS_WIN:
        subprocess.run(["taskkill", "/T", "/F", "/PID", str(proc.pid)],
                       capture_output=True)
    else:
        try:
            os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        pass


def attach(port, timeout=ATTACH_TIMEOUT):
    """CDP(page) with a deadline; raises ConnectionError on failure."""
    return CDP(port, timeout=timeout)


def wait_version(port, timeout=ATTACH_TIMEOUT):
    """/json/version once it answers; returns the parsed dict."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            return http_get(f"http://127.0.0.1:{port}/json/version")
        except Exception:
            time.sleep(0.25)
    raise ConnectionError(f"no /json/version on port {port}")


ORIGIN_PAGE = b"""<!doctype html><html><head><title>gate</title></head>
<body><h1>gate</h1></body></html>"""


class Origin:
    """Single-page http origin on 127.0.0.1; any path serves the page,
    so legs can navigate to a.html/b.html freely."""

    def __init__(self):
        self.port = free_port()

        class H(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                self.send_response(200)
                self.send_header("Content-Type", "text/html")
                self.send_header("Content-Length", str(len(ORIGIN_PAGE)))
                self.end_headers()
                self.wfile.write(ORIGIN_PAGE)

            def log_message(self, *a):
                pass

        self.server = http.server.ThreadingHTTPServer(
            ("127.0.0.1", self.port), H)
        threading.Thread(target=self.server.serve_forever,
                         daemon=True).start()

    def url(self, path="/"):
        return f"http://127.0.0.1:{self.port}{path}"


class Gate:
    """Leg collector.

    A leg is a callable returning ``(ok, note, data)``; ``ok=None``
    means SKIP (recorded, never red). ``finish()`` writes the JSON
    receipt (workdir/receipt.json), echoes it to stdout, and returns
    the process exit code.
    """

    def __init__(self, work):
        self.work = work
        self.results = []

    def run(self, name, fn):
        try:
            ok, note, data = fn()
        except Exception as e:  # leg crash counts red, not suite death
            ok, note, data = False, f"leg crashed: {e!r}", None
        if ok is None and "SKIP" not in note:
            ok, note = False, f"skip without note: {note}"
        self.results.append({"name": name, "ok": bool(ok) if ok is not None
                             else None,
                             "skipped": ok is None,
                             "note": note, "data": data})
        tag = "SKIP" if ok is None else ("PASS" if ok else "FAIL")
        print(f"{tag} {name}: {note}", flush=True)
        return ok

    def finish(self, label):
        failed = [r for r in self.results
                  if not r["skipped"] and not r["ok"]]
        skipped = [r for r in self.results if r["skipped"]]
        receipt = {
            "gate": label,
            "ts": time.strftime("%Y-%m-%dT%H:%M:%S"),
            "passed": len(self.results) - len(failed) - len(skipped),
            "failed": len(failed),
            "skipped": len(skipped),
            "total": len(self.results),
            "work": self.work,
            "legs": self.results,
        }
        path = os.path.join(self.work, "receipt.json")
        with open(path, "w") as f:
            json.dump(receipt, f, ensure_ascii=False, indent=1)
        print(json.dumps(receipt, ensure_ascii=False))
        print(f"GATE {label}: {receipt['passed']}/{receipt['total']} pass, "
              f"{len(skipped)} skip (receipt={path})", flush=True)
        return 1 if failed else 0


def make_work(preferred, prefix):
    """Workdir: pinned path or a fresh tempdir."""
    if preferred:
        os.makedirs(preferred, exist_ok=True)
        return preferred
    return tempfile.mkdtemp(prefix=prefix)


def crash_dumps(profile):
    """Count crashpad dumps the browser left under the profile; legs
    that must prove 'no self-kill' (REQ-021 face) read this."""
    pats = [os.path.join(profile, "Crashpad", "reports", "*.dmp"),
            os.path.join(profile, "Crashpad", "pending", "*.dmp"),
            os.path.join(profile, "Crash Reports", "*.dmp")]
    return sum(len(glob.glob(p)) for p in pats)


def main_binary_hints():
    """Default binary guesses per platform, for --chrome fallback."""
    home = os.path.expanduser("~")
    if IS_WIN:
        return [r"C:\dev-chrome\trees\chromium-155\src\out\Release"]
    if IS_MAC:
        return [os.path.join(home, "dev-chrome/trees/chromium-155/src/"
                                  "out/Release/Chromium.app")]
    return [os.path.join(home, "dev-chrome/trees/chromium-155/src/"
                              "out/Release/chrome")]
