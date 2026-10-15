# /// script
# requires-python = ">=3.9"
# dependencies = []
# ///
"""storage_gate.py -- P0.5 userscripts storage-face extreme gate.

Ported from clean-chrome tools/e2e/extreme_gate.py (REQ-020 corpus;
12 legs) so the browse-side suite carries the storage-defect grabber
against every new binary without touching the build repo.

Born from #83: the deep-test rack's storage-loss verdict was a profile
plumbing phantom, and the session-side repro kits lived in /tmp that a
WSL restart wiped. This gate is the durable, self-contained form:
raw chrome on a throwaway profile, user-data-dir always equal to the
store under assertion, so an engine/profile divergence cannot fake a
verdict (diary 2026-10-11).

Legs:
  d1x-<close|kill>  quick-kill storage xN: seed writes settle to disk,
                    then CDP Browser.close (graceful, the browse down
                    path) or SIGKILL the browser process group; restart
                    on the same profile and read back k1/n.
  w1b5              corrupt values JSON: the merge write must fail and
                    leave the original bytes on disk (B5: never adopt an
                    empty dict and overwrite).
  r10v              uninstall: values file deleted with the script and
                    never resurrected; writes on a dead id are refused.

Usage:
  uv run tests/clean-chrome/storage_gate.py --chrome <chrome> [--work DIR]
      [--iterations N] (default 5)
Exit code 0 = all legs green.
"""

import argparse
import http.server
import json
import os
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time

sys.path.insert(0, os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "lib"))
from minicdp import CDP  # noqa: E402


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


class Origin:
    """Single-page http origin on 127.0.0.1 for @match targets."""

    PAGE = b"<html><body>gate</body></html>"

    def __init__(self):
        self.port = free_port()

        class H(http.server.BaseHTTPRequestHandler):
            origin = self

            def do_GET(self):
                body = Origin.PAGE
                self.send_response(200)
                self.send_header("Content-Type", "text/html")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *a):
                pass

        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", self.port), H)
        threading.Thread(target=self.server.serve_forever, daemon=True).start()

    def url(self, query=""):
        q = f"?{query}" if query else ""
        return f"http://127.0.0.1:{self.port}/inner.html{q}"


def boot(chrome, profile, port, log):
    """Start chrome on <profile>; returns the Popen (session leader)."""
    os.makedirs(profile, exist_ok=True)
    f = open(log, "ab")
    args = [chrome, "--headless=new", "--disable-gpu",
            f"--user-data-dir={profile}",
            f"--remote-debugging-port={port}", "--no-first-run", "about:blank"]
    if os.name != "nt":
        args[1:1] = ["--no-sandbox", "--ozone-platform=headless"]
    proc = subprocess.Popen(
        args,
        stdin=subprocess.DEVNULL, stdout=f, stderr=f,
        start_new_session=(os.name != "nt"))
    return proc


def kill_group(proc):
    if os.name == "nt":
        # TerminateProcess the tree; children follow the browser down.
        subprocess.run(["taskkill", "/T", "/F", "/PID", str(proc.pid)],
                       capture_output=True)
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            pass
        return
    try:
        os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
    except (ProcessLookupError, PermissionError):
        pass
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        pass


def script_source(name, ns, body):
    return ("// ==UserScript==\n"
            f"// @name         {name}\n// @namespace    {ns}\n"
            "// @version      1.0\n// @match        http://127.0.0.1/*\n"
            "// @grant        GM_setValue\n// @grant        GM_getValue\n"
            "// @run-at       document-idle\n// ==/UserScript==\n" + body)


def wait_attr(cdp, needle, tries=30, step=0.3):
    attr = None
    for _ in range(tries):
        time.sleep(step)
        attr = cdp.eval("document.documentElement.getAttribute('probe')")
        if attr and needle in str(attr):
            return str(attr)
    return str(attr)


def leg_d1x(chrome, work, origin, form, iteration):
    """Seed -> settled write -> close|kill -> restart -> read back."""
    profile = os.path.join(work, f"d1x-{form}-{iteration}")
    port = free_port()
    log = os.path.join(work, f"chrome-d1x-{form}-{iteration}.log")
    proc = boot(chrome, profile, port, log)
    try:
        cdp = CDP(port)
        seed_n = f"GM_setValue('n',{iteration});"
        src = script_source(
            "d1x", "dx",
            "(function(){var m=new URLSearchParams(location.search)"
            ".get('m')||'seed';var o={m:m};"
            "if(m==='seed'){GM_setValue('k1','v1');" + seed_n +
            "o.k=GM_getValue('k1');}"
            "else{o.k=GM_getValue('k1');o.n=GM_getValue('n');}"
            "document.documentElement.setAttribute('probe',"
            "JSON.stringify(o));})();")
        r = cdp.send("Userscript.install", {"source": src})
        sid = r.get("result", {}).get("scriptId")
        if not sid:
            return False, f"install failed: {json.dumps(r)[:160]}"
        cdp.send("Page.enable")
        cdp.send("Page.navigate", {"url": origin.url("m=seed")})
        attr = wait_attr(cdp, '"v1"')
        if '"v1"' not in attr:
            return False, f"seed no attr: {attr}"
        vpath = os.path.join(profile, "Default", "Userscripts", "values",
                             f"{sid}.json")
        settled = False
        for _ in range(1500):  # write settles in ms; 3s ceiling
            try:
                if os.path.getsize(vpath) > 0:
                    settled = True
                    break
            except OSError:
                pass
            time.sleep(0.002)
        if not settled:
            return False, "seed write never settled to disk"
        if form == "close":
            try:
                cdp.send("Browser.close", {})
            except Exception:
                pass  # socket dies with the browser; that is the path
            try:
                proc.wait(timeout=6)
            except subprocess.TimeoutExpired:
                kill_group(proc)
        else:
            kill_group(proc)
    finally:
        kill_group(proc)
    # restart on the same profile and read back
    port = free_port()
    proc = boot(chrome, profile, port, log)
    try:
        cdp = CDP(port)
        cdp.send("Page.enable")
        cdp.send("Page.navigate", {"url": origin.url("m=read")})
        attr = wait_attr(cdp, '"k"')
        ok = '"k":"v1"' in attr and f'"n":{iteration}' in attr
        return ok, f"read=[{attr}]"
    finally:
        kill_group(proc)


def leg_w1b5(chrome, work, origin):
    """Corrupt values JSON: write must fail, original bytes preserved."""
    profile = os.path.join(work, "w1b5")
    port = free_port()
    proc = boot(chrome, profile, port, os.path.join(work, "chrome-w1b5.log"))
    try:
        cdp = CDP(port)
        src = script_source(
            "w1b5", "wx",
            "(function(){GM_setValue('k2','v2');"
            "document.documentElement.setAttribute('probe',"
            "JSON.stringify({k1:GM_getValue('k1','D')}));})();")
        r = cdp.send("Userscript.install", {"source": src})
        sid = r.get("result", {}).get("scriptId")
        if not sid:
            return False, f"install failed: {json.dumps(r)[:160]}"
        vpath = os.path.join(profile, "Default", "Userscripts", "values",
                             f"{sid}.json")
        os.makedirs(os.path.dirname(vpath), exist_ok=True)
        bad = '{"k1": "unterminated'
        with open(vpath, "w") as f:
            f.write(bad)
        cdp.send("Page.enable")
        cdp.send("Page.navigate", {"url": origin.url()})
        attr = wait_attr(cdp, "k1")
        time.sleep(1)
        with open(vpath) as f:
            disk = f.read()
        ok = disk == bad and '"D"' in attr
        return ok, f"disk_unchanged={disk == bad} default_seen={'D' in attr}"
    finally:
        kill_group(proc)


def leg_r10v(chrome, work, origin):
    """Uninstall deletes the values file and it never resurrects."""
    profile = os.path.join(work, "r10v")
    port = free_port()
    proc = boot(chrome, profile, port, os.path.join(work, "chrome-r10v.log"))
    try:
        cdp = CDP(port)
        src = script_source(
            "r10v", "rx",
            "(function(){GM_setValue('k1','v1');})();")
        r = cdp.send("Userscript.install", {"source": src})
        sid = r.get("result", {}).get("scriptId")
        if not sid:
            return False, f"install failed: {json.dumps(r)[:160]}"
        cdp.send("Page.enable")
        cdp.send("Page.navigate", {"url": origin.url()})
        vpath = os.path.join(profile, "Default", "Userscripts", "values",
                             f"{sid}.json")
        seeded = False
        for _ in range(50):
            time.sleep(0.2)
            if os.path.exists(vpath):
                seeded = True
                break
        if not seeded:
            return False, "seed write never settled"
        cdp.send("Userscript.uninstall", {"scriptId": sid})
        time.sleep(2)
        gone = not os.path.exists(vpath)
        r2 = cdp.send("Userscript.setValue",
                      {"scriptId": sid, "key": "z", "value": '"z"'})
        # Pin the exact refusal shape: a handler regression to
        # "Method not found" must not pass as a dead-id refusal.
        err = r2.get("error") or {}
        refused = err.get("message") == "setValue failed"
        time.sleep(1)
        still_gone = not os.path.exists(vpath)
        ok = gone and refused and still_gone
        return ok, (f"gone={gone} refused={refused} still_gone={still_gone}")
    finally:
        kill_group(proc)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--chrome", required=True,
                    help="chrome binary (build output or deployment)")
    ap.add_argument("--work", default=None,
                    help="scratch root (default: a fresh tempdir)")
    ap.add_argument("--iterations", type=int, default=5,
                    help="d1x repetitions per close form")
    args = ap.parse_args()
    if not os.path.isfile(args.chrome) or (
            os.name != "nt" and not os.access(args.chrome, os.X_OK)):
        print(f"chrome not executable: {args.chrome}")
        return 2
    work = args.work or tempfile.mkdtemp(prefix="cc-extreme-")
    os.makedirs(work, exist_ok=True)
    origin = Origin()
    results = []
    for form in ("close", "kill"):
        for i in range(1, args.iterations + 1):
            name = f"d1x-{form}#{i}"
            try:
                ok, note = leg_d1x(args.chrome, work, origin, form, i)
            except Exception as e:
                ok, note = False, f"leg crashed: {e!r}"
            results.append((name, ok, note))
            print(f"{'PASS' if ok else 'FAIL'} {name}: {note}",
                  flush=True)
    for name, fn in (("w1b5", leg_w1b5), ("r10v", leg_r10v)):
        try:
            ok, note = fn(args.chrome, work, origin)
        except Exception as e:
            ok, note = False, f"leg crashed: {e!r}"
        results.append((name, ok, note))
        print(f"{'PASS' if ok else 'FAIL'} {name}: {note}", flush=True)
    failed = [r for r in results if not r[1]]
    print(f"EXTREME GATE: {len(results) - len(failed)}/{len(results)} pass"
          f" (work={work})")
    receipt = {
        "gate": "storage",
        "ts": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "passed": len(results) - len(failed),
        "failed": len(failed),
        "skipped": 0,
        "total": len(results),
        "work": work,
        "legs": [{"name": n, "ok": bool(ok), "skipped": False,
                  "note": note, "data": None}
                 for n, ok, note in results],
    }
    with open(os.path.join(work, "receipt.json"), "w") as f:
        json.dump(receipt, f, ensure_ascii=False, indent=1)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
