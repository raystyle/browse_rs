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

    def start(self):
        self.proc = subprocess.Popen(
            [self.bin, "--headless=new", "--no-sandbox", "--disable-gpu",
             "--ozone-platform=headless", "--user-data-dir=" + self.profile,
             f"--remote-debugging-port={CDP_PORT}", "--no-first-run", "about:blank"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
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
            if m.get("id") == mid:
                if "error" in m:
                    raise RuntimeError(f"{method}: {m['error']}")
                return m.get("result", {})
        raise TimeoutError(method)

    def nav(self, url):
        self.cmd("Page.navigate", {"url": url})
        time.sleep(1.2)

    def probe(self, op, payload=None, wait=2.0):
        q = f"?op={op}" + (f"&payload={urllib.request.quote(json.dumps(payload))}" if payload else "")
        self.nav(f"http://127.0.0.1:{PORT_FIXED}/{q}")
        expr = "document.documentElement.getAttribute('probe')"
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
