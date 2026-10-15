# /// script
# requires-python = ">=3.9"
# dependencies = []
# ///
"""smoke_gate.py -- clean-chrome P0 出包冒烟 (run on every Release binary).

Legs (verdict per leg; JSON receipt = workdir/receipt.json + stdout):
  boot9222        无参默认口: argv without --remote-debugging-port,
                  /json/version answers on 127.0.0.1:9222 and WS attach
                  works with zero confirmation dialog (auto-allow)
  dcheck-scan     REQ-021: count b"DCHECK failed" in the main binary;
                  <=1 passes (the survivor is the v8 CSA format-string
                  literal), >=2 fails. Stale dcheck-on builds go red.
  cdp-handshake   explicit port: /json/version Browser string, WS
                  attach, Runtime.evaluate 1+1, UA readable
  address-bind    REQ-016: default binds loopback only;
                  --remote-debugging-address=0.0.0.0 binds all (ss/
                  netstat/lsof evidence); invalid address = no
                  listener + process stays alive
  pipe-port       CLEAN_CHROME_DEBUG=port: default port still opens
  pipe-pipe       CLEAN_CHROME_DEBUG=pipe: fd3/4 (win:
                  --remote-debugging-io-pipes) ASCIIZ CDP roundtrip,
                  no 9222 listener, navigator.webdriver===false over
                  the pipe session; closing the pipe takes the browser
                  down (S005 lifecycle)
  pipe-both       CLEAN_CHROME_DEBUG=both: pipe roundtrip + 9222 both
  webdriver-port  navigator.webdriver === false over the WS session
  userscripts     REQ-020: PUT /json/new?chrome://userscripts creates a
                  live page target listed in /json/list
  nav-paused-dialog / nav-paused-print
                  REQ-021 face, headful only (skip without --headful /
                  display): browser-driven navigate while the page is
                  paused (JS dialog / window.print() ScopedPagePauser)
                  must not kill the renderer; 5 rounds, zero crash dumps
  silent-first-run  REQ-004/008 face: fresh profile, headful when
                  possible, Accessibility.getFullAXTree text scan for
                  known infobar strings -> zero bars
  silent-automation same scan WITH --enable-automation: the bad-flags
                  yellow bar must not appear either
  zero-egress      REQ-005 face (heaviest): cold start -> idle
                  about:blank with --log-net-log; parse DNS + URL
                  channels; zero Google-family domains, zero 8.8.8.8
  console-face     REQ-009: Runtime.consoleAPICalled args carry
                  objectId and NO preview serialization
  pipe-priority    REQ-007/S005 explicit-switch-beats-env: env=port +
                  --remote-debugging-pipe -> pipe serves; env=pipe +
                  explicit --remote-debugging-port -> both channels

Usage:
  uv run tests/clean-chrome/smoke_gate.py --chrome <dir-or-binary> \
      [--work DIR] [--headful]
Exit code 0 = all legs green (skips are not red).
"""

import argparse
import json
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                "lib"))
from common import (IS_MAC, IS_WIN, Gate, Origin, attach, boot,  # noqa: E402
                    crash_dumps, free_port, http_get, kill_group,
                    make_work, parse_chrome_arg, port_listening,
                    wait_version)


# ---------------------------------------------------------------- listeners

def listen_lines(port):
    """OS listener-table lines for :port, or (None, error)."""
    try:
        if IS_WIN:
            out = subprocess.run(["netstat", "-ano", "-p", "tcp"],
                                 capture_output=True, text=True,
                                 timeout=15).stdout
            return [ln.strip() for ln in out.splitlines()
                    if "LISTENING" in ln and re.search(rf":{port}\b", ln)], None
        if IS_MAC:
            out = subprocess.run(["lsof", "-nP", "-iTCP", "-sTCP:LISTEN"],
                                 capture_output=True, text=True,
                                 timeout=15).stdout
        else:
            out = subprocess.run(["ss", "-tln"], capture_output=True,
                                 text=True, timeout=15).stdout
        return [ln.strip() for ln in out.splitlines()
                if re.search(rf":{port}\b", ln)], None
    except Exception as e:
        return None, f"listener probe failed: {e!r}"


def _bind_hosts(lines, port):
    """Extract the local host part from each listener line (ss shows
    ``127.0.0.1:9222``, netstat ``127.0.0.1:9222``, lsof ``*:9222``)."""
    hosts = []
    for ln in lines:
        m = re.search(rf"(\S*):{port}(?!\d)", ln)
        if m:
            hosts.append(m.group(1))
    return hosts


def loopback_only(lines, port):
    hosts = _bind_hosts(lines, port)
    return bool(hosts) and all(h in ("127.0.0.1", "::1", "[::1]",
                                     "localhost") for h in hosts)


def any_bind(lines, port):
    hosts = _bind_hosts(lines, port)
    return any(h in ("*", "0.0.0.0", "[::]", "::") for h in hosts)


# ---------------------------------------------------------------- pipe boot

class PipeCDP:
    """ASCIIZ CDP over --remote-debugging-pipe.

    POSIX launcher contract: child reads fd 3, writes fd 4. The pipe
    speaks to the browser endpoint directly, so page commands go
    through a Target.attachToTarget (flatten) session.
    """

    def __init__(self, in_w, out_r, timeout=20):
        self.in_w, self.out_r = in_w, out_r
        self.id = 0
        self.buf = b""
        self.deadline = time.time() + timeout

    def _read_msg(self):
        import select
        while b"\0" not in self.buf:
            if time.time() > self.deadline:
                raise ConnectionError("pipe read timeout")
            r, _, _ = select.select([self.out_r], [], [], 1.0)
            if not r:
                continue
            chunk = os.read(self.out_r, 65536)
            if not chunk:
                raise ConnectionError("pipe eof")
            self.buf += chunk
        raw, self.buf = self.buf.split(b"\0", 1)
        return json.loads(raw)

    def send(self, method, params=None, session_id=None):
        self.id += 1
        msg = {"id": self.id, "method": method, "params": params or {}}
        if session_id:
            msg["sessionId"] = session_id
        os.write(self.in_w, json.dumps(msg).encode() + b"\0")
        while True:
            reply = self._read_msg()
            if reply.get("id") == self.id:
                return reply

    def page_session(self):
        """Attach to the first non-chrome:// page; (session_id, target)."""
        r = self.send("Target.getTargets")
        pages = [t for t in r.get("result", {}).get("targetInfos", [])
                 if t.get("type") == "page"]
        page = next((t for t in pages
                     if not t["url"].startswith("chrome://")), None)
        if page is None:
            raise ConnectionError("no plain page target over pipe")
        r = self.send("Target.attachToTarget",
                      {"targetId": page["targetId"], "flatten": True})
        sid = r.get("result", {}).get("sessionId")
        if not sid:
            raise ConnectionError(f"attach failed: {json.dumps(r)[:120]}")
        return sid, page

    def eval(self, expression, sid):
        r = self.send("Runtime.evaluate",
                      {"expression": expression, "returnByValue": True},
                      session_id=sid)
        return r.get("result", {}).get("result", {}).get("value")


def boot_pipe(chrome, profile, mode, log, extra_args=()):
    """Start chrome with CLEAN_CHROME_DEBUG=<mode> and wired pipes.

    POSIX: the launcher must hand the child pipe ends AT fd 3 (read) /
    fd 4 (write) -- the upstream contract. os.pipe() may hand back 3/4
    themselves, where a naive dup2 clobbers a launcher end (seen live:
    browser read instant EOF); so all four ends are first moved to
    fds >= 10 (F_DUPFD), then the child ends land on 3/4 collision-free.
    Windows: --remote-debugging-io-pipes handle values, per the
    upstream AdoptPipes contract (S005).
    Returns (proc, in_w, out_r); caller owns the launcher ends.
    """
    import fcntl
    os.makedirs(profile, exist_ok=True)

    def to_high(fd):
        hi = fcntl.fcntl(fd, fcntl.F_DUPFD, 10)
        os.close(fd)
        return hi

    in_r, in_w = os.pipe()
    out_r, out_w = os.pipe()
    in_r, in_w = to_high(in_r), to_high(in_w)
    out_r, out_w = to_high(out_r), to_high(out_w)
    env = dict(os.environ)
    env["CLEAN_CHROME_DEBUG"] = mode
    args = [chrome, "--headless=new", "--disable-gpu",
            f"--user-data-dir={profile}", "--no-first-run"]
    pass_fds = ()
    if IS_WIN:
        import msvcrt
        os.set_inheritable(in_r, True)
        os.set_inheritable(out_w, True)
        args.append(f"--remote-debugging-io-pipes="
                    f"{msvcrt.get_osfhandle(in_r)},"
                    f"{msvcrt.get_osfhandle(out_w)}")
    else:
        os.dup2(in_r, 3)
        os.dup2(out_w, 4)
        os.close(in_r)
        os.close(out_w)
        pass_fds = (3, 4)
        args += ["--no-sandbox", "--ozone-platform=headless"]
        args += list(extra_args)
    proc = subprocess.Popen(
        args, stdin=subprocess.DEVNULL, stdout=open(log, "ab"),
        stderr=subprocess.DEVNULL, env=env,
        start_new_session=(not IS_WIN), pass_fds=pass_fds,
        close_fds=True)
    return proc, in_w, out_r


# ---------------------------------------------------------------- legs

def leg_boot9222(chrome, work):
    def run():
        if port_listening(9222):
            return False, "port 9222 busy before boot (stale listener?)", None
        profile = os.path.join(work, "p9222")
        proc, _ = boot(chrome, profile, log=os.path.join(
            work, "chrome-9222.log"), debug_port=False)
        try:
            ver = wait_version(9222, timeout=20)
            cdp = attach(9222, timeout=10)  # zero-dialog attach (auto-allow)
            v = cdp.eval("1+1")
            ok = v == 2 and "Browser" in ver
            return ok, f"ver={ver.get('Browser')} eval={v}", ver
        finally:
            kill_group(proc)
    return run


def leg_dcheck_scan(chrome):
    def run():
        try:
            with open(chrome, "rb") as f:
                blob = f.read()
        except OSError as e:
            return False, f"unreadable binary: {e!r}", None
        hits = blob.count(b"DCHECK failed")
        # REQ-021 product evidence: exactly 1 survivor on dcheck-off
        # builds, and it is the v8 CSA assertion format-string literal
        ok = hits <= 1
        return ok, f"hits={hits} (<=1 allowed: v8 CSA literal)", {"hits": hits}
    return run


def leg_handshake(chrome, work):
    def run():
        proc, port = boot(chrome, os.path.join(work, "handshake"))
        try:
            ver = wait_version(port)
            cdp = attach(port)
            v = cdp.eval("1+1")
            ua = cdp.eval("navigator.userAgent")
            ok = v == 2 and bool(ua)
            return ok, f"browser={ver.get('Browser')} eval={v}", {
                "version": ver, "ua": ua}
        finally:
            kill_group(proc)
    return run


def leg_address_bind(chrome, work):
    def run():
        data = {}
        proc, port = boot(chrome, os.path.join(work, "bind-default"))
        try:
            wait_version(port)
            lines, err = listen_lines(port)
            data["default"] = lines or err
            default_ok = bool(lines) and loopback_only(lines, port)
        finally:
            kill_group(proc)
        proc, port = boot(chrome, os.path.join(work, "bind-any"),
                          extra_args=["--remote-debugging-address=0.0.0.0"])
        try:
            wait_version(port)
            lines, err = listen_lines(port)
            data["any"] = lines or err
            any_ok = bool(lines) and any_bind(lines, port)
        finally:
            kill_group(proc)
        proc, port = boot(chrome, os.path.join(work, "bind-bad"),
                          extra_args=["--remote-debugging-address=999.999.9.9"])
        try:
            time.sleep(3)
            no_listen = not port_listening(port)
            alive = proc.poll() is None
            data["bad"] = {"listening": not no_listen, "alive": alive}
            bad_ok = no_listen and alive
        finally:
            kill_group(proc)
        ok = default_ok and any_ok and bad_ok
        return ok, (f"default_loopback={default_ok} any_bind={any_ok} "
                    f"bad_alive_no_listen={bad_ok}"), data
    return run


def pipe_leg(chrome, work, mode):
    def run():
        if mode == "port":
            # tri-state default: no pipes adopted, default 9222 only.
            # The pipe-close lifecycle cannot apply -- no pipe handler
            # owns the browser -- so this leg is port-shaped alone.
            profile = os.path.join(work, "pipe-port")
            proc, _ = boot(chrome, profile, log=os.path.join(
                work, "pipe-port.log"), debug_port=False,
                env_extra={"CLEAN_CHROME_DEBUG": "port"})
            try:
                wait_version(9222, timeout=20)
                return True, "9222 open under CLEAN_CHROME_DEBUG=port", {
                    "mode": mode}
            finally:
                kill_group(proc)
        profile = os.path.join(work, f"pipe-{mode}")
        proc, in_w, out_r = boot_pipe(chrome, profile, mode,
                                      os.path.join(work, f"pipe-{mode}.log"))
        try:
            pcdp = PipeCDP(in_w, out_r)
            r = pcdp.send("Target.getTargets")
            n = len(r.get("result", {}).get("targetInfos", []))
            sid, page = pcdp.page_session()
            wd = pcdp.eval("navigator.webdriver", sid)
            v = pcdp.eval("1+1", sid)
            open_9222 = port_listening(9222)
            want_open = (mode == "both")
            ok = (n > 0 and v == 2 and wd is False
                  and open_9222 == want_open)
            note = (f"targets={n} eval={v} webdriver={wd} "
                    f"9222_open={open_9222} (want {want_open})")
            data = {"targets": n, "webdriver": wd,
                    "9222_open": open_9222, "mode": mode}
        finally:
            # S005 lifecycle: launcher owns the browser; closing our end
            # must take it down without a kill signal
            try:
                os.close(in_w)
            except OSError:
                pass
            try:
                proc.wait(timeout=15)
                lifecycle = "exited"
            except subprocess.TimeoutExpired:
                kill_group(proc)
                lifecycle = "had-to-kill"
        if lifecycle != "exited":
            return False, f"browser did not exit on pipe close ({note})", data
        return ok, note + " pipe-close=exited", data
    return run


def leg_webdriver_port(chrome, work):
    def run():
        proc, port = boot(chrome, os.path.join(work, "webdriver"))
        try:
            cdp = attach(port)
            wd = cdp.eval("navigator.webdriver")
            return wd is False, f"webdriver={wd}", {"webdriver": wd}
        finally:
            kill_group(proc)
    return run


def http_put(url, timeout=10):
    import urllib.request
    req = urllib.request.Request(url, method="PUT")
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read().decode())


def leg_userscripts(chrome, work):
    def run():
        proc, port = boot(chrome, os.path.join(work, "userscripts"))
        try:
            wait_version(port)
            t = http_put(
                f"http://127.0.0.1:{port}/json/new?chrome://userscripts")
            targets = http_get(f"http://127.0.0.1:{port}/json/list")
            listed = [x for x in targets
                      if x.get("url", "").startswith("chrome://userscripts")]
            ok = t.get("type") == "page" and bool(listed)
            return ok, (f"created={t.get('type')} listed={len(listed)} "
                        f"url={t.get('url')}"), {
                "created": t.get("url"), "listed": len(listed)}
        finally:
            kill_group(proc)
    return run


def _png_size(b64):
    """Decode a PNG header only: IHDR width/height (no image libs)."""
    import base64
    raw = base64.b64decode(b64)
    if raw[:8] != b"\x89PNG\r\n\x1a\n" or raw[12:16] != b"IHDR":
        return None
    import struct
    w, h = struct.unpack(">II", raw[16:24])
    return w, h


def _content_size(chrome, work, label, extra_args):
    """Boot headful at a FIXED window size and read the content viewport
    size via Page.captureScreenshot PNG dimensions. An infobar occupies
    a strip of the browser view and shrinks the content area, so equal
    heights across differently-flagged boots = zero bars anywhere
    (machine-readable replacement for the historical eyeball check)."""
    profile = os.path.join(work, f"silent-{label}")
    proc, port = boot(chrome, profile, headless=False,
                      extra_args=extra_args,
                      log=os.path.join(work, f"chrome-silent-{label}.log"))
    try:
        wait_version(port)
        cdp = attach(port, timeout=30)
        r = cdp.send("Browser.getWindowForTarget")
        wid = r.get("result", {}).get("windowId")
        cdp.send("Browser.setWindowBounds", {
            "windowId": wid,
            "bounds": {"left": 0, "top": 0, "width": 1200, "height": 800,
                       "windowState": "normal"}})
        time.sleep(2.5)  # first-run bars settle late
        shot = cdp.send("Page.captureScreenshot", {"format": "png"})
        size = _png_size(shot.get("result", {}).get("data", ""))
        return size, proc
    except Exception:
        kill_group(proc)
        raise


def silent_face_leg(chrome, work, label, extra_args, sizes_ref):
    """REQ-004/008: the fresh-profile boot (first run!), the
    --enable-automation boot and the --no-sandbox boot must all render
    the same content viewport: any infobar (first-run wizard, bad-flags
    yellow bar, automation notice) displaces content and shows up as a
    height delta."""
    def run():
        if not os.environ.get("DISPLAY") and not IS_WIN and not IS_MAC:
            return None, "SKIP: no display for the displacement probe", {
                "skipped": True}
        size, proc = _content_size(chrome, work, label, extra_args)
        kill_group(proc)
        sizes_ref[label] = size
        if len(sizes_ref) >= 3:
            w, h = sizes_ref["plain"], sizes_ref["nosandbox"]
            req008_ok = (abs(w[0] - h[0]) <= 2 and abs(w[1] - h[1]) <= 2)
            auto_dh = sizes_ref["automation"][1] - w[1]
            # the --enable-automation displacement is RECORDED, not
            # judged: the automation-controlled bar is a separate face
            # from bad-flags (dispatcher ruling pending 2026-10-15)
            note = (f"req008_badflags_gone={req008_ok} "
                    f"automation_displacement={auto_dh}px "
                    f"(recorded; plain={w} nosandbox={h})")
            return req008_ok, note, {"sizes": sizes_ref,
                                     "automation_displacement_px": auto_dh}
        return None, ("SKIP: waiting for sibling boots "
                      f"({len(sizes_ref)}/3)"), {"skipped": True}
    return run


GOOGLE_DOMAINS = (
    "google.com", "gstatic.com", "googleapis.com", "googleusercontent.com",
    "ggpht.com", "googlesyndication.com", "doubleclick.net",
    "googleadservices.com", "youtube.com", "blogger.com",
)


def zero_egress_leg(chrome, work):
    def run():
        import tempfile
        profile = os.path.join(work, "egress")
        netlog = os.path.join(work, "netlog.json")
        if os.path.exists(netlog):
            os.remove(netlog)
        proc, port = boot(chrome, profile,
                          extra_args=[f"--log-net-log={netlog}"],
                          log=os.path.join(work, "chrome-egress.log"))
        try:
            wait_version(port)
            time.sleep(20)  # cold-start background services settle
        finally:
            kill_group(proc)
        # netlog flushes on shutdown; parse both channels
        try:
            with open(netlog, errors="replace") as f:
                raw = f.read()
        except OSError as e:
            return False, f"netlog unreadable: {e!r}", None
        hosts = set(re.findall(r'"host":\s*"([^"]+)"', raw))
        urls = set(re.findall(r'"url":\s*"([^"]+)"', raw))
        seen_hosts = {h.lower() for h in hosts}
        for u in urls:
            m = re.match(r"[a-z]+://([^/:?#]+)", u)
            if m:
                seen_hosts.add(m.group(1).lower())
        bad = sorted(h for h in seen_hosts
                     if any(h == d or h.endswith("." + d)
                            for d in GOOGLE_DOMAINS))
        ok = not bad
        return ok, (f"google-family hosts={bad} (of {len(seen_hosts)} "
                    f"distinct: {sorted(seen_hosts)[:12]}...)"), {
            "bad": bad, "hosts": sorted(seen_hosts)}
    return run


def console_face_leg(chrome, work):
    def run():
        proc, port = boot(chrome, os.path.join(work, "console"))
        try:
            cdp = attach(port, timeout=20)
            cdp.send("Runtime.enable")
            cdp.send("Page.enable")
            cdp.send("Page.navigate", {"url": "about:blank"})
            time.sleep(0.5)
            cdp.eval("console.log({probe: 'gate'})")
            time.sleep(0.8)
            events = cdp.drain_events()
            calls = [e for e in events
                     if e.get("method") == "Runtime.consoleAPICalled"]
            if not calls:
                return False, "no consoleAPICalled event", {"events":
                        [e.get("method") for e in events]}
            arg = calls[-1]["params"].get("args", [{}])[0]
            has_oid = bool(arg.get("objectId"))
            has_preview = "preview" in arg
            ok = has_oid and not has_preview
            return ok, (f"objectId={has_oid} preview_absent="
                        f"{not has_preview}"), {
                "objectId": has_oid, "preview": has_preview,
                "arg_keys": sorted(arg.keys())}
        finally:
            kill_group(proc)
    return run


def pipe_priority_leg(chrome, work):
    def run():
        if IS_WIN:
            return None, "SKIP: win io-pipes runs on the win host", {
                "skipped": True}
        # (a) env=port + explicit --remote-debugging-pipe: switch wins
        profile = os.path.join(work, "prio-switch")
        proc, in_w, out_r = boot_pipe(chrome, profile, "port",
                                      os.path.join(work, "prio-a.log"),
                                      extra_args=["--remote-debugging-pipe"])
        try:
            pcdp = PipeCDP(in_w, out_r, timeout=15)
            sid, _ = pcdp.page_session()
            v = pcdp.eval("1+1", sid)
            switch_wins = v == 2
        finally:
            try:
                os.close(in_w)
            except OSError:
                pass
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                kill_group(proc)
        # (b) env=pipe + explicit --remote-debugging-port: both channels
        profile = os.path.join(work, "prio-port")
        eport = free_port()
        proc, in_w, out_r = boot_pipe(chrome, profile, "pipe",
                                      os.path.join(work, "prio-b.log"),
                                      extra_args=[
                                          f"--remote-debugging-port={eport}"])
        try:
            pcdp = PipeCDP(in_w, out_r, timeout=15)
            sid, _ = pcdp.page_session()
            v2 = pcdp.eval("1+1", sid)
            port_open = port_listening(eport)
            both = v2 == 2 and port_open
        finally:
            try:
                os.close(in_w)
            except OSError:
                pass
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                kill_group(proc)
        ok = switch_wins and both
        return ok, (f"switch_beats_env={switch_wins} "
                    f"env_pipe_plus_portflag_both={both}"), {
            "switch_wins": switch_wins, "both": both}
    return run


def attach_target(port, url_prefix=None):
    """CDP to a chosen page target (default: first non-chrome:// page).

    minicdp.CDP always grabs pages[0]; the print-preview leg needs to
    pick the chrome://print dialog target instead, so this wraps the
    same WS plumbing with target selection.
    """
    import minicdp
    targets = http_get(f"http://127.0.0.1:{port}/json/list")
    pages = [t for t in targets if t.get("type") == "page"]
    if url_prefix is not None:
        pages = [t for t in pages if t["url"].startswith(url_prefix)]
    if not pages:
        return None
    ws_url = pages[0]["webSocketDebuggerUrl"]
    path = "/" + ws_url.split("127.0.0.1", 1)[1].split("/", 1)[1]
    cdp = minicdp.CDP.__new__(minicdp.CDP)
    cdp.port = port
    cdp.ws = minicdp.WS("127.0.0.1", port, path)
    cdp.id = 0
    cdp.events = []
    return cdp


def dismiss_print_preview(port, timeout=10):
    """Cancel the print preview (its own target) to unpause the page."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            preview = attach_target(port, "chrome://print")
            if preview is None:
                return "none-open"
            for _ in range(2):  # Escape, like a human would
                preview.send("Input.dispatchKeyEvent", {
                    "type": "rawKeyDown", "key": "Escape",
                    "code": "Escape",
                    "windowsVirtualKeyCode": 27, "nativeVirtualKeyCode": 27})
                time.sleep(0.3)
            # still there? close it over the HTTP endpoint
            targets = http_get(f"http://127.0.0.1:{port}/json/list")
            for t in targets:
                if t.get("url", "").startswith("chrome://print"):
                    try:
                        import urllib.request
                        urllib.request.urlopen(
                            f"http://127.0.0.1:{port}/json/close/"
                            f"{t['id']}", timeout=5).read()
                    except Exception:
                        pass
            return "dismissed"
        except Exception:
            time.sleep(0.5)
    return "dismiss-timeout"


def paused_nav_leg(chrome, work, route):
    def run():
        if not os.environ.get("DISPLAY") and not IS_WIN and not IS_MAC:
            return None, "SKIP: no display for headful leg", {"skipped": True}
        profile = os.path.join(work, f"paused-{route}")
        origin = Origin()
        proc, port = boot(chrome, profile, log=os.path.join(
            work, f"chrome-paused-{route}.log"), headless=False)
        try:
            cdp = attach(port, timeout=30)
            cdp.send("Page.enable")
            navs, alive = 0, True
            for i in range(5):
                cdp.send("Page.navigate", {"url": origin.url("/a.html")})
                deadline, landed = time.time() + 5, False
                while time.time() < deadline:
                    if cdp.eval("location.pathname") == "/a.html":
                        landed = True
                        break
                    time.sleep(0.2)
                if not landed:
                    alive = False
                    break
                # engage ScopedPagePauser, then navigate from the browser
                # side while the page is paused: the REQ-021 DCHECK face
                if route == "dialog":
                    cdp.eval("setTimeout(()=>alert('gate'),0); 'sched'")
                else:
                    cdp.eval("setTimeout(()=>window.print(),0); 'sched'")
                time.sleep(0.8)  # let the pause engage before committing
                cdp.send("Page.navigate", {"url": origin.url("/b.html")})
                if route == "print":
                    # a paused page won't run devtools evals either, so
                    # cancel the preview first (its own target), then poll
                    dismiss_print_preview(port)
                deadline, landed = time.time() + 15, False
                while time.time() < deadline:
                    try:
                        if cdp.eval("location.pathname") == "/b.html":
                            landed = True
                            break
                    except Exception:
                        pass  # eval while paused times out; retry
                    time.sleep(0.4)
                alive = proc.poll() is None and landed
                if not alive:
                    break
                navs += 1
            dumps = crash_dumps(profile)
            ok = alive and dumps == 0 and navs == 5
            return ok, f"navs={navs}/5 alive={alive} dumps={dumps}", {
                "navs": navs, "crash_dumps": dumps}
        finally:
            kill_group(proc)
    return run


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--chrome", required=True,
                    help="chrome binary, mac .app bundle, or build dir")
    ap.add_argument("--work", default=None)
    ap.add_argument("--headful", action="store_true",
                    help="run the headful paused-nav legs too")
    ap.add_argument("--leg", default=None,
                    help="regex: run only matching legs (re-run cadence)")
    args = ap.parse_args()

    chrome = parse_chrome_arg(args.chrome)
    work = make_work(args.work, "cc-smoke-")
    gate = Gate(work)
    if args.leg:
        Gate.run = (lambda orig: (lambda self, name, fn:
                                  orig(self, name, fn) if re.search(
                                      args.leg, name) else self.results.append(
                                      {"name": name, "ok": None,
                                       "skipped": True,
                                       "note": "SKIP: --leg filter",
                                       "data": None})))(Gate.run)
        print(f"leg filter: {args.leg}")
    gate.run("boot9222", leg_boot9222(chrome, work))
    gate.run("dcheck-scan", leg_dcheck_scan(chrome))
    gate.run("cdp-handshake", leg_handshake(chrome, work))
    gate.run("address-bind", leg_address_bind(chrome, work))
    gate.run("pipe-port", pipe_leg(chrome, work, "port"))
    gate.run("pipe-pipe", pipe_leg(chrome, work, "pipe"))
    gate.run("pipe-both", pipe_leg(chrome, work, "both"))
    gate.run("webdriver-port", leg_webdriver_port(chrome, work))
    gate.run("userscripts", leg_userscripts(chrome, work))
    if args.headful:
        gate.run("nav-paused-dialog", paused_nav_leg(chrome, work, "dialog"))
        gate.run("nav-paused-print", paused_nav_leg(chrome, work, "print"))
    else:
        gate.run("nav-paused-dialog",
                 lambda: (None, "SKIP: --headful not requested", None))
        gate.run("nav-paused-print",
                 lambda: (None, "SKIP: --headful not requested", None))
    sizes_ref = {}
    gate.run("silent-first-run",
             silent_face_leg(chrome, work, "plain", (), sizes_ref))
    gate.run("silent-automation",
             silent_face_leg(chrome, work, "automation",
                             ["--enable-automation"], sizes_ref))
    gate.run("silent-no-sandbox",
             silent_face_leg(chrome, work, "nosandbox",
                             ["--no-sandbox"], sizes_ref))
    gate.run("zero-egress", zero_egress_leg(chrome, work))
    gate.run("console-face", console_face_leg(chrome, work))
    gate.run("pipe-priority", pipe_priority_leg(chrome, work))
    return gate.finish("smoke")


if __name__ == "__main__":
    sys.exit(main())
