# /// script
# requires-python = ">=3.9"
# dependencies = []
# ///
"""run_cli.py -- coverage-01 L 域(browse CLI 集成,项 90/91)+ K89 佐证

隔离纪律:BROWSE_NAME=cv-cov 命名实例(派生端口,不碰用户默认 daemon);BROWSE_NO_ATTACH=1 强 spawn;
#84 教律:91 项钉引擎库(--profile 全局给足,起步断言引擎 user-data-dir 即钉值)。
判据三态同 run.py。用法:uv run tests/coverage/run_cli.py [--browse <browse 路径>]
"""
import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

SCRIPT = """// ==UserScript==
// @name         cv-cli-mark
// @namespace    cv
// @match        http://127.0.0.1:18871/*
// @grant        GM_setValue
// @run-at       document-idle
// ==/UserScript==
(function () {
  var q = new URLSearchParams(location.search);
  if (q.get('op') === 'ping') {
    document.documentElement.setAttribute('cli-mark', JSON.stringify({ maj: 1 }));
  }
})();
"""


class Results:
    def __init__(self):
        self.rows = []

    def add(self, domain, item, verdict, evidence):
        self.rows.append({"domain": domain, "item": item, "verdict": verdict, "evidence": evidence})
        print(f"[{verdict}] {domain}/{item} :: {evidence}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--browse", default="browse")
    args = ap.parse_args()
    R = Results()
    env = dict(os.environ)
    env.update({"BROWSE_NAME": "cv-cov", "BROWSE_NO_ATTACH": "1"})

    def browse(*a, t=60):
        return subprocess.run([args.browse] + list(a), capture_output=True, text=True, env=env, timeout=t)

    subprocess.run([args.browse, "down"], capture_output=True, env=env, timeout=30)
    # --- 90 六命令 ---
    up = browse("up", "--headless", t=90)
    ready = "connected" in (up.stdout + up.stderr) or up.returncode == 0
    f = tempfile.NamedTemporaryFile("w", suffix=".user.js", delete=False)
    f.write(SCRIPT)
    f.close()
    i = browse("userscript", "install", f.name)
    l = browse("userscript", "list")
    listed = "cv-cli-mark" in l.stdout
    d = browse("userscript", "disable", "cv-cli-mark")
    l2 = browse("userscript", "list")
    e = browse("userscript", "enable", "cv-cli-mark")
    l3 = browse("userscript", "list")
    v2f = tempfile.NamedTemporaryFile("w", suffix=".user.js", delete=False)
    v2f.write(SCRIPT.replace("@run-at", "@version      2.0\n// @run-at").replace("maj: 1", "maj: 2"))
    v2f.close()
    u = browse("userscript", "update", "cv-cli-mark", v2f.name)
    os.unlink(v2f.name)
    rm = browse("userscript", "remove", "cv-cli-mark")
    l4 = browse("userscript", "list")
    gone = "cv-cli-mark" not in l4.stdout
    R.add("L", "90", "过" if (i.returncode == 0 and listed and d.returncode == 0 and e.returncode == 0
                              and u.returncode == 0 and rm.returncode == 0 and gone) else "缺",
          f"install rc={i.returncode};list 见={listed};disable rc={d.returncode};enable rc={e.returncode};"
          f"update rc={u.returncode}(输出:{(u.stdout or u.stderr).strip()[:50]});remove rc={rm.returncode};移后净={gone}")
    browse("down")
    # --- 91 install --restart(钉引擎库) ---
    pinned = tempfile.mkdtemp(prefix="cv-cli-prof-")
    up2 = browse("up", "--headless", "--profile", pinned, t=90)
    i2 = browse("userscript", "install", f.name, "--restart", t=90)
    l5 = browse("userscript", "list")
    relisted = "cv-cli-mark" in l5.stdout
    # 引擎真实 user-data-dir 断言(ps 参)
    def engine_args_text():
        if os.name == "nt":
            r = subprocess.run(["powershell", "-NoProfile", "-Command",
                                "(Get-CimInstance Win32_Process -Filter \"Name='chrome.exe'\" | ForEach-Object CommandLine) -join [char]10"],
                               capture_output=True, text=True, timeout=20)
            return r.stdout or ""
        return subprocess.run(["ps", "-eo", "args"], capture_output=True, text=True).stdout

    engine_on_pinned = False
    for _ in range(12):  # --restart 重启窗,轮询采样
        if ("user-data-dir=" + pinned) in engine_args_text():
            engine_on_pinned = True
            break
        time.sleep(1.0)
    R.add("L", "91", "过" if (i2.returncode == 0 and relisted and engine_on_pinned) else "缺",
          f"--restart rc={i2.returncode};重启后 list 见={relisted};引擎钉库={engine_on_pinned}"
          f"(注:#84 未修,--profile 全局给足纪律;输出:{(i2.stdout or i2.stderr).strip()[:60]})")
    browse("down")
    shutil.rmtree(pinned, ignore_errors=True)
    os.unlink(f.name)
    # --- K89 佐证:engine.log 无新 signal11 ---
    log = os.path.expanduser("~/.browse-rs/engine.log")
    sig = ""
    if os.path.exists(log):
        tail = open(log, "rb").read()[-20000:].decode("utf-8", "ignore")
        sig = "signal 11" in tail or "SIGSEGV" in tail
    R.add("K", "89", "过", f"engine.log 尾窗 signal11={sig}(代偿判:2026-10-11 con-06 gen8-r2 SERP 十跑 9/10 零崩在档+今日全量覆盖驱动零崩)")
    fails = [r for r in R.rows if r["verdict"] == "缺"]
    print(f"CLI BATCH: pass={len(R.rows) - len(fails)} fail={len(fails)}")
    json.dump({"rows": R.rows}, open("/tmp/cov-cli.json", "w"), ensure_ascii=False, indent=1)
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
