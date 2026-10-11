# /// script
# requires-python = ">=3.9"
# dependencies = []
# ///
"""run_live.py -- coverage-01 活体探针腿(grok-cases-01 两支 MIT 真脚本)

- utags userscript-api-benchmark 0.1.7:管理页菜单链触发 Run Benchmark,读页面成绩表
- Greasy Fork 598250 v4.3:装前本地补机读出口(attachShadow 钩子+attr 镜像;MIT 允许),加载自动跑
判读:录真脚本活体成绩,与台架判定对照(分歧记不遮)。用法:uv run tests/coverage/run_live.py --chrome <chrome> --sha <sha>
"""
import argparse
import json
import os
import shutil
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from run import Chrome, Results, start_fixtures, sha256_of, read_attr, ws_eval, PORT_FIXED  # noqa: E402

LIVE = "/tmp/cv-live"

GF_HOOK = """
(function () {
  var _as = Element.prototype.attachShadow;
  Element.prototype.attachShadow = function (init) {
    var r = _as.call(this, init);
    (window.__cvShadows = window.__cvShadows || []).push(r);
    return r;
  };
  setInterval(function () {
    if (window.__cvShadows && window.__cvShadows.length) {
      var t = window.__cvShadows.map(function (r) { return r.textContent || ''; }).join('\\n');
      if (t) document.documentElement.setAttribute('gf-results', t.slice(0, 60000));
    }
  }, 500);
})();
"""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--chrome", required=True)
    ap.add_argument("--sha", default=None)
    args = ap.parse_args()
    real = sha256_of(args.chrome)
    if args.sha and real != args.sha:
        print(f"台架断言败:sha {real} != {args.sha}")
        return 2
    print(f"sha-ok {real[:16]}…")
    R = Results()
    srv = start_fixtures()
    profile = tempfile.mkdtemp(prefix="cv-live-")
    B = f"http://127.0.0.1:{PORT_FIXED}"

    gf_raw = open(os.path.join(LIVE, "gf598250.user.js"), encoding="utf-8").read()
    marker = "// ==/UserScript=="
    gf_patched = gf_raw.replace(marker, marker + "\n" + GF_HOOK, 1)

    ch = Chrome(args.chrome, profile)
    try:
        ch.start()
        ch.cmd("Userscript.install", {"source": open(os.path.join(LIVE, "utags.user.js"), encoding="utf-8").read(), "enabled": True})
        ch.cmd("Userscript.install", {"source": gf_patched, "enabled": True})
        # --- GF 598250:加载自动跑 ---
        ch.nav(f"{B}/?op=live-gf")
        gf = None
        for _ in range(40):  # 20s 窗
            gf = read_attr(ch, "gf-results", 0.2)
            if gf:
                break
            time.sleep(0.5)
        gft = (gf or {}).get("raw") or ""
        R.add("LIVE", "gf598250", "过" if gft else "缺",
              f"机读出口={bool(gft)};成绩面摘录见回执(原文 {len(gft)} 字)")
        open("/tmp/cv-live/gf-results.txt", "w", encoding="utf-8").write(gft)
        # --- utags:菜单链触发 ---
        ch.nav(f"{B}/?op=live-utags")  # 匹配页在主 tab(脚本实例在)
        ch.cmd("Target.createTarget", {"url": "chrome://userscripts/"})
        time.sleep(2.0)
        ws_eval("chrome://userscripts", "(function(){var els=Array.prototype.slice.call(document.querySelectorAll('*'))"
                ".filter(function(e){return e.children.length===0&&e.textContent.trim()==='菜单';});"
                "if(els.length){els[0].click();return 1;}return 0;})()")
        time.sleep(1.0)
        menu_items = ws_eval("chrome://userscripts", "document.body ? document.body.innerText : ''") or ""
        clicked = ws_eval("chrome://userscripts", "(function(){var els=Array.prototype.slice.call(document.querySelectorAll('*'))"
                          ".filter(function(e){return e.children.length===0&&e.textContent.trim()==='Run Benchmark';});"
                          "if(els.length){els[0].click();return 'clicked';}return 'nofind';})()")
        time.sleep(30)  # benchmark 跑窗(iframe 面 1s 超时档多)
        tbl = None
        for _ in range(20):
            tbl = None
            try:
                r = ch.cmd("Runtime.evaluate", {"expression":
                           "(document.querySelector('table')?document.querySelector('table').innerText:"
                           "(document.body?document.body.innerText.slice(0,3000):''))",
                           "returnByValue": True})
                tbl = (r.get("result") or {}).get("value") or ""
            except Exception:
                pass
            if tbl and len(tbl) > 200:
                break
            time.sleep(2)
        R.add("LIVE", "utags", "过" if (clicked.startswith("clicked") and tbl and len(tbl) > 200) else "缺",
              f"菜单触发={clicked!r};成绩表面={len(tbl or '')} 字(菜单页快照:{menu_items[:60]!r})")
        open("/tmp/cv-live/utags-results.txt", "w", encoding="utf-8").write(tbl or "")
    finally:
        ch.stop()
        srv.shutdown()
        shutil.rmtree(profile, ignore_errors=True)
    fails = [r for r in R.rows if r["verdict"] == "缺"]
    print(f"LIVE BATCH: pass={len(R.rows) - len(fails)} fail={len(fails)}")
    json.dump({"rows": R.rows}, open("/tmp/cov-live.json", "w"), ensure_ascii=False, indent=1)
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
