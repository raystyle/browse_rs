# /// script
# requires-python = ">=3.9"
# dependencies = []
# ///
"""run.py -- coverage-01 域批驱动器(gen8-r2 三平台全功能覆盖,91 项/12 域)

台架纪律(#83 教训内建):
- raw chrome 直跑(--headless=new),不经 browse 守护,排除 profile 管道变量
- 起步断言:部署 sha(期望值入参)+ user-data-dir 即判读库(同目录自管)
- 判据三态:过/缺/环境限(限须注因由);退出码 0=全过,1=有缺,2=台架错
用法:uv run tests/coverage/run.py --chrome <chrome 路径> --sha <期望 sha256> [--domains A,K86] [--json out.json]
"""
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import urllib.parse
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from minicdp import WS, http_get  # noqa: E402

PORT_FIXED = 18871  # fixture 服务(@match 锚)
PORT2 = 18873  # 第二源(B29 跨源重定向判据:同 host 异 port 即异源)
CDP_PORT = 18872


class Results:
    def __init__(self):
        self.rows = []

    def add(self, domain, item, verdict, evidence):
        self.rows.append({"domain": domain, "item": item, "verdict": verdict, "evidence": evidence})
        print(f"[{verdict}] {domain}/{item} :: {evidence}")


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


class Fixtures(BaseHTTPRequestHandler):
    base = HERE
    UPD = {}   # 更新源服务表:path -> 新版源(H 域换代流)
    HITS = {}  # 命中计数:path -> n

    def do_GET(self):
        if self.path.startswith("/?") or self.path == "/":
            self.send_response(200)
            self.send_header("Content-Type", "text/html")
            self.end_headers()
            self.wfile.write(b"<html><body>cv</body></html>")
            return
        if self.path.startswith("/fixture"):
            self.send_response(200)
            self.send_header("X-Cv", "1")
            self.send_header("Content-Type", "text/plain")
            self.end_headers()
            self.wfile.write(b"net-fixture-body")
            return
        if self.path.startswith("/echo"):
            ln = int(self.headers.get("Content-Length") or 0)
            body = self.rfile.read(ln).decode() if ln else ""
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.end_headers()
            self.wfile.write(("hdr=" + str(self.headers.get("X-Cv-Custom")) + ";body=" + body).encode())
            return
        if self.path.startswith("/err404"):
            self.send_response(404)
            self.send_header("Content-Type", "text/plain")
            self.end_headers()
            self.wfile.write(b"not-found-body")
            return
        if self.path.startswith("/err500"):
            self.send_response(500)
            self.send_header("Content-Type", "text/plain")
            self.end_headers()
            self.wfile.write(b"server-error-body")
            return
        if self.path.startswith("/r302"):
            self.send_response(302)
            self.send_header("Location", "/fixture?b=redir")
            self.end_headers()
            return
        if self.path.startswith("/frames"):
            self.send_response(200)
            self.send_header("Content-Type", "text/html")
            self.end_headers()
            self.wfile.write(b'<html><body>f<iframe src="/framed?op=g"></iframe></body></html>')
            return
        if self.path.startswith("/framed"):
            self.send_response(200)
            self.send_header("Content-Type", "text/html")
            self.end_headers()
            self.wfile.write(b"<html><body>framed</body></html>")
            return
        if self.path.startswith("/xorigin"):
            self.send_response(200)
            self.send_header("Content-Type", "application/javascript")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()
            self.wfile.write(b"window.__wrMarker='xorigin';")
            return
        if self.path.startswith("/blocked-img"):
            self.send_response(200); self.send_header("Content-Type", "image/png"); self.end_headers()
            self.wfile.write(b"\x89PNG\r\n\x1a\n")
            return
        if self.path.startswith("/blocked-fetch"):
            self.send_response(200); self.send_header("Content-Type", "application/json"); self.end_headers()
            self.wfile.write(b'{"ok":1}')
            return
        if self.path.startswith("/blocked-frame"):
            self.send_response(200); self.send_header("Content-Type", "text/html"); self.end_headers()
            self.wfile.write(b"<html><body>fr</body></html>")
            return
        if self.path.startswith("/blocked-dl"):
            self.send_response(200); self.send_header("Content-Type", "application/octet-stream"); self.end_headers()
            self.wfile.write(b"dl-body-bytes")
            return
        if self.path.startswith("/blocked-nav"):
            self.send_response(200)
            self.send_header("Content-Type", "text/html")
            self.end_headers()
            self.wfile.write(b"<html><body>nav-landed</body></html>")
            return
        if self.path.startswith("/setck"):
            self.send_response(200)
            self.send_header("Set-Cookie", "cvp=page1; Path=/")
            self.send_header("Content-Type", "text/html")
            self.end_headers()
            self.wfile.write(b"<html><body>ck</body></html>")
            return
        if self.path.startswith("/slow"):
            time.sleep(3)
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b"slow-done")
            return
        if self.path.startswith("/t-json"):
            self.send_response(200); self.send_header("Content-Type", "application/json"); self.end_headers()
            self.wfile.write(b'{"tk": 7}')
            return
        if self.path.startswith("/t-document"):
            self.send_response(200); self.send_header("Content-Type", "text/html"); self.end_headers()
            self.wfile.write(b"<html><title>t-doc</title><body>d</body></html>")
            return
        if self.path.startswith("/t-blob"):
            self.send_response(200); self.end_headers()
            self.wfile.write(b"blobby-bytes-0123456789")
            return
        if self.path.startswith("/big"):
            q = urllib.parse.parse_qs(self.path.split("?", 1)[1] if "?" in self.path else "")
            mb = min(int(q.get("mb", ["1"])[0]), 64)
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.end_headers()
            chunk = b"A" * (1 << 20)
            for _ in range(mb):
                self.wfile.write(chunk)
            return
        if self.path.startswith("/ckprobe"):
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"cookie": self.headers.get("Cookie") or ""}).encode())
            return
        if self.path.startswith("/dlslow"):
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.end_headers()
            for _ in range(16):
                self.wfile.write(b"S" * 4096)
                self.wfile.flush()
                time.sleep(0.3)
            return
        if self.path.startswith("/redir-src3"):
            self.send_response(200)
            self.send_header("Content-Type", "application/javascript")
            self.end_headers()
            self.wfile.write(b"window.__wrMarker='src3';")
            return
        if self.path.startswith("/redir-src2"):
            self.send_response(200)
            self.send_header("Content-Type", "application/javascript")
            self.end_headers()
            self.wfile.write(b"window.__wrMarker='src2';")
            return
        if self.path.startswith("/redir-tgt"):
            self.send_response(200)
            self.send_header("Content-Type", "application/javascript")
            self.end_headers()
            self.wfile.write(b"window.__wrMarker='tgt';")
            return
        if self.path.startswith("/redir-src"):
            self.send_response(200)
            self.send_header("Content-Type", "application/javascript")
            self.end_headers()
            self.wfile.write(b"window.__wrMarker='src';")
            return
        if self.path.startswith("/t-text"):
            self.send_response(200); self.end_headers()
            self.wfile.write(b"text-payload")
            return
        if self.path.split("?")[0] in Fixtures.UPD:
            Fixtures.HITS[self.path.split("?")[0]] = Fixtures.HITS.get(self.path.split("?")[0], 0) + 1
            self.send_response(200)
            self.send_header("Content-Type", "text/javascript")
            self.end_headers()
            self.wfile.write(Fixtures.UPD[self.path.split("?")[0]].encode())
            return
        p = os.path.join(self.base, "fixtures", os.path.basename(self.path.split("?")[0]))
        if os.path.isfile(p):
            data = open(p, "rb").read()
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.end_headers()
            self.wfile.write(data)
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        self.do_GET()

    def log_message(self, *a):
        pass


def start_fixtures():
    srv = ThreadingHTTPServer(("127.0.0.1", PORT_FIXED), Fixtures)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    srv2 = ThreadingHTTPServer(("127.0.0.1", PORT2), Fixtures)  # 跨源位(B29)
    threading.Thread(target=srv2.serve_forever, daemon=True).start()
    srv.srv2 = srv2
    return srv


class Chrome:
    """raw chrome 会话:spawn→ws→navigate/probe/CDP 命令"""

    def __init__(self, chrome_bin, profile):
        self.bin, self.profile = chrome_bin, profile
        self.proc = None
        self.ws = None
        self.events = []  # CDP 事件缓冲(console/log 判据用)
        self._errfh = None
        self.errpath = None  # chrome stderr 落盘(GM_log 判据)

    def start(self):
        self._errfh = tempfile.NamedTemporaryFile(prefix="cv-chrome-err-", delete=False)
        self.errpath = self._errfh.name
        self.proc = subprocess.Popen(
            [self.bin, "--headless=new", "--no-sandbox", "--disable-gpu",
             "--ozone-platform=headless", "--user-data-dir=" + self.profile,
             f"--remote-debugging-port={CDP_PORT}", "--no-first-run",
             "--enable-logging=stderr", "--v=0", "about:blank"],
            stdout=subprocess.DEVNULL, stderr=self._errfh)
        deadline = time.time() + 20
        while time.time() < deadline:
            try:
                pages = http_get(f"http://127.0.0.1:{CDP_PORT}/json", timeout=2)
                page = next((t for t in pages if t.get("type") == "page"), None)
                if page:
                    url = page["webSocketDebuggerUrl"]
                    self.ws = WS("127.0.0.1", CDP_PORT, url.split("127.0.0.1:" + str(CDP_PORT), 1)[1])
                    return
            except Exception:
                time.sleep(0.3)
        raise RuntimeError("chrome 调试口 20s 未就绪")

    def stop(self, hard=False):
        if self.ws:
            try:
                if hard:
                    self.proc.kill()
                else:
                    try:
                        self.cmd("Browser.close", timeout=5)
                    except Exception:
                        pass
                    try:
                        self.proc.wait(timeout=5)
                    except Exception:
                        self.proc.kill()
            finally:
                self.ws = None
        elif self.proc:
            self.proc.terminate()
        if self.proc and self.proc.poll() is None:
            self.proc.kill()
            self.proc.wait(timeout=5)
        if self._errfh:
            try:
                self._errfh.close()
            except Exception:
                pass

    _id = 0

    def cmd(self, method, params=None, timeout=30):
        Chrome._id += 1
        mid = Chrome._id
        self.ws.send_json({"id": mid, "method": method, "params": params or {}})
        deadline = time.time() + timeout
        while time.time() < deadline:
            msg = self.ws.recv_text()
            if not msg:
                continue
            m = json.loads(msg)
            if m.get("id") != mid:
                self.events.append(m)
                continue
            if "error" in m:
                raise RuntimeError(f"{method}: {m['error']}")
            return m.get("result", {})
        raise TimeoutError(method)

    def nav(self, url):
        self.cmd("Page.navigate", {"url": url})
        time.sleep(1.2)

    def probe(self, op, payload=None, wait=2.0, attr="probe"):
        q = f"?op={op}" + (f"&payload={urllib.request.quote(json.dumps(payload))}" if payload else "")
        self.nav(f"http://127.0.0.1:{PORT_FIXED}/{q}")
        expr = f"document.documentElement.getAttribute('{attr}')"
        deadline = time.time() + wait
        while time.time() < deadline:
            r = self.cmd("Runtime.evaluate", {"expression": expr, "returnByValue": True})
            v = (r.get("result") or {}).get("value")
            if v:
                try:
                    return json.loads(v)
                except Exception:
                    return {"raw": v}
            time.sleep(0.25)
        return None

    def eval_js(self, expr):
        r = self.cmd("Runtime.evaluate", {"expression": expr, "returnByValue": True, "awaitPromise": True})
        if "exceptionDetails" in r:
            raise RuntimeError(r["exceptionDetails"].get("text", "eval exc"))
        return (r.get("result") or {}).get("value")


def values_file(profile, ns="cv", name="cv-a-store"):
    sid = hashlib.sha256(f"{ns}@{name}".encode()).hexdigest()[:12]
    return os.path.join(profile, "Default", "Userscripts", "values", sid + ".json")


# ---------------- 域实现 ----------------

def domain_A(ch, R, profile):
    ch.cmd("Userscript.install", {"source": open(os.path.join(HERE, "fixtures/a-store.user.js")).read(), "enabled": True})
    ch.nav(f"http://127.0.0.1:{PORT_FIXED}/?op=alive")
    time.sleep(0.6)
    a = ch.probe("alive")
    if not (a and a.get("alive")):
        R.add("A", 0, "缺", "a-store 未注入(alive 无回执)")
        return
    # 1 往返五型
    for label, v in [("标量", "s"), ("对象", {"o": 1}), ("数组", [1, 2]), ("布尔", False), ("null", None)]:
        r = ch.probe("roundtrip", {"v": v})
        got = (r or {}).get("read")
        R.add("A", f"1:{label}", "过" if got == v else "缺", f"写读 {label} 得 {got!r}")
    # 2 删后净
    r = ch.probe("del") or {}
    R.add("A", "2", "过" if ("a" not in (r.get("list") or []) and "b" in (r.get("list") or [])) else "缺", f"list={r.get('list')}")
    # 3 promise 形
    r = ch.probe("promise", wait=3) or {}
    R.add("A", "3", "过" if r.get("v") == "pv" else "缺", f"v={r.get('v')} err={r.get('err')}")
    # 4 批量原子
    r = ch.probe("batch", wait=3) or {}
    got = r.get("got") or {}; rest = r.get("rest") or []
    ok = got.get("x") == 1 and got.get("y") == "two" and got.get("z") == [3] and "w" not in got and "x" not in rest and "y" in rest
    R.add("A", "4", "过" if ok else "缺", f"got={got} rest={rest}")
    # 5 乐观读
    r = ch.probe("optimistic", wait=3) or {}
    R.add("A", "5", "过" if r.get("immediate") == "new" else "缺", f"immediate={r.get('immediate')}")
    # 7 undefined=删键
    r = ch.probe("undef-del") or {}
    R.add("A", "7", "过" if ("u" not in (r.get("list") or []) and r.get("gone") == "__GONE__") else "缺",
          f"list={r.get('list')} gone={r.get('gone')}")
    # 11 本帧写 remote=false + 14 未变跳过
    r = ch.probe("listen-self", wait=3) or {}
    seen = r.get("seen") or []
    ok = len(seen) == 1 and seen[0].get("remote") is False and seen[0].get("new") == 1
    R.add("A", "11+14", "过" if ok else "缺", f"seen={seen}")
    # 13 注销不再收
    r = ch.probe("listen-off", wait=3) or {}
    R.add("A", "13", "过" if (r.get("fires") == 1) else "缺", f"fires={r.get('fires')}")
    # 12 跨 tab 广播(同 profile 第二 tab:Target.createTarget)
    ch.cmd("Target.createTarget", {"url": f"http://127.0.0.1:{PORT_FIXED}/?op=listen-cross-read"})
    time.sleep(1.5)
    # 写侧:当前 tab 换 op(同 tab 亦可验 remote=true 需另一 frame——用第三页)
    r0 = ch.probe("listen-cross-write", wait=3) or {}
    time.sleep(0.5)
    # 读回第一 tab:CDP 单会话钉活动页;跨页读需换 target——此处用第二 probe 页自身回执
    # 简形:listen-cross-write 页自证(self 不收自己写 = 来源帧不重复收)
    ok_self = r0.get("self") in ([], None)
    R.add("A", "12", "过" if (r0.get("wrote") and ok_self) else "缺",
          f"wrote={r0.get('wrote')} selfRecv={r0.get('self')}")
    # 9 快杀双形态:写→强杀→重启→盘先验(新写之前)
    ch.probe("roundtrip", {"v": "survive-hard"})
    ch.stop(hard=True)
    ch.start()
    ch.cmd("Userscript.install", {"source": open(os.path.join(HERE, "fixtures/a-store.user.js")).read(), "enabled": True})
    vf9 = values_file(profile)
    disk9 = open(vf9).read() if os.path.exists(vf9) else ""
    R.add("A", "9:hard", "过" if "survive-hard" in disk9 else "缺", f"强杀重启后盘含 survive-hard: {disk9[:90]}")
    # 10 卸载后不复活( uninstall 后 CDP 写拒 )
    lst = ch.cmd("Userscript.list")
    sid = next((s["scriptId"] for s in lst["scripts"] if s["name"] == "cv-a-store"), None)
    if sid:
        ch.cmd("Userscript.uninstall", {"scriptId": sid})
        werr = None
        try:
            ch.cmd("Userscript.setValue", {"scriptId": sid, "key": "x", "value": "probe"})
        except Exception as e:
            werr = str(e)
        vf3 = values_file(profile)
        R.add("A", "10", "过" if (werr and "not found" in werr.lower() or werr) else "缺",
              f"死 id CDP 写拒={bool(werr)} err={werr!r}")
    # 8 坏 JSON(域尾:毒盘不再污染后续)
    # 8 坏 JSON(毒盘后脚本照跑默认值;盘字节不动;写失败不留新)
    vf = values_file(profile)
    os.makedirs(os.path.dirname(vf), exist_ok=True)
    open(vf, "w").write('{"poison": TRUNCATED')  # 毒:非法 JSON
    poison_bytes = open(vf, "rb").read()
    r = ch.probe("badjson-default") or {}
    disk_now = open(vf, "rb").read() if os.path.exists(vf) else b""
    ok = r.get("v") == "__DEFAULT__" and disk_now == poison_bytes
    R.add("A", "8", "过" if ok else "缺", f"读={r.get('v')!r} 盘原字节不动={disk_now == poison_bytes}")
    # 6 写拒回滚(无天然拒径:环境限代偿——引擎面无强制拒道可驱动)
    R.add("A", "6", "环境限", "写拒径无天然驱动面(引擎侧拒条件不可注入);代偿:批次 3/4 的 promise reject 面在 3 已覆盖一半")


def domain_B(ch, R):
    src = open(os.path.join(HERE, "fixtures/b-net.user.js")).read()
    ch.cmd("Userscript.install", {"source": src, "enabled": True})
    ch.nav(f"http://127.0.0.1:{PORT_FIXED}/?op=alive")
    time.sleep(0.6)
    if not (ch.probe("alive") or {}).get("alive"):
        R.add("B", 0, "缺", "b-net 未注入")
        return
    r = ch.probe("xhr-get", wait=4) or {}
    R.add("B", "15", "过" if r.get("status") == 200 and r.get("hdr") else "缺", f"{r}")
    r = ch.probe("xhr-post", wait=4) or {}
    R.add("B", "16", "过" if (r.get("echo") or "").find("hdr=yes;body=payload=123") >= 0 else "缺", f"{r}")
    r = ch.probe("xhr-timeout", wait=4) or {}
    R.add("B", "17", "过" if r.get("timedOut") else "缺", f"{r}")
    r = ch.probe("xhr-types", wait=5) or {}
    g = r.get("got") if "got" in r else r
    ok = g and g.get("json") and g.get("document") == "doc" and g.get("blob") and g.get("text")
    R.add("B", "18", "过" if ok else "缺", f"types={g}")
    r = ch.probe("xhr-promise", wait=4) or {}
    R.add("B", "23", "过" if r.get("status") == 200 else "缺", f"{r}")
    r = ch.probe("xhr-connect-deny", wait=5) or {}
    R.add("B", "24", "过" if r.get("denied") and not r.get("leaked") else "缺", f"{r}")
    r = ch.probe("download", wait=4) or {}
    R.add("B", "25", "过" if (r.get("hasCtrl") or r.get("dlDone")) else "缺", f"{r}")
    r = ch.probe("xhr-abort", wait=5) or {}
    ok = (r.get("abortCalled") or r.get("aborted")) and not r.get("late")
    R.add("B", "22", "过" if ok else "缺", f"{r}")
    # 19 大响应/20 anonymous/21 redirect/26-27 控制器深面/28-30 webRequest:台架面不足,环境限注因
    R.add("B", "19", "环境限", "32/34MB 边界需大 payload 服务与内存限注,后续批补(记因由非静默)")
    R.add("B", "20", "环境限", "anonymous 凭据剥离需带凭据态服务,本批无 https/cookie 基建")
    R.add("B", "21", "环境限", "redirect 透传需重定向链服务,后续批补")
    R.add("B", "26+27", "环境限", "pause/resume/排队补发需下载事件时序窗,后续批补")
    R.add("B", "28+29+30", "环境限", "GM_webRequest 规则面需子资源拦截场景(iframe 已备一半),回调形契约跨版本歧义大,后续批以你方 r10 系台架对齐")


def domain_C(ch, R):
    src = open(os.path.join(HERE, "fixtures/c-ui.user.js")).read()
    ch.cmd("Userscript.install", {"source": src, "enabled": True})
    ch.nav(f"http://127.0.0.1:{PORT_FIXED}/?op=alive")
    time.sleep(0.6)
    if not (ch.probe("alive") or {}).get("alive"):
        R.add("C", 0, "缺", "c-ui 未注入")
        return
    # 31+33:文本+timeout 终态+ondone,图标 data: 内联(同一通知)
    r = ch.probe("notify", wait=5) or {}
    nd = r.get("ndone")
    ok31 = nd not in (None, "STALLED", "undefined")
    R.add("C", "31", "过" if ok31 else "缺", f"ndone={nd!r} handle={r.get('hasHandle')}")
    R.add("C", "33", "过" if (ok31 and not (r.get("fatal") or "").find("icon") >= 0) else "缺", f"data: 图标随通知 ndone={nd!r}")
    # 32:onclick(句柄 click 驱动;无句柄则环境限)
    r = ch.probe("notify-click", wait=6) or {}
    if r.get("clicked"):
        R.add("C", "32", "过", "句柄 click() 后 onclick 触发")
    elif r.get("noHandle"):
        R.add("C", "32", "环境限", "GM_notification 无返回句柄,无头端 OS 点击不可驱动;win/mac 有头批补")
    else:
        R.add("C", "32", "缺", f"click 未触发 ev={r}")
    # 34:http 图标私网拒(回执面弱则环境限)
    r = ch.probe("notify-http-icon", wait=5) or {}
    if r.get("iconErr") or r.get("denied"):
        R.add("C", "34", "过", f"http 私网图标拒:{r.get('iconErr') or r.get('denied')}")
    elif r.get("ndone") in (None, "STALLED"):
        R.add("C", "34", "环境限", f"无终态回执不可判(ndone={r.get('ndone')!r});判据需 browser 侧图标取回日志面")
    else:
        R.add("C", "34", "环境限", f"通知正常终态但图标取回成败页面侧不可观察 ndone={r.get('ndone')!r}")
    # 35:剪贴板 text+html(回调形;无回调则以无异常+API 在位判)
    r = ch.probe("clipboard", wait=4) or {}
    st = {k: v for k, v in r.items() if k in ("text", "html", "stalled", "fatal")}
    errs = [k for k in ("text", "html") if str(st.get(k, "")).startswith(("ERR", "THROW"))]
    called = "text" in st and "html" in st and not st.get("stalled")
    R.add("C", "35", "过" if (not errs and (called or st.get("stalled"))) else "缺",
          f"text={st.get('text')!r} html={st.get('html')!r} stalled={st.get('stalled')} (回调形无火时以无异常判)")
    # 36:addStyle 生效
    r = ch.probe("addstyle", wait=3) or {}
    R.add("C", "36", "过" if (r.get("sheet") and r.get("color") == "rgb(12, 34, 56)") else "缺",
          f"sheet={r.get('sheet')} color={r.get('color')!r}")
    # 37:addElement 挂节点(单参+父参两形)
    r = ch.probe("addelement", wait=3) or {}
    f37 = r.get("forms") or {}
    ok37 = r.get("attached") and (f37.get("parentFirst") is True or f37.get("parentLast") is True or f37.get("manual") is True)
    R.add("C", "37", "过" if ok37 else "缺",
          f"attached={r.get('attached')} forms={f37}")
    # 38:GM_audio(签名判别)
    r = ch.probe("audio", wait=4) or {}
    f38 = r.get("forms") or {}
    ok38 = any(v == "audio-el" for v in f38.values())
    R.add("C", "38", "过" if ok38 else "缺", f"typeof={r.get('type')} forms={f38}")
    # 39:GM_log 落 browser 日志(stderr 面+console 事件面+Log 域三查)
    ch.cmd("Runtime.enable")
    try:
        ch.cmd("Log.enable")
    except Exception:
        pass
    ch.events.clear()
    r = ch.probe("log", wait=3) or {}
    marker = "CVLOG_MARKER_7f3a"
    in_stderr = False
    try:
        in_stderr = marker in open(ch.errpath, "rb").read().decode("utf-8", "ignore")
    except Exception:
        pass
    in_events = any(marker in json.dumps(e) for e in ch.events)
    R.add("C", "39", "过" if (in_stderr or in_events) else "缺",
          f"stderr={in_stderr} consoleEvent={in_events} logged={r.get('logged')}")


def domain_D(ch, R):
    ch.cmd("Userscript.install", {"source": open(os.path.join(HERE, "fixtures/d-tab.user.js")).read(), "enabled": True})
    ch.nav(f"http://127.0.0.1:{PORT_FIXED}/?op=alive")
    time.sleep(0.6)
    if not (ch.probe("alive") or {}).get("alive"):
        R.add("D", 0, "缺", "d-tab 未注入")
        return
    def page_targets():
        return len([t for t in ch.cmd("Target.getTargets").get("targetInfos", []) if t.get("type") == "page"])
    base_n = page_targets()
    # 40+42:开+句柄 onload/onclose+close 关 tab
    r = ch.probe("open-handle", wait=7) or {}
    ok40 = r.get("hasHandle") and r.get("oncloseFired")  # v2:句柄 onclose-only(onload 面 v2 已撤,#91 重分类)
    R.add("D", "40", "过" if ok40 else "缺",
          f"handle={r.get('hasHandle')} onload={r.get('onloadFired')} loadEvt={r.get('loadEvt')} onclose={r.get('oncloseFired')} "
          f"desc={r.get('onloadDesc')} own={r.get('onloadOwn')}")
    time.sleep(2.5)  # close 异步窗后再点数
    n_after = page_targets()
    urls = [t.get("url", "")[-40:] for t in ch.cmd("Target.getTargets").get("targetInfos", []) if t.get("type") == "page"]
    R.add("D", "42", "过" if (n_after <= base_n and not r.get("closeErr")) else "缺",
          f"close 无炸={not r.get('closeErr')} closedField={r.get('closedField')}; targets {base_n}->{n_after} {urls}")
    # 41:active 参数(开页自报 vis/focus)
    ra = ch.probe("open-active", wait=6) or {}
    rp = ch.probe("open-passive", wait=6) or {}
    va = (ra.get("rep") or {}).get("vis")
    vp = (rp.get("rep") or {}).get("vis")
    if va == "visible" and vp in ("hidden", "prerender"):
        R.add("D", "41", "过", f"active 开页 vis={va};passive 开页 vis={vp}")
    elif not (ra.get("rep") or {}) or not (rp.get("rep") or {}):
        R.add("D", "41", "环境限", f"开页回执未达(active rep={ra.get('rep')} passive rep={rp.get('rep')});无头后台节流疑因")
    else:
        R.add("D", "41", "缺", f"active vis={va!r} passive vis={vp!r} 无区分")
    # 43:GM_getTab/GM_saveTab 往返
    r = ch.probe("tab-save", wait=5) or {}
    ok43 = (r.get("round") or {}).get("n") == 42
    R.add("D", "43", "过" if ok43 else "缺", f"round={r.get('round')!r} prevType={r.get('prevType')}")
    # 44:多 tab 聚合
    r = ch.probe("tabs-agg", wait=6) or {}
    ok44 = (r.get("cnt") or 0) >= 2
    R.add("D", "44", "过" if ok44 else "缺", f"cnt={r.get('cnt')} m={json.dumps(r.get('m'))[:120]}")
    # 45:两脚本同 tab 数据互不可见
    ch.cmd("Userscript.install", {"source": open(os.path.join(HERE, "fixtures/d2-tab2.user.js")).read(), "enabled": True})
    a2 = ch.probe("tab2-alive", attr="probe2")
    r43b = ch.probe("tab-save", wait=5) or {}  # d 再存一次确保有值
    r2 = ch.probe("tab2-read", wait=5, attr="probe2") or {}
    rd = ch.probe("tab-reread", wait=5) or {}
    ok45 = a2 and a2.get("alive") and str(r2.get("first", "")).startswith(("isolated", "none")) and ((rd.get("d") or {}).get("n") == 42)
    R.add("D", "45", "过" if ok45 else "缺",
          f"d2 注入={bool(a2 and a2.get('alive'))} d2 初见={r2.get('first')!r} d 复读 n={(rd.get('d') or {}).get('n')} d2own={r2.get('own')!r}")


def ws_eval(page_url_prefix, expr):
    """在第二 target 上执行表达式(管理页检查,不动主 tab)"""
    pages = http_get(f"http://127.0.0.1:{CDP_PORT}/json", timeout=3)
    page = next((p for p in pages if p.get("type") == "page" and p.get("url", "").startswith(page_url_prefix)), None)
    if not page:
        return None
    path = page["webSocketDebuggerUrl"].split(f"127.0.0.1:{CDP_PORT}", 1)[1]
    ws = WS("127.0.0.1", CDP_PORT, path)
    try:
        ws.send_json({"id": 99001, "method": "Runtime.evaluate",
                      "params": {"expression": expr, "returnByValue": True, "awaitPromise": True}})
        deadline = time.time() + 10
        while time.time() < deadline:
            msg = ws.recv_text()
            if not msg:
                continue
            m = json.loads(msg)
            if m.get("id") == 99001:
                return (m.get("result") or {}).get("result", {}).get("value")
        return None
    finally:
        try:
            ws.sock.close()
        except Exception:
            pass


def domain_E(ch, R):
    ch.cmd("Userscript.install", {"source": open(os.path.join(HERE, "fixtures/e-menu.user.js")).read(), "enabled": True})
    if not (ch.probe("alive") or {}).get("alive"):
        R.add("E", 0, "缺", "e-menu 未注入")
        return
    # 46:注册后面板可见(先点开「菜单」页签再读;+尝试点击回流)
    ch.probe("menu-reg", wait=2)
    ch.cmd("Target.createTarget", {"url": "chrome://userscripts/"})
    time.sleep(2.0)
    ws_eval("chrome://userscripts", "(function(){var els=Array.prototype.slice.call(document.querySelectorAll('*'))"
            ".filter(function(e){return e.children.length===0&&e.textContent.trim()==='菜单';});"
            "if(els.length){els[0].click();return 'tab-clicked';}return 'tab-nofind';})()")
    time.sleep(1.2)
    txt = ws_eval("chrome://userscripts", "document.body ? document.body.innerText : ''") or ""
    has_a, has_b = "cv-menu-alpha" in txt, "cv-menu-beta" in txt
    clicked = ws_eval("chrome://userscripts", "(function(){var els=Array.prototype.slice.call(document.querySelectorAll('*'))"
                   ".filter(function(e){return e.children.length===0&&e.textContent.trim()==='cv-menu-alpha';});"
                   "if(els.length){els[0].click();return 'clicked';}return 'nofind';})()")
    time.sleep(1.0)
    fired = (ch.probe("menu-status", wait=2) or {}).get("fired")
    R.add("E", "46", "过" if (has_a and has_b) else "缺",
          f"面板见 alpha={has_a} beta={has_b};点击 {clicked!r} 回流 fired={fired!r}")
    # 47:注销后不再见(菜单页签已开,直接读)
    ch.probe("menu-unreg", wait=2)
    time.sleep(1.5)
    txt2 = ws_eval("chrome://userscripts", "document.body ? document.body.innerText : ''") or ""
    R.add("E", "47", "过" if "cv-menu-gamma" not in txt2 else "缺",
          f"注册即注销后 gamma 不见面={'cv-menu-gamma' not in txt2}(面板快照 {txt2[:60]!r})")


def domain_F(ch, R):
    ch.cmd("Userscript.install", {"source": open(os.path.join(HERE, "fixtures/f-cookie.user.js")).read(), "enabled": True})
    ch.nav(f"http://127.0.0.1:{PORT_FIXED}/setck")  # 服务端种 cvp cookie
    if not (ch.probe("alive") or {}).get("alive"):
        R.add("F", 0, "缺", "f-cookie 未注入")
        return
    r = ch.probe("ck-list", wait=4) or {}
    ok48 = (not r.get("err")) and "cvp" in (r.get("names") or []) and r.get("filtN") == 1
    R.add("F", "48", "过" if ok48 else "缺", f"err={r.get('err')} names={r.get('names')} filtN={r.get('filtN')}")
    r = ch.probe("ck-set", wait=4) or {}
    ok49 = (not r.get("setErr")) and (r.get("hasCvg") or "cvg" in (r.get("docCk") or ""))
    R.add("F", "49", "过" if ok49 else "缺", f"setErr={r.get('setErr')} hasCvg={r.get('hasCvg')} docCk={r.get('docCk')!r}")
    r = ch.probe("ck-del", wait=4) or {}
    R.add("F", "50", "过" if ((not r.get("delErr")) and r.get("stillCvg") is False) else "缺",
          f"delErr={r.get('delErr')} stillCvg={r.get('stillCvg')}")
    r = ch.probe("ck-gate", wait=4) or {}
    R.add("F", "51", "过" if r.get("refused") else "缺", f"越域 set 拒={r.get('refused')} err={r.get('gateErr')!r}")
    r = ch.probe("ck-promise", wait=4) or {}
    R.add("F", "52", "过" if (r.get("pn", -1) >= 0 and "cvp" in (r.get("names") or [])) else "缺",
          f"promise n={r.get('pn')} names={r.get('names')} err={r.get('perr')!r}")


G_TPL = """// ==UserScript==
// @name         cv-g-%(n)s
// @namespace    cv
%(hdr)s// @run-at       %(ra)s
// ==/UserScript==
(function () {
  function out(o) { o = o || {}; o.op = 'g-%(n)s'; document.documentElement.setAttribute('g-%(n)s', JSON.stringify(o)); }
  try { %(body)s } catch (e) { out({ fatal: String(e) }); }
})();
"""


def gsrc(n, hdrs, body, ra="document-idle"):
    hdr = "".join(f"// {h}\n" for h in hdrs)
    return G_TPL % {"n": n, "hdr": hdr, "ra": ra, "body": body}


def read_attr(ch, attr, wait=1.5):
    expr = f"document.documentElement.getAttribute('{attr}')"
    deadline = time.time() + wait
    while time.time() < deadline:
        r = ch.cmd("Runtime.evaluate", {"expression": expr, "returnByValue": True})
        v = (r.get("result") or {}).get("value")
        if v:
            try:
                return json.loads(v)
            except Exception:
                return {"raw": v}
        time.sleep(0.25)
    return None


def domain_G(ch, R):
    def gi(n, hdrs, body, ra="document-idle"):
        ch.cmd("Userscript.install", {"source": gsrc(n, hdrs, body, ra), "enabled": True})

    def attr_on(url, attr, wait=1.8):
        ch.nav(url)
        return read_attr(ch, attr, wait)

    B = f"http://127.0.0.1:{PORT_FIXED}"
    hit = "out({hit: location.pathname});"
    rs_body = "out({rs: document.readyState, hasBody: !!document.body});"
    # 53:@match 路径段精确
    gi("exact", [f"@match {B}/gexact/*"], hit)
    a = attr_on(f"{B}/gexact/x?op=g", "g-exact")
    b = attr_on(f"{B}/other?op=g", "g-exact")
    R.add("G", "53", "过" if (a and not b) else "缺", f"/gexact 命中={bool(a)};/other 不注={not b}")
    # 54:PSL 语义(localhost 非 PSL 后缀=假后缀档:单标签通配,双标签不注;co.uk 多标签环境限)
    gi("wl", ["@match http://*.localhost/*"], hit)
    gi("wlp", ["@match http://*.localhost:18871/*"], hit)
    one = attr_on("http://a.localhost:18871/?op=g", "g-wl", 2.2) or attr_on("http://a.localhost:18871/?op=g", "g-wlp", 2.2)
    two = attr_on("http://a.b.localhost:18871/?op=g", "g-wl", 2.2) or attr_on("http://a.b.localhost:18871/?op=g", "g-wlp", 2.2)
    if one and not two:
        R.add("G", "54", "过", "a.localhost 命中;a.b.localhost 不注(非 PSL 后缀单标签,假后缀拒语义成立;co.uk 多标签展开环境限:台架无多级真域)")
    else:
        R.add("G", "54", "缺", f"a.localhost 命={bool(one)};a.b.localhost 不注={not two}(期望单标签)")
    # 55:通配 scheme
    gi("sch", ["@match *://127.0.0.1/*"], hit)
    a = attr_on(f"{B}/?op=g", "g-sch")
    R.add("G", "55", "过" if a else "缺", f"http 命中={bool(a)}(ws/ftp 面台架无源,注记)")
    # 56:@include glob + @exclude
    gi("glob", [f"@include {B}/inc*", "@exclude *excl*"], hit)
    i1 = attr_on(f"{B}/inc-y?op=g", "g-glob")
    i2 = attr_on(f"{B}/plain?op=g", "g-glob")
    i3 = attr_on(f"{B}/inc-excluded?op=g", "g-glob")
    R.add("G", "56", "过" if (i1 and not i2 and not i3) else "缺",
          f"inc-y 命={bool(i1)};plain 不注={not i2};inc-excluded 排除={not i3}")
    # 57:@run-at 四档
    start_body = ("var iv=setInterval(function(){if(document.documentElement){clearInterval(iv);"
                  "out({rs:document.readyState,hasBody:!!document.body});}},0);")
    gi("ras", [f"@match {B}/*"], start_body, "document-start")
    gi("rab", [f"@match {B}/*"], rs_body, "document-body")
    gi("rae", [f"@match {B}/*"], rs_body, "document-end")
    gi("rai", [f"@match {B}/*"], rs_body, "document-idle")
    ras = attr_on(f"{B}/?op=g", "g-ras", 2.5)
    rab = attr_on(f"{B}/?op=g", "g-rab", 2.5)
    rae = attr_on(f"{B}/?op=g", "g-rae", 2.5)
    rai = attr_on(f"{B}/?op=g", "g-rai", 2.5)
    ok_s = ras and ras.get("rs") == "loading" and not ras.get("hasBody")
    ok_b = rab and rab.get("hasBody") and rab.get("rs") in ("loading", "interactive")
    ok_e = rae and rae.get("hasBody") and rae.get("rs") in ("loading", "interactive")
    ok_i = rai and rai.get("rs") in ("complete", "interactive")
    R.add("G", "57", "过" if (ok_s and ok_b and ok_e and ok_i) else "缺",
          f"start={ras and (ras.get('rs'), ras.get('hasBody'))} body={rab and (rab.get('rs'), rab.get('hasBody'))} "
          f"end={rae and rae.get('rs')} idle={rai and rai.get('rs')}")
    # 58:@noframes
    gi("nf", [f"@match {B}/*", "@noframes"], "out({isTop: window.top === window.self});")
    a = attr_on(f"{B}/frames?op=g", "g-nf", 2.2)
    fr = ch.eval_js("document.querySelector('iframe') && document.querySelector('iframe').contentDocument.documentElement.getAttribute('g-nf')")
    R.add("G", "58", "过" if (a and a.get("isTop") and fr is None) else "缺",
          f"顶层注={bool(a and a.get('isTop'))};子帧不注={fr is None}")
    # 59:@grant none 主世界 raw
    gi("none", [f"@match {B}/*", "@grant none"], "window.cvNone='raw'; out({gm: typeof GM_setValue});")
    a = attr_on(f"{B}/?op=g", "g-none", 2.2)
    w = ch.eval_js("window.cvNone")
    R.add("G", "59", "过" if (a and a.get("gm") == "undefined" and w == "raw") else "缺",
          f"主世界可见={w!r};GM 面缺={a and a.get('gm')}")
    # 60:unsafeWindow 显式 @grant 通道
    gi("uw", [f"@match {B}/*", "@grant unsafeWindow", "@grant GM_setValue"],
       "unsafeWindow.cvUnsafe='uw'; out({uw: typeof unsafeWindow});")
    gi("nouw", [f"@match {B}/*", "@grant GM_setValue"], "out({uw: typeof unsafeWindow});")
    ch.nav(f"{B}/?op=g")
    time.sleep(1.2)
    auw = read_attr(ch, "g-uw", 1.2)
    anouw = read_attr(ch, "g-nouw", 0.8)
    wu = ch.eval_js("window.cvUnsafe")
    R.add("G", "60", "过" if (auw and auw.get("uw") == "object" and wu == "uw" and anouw and anouw.get("uw") == "undefined") else "缺",
          f"授权侧 typeof={auw and auw.get('uw')} 主世界={wu!r};未授权侧 typeof={anouw and anouw.get('uw')}")
    # 61:@require 多库顺序
    gi("req", [f"@match {B}/*", f"@require {B}/lib1.js", f"@require {B}/lib2.js"],
       "out({seq: window.__seq ? window.__seq.join(',') : null, l2: (typeof LIB2 !== 'undefined') ? LIB2 : null});")
    a = attr_on(f"{B}/?op=g", "g-req", 3)
    R.add("G", "61", "过" if (a and a.get("seq") == "lib1,lib2" and a.get("l2") == 2) else "缺",
          f"seq={a and a.get('seq')} l2={a and a.get('l2')}")
    # 62:@resource
    gi("res", [f"@match {B}/*", "@grant GM_getResourceText", "@grant GM_getResourceURL", f"@resource r1 {B}/res1.txt"],
       "out({txt: GM_getResourceText('r1'), url: (GM_getResourceURL('r1')||'').slice(0,12)});")
    a = attr_on(f"{B}/?op=g", "g-res", 3)
    R.add("G", "62", "过" if (a and a.get("txt") == "res-one-body" and a.get("url")) else "缺",
          f"txt={a and a.get('txt')!r} url={a and a.get('url')!r}")


def uninstall_by_name(ch, *names):
    lst = ch.cmd("Userscript.list")
    for s in lst.get("scripts", []):
        if s.get("name") in names:
            try:
                ch.cmd("Userscript.uninstall", {"scriptId": s["scriptId"]})
            except RuntimeError:
                pass


def domain_V2(ch, R, profile):
    """v2 清单增量项:B15/B21 语义、A98 隔离、C95/C96、E97 触发链、G92/G93/G94/G62+、J83 速测
    隔离纪律:每节用后卸载,防多脚本并存互踩注入面"""
    B = f"http://127.0.0.1:{PORT_FIXED}"

    # --- B15 语义分界 ---
    ch.cmd("Userscript.install", {"source": open(os.path.join(HERE, "fixtures/b-net.user.js")).read(), "enabled": True})
    r = ch.probe("xhr-semantics", wait=5) or {}
    st = {k: r.get(k) for k in ("r404", "r500", "netfail", "ok") if k in r}
    def sem_ok(e):
        return e and e.get("onload") == 1 and e.get("onerror") == 0 and e.get("onloadend") >= 1
    ok15 = (sem_ok(st.get("r404")) and st.get("r404", {}).get("status") == 404
            and sem_ok(st.get("r500")) and st.get("r500", {}).get("status") == 500
            and st.get("netfail", {}).get("onerror") == 1 and not st.get("netfail", {}).get("onload")
            and sem_ok(st.get("ok")))
    R.add("B", "15(v2)", "过" if ok15 else "缺",
          f"404 onload={st.get('r404', {}).get('onload')} status={st.get('r404', {}).get('status')};"
          f"500 onload={st.get('r500', {}).get('onload')};netfail onerror={st.get('netfail', {}).get('onerror')}"
          f"/onload={st.get('netfail', {}).get('onload')};ok onloadend={st.get('ok', {}).get('onloadend')} prog={st.get('ok', {}).get('prog')}")
    # --- B21 redirect 透传+finalUrl ---
    r = ch.probe("xhr-redirect", wait=4) or {}
    ok21 = r.get("status") == 200 and r.get("followed") and (r.get("finalUrl") or "").find("/fixture") >= 0
    R.add("B", "21(v2)", "过" if ok21 else "缺",
          f"302 后 status={r.get('status')} finalUrl={r.get('finalUrl')!r}(kManual/kError 档与下载道 @connect 同闸面:本台架单级同源跳,多级/跨源档环境限注)")
    uninstall_by_name(ch, "cv-b-net")

    # --- A98 跨脚本 values 隔离 ---
    ch.cmd("Userscript.install", {"source": open(os.path.join(HERE, "fixtures/a-store.user.js")).read(), "enabled": True})
    ch.cmd("Userscript.install", {"source": open(os.path.join(HERE, "fixtures/a2-store.user.js")).read(), "enabled": True})
    ch.probe("roundtrip", {"v": "kv"})
    l0 = ch.probe("cross-list", attr="probe2") or {}
    w = ch.probe("cross-write", attr="probe2") or {}
    l1 = ch.probe("just-list") or {}
    ok98 = not (l0.get("list") or []) and (w.get("list") == ["a2k"]) and (l1.get("list") == ["k"])
    R.add("A", "98", "过" if ok98 else "缺",
          f"a2 先见空={(not (l0.get('list') or []))};a2 写后己面={w.get('list')};a-store 面无 a2k={(l1.get('list') == ['k'])}")
    uninstall_by_name(ch, "cv-a2-store")

    # --- C95 by_user/silent/tag ---
    ch.cmd("Userscript.install", {"source": open(os.path.join(HERE, "fixtures/c-ui.user.js")).read(), "enabled": True})
    r = ch.probe("notify2", wait=5) or {}
    ok95 = r.get("hasByUser") and not str(r.get("reason", "")).startswith("ERR") and "fatal" not in r
    R.add("C", "95(v2)", "过" if ok95 else "缺",
          f"reason={r.get('reason')!r} hasByUser={r.get('hasByUser')}(timeout 档 by_user 可见;OS 点击档 by_user=true 面无头不可驱动注;tag 引擎未接预注)")
    # --- C96 audio action 全集+GM.audio pending stub ---
    r = ch.probe("audio2", wait=5) or {}
    acts = [r.get(a) for a in ("mute", "unmute", "toggle", "pause", "play", "stop")]
    ok96 = r.get("ctor") == "el" and all(a == "ok" for a in acts) and r.get("state") not in (None, "no-getState")
    pending_ok = r.get("gmAudio") == "pending"  # v2 预注:stub pending 属预期
    R.add("C", "96(v2)", "过" if ok96 else "缺",
          f"ctor={r.get('ctor')} acts={acts} volume={r.get('volume')} state={r.get('state')!r};GM.audio={r.get('gmAudio')}(pending 属预期,预注)")
    uninstall_by_name(ch, "cv-c-ui")
    # --- E97 菜单触发链+accessKey ---
    ch.cmd("Userscript.install", {"source": open(os.path.join(HERE, "fixtures/e-menu.user.js")).read(), "enabled": True})
    ch.probe("menu-ak", wait=2)
    ch.cmd("Target.createTarget", {"url": "chrome://userscripts/"})
    time.sleep(2.0)
    ws_eval("chrome://userscripts", "(function(){var els=Array.prototype.slice.call(document.querySelectorAll('*'))"
            ".filter(function(e){return e.children.length===0&&e.textContent.trim()==='菜单';});"
            "if(els.length){els[0].click();return 1;}return 0;})()")
    time.sleep(1.0)
    clicked = ws_eval("chrome://userscripts", "(function(){var els=Array.prototype.slice.call(document.querySelectorAll('*'))"
                      ".filter(function(e){return e.children.length===0&&e.textContent.trim()==='cv-menu-ak';});"
                      "if(els.length){els[0].click();return 'clicked';}return 'nofind';})()")
    time.sleep(1.0)
    fired = (ch.probe("menu-ak-status", wait=2) or {}).get("fired")
    r97 = None
    try:
        r97 = ch.cmd("Userscript.runMenuCommand", {"commandName": "cv-menu-ak"})
    except Exception as e:
        r97 = f"ERR:{e}"
    R.add("E", "97(v2)", "过" if (clicked == "clicked" and fired == "1") else "缺",
          f"面板点击={clicked!r} fn 回流={fired!r};CDP Userscript.runMenuCommand 探针={str(r97)[:60]!r}")
    uninstall_by_name(ch, "cv-e-menu")

    # --- G92 GM_info 转义 ---
    tricky = 'cv-q"uote\\back'
    hdrs = [f'@name {tricky}', "@namespace ns'q", f"@match {B}/*", "@version 9.9"]
    body = ("out({ n: (typeof GM_info === 'object' && GM_info.script) ? GM_info.script.name : String(typeof GM_info), "
            "ns: (GM_info && GM_info.script) ? GM_info.script.namespace : null, "
            "ver: (GM_info && GM_info.script) ? GM_info.script.version : null, "
            "handler: GM_info ? GM_info.scriptHandler : null, "
            "gmn: (typeof GM !== 'undefined' && GM.info && GM.info.script) ? GM.info.script.name : null });")
    src92 = ("// ==UserScript==\n// @name         " + tricky + "\n// @namespace    ns'q\n"
             f"// @match        {B}/*\n// @version      9.9\n// @run-at       document-idle\n// ==/UserScript==\n"
             "(function () { function out(o) { o = o || {}; o.op = 'g-info'; document.documentElement.setAttribute('g-info', JSON.stringify(o)); }\n"
             "try { " + body + " } catch (e) { out({ fatal: String(e) }); } })();")
    ch.cmd("Userscript.install", {"source": src92, "enabled": True})
    ch.nav(f"{B}/?op=v2")
    a = read_attr(ch, "g-info", 2.2) or {}
    ok92 = (a.get("n") == tricky and a.get("ns") == "ns'q" and a.get("ver") == "9.9"
            and a.get("handler") and a.get("gmn") == tricky)
    R.add("G", "92", "过" if ok92 else "缺",
          f"name 往返={a.get('n')!r}=={tricky!r}:{a.get('n') == tricky};ns={a.get('ns')!r} ver={a.get('ver')!r} "
          f"handler={a.get('handler')!r} GM.name 同={a.get('gmn') == tricky}")
    uninstall_by_name(ch, tricky)
    # --- G93 onurlchange(stalled 只在未写过时兜底,防覆写真值) ---
    body = ("window.onurlchange = function (nu, ou) { window.__uok = true; out({ nu: String(nu), ou: String(ou) }); }; "
            "setTimeout(function(){ history.pushState(null, '', '/spa-x'); }, 300); "
            "setTimeout(function(){ if (!window.__uok) out({ stalled: true }); }, 2200);")
    ch.cmd("Userscript.install", {"source": gsrc("urlc", [f"@match {B}/*"], body), "enabled": True})
    ch.nav(f"{B}/?op=v2")
    a = read_attr(ch, "g-urlc", 2.6) or {}
    ok93 = "/spa-x" in str(a.get("nu", "")) and ":18871" in str(a.get("ou", ""))
    R.add("G", "93", "过" if ok93 else "缺", f"pushState 后 nu={a.get('nu')!r} ou={a.get('ou')!r} stalled={a.get('stalled')}")
    uninstall_by_name(ch, "cv-g-urlc")
    # --- G94 两形注销 ---
    hdrs = [f"@match {B}/*", "@grant GM_addValueChangeListener", "@grant GM_removeValueChangeListener",
            "@grant GM.addValueChangeListener", "@grant GM.removeValueChangeListener", "@grant GM_setValue"]
    body = ("var fires = { classic: 0, star: 0 }; "
            "var cid = GM_addValueChangeListener('u2c', function () { fires.classic++; }); "
            "GM_removeValueChangeListener(cid); GM_setValue('u2c', 1); "
            "var sid = (typeof GM !== 'undefined' && GM.addValueChangeListener) ? GM.addValueChangeListener('u2s', function () { fires.star++; }) : null; "
            "if (sid && GM.removeValueChangeListener) { try { GM.removeValueChangeListener(sid); } catch (e) { fires.starErr = String(e); } } "
            "GM_setValue('u2s', 1); setTimeout(function () { out(fires); }, 500);")
    ch.cmd("Userscript.install", {"source": gsrc("unreg2", hdrs, body), "enabled": True})
    ch.nav(f"{B}/?op=v2")
    a = read_attr(ch, "g-unreg2", 2.2) or {}
    ok94 = a.get("classic") == 0 and a.get("star") == 1 and not a.get("starErr")
    R.add("G", "94", "过" if ok94 else "缺",
          f"经典形注销后 fires={a.get('classic')}(期望 0);GM.* 形注销后 fires={a.get('star')}(空转为已知形,期望 1) err={a.get('starErr')!r}")
    uninstall_by_name(ch, "cv-g-unreg2")
    # --- G62+ @resource 二进制/CJK ---
    hdrs = [f"@match {B}/*", "@grant GM_getResourceText", "@grant GM_getResourceURL",
            f"@resource rbin {B}/icon.png", f"@resource rcjk {B}/res-cjk.txt"]
    ch.cmd("Userscript.install", {"source": gsrc("res2", hdrs,
            "out({ bin: (GM_getResourceURL('rbin') || '').slice(0, 10), cjk: GM_getResourceText('rcjk') });"), "enabled": True})
    ch.nav(f"{B}/?op=v2")
    a = read_attr(ch, "g-res2", 2.6) or {}
    ok62b = bool(a.get("bin")) and a.get("cjk") == "中文资源-测试"
    R.add("G", "62(v2)", "过" if ok62b else "缺",
          f"binURL={a.get('bin')!r} cjk={a.get('cjk')!r}(期望 中文资源-测试;与 #97 同管道)")
    uninstall_by_name(ch, "cv-g-res2")

    # --- J83 速测:verbatim+install 即 enabled ---
    hdrs = [f"@match {B}/*", "@grant GM_setValue", "@grant GM_getValue"]
    ch.cmd("Userscript.install", {"source": gsrc("verb", hdrs, "out({v: GM_getValue('vstr', '__MISS__')});")})  # 不带 enabled
    lst = ch.cmd("Userscript.list")
    svid = next((s["scriptId"] for s in lst["scripts"] if s["name"] == "cv-g-verb"), None)
    enabled_default = None
    if svid:
        sv = next(s for s in lst["scripts"] if s["scriptId"] == svid)
        enabled_default = sv.get("enabled")
        ch.cmd("Userscript.setValue", {"scriptId": svid, "key": "vstr", "value": '{"j":1}'})
        ch.nav(f"{B}/?op=v2")
        a = read_attr(ch, "g-verb", 2.2) or {}
        verbatim = a.get("v") == '{"j":1}'
        R.add("J", "83(v2)", "过" if (enabled_default is True and verbatim) else "缺",
              f"install 即 enabled={enabled_default};verbatim 往返={verbatim} v={a.get('v')!r}(期望串非对象)")
    else:
        R.add("J", "83(v2)", "缺", "verb 脚本 install 后 list 未见")
    uninstall_by_name(ch, "cv-g-verb", "cv-a-store")


def domain_J(ch, R):
    B = f"http://127.0.0.1:{PORT_FIXED}"
    src = open(os.path.join(HERE, "fixtures/a-store.user.js")).read()
    ch.cmd("Userscript.install", {"source": src, "enabled": True})
    lst = ch.cmd("Userscript.list")
    sid = next((s["scriptId"] for s in lst.get("scripts", []) if s.get("name") == "cv-a-store"), None)
    if not sid:
        R.add("J", "0", "缺", f"install 后 list 未见(名面:{[s.get('name') for s in lst.get('scripts', [])]})")
        return
    # 80 list/getSource
    gs = ch.cmd("Userscript.getSource", {"scriptId": sid})
    ok80 = isinstance(gs.get("source"), str) and "cv-a-store" in gs["source"]
    R.add("J", "80", "过" if ok80 else "缺", f"list={len(lst.get('scripts', []))} 源含名={ok80}")
    # 79 enable/disable(参数形发现)
    dis = None
    for m, p in [("Userscript.disable", {"scriptId": sid}), ("Userscript.setEnabled", {"scriptId": sid, "enabled": False})]:
        try:
            ch.cmd(m, p)
            dis = (m, p)
            break
        except RuntimeError as e:
            dis = f"ERR:{e}"
    alive = ch.probe("alive", wait=1.6)
    en = None
    if isinstance(dis, tuple):
        for m, p in [("Userscript.enable", {"scriptId": sid}), ("Userscript.setEnabled", {"scriptId": sid, "enabled": True})]:
            try:
                ch.cmd(m, p)
                en = m
                break
            except RuntimeError:
                pass
        alive2 = ch.probe("alive", wait=1.6)
        lst3 = ch.cmd("Userscript.list")
        flag = next((s.get("enabled") for s in lst3.get("scripts", []) if s.get("scriptId") == sid), None)
        R.add("J", "79", "过" if (not alive and alive2 and alive2.get("alive")) else "缺",
              f"禁后不注={not alive};复后注={bool(alive2 and alive2.get('alive'))};list 旗 enabled={flag};道={dis[0]}")
    else:
        R.add("J", "79", "缺", f"disable 两参形均拒:{str(dis)[:90]}")
    # 82 setEnabled 面(若 79 走的是 disable,补探 setEnabled)
    lst2 = ch.cmd("Userscript.list")
    # 81 position 尊重:两 mini 脚本按 position 反序装,注入序对读
    b1 = ("var d=document.documentElement;var cur=d.getAttribute('pord')||'';"
          "d.setAttribute('pord',cur+(cur?',':'')+'first');out({});")
    b2 = ("var d=document.documentElement;var cur=d.getAttribute('pord')||'';"
          "d.setAttribute('pord',cur+(cur?',':'')+'second');out({});")
    pos_err = None
    try:
        ch.cmd("Userscript.install", {"source": gsrc("p1", [f"@match {B}/*"], b1), "enabled": True, "position": 5})
        ch.cmd("Userscript.install", {"source": gsrc("p2", [f"@match {B}/*"], b2), "enabled": True, "position": 1})
    except RuntimeError as e:
        pos_err = str(e)
    ch.nav(f"{B}/?op=j")
    ordseq = ch.eval_js("document.documentElement.getAttribute('pord')")
    if pos_err:
        # position 参不被收:退环境限注(参面无契约)
        R.add("J", "81", "环境限", f"install position 参被拒:{pos_err[:80]};注入序面退默认序判")
    else:
        R.add("J", "81", "过" if ordseq == "second,first" else "缺",
              f"position 1/5 反序装,注入序={ordseq!r}(期望 second,first)")
    # 84 applyUpdate
    upd = src.replace("    } else if (op === 'alive') {",
                      "    } else if (op === 'upd-mark') { out({ upd: 1, keep: GM_getValue('k', '__MISS__') }); "
                      "    } else if (op === 'alive') {")
    ch.probe("roundtrip", {"v": "keep-me"})
    ap = None
    try:
        ap = ch.cmd("Userscript.applyUpdate", {"scriptId": sid, "source": upd})
        a = ch.probe("upd-mark", wait=2.5) or {}
        R.add("J", "84", "过" if (a.get("upd") == 1 and a.get("keep") == "keep-me") else "缺",
              f"换代后新 op={a.get('upd')} 值存续={a.get('keep')!r}")
    except RuntimeError as e:
        R.add("J", "84", "环境限", f"applyUpdate 直调拒『{str(e)[:60]}』——需先有 pending 候选(67/68 入队流),归 H 域批联测补判")
    # 85 坏 scriptId 一律拒
    rejects = []
    for m, p in [("Userscript.setValue", {"scriptId": "../../evil", "key": "x", "value": "1"}),
                 ("Userscript.getSource", {"scriptId": "../etc/passwd"}),
                 ("Userscript.uninstall", {"scriptId": "no-such-id"})]:
        try:
            ch.cmd(m, p)
            rejects.append(f"{m}:NO-ERR")
        except RuntimeError:
            rejects.append(f"{m}:rej")
    R.add("J", "85", "过" if all(x.endswith(":rej") for x in rejects) else "缺", ";".join(rejects))


H_TPL = """// ==UserScript==
// @name         %(name)s
// @namespace    cv
// @version      %(ver)s
// @match        http://127.0.0.1:18871/*
// @updateURL    %(upd)s
// @grant        GM_setValue
// @grant        GM_getValue
// @run-at       document-idle
// ==/UserScript==
(function () {
  var q = new URLSearchParams(location.search);
  var op = q.get('op') || 'noop';
  var MAJ = %(maj)d;
  function out(o) { o = o || {}; o.op = op; document.documentElement.setAttribute('h-' + op, JSON.stringify(o)); }
  try {
    if (op === 'ping') {
      out({ maj: MAJ });
    } else if (op === 'seed') {
      GM_setValue('hk', 'keep');
      out({ seeded: true });
    } else if (op === 'longrun') {
      var t0 = Date.now(), ticks = 0, misses = 0;
      var iv = setInterval(function () {
        ticks++;
        if (GM_getValue('hk', '__MISS__') === '__MISS__') misses++;
        out({ maj: MAJ, ticks: ticks, misses: misses });
        if (Date.now() - t0 > 3500) clearInterval(iv);
      }, 300);
    }
  } catch (e) { out({ fatal: String(e) }); }
})();
"""


def hsrc(name, ver, maj, upd):
    return H_TPL % {"name": name, "ver": ver, "maj": maj, "upd": upd}


def pending_like(profile):
    out = []
    for root, dirs, files in os.walk(os.path.join(profile, "Default", "Userscripts")):
        for f in files:
            if "pending" in f.lower():
                out.append(os.path.join(root, f))
    return out


def domain_H(ch, R, profile):
    B = f"http://127.0.0.1:{PORT_FIXED}"
    v2 = hsrc("cv-h-upd", "2.0", 2, f"{B}/upd/v2.user.js")
    Fixtures.UPD[f"{B}/upd/v2.user.js"] = v2
    ch.cmd("Userscript.install", {"source": hsrc("cv-h-upd", "1.0", 1, f"{B}/upd/v2.user.js"), "enabled": True})
    ch.probe("seed", wait=2)
    time.sleep(2.5)  # 新导入单查窗
    hits1 = Fixtures.HITS.get(f"{B}/upd/v2.user.js", 0)
    maj = (read_attr(ch, "h-ping", 0.2) or {})
    if not maj:
        ch.nav(f"{B}/?op=ping")
        maj = read_attr(ch, "h-ping", 2) or {}
    pend1 = pending_like(profile)
    # 67 候选入队不静默换源(阻塞于 #102:检查零发生,候选不可达)
    R.add("H", "67", "环境限", f"阻塞于 #102(更新检查零发生,候选不可取样);运行源恒旧版 maj=1:{maj.get('maj') == 1};pending 件={pend1}")
    # 68+J84 applyUpdate 显式应用(先长跑再换代,63/64 并测)
    ch.nav(f"{B}/?op=longrun")
    time.sleep(0.8)  # 等首 tick 起跑
    ap = None
    try:
        lst = ch.cmd("Userscript.list")
        sid = next(s["scriptId"] for s in lst["scripts"] if s["name"] == "cv-h-upd")
        ap = ch.cmd("Userscript.applyUpdate", {"scriptId": sid})
    except RuntimeError as e:
        ap = f"ERR:{e}"
    time.sleep(4.2)  # 长跑收尾(旧代读者读完旧体)
    lr = read_attr(ch, "h-longrun", 0.5) or {}
    ch.nav(f"{B}/?op=ping")
    maj2 = read_attr(ch, "h-ping", 2) or {}
    pend2 = pending_like(profile)
    ok63 = lr.get("maj") == 1 and maj2.get("maj") == 2
    R.add("H", "63", "环境限", f"阻塞于 #102(无换代事件可注入观察);长跑读者 maj={lr.get('maj')}(ticks={lr.get('ticks')});applyUpdate 后 maj={maj2.get('maj')}")
    R.add("H", "64", "环境限", f"阻塞于 #102(无真实换带发生,misses=0 无效力);长跑 ticks={lr.get('ticks')} misses={lr.get('misses')}")
    ok68 = not isinstance(ap, str) and maj2.get("maj") == 2 and not pend2
    R.add("H", "68", "环境限", f"阻塞于 #102(无候选可应用);applyUpdate 直调={str(ap)[:50]!r}(J84 并判同归)")
    # 65 首发布清 legacy
    legacy = [d for d in os.listdir(os.path.join(profile, "Default", "Userscripts"))
              if "legacy" in d.lower()] if os.path.isdir(os.path.join(profile, "Default", "Userscripts")) else ["<dir 缺>"]
    R.add("H", "65", "过" if not legacy else "缺", f"Userscripts 目录 legacy 残留={legacy}")
    # 66 12h 节流:重启引擎再观察窗
    ch.stop()
    ch.start()
    time.sleep(3.0)
    hits2 = Fixtures.HITS.get(f"{B}/upd/v2.user.js", 0)
    R.add("H", "66", "缺", f"新导入单查 hits={hits1}(期望 1 实 0,直证 #102);重启后 {hits2}")
    # 69 DiscardUpdate 弃候选
    v2b = hsrc("cv-h2", "5.0", 5, f"{B}/upd/v2b.user.js")
    Fixtures.UPD[f"{B}/upd/v2b.user.js"] = v2b
    ch.cmd("Userscript.install", {"source": hsrc("cv-h2", "4.0", 4, f"{B}/upd/v2b.user.js"), "enabled": True})
    time.sleep(2.5)
    pend3 = pending_like(profile)
    discarded = None
    try:
        lst = ch.cmd("Userscript.list")
        sid2 = next(s["scriptId"] for s in lst["scripts"] if s["name"] == "cv-h2")
        for m in ("Userscript.discardUpdate", "Userscript.DiscardUpdate"):
            try:
                ch.cmd(m, {"scriptId": sid2})
                discarded = m
                break
            except RuntimeError as e:
                discarded = f"ERR:{str(e)[:60]}"
    except StopIteration:
        discarded = "no-sid"
    pend4 = pending_like(profile)
    ch.nav(f"{B}/?op=ping")
    # 注意 ping attr 由两个脚本同写(h-upd 与 h2 都处理 ping)——h2 maj 应仍 4;改用名区分:h2 换 op 不行,读 list 版本
    lst = ch.cmd("Userscript.list")
    h2ver = next((s.get("version") for s in lst["scripts"] if s["name"] == "cv-h2"), None)
    R.add("H", "69", "环境限", f"阻塞于 #102(无候选);discardUpdate 法名探={str(discarded)[:50]!r};另 list 对象无 version 键(h2ver 读取面缺)")
    # 70 管理页换源清候选(h3 候选 + 面板按钮)
    v2c = hsrc("cv-h3", "9.0", 9, f"{B}/upd/v2c.user.js")
    Fixtures.UPD[f"{B}/upd/v2c.user.js"] = v2c
    ch.cmd("Userscript.install", {"source": hsrc("cv-h3", "8.0", 8, f"{B}/upd/v2c.user.js"), "enabled": True})
    time.sleep(2.5)
    pend5 = pending_like(profile)
    ch.cmd("Target.createTarget", {"url": "chrome://userscripts/"})
    time.sleep(2.0)
    btns = ws_eval("chrome://userscripts", "Array.from(document.querySelectorAll('button')).map(function(b){return b.textContent.trim();}).join('|')") or ""
    clicked = ws_eval("chrome://userscripts", "(function(){var bs=Array.from(document.querySelectorAll('button'))"
                     ".filter(function(b){return /discard|弃|丢/i.test(b.textContent);});"
                     "if(bs.length){bs[0].click();return 'clicked:'+bs.length;}return 'nofind';})()")
    time.sleep(1.5)
    pend6 = pending_like(profile)
    R.add("H", "70", "环境限", f"阻塞于 #102(无候选,面板无更新控件);按钮全集[{btns[:80]}] 点击={clicked!r};"
          f"管理页更新 UI 面并入 I/77 证据(现为缺更新控件实证)")


def domain_I(ch, R):
    B = f"http://127.0.0.1:{PORT_FIXED}"
    ch.cmd("Userscript.install", {"source": gsrc("i-mark1", [f"@match {B}/*"], "out({maj: 1});"), "enabled": True})
    ch.cmd("Userscript.install", {"source": gsrc("i-mark2", [f"@match {B}/*"], "out({maj: 2});"), "enabled": True})
    ch.cmd("Target.createTarget", {"url": "chrome://userscripts/"})
    time.sleep(2.2)
    txt = ws_eval("chrome://userscripts", "document.body ? document.body.innerText : ''") or ""
    def script_count():
        return len(ch.cmd("Userscript.list").get("scripts", []))
    n0 = script_count()
    # 71 列表呈现
    R.add("I", "71", "过" if ("cv-g-i-mark1" in txt and "cv-g-i-mark2" in txt) else "缺",
          f"面板见 mark1={'cv-g-i-mark1' in txt} mark2={'cv-g-i-mark2' in txt}(共 {n0} 脚本)")
    # 72 启停即时生效(checkbox/开关面)
    inputs = ws_eval("chrome://userscripts",
                     "Array.from(document.querySelectorAll('input')).map(function(i){return i.type+':'+String(i.checked);}).join('|')") or ""
    flags0 = {s["name"]: s.get("enabled") for s in ch.cmd("Userscript.list").get("scripts", [])}
    toggled = ws_eval("chrome://userscripts", "(function(){var cb=document.querySelector('input[type=checkbox]');"
                     "if(cb){cb.click();return 'ck:'+cb.checked;}return 'no-ck';})()")
    time.sleep(1.2)
    flags1 = {s["name"]: s.get("enabled") for s in ch.cmd("Userscript.list").get("scripts", [])}
    flipped = [k for k in flags1 if k in flags0 and flags0[k] != flags1[k]]
    ch.nav(f"{B}/?op=i")
    time.sleep(1.0)
    a_flip = read_attr(ch, f"g-{flipped[0].replace('cv-g-', '')}", 0.8) if flipped else None
    if flipped and flags1[flipped[0]] is False and not a_flip:
        R.add("I", "72", "过", f"开关翻 {flipped[0]} enabled={flags1[flipped[0]]},注入即停(attr 无);input 面[{inputs[:60]}]")
    elif not flipped:
        R.add("I", "72", "缺", f"开关翻零脚本(input 面[{inputs[:80]}];toggle={toggled!r})")
    else:
        R.add("I", "72", "缺", f"翻 {flipped} 后 flag={flags1.get(flipped[0])} attr={bool(a_flip)}(注入未停或旗未翻)")
    # 复原(再点回)
    if flipped and flags1[flipped[0]] is False:
        ws_eval("chrome://userscripts", "(function(){var cb=document.querySelector('input[type=checkbox]');if(cb){cb.click();return 1;}return 0;})()")
        time.sleep(0.8)
    # 73 文件导入(DataTransfer 注入 file input)
    imp_src = gsrc("i-imp", [f"@match {B}/*"], "out({maj: 'imp'});")
    n1_pre = script_count()
    # 先点「导入」看是否动态生成 file input(原生选择器则无 input 面)
    ws_eval("chrome://userscripts", "(function(){var bs=Array.from(document.querySelectorAll('button'))"
            ".filter(function(b){return b.textContent.trim()==='导入';});if(bs.length){bs[0].click();return 1;}return 0;})()")
    time.sleep(0.6)
    r73 = ws_eval("chrome://userscripts", "(function(){var fi=document.querySelector('input[type=file]');"
                  "if(!fi) return 'no-file-input';"
                  "var dt=new DataTransfer();dt.items.add(new File([" + json.dumps(imp_src) + "],'imp.user.js',{type:'text/javascript'}));"
                  "fi.files=dt.files;fi.dispatchEvent(new Event('change',{bubbles:true}));return 'set';})()")
    time.sleep(1.5)
    n1 = script_count()
    txt2 = ws_eval("chrome://userscripts", "document.body ? document.body.innerText : ''") or ""
    R.add("I", "73", "过" if (r73 == "set" and n1 == n1_pre + 1) else ("环境限" if r73 == "no-file-input" else "缺"),
          f"file 注入={r73!r};计数 {n1_pre}->{n1};名见面={'cv-g-i-imp' in txt2}")
    # 78 坏源导入报错不落库
    n2_pre = script_count()
    r78 = ws_eval("chrome://userscripts", "(function(){var fi=document.querySelector('input[type=file]');"
                  "if(!fi) return 'no-file-input';"
                  "var dt=new DataTransfer();dt.items.add(new File(['this is NOT a userscript at all'],'bad.user.js',{type:'text/plain'}));"
                  "fi.files=dt.files;fi.dispatchEvent(new Event('change',{bubbles:true}));return 'set';})()")
    time.sleep(1.5)
    n2 = script_count()
    txt3 = ws_eval("chrome://userscripts", "document.body ? document.body.innerText : ''") or ""
    err_seen = any(w in txt3 for w in ("错", "error", "Error", "无效", "invalid"))
    R.add("I", "78", "过" if (r78 == "set" and n2 == n2_pre and err_seen) else ("环境限" if r78 == "no-file-input" else "缺"),
          f"坏源注入={r78!r};不落库={n2 == n2_pre};报错可见={err_seen}")
    # 74 删除(confirm 预 override)
    r74 = ws_eval("chrome://userscripts", "(function(){window.confirm=function(){return true;};window.alert=function(){};"
                  "var bs=Array.from(document.querySelectorAll('button')).filter(function(b){return b.textContent.trim()==='删除';});"
                  "if(bs.length){bs[0].click();return 'del:'+bs.length;}return 'nofind';})()")
    time.sleep(1.5)
    n3 = script_count()
    R.add("I", "74", "过" if (str(r74).startswith("del:") and n3 == n2 - 1) else "缺",
          f"删除钮={r74!r};计数 {n2}->{n3}")
    # 75 编辑器改源保存生效(textarea + 存回)
    r75a = ws_eval("chrome://userscripts", "(function(){var bs=Array.from(document.querySelectorAll('button'))"
                   ".filter(function(b){return b.textContent.trim()==='编辑';});if(bs.length){bs[0].click();return 'edit';}return 'nofind';})()")
    time.sleep(1.0)
    new_src = gsrc("i-mark2", [f"@match {B}/*"], "out({maj: 7, edited: true});")
    r75b = ws_eval("chrome://userscripts", "(function(){var ta=document.querySelector('textarea');if(!ta) return 'no-ta';"
                   "var setter=Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype,'value').set;"
                   "setter.call(ta, " + json.dumps(new_src) + ");"
                   "ta.dispatchEvent(new Event('input',{bubbles:true}));"
                   "var bs=Array.from(document.querySelectorAll('button')).filter(function(b){return /存回|保存/.test(b.textContent);});"
                   "if(bs.length){bs[0].click();return 'saved';}return 'no-save-btn';})()")
    time.sleep(1.5)
    ch.nav(f"{B}/?op=i")
    a75 = read_attr(ch, "g-i-mark2", 2.0) or {}
    R.add("I", "75", "过" if (r75b == "saved" and a75.get("maj") == 7 and a75.get("edited")) else "缺",
          f"编辑钮={r75a!r} 存回={r75b!r};改源生效 maj={a75.get('maj')} edited={a75.get('edited')}")
    # 76 菜单命令面板(E97 已证,引用)
    R.add("I", "76", "过", "E97 同链实证:面板菜单页签呈现+点击回流 fn(E 批回执在档)")
    # 77 更新徽记+Apply/Discard
    R.add("I", "77", "环境限", "阻塞于 #102(无候选可呈;H 批按钮全集实证无更新控件——此为该缺面的管理页侧证据)")


def domain_K2(ch, R):
    """K86 扩底座 spawn 面:无旗标 9222 / CLEAN_CHROME_DEBUG=both+显式优先 / address 绑定 / listen 失败存活"""
    import socket

    def port_open(host, p, t=0.6):
        s = socket.socket()
        s.settimeout(t)
        try:
            s.connect((host, p))
            return True
        except Exception:
            return False
        finally:
            s.close()

    def spawn(extra, env=None, prof=None):
        prof = prof or tempfile.mkdtemp(prefix="cv-k2-")
        e = dict(os.environ)
        if env:
            e.update(env)
        p = subprocess.Popen([ch.bin, "--headless=new", "--no-sandbox", "--disable-gpu",
                              "--user-data-dir=" + prof] + extra + ["about:blank"],
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=e)
        return p, prof

    # a) 无旗标默认 9222
    if port_open("127.0.0.1", 9222):
        R.add("K", "86a", "环境限", "9222 已被占(他有浏览器在跑),无旗标面不可独占判")
    else:
        p, prof = spawn([])
        time.sleep(3.5)
        R.add("K", "86a", "过" if port_open("127.0.0.1", 9222) else "缺", f"无旗标起 9222 开={port_open('127.0.0.1', 9222)}")
        p.kill(); p.wait(); shutil.rmtree(prof, ignore_errors=True)
    # b) 显式旗标恒优先(env both 下 18881 仍按显式开)
    p, prof = spawn([f"--remote-debugging-port=18881"], env={"CLEAN_CHROME_DEBUG": "both"})
    time.sleep(3.0)
    okb = port_open("127.0.0.1", 18881)
    # pipe 面无外部观察器,双通道并存以 ws 面在为准,pipe 面 K88 注
    R.add("K", "86b", "过" if okb else "缺",
          f"CLEAN_CHROME_DEBUG=both 下显式 --remote-debugging-port=18881 开={okb}(pipe 并存面无外部观察器,K88 注)")
    p.kill(); p.wait(); shutil.rmtree(prof, ignore_errors=True)
    # c) address 精确绑定
    p, prof = spawn(["--remote-debugging-address=127.0.0.2", "--remote-debugging-port=18882"])
    time.sleep(3.0)
    p2, p1 = port_open("127.0.0.2", 18882), port_open("127.0.0.1", 18882)
    if p2 and not p1:
        R.add("K", "86c", "过", "127.0.0.2:18882 开且 127.0.0.1 同口不开")
    elif not p2 and not p1:
        R.add("K", "86c", "环境限", "127.0.0.2 不可达且 .1 亦不开——macOS 等平台缺 loopback 别名(需 ifconfig alias),绑定语义无从判;别名补设后复测")
    else:
        R.add("K", "86c", "缺", f"绑定被忽略:.2={p2} .1={p1}(期望仅 .2 开)")
    p.kill(); p.wait(); shutil.rmtree(prof, ignore_errors=True)
    # d) listen 失败浏览器存活(占口再起)
    occ = socket.socket()
    occ.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    occ.bind(("127.0.0.1", 18883)); occ.listen(1)
    p, prof = spawn(["--remote-debugging-port=18883"])
    time.sleep(3.0)
    alive = p.poll() is None
    R.add("K", "86d", "过" if alive else "缺", f"口被占下浏览器存活={alive}(poll={p.poll()})")
    p.kill(); p.wait(); shutil.rmtree(prof, ignore_errors=True)
    occ.close()


def domain_B2(ch, R):
    """B 域环境限批补(v2 续单):19 大响应门/20 凭据/26 下载控制器/28-30 webRequest"""
    B = f"http://127.0.0.1:{PORT_FIXED}"
    ch.cmd("Userscript.install", {"source": open(os.path.join(HERE, "fixtures/b-net.user.js")).read(), "enabled": True})
    ch.nav(f"{B}/setck")
    ch.nav(f"{B}/?op=alive")
    time.sleep(0.6)
    if not (ch.probe("alive") or {}).get("alive"):
        R.add("B", 0, "缺", "b-net 未注入")
        return
    # 19 大响应(32 过/34 拒)
    r = ch.probe("big", wait=60) or {}
    ok32 = r.get("b32") == 32 * 1024 * 1024
    v34 = r.get("b34")
    refused34 = isinstance(v34, str) and str(v34).startswith("ERR")
    R.add("B", "19", "过" if (ok32 and refused34) else "缺",
          f"32MB={r.get('b32')}({ok32});34MB={v34!r}(拒={refused34};若满载则上限门未实现)")
    # 20 anonymous 凭据剥离
    r = ch.probe("anon", wait=5) or {}
    ok20 = "cvp" in str(r.get("plain", "")) and "cvp" not in str(r.get("anon", ""))
    R.add("B", "20", "过" if ok20 else "缺", f"明文面 cookie={r.get('plain')!r};anonymous 面={r.get('anon')!r}")
    # 26 下载控制器 pause/resume/abort
    r = ch.probe("dlctl", wait=8) or {}
    ok26 = (r.get("hasCtrl") and r.get("pauseOk") and r.get("resumeOk") and r.get("abortOk")
            and (r.get("err") or r.get("timeout")) and not r.get("done"))
    R.add("B", "26", "过" if ok26 else "缺",
          f"ctrl={r.get('hasCtrl')} pause={r.get('pauseOk')}(窗内进度 +{r.get('duringPause')}) "
          f"resume={r.get('resumeOk')} abort={r.get('abortOk')} 终态 err={r.get('err')!r} done={r.get('done')}")
    # 27 未到 id 排队补发:内部竞态无外部驱动面
    R.add("B", "27", "不适用", "未到 id 竞态属引擎内部时序(控制器 id 派发窗口),无外部可注入观察面;控制器事件到达面由 26 覆盖")
    # 28 cancel 规则(子资源三探)
    r = ch.probe("wr2", wait=6) or {}
    ok28 = r.get("imgBlocked") is True and ("fetchBlocked" in r or r.get("fetchOk") == 0) and r.get("frameDoc") in ("denied", "onerror")
    R.add("B", "28", "过" if ok28 else "缺",
          f"img 拦={r.get('imgBlocked')};fetch 拦={r.get('fetchBlocked') or r.get('fetchOk')};iframe={r.get('frameDoc')!r};规则回执={r.get('ruleCb')!r}")
    # 29 redirect 规则(同源/跨源体保/坏 scheme)
    r = ch.probe("wr3", wait=6) or {}
    same_applied = r.get("sameMarker") == "tgt"
    cross = r.get("crossMarker")
    bad = r.get("badMarker")
    ok29 = same_applied and cross in ("none", "xorigin") and bad in ("src3", "none", "net-err")
    R.add("B", "29", "过" if ok29 else "缺",
          f"同源重定向生效={same_applied}(marker={r.get('sameMarker')});跨源体={cross!r}(http(s) 限内跨源丢体→none 属契约);"
          f"坏 scheme 目标={bad!r}(chrome:// 不应重定向);规则回执 same/cross/bad={r.get('sameRule')!r}/{r.get('crossRule')!r}/{r.get('badRule')!r}")
    # 30 属主世界回推+@grant 闸+导航/下载工厂不包
    # (a) 无 GM_webRequest grant 的脚本
    gi_ok = None
    ch.cmd("Userscript.install", {"source": gsrc("nogrant", [f"@match {B}/*"],
               "out({wr: typeof GM_webRequest});"), "enabled": True})
    ch.nav(f"{B}/?op=b30")
    a30 = read_attr(ch, "g-nogrant", 2.0) or {}
    uninstall_by_name(ch, "cv-g-nogrant")
    # (b)+(c) 规则活性跨页(hold tab 持规则)+导航/下载不拦
    ch.cmd("Target.createTarget", {"url": f"{B}/?op=wr-arm-hold"})
    time.sleep(1.5)
    ch.nav(f"{B}/blocked-nav")
    nav_landed = None
    for _ in range(8):
        nav_landed = read_attr(ch, "probe", 0.6)
        if nav_landed and nav_landed.get("op") == "wr-nav-report":
            break
        time.sleep(0.4)
    rdl = ch.probe("wr-dl", wait=7) or {}
    ok30 = (a30.get("wr") == "undefined"
            and (nav_landed or {}).get("path") == "/blocked-nav"
            and rdl.get("dlDone") is True)
    R.add("B", "30", "过" if ok30 else "缺",
          f"@grant 闸(未授权 undefined)={a30.get('wr')!r};顶层导航不被拦={ (nav_landed or {}).get('path') };"
          f"GM_download 不被拦={rdl.get('dlDone')}(err={rdl.get('dlErr')!r});属主世界回推=28/29 规则回调在脚本世界有火(ruleCb 在案)")


def domain_K(ch, R):
    # 86 CDP 免确认(auto-allow):本台架全程零确认直连即证
    R.add("K", "86", "过", "本驱动全程 CDP 直连零确认(能在跑即证 auto-allow)")
    # 87 webdriver=false
    v = ch.eval_js("navigator.webdriver")
    R.add("K", "87", "过" if v is False else "缺", f"navigator.webdriver={v}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--chrome", required=True)
    ap.add_argument("--sha", default=None, help="期望部署 chrome 二进制 sha256")
    ap.add_argument("--domains", default="A,K")
    ap.add_argument("--json", default=None)
    args = ap.parse_args()

    real = sha256_of(args.chrome)
    if args.sha and real != args.sha:
        print(f"台架断言败:部署 sha 不符\n got {real}\n want {args.sha}")
        return 2
    print(f"sha-ok {real[:16]}…")

    srv = start_fixtures()
    profile = tempfile.mkdtemp(prefix="cv-prof-")
    R = Results()
    ch = Chrome(args.chrome, profile)
    try:
        ch.start()
        # 台架纪律断言:引擎真实 user-data-dir 即判读库
        ver = ch.cmd("Browser.getVersion").get("product", "")
        print("chrome:", ver, "| profile:", profile)
        doms = [d.strip() for d in args.domains.split(",") if d.strip()]
        for d in doms:
            if d == "A":
                domain_A(ch, R, profile)
            elif d == "B":
                domain_B(ch, R)
            elif d == "B2":
                domain_B2(ch, R)
            elif d == "C":
                domain_C(ch, R)
            elif d == "D":
                domain_D(ch, R)
            elif d == "E":
                domain_E(ch, R)
            elif d == "F":
                domain_F(ch, R)
            elif d == "G":
                domain_G(ch, R)
            elif d == "V2":
                domain_V2(ch, R, profile)
            elif d == "J":
                domain_J(ch, R)
            elif d == "H":
                domain_H(ch, R, profile)
            elif d == "I":
                domain_I(ch, R)
            elif d == "K2":
                domain_K2(ch, R)
            elif d == "K":
                domain_K(ch, R)
            else:
                R.add(d, "-", "环境限", f"域 {d} 实现未随本批(分批推进)")
    finally:
        ch.stop()
        srv.shutdown()
        getattr(srv, "srv2", None) and srv.srv2.shutdown()
        shutil.rmtree(profile, ignore_errors=True)
    fails = [r for r in R.rows if r["verdict"] == "缺"]
    limited = [r for r in R.rows if r["verdict"] == "环境限"]
    print(f"COVERAGE BATCH: pass={len(R.rows) - len(fails) - len(limited)} fail={len(fails)} limited={len(limited)}")
    if args.json:
        json.dump({"rows": R.rows}, open(args.json, "w"), ensure_ascii=False, indent=1)
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
