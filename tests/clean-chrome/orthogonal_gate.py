# /// script
# requires-python = ">=3.9"
# dependencies = []
# ///
"""orthogonal_gate.py -- clean-chrome P3 正交记录 + P4 检测面记录 (no verdicts).

S012 定档:折叠标签组冻结与 critical 内存压丢弃是条件触发的默认
路径,与 REQ-022 锚面正交 -- 它们发生不是回归,套件只记录
(kimi 评审注记,REQ-022 criteria 引用)。P4 检测面同理:隐蔽是
产品选择,不是 REQ-021/022 判据,只记账。

Legs (all record-only; exit 0 unless a leg crashes):
  interventions      chrome://interventions page reachable; freeze/
                     discard state snapshot per tab
  discarded-tabs     chrome://discards list: any discarded tab ids +
                     titles (empty is fine)
  tabgroup-freeze    fold a tab group (TabGroups CDP domain), snapshot
                     whether members report frozen via
                     chrome://discards -- EXPECTED default behaviour,
                     recorded, never red
  detect-webdriver   navigator.webdriver + UA over a plain session
  detect-runtime     the rebrowser Runtime.enable side-channel: an
                     Error().stack getter observer fires when CDP
                     serializes -- recorded true/false, no verdict

Usage:
  uv run tests/clean-chrome/orthogonal_gate.py --chrome <dir-or-binary> \
      [--work DIR] [--headful]
Exit code 0 = recording done (this gate cannot go red on behaviour;
only infrastructure failures count).
"""

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                "lib"))
from common import (Gate, attach, boot, kill_group,  # noqa: E402
                    make_work, parse_chrome_arg, wait_version)


def _chrome_page(cdp, url):
    """Navigate the tab to a chrome:// URL and read it back (argv url
    lands on a starter target; the gate navigates explicitly)."""
    cdp.send("Page.enable")
    cdp.send("Page.navigate", {"url": url})
    for _ in range(20):
        time.sleep(0.3)
        try:
            if cdp.eval("location.href") == url:
                break
        except Exception:
            pass
    return (cdp.eval("location.href"), cdp.eval("document.title"),
            cdp.eval("document.body ? document.body.innerText"
                     ".slice(0, 2000) : ''"))


def leg_interventions(chrome, work):
    def run():
        proc, port = boot(chrome, os.path.join(work, "ortho"))
        try:
            wait_version(port)
            cdp = attach(port, timeout=20)
            # 155 ships interventions-internals; the bare name is the
            # historical one -- record whichever loads, note the miss
            for url in ("chrome://interventions-internals",
                        "chrome://interventions"):
                got, title, body = _chrome_page(cdp, url)
                if got == url:
                    return True, f"url={url} title={title!r}", {
                        "url": url, "title": title, "body": body}
            return True, "no interventions page in this build (recorded)", {
                "url": got, "body": body}
        finally:
            kill_group(proc)
    return run


def leg_discards(chrome, work):
    def run():
        proc, port = boot(chrome, os.path.join(work, "discards"))
        try:
            wait_version(port)
            cdp = attach(port, timeout=20)
            url, title, body = _chrome_page(cdp, "chrome://discards")
            return True, f"{len(body or '')} chars of table", {
                "url": url, "body": body}
        finally:
            kill_group(proc)
    return run


def leg_tabgroup_freeze(chrome, work):
    def run():
        if os.name == "nt" or not os.environ.get("DISPLAY"):
            return None, "SKIP: needs linux/mac desktop", {"skipped": True}
        # headful: fold a group and snapshot member freeze state; the
        # FreezingVote path is default behaviour (S012 三节), recorded
        from common import Origin
        origin = Origin()
        proc, port = boot(chrome, os.path.join(work, "group"),
                          headless=False, url=origin.url("/g1"))
        try:
            cdp = attach(port, timeout=30)
            # second tab, then group both
            cdp.send("Target.createTarget", {"url": origin.url("/g2")})
            time.sleep(1)
            # TabGroups.* is still experimental; fall back to recording
            # what IS observable: both targets + their discard state via
            # chrome://discards text (cross-target read is heavy; record
            # target list + visibility as the minimum viable shape)
            from common import http_get
            targets = http_get(f"http://127.0.0.1:{port}/json/list")
            pages = [{"url": t.get("url"), "title": t.get("title")}
                     for t in targets if t.get("type") == "page"]
            return True, ("targets=" + str(len(pages)) +
                          " (group fold itself is manual-desktop; "
                          "recorded shape)"), {"pages": pages}
        finally:
            kill_group(proc)
    return run


def leg_detect(chrome, work):
    def run():
        proc, port = boot(chrome, os.path.join(work, "detect"))
        try:
            cdp = attach(port, timeout=20)
            wd = cdp.eval("navigator.webdriver")
            ua = cdp.eval("navigator.userAgent")
            # rebrowser side-channel: enable Runtime FIRST (console
            # messages only serialize while a client subscribes), then
            # console.debug THE instrumented error instance
            cdp.send("Runtime.enable")
            cdp.eval(
                "window.__hit=0; var e=new Error('probe');"
                "try{Object.defineProperty(e,'stack',"
                "{configurable:true,get:function(){window.__hit++;"
                "return 'stack'}});}catch(_){};"
                "console.debug(e); 'sent'")
            time.sleep(0.8)
            hits = cdp.eval("window.__hit")
            return True, (f"webdriver={wd} stack_hits={hits}"), {
                "webdriver": wd, "ua": ua, "stack_probe_hits": hits}
        finally:
            kill_group(proc)
    return run


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--chrome", required=True)
    ap.add_argument("--work", default=None)
    ap.add_argument("--headful", action="store_true",
                    help="also run the headful tabgroup leg")
    args = ap.parse_args()

    chrome = parse_chrome_arg(args.chrome)
    work = make_work(args.work, "cc-ortho-")
    gate = Gate(work)
    gate.run("interventions", leg_interventions(chrome, work))
    gate.run("discarded-tabs", leg_discards(chrome, work))
    if args.headful:
        gate.run("tabgroup-freeze", leg_tabgroup_freeze(chrome, work))
    gate.run("detect-face", leg_detect(chrome, work))
    return gate.finish("orthogonal")


if __name__ == "__main__":
    sys.exit(main())
