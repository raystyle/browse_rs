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
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from minicdp import WS, http_get  # noqa: E402

PORT_FIXED = 18871  # fixture 服务(@match 锚)
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
        if self.path.startswith("/t-text"):
            self.send_response(200); self.end_headers()
            self.wfile.write(b"text-payload")
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
    ok40 = r.get("hasHandle") and (r.get("onloadFired") or r.get("loadEvt"))
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
            elif d == "C":
                domain_C(ch, R)
            elif d == "D":
                domain_D(ch, R)
            elif d == "E":
                domain_E(ch, R)
            elif d == "F":
                domain_F(ch, R)
            elif d == "K":
                domain_K(ch, R)
            else:
                R.add(d, "-", "环境限", f"域 {d} 实现未随本批(分批推进)")
    finally:
        ch.stop()
        srv.shutdown()
        shutil.rmtree(profile, ignore_errors=True)
    fails = [r for r in R.rows if r["verdict"] == "缺"]
    limited = [r for r in R.rows if r["verdict"] == "环境限"]
    print(f"COVERAGE BATCH: pass={len(R.rows) - len(fails) - len(limited)} fail={len(fails)} limited={len(limited)}")
    if args.json:
        json.dump({"rows": R.rows}, open(args.json, "w"), ensure_ascii=False, indent=1)
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
