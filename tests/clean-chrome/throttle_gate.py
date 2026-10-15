# /// script
# requires-python = ">=3.9"
# dependencies = []
# ///
"""throttle_gate.py -- clean-chrome P1 后台节流回归 (headful boxes, per window).

REQ-022 face: the four-layer source-level de-throttling must hold with
NO flags. Behaviour probe = the browse-side heartbeat experiment this
REQ was born from: a page 200ms setInterval, window minimized via CDP
``Browser.setWindowBounds`` (cross-platform; no xdotool/osascript
dependency), gaps between beats read back over CDP.

  heartbeat-main     无旗标 headful: 200ms heartbeat while minimized,
                     6.5 min, no clamping. Regression thresholds
                     (precedents doc, 2026-10-15): >=3 consecutive gaps
                     >=1000ms = L1 regression; any gap >=50000ms = L3
                     regression. Single spikes are scheduling noise.
  heartbeat-depth    L3 line: quantiles before/after the 5-minute mark
                     (intensive throttling engages at 5 min hidden on
                     stock builds; here it must not).
  reverse-blink      A/B 反证 leg A: --enable-blink-features=
                     TimerThrottlingForBackgroundTabs must RE-INTRODUCE
                     ~1s alignment on the hidden page (proves a green
                     main run measured the source default, not a dead
                     probe; green-without-clamping = A/B VOID = red).
  reverse-feature    leg B: --enable-features=IntensiveWakeUpThrottling
                     ALONE must stay healthy (adjudicated 2026-10-15:
                     L3 nests inside the L1 gate in
                     frame_scheduler_impl.cc, so with L1 source-default
                     off, kBackgroundIntensive is structurally
                     unreachable -- a single flag cannot restore L3).
  reverse-both       leg C (the nesting proof): BOTH flags together.
                     Hidden 6.5 min: gaps ~1000ms before the 5-minute
                     grace line (kBackground), then ~60000ms after it
                     (kBackgroundIntensive phase transition).
  occluded-heartbeat L2 face: a second big window covers the first;
                     heartbeat keeps running sub-second (occlusion no
                     longer demotes to hidden post-REQ-022).
  priority-note      L3b face, record-only: renderer /proc shape
                     before/after minimize (no linux verdict: nice is
                     kernel-shaped; the record feeds the win QoS
                     spot-check comparison).

Usage:
  uv run tests/clean-chrome/throttle_gate.py --chrome <dir-or-binary> \
      [--work DIR] [--main-seconds 390] [--reverse-seconds 120] \
      [--skip-reverse]
Exit code 0 = all legs green. 快跑联调用 --main-seconds 60.
"""

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                "lib"))
from common import (Gate, Origin, attach, boot, kill_group,  # noqa: E402
                    make_work, parse_chrome_arg)

IS_MAC = os.name != "nt" and sys.platform == "darwin"

# precedents doc thresholds (single spike = noise, not regression)
L1_RUN_LEN, L1_GAP_MS = 3, 1000
L3_GAP_MS = 50000
DEPTH_LINE_S = 300  # stock intensive-throttling engages at 5 min hidden

HEARTBEAT_JS = (
    "(function(){window.__t=[];window.__s=performance.now();"
    "window.__id=setInterval(function(){window.__t.push(performance.now()-window.__s)},200);"
    "return 'armed';})()"
)

READ_GAPS_JS = (
    "(function(){var a=window.__t||[];var g=[];"
    "for(var i=1;i<a.length;i++)g.push(Math.round(a[i]-a[i-1]));"
    "return JSON.stringify(g);})()"
)


def quantiles(sorted_vals, q):
    if not sorted_vals:
        return None
    idx = min(len(sorted_vals) - 1,
              max(0, int(round(q * (len(sorted_vals) - 1)))))
    return sorted_vals[idx]


def best_run(gaps, threshold):
    """Longest run of consecutive gaps >= threshold (L1 alignment
    shape; single spikes are noise)."""
    run = best = 0
    for g in gaps:
        run = run + 1 if g >= threshold else 0
        best = max(best, run)
    return best


def classify(gaps):
    """(ok, reason): L1 regression = >=3 consecutive >=1s; L3 = any
    >=50s gap; healthy otherwise. Aligned with the precedents doc."""
    best = best_run(gaps, L1_GAP_MS)
    n_l3 = sum(1 for g in gaps if g >= L3_GAP_MS)
    run = best
    mx = max(gaps) if gaps else 0
    if n_l3:
        return False, (f"L3 regression: {n_l3} gap(s) >= {L3_GAP_MS}ms "
                       f"(max {mx}ms)")
    if best >= L1_RUN_LEN:
        return False, (f"L1 regression: {best} consecutive gaps "
                       f">= {L1_GAP_MS}ms (max {mx}ms)")
    return True, f"max_gap={mx}ms beats={len(gaps) + 1} run1s={best}"


def window_id(cdp):
    r = cdp.send("Browser.getWindowForTarget")
    return r.get("result", {}).get("windowId")


def hidden(cdp):
    try:
        return cdp.eval("document.visibilityState") == "hidden"
    except Exception:
        return False


def _x_window(port):
    """X11 window id by page title (xdotool fallback path)."""
    import subprocess
    from common import http_get
    try:
        targets = http_get(f"http://127.0.0.1:{port}/json/list")
        title = next(t["title"] for t in targets if t.get("type") == "page")
        out = subprocess.run(["xdotool", "search", "--name", title],
                             capture_output=True, text=True).stdout
        ids = [int(x) for x in out.split()]
        return ids[-1] if ids else None
    except Exception:
        return None


def minimize_hidden(cdp, port):
    """Minimize the window AND prove the page went hidden; tries CDP
    setWindowBounds, then xdotool (X11), then osascript (mac). Returns
    the method that worked or None (WM ignored every form -- the leg
    must SKIP, a vacuous green would lie)."""
    import subprocess
    wid = window_id(cdp)
    if wid is not None:
        cdp.send("Browser.setWindowBounds", {
            "windowId": wid, "bounds": {"windowState": "minimized"}})
        for _ in range(10):
            if hidden(cdp):
                return "cdp"
            time.sleep(0.3)
    xwid = _x_window(port)
    if xwid is not None:
        subprocess.run(["xdotool", "windowminimize", str(xwid)],
                       capture_output=True)
        for _ in range(10):
            if hidden(cdp):
                return "xdotool"
            time.sleep(0.3)
    if IS_MAC:
        for proc_name in ("Chromium", "chromium"):
            subprocess.run(["osascript", "-e",
                            f'tell application "System Events" to tell '
                            f'process "{proc_name}" to set miniaturized '
                            f'of window 1 to true'], capture_output=True)
            for _ in range(10):
                if hidden(cdp):
                    return "osascript"
                time.sleep(0.3)
    return None


def restore(cdp):
    r = cdp.send("Browser.getWindowForTarget")
    wid = r.get("result", {}).get("windowId")
    if wid is not None:
        cdp.send("Browser.setWindowBounds", {
            "windowId": wid, "bounds": {"windowState": "normal"}})
    if os.name != "nt" and not IS_MAC:
        import subprocess
        subprocess.run(["xdotool", "key", "alt+Tab"], capture_output=True)




def wait_page(cdp, path, timeout=15):
    """Poll until the tab is on <path> and loaded (attach can land on
    the about:blank starter before the boot URL commits)."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            if (cdp.eval("location.pathname") == path
                    and cdp.eval("document.readyState") in
                    ("interactive", "complete")):
                return True
        except Exception:
            pass
        time.sleep(0.3)
    return False


def arm_heartbeat(cdp, path):
    """Wait for the page, arm the heartbeat, verify; None on failure
    with the reason. A dead probe must fail the leg, not pass it."""
    if not wait_page(cdp, path):
        return f"page {path} never became ready"
    if cdp.eval(HEARTBEAT_JS) != "armed":
        return "heartbeat did not arm"
    return None


def min_beats(seconds):
    """Plausible beat floor: 200ms cadence, we accept 30% starvation."""
    return max(5, int(seconds / 0.2 * 0.3))


# ---------------------------------------------------------------- renderer prio (L3b note)

def renderer_pids(profile):
    """Renderer pids: /proc scan for --type=renderer + same profile."""
    pids = []
    for pid in os.listdir("/proc"):
        if not pid.isdigit():
            continue
        try:
            with open(f"/proc/{pid}/cmdline", "rb") as f:
                cmd = f.read().replace(b"\0", b" ").decode(errors="replace")
            if "--type=renderer" in cmd and profile in cmd:
                pids.append(int(pid))
        except OSError:
            continue
    return pids


def proc_priority_note(pid):
    """Record-only snapshot (linux shape); nice field 19 of stat."""
    out = {}
    try:
        with open(f"/proc/{pid}/stat") as f:
            parts = f.read().rsplit(")", 1)[1].split()
        out["nice"] = int(parts[16])  # field 19, minus pid+comm prefix
    except Exception:
        pass
    try:
        with open(f"/proc/{pid}/cgroup") as f:
            out["cgroup"] = f.read().strip().replace("\n", "; ")
    except Exception:
        pass
    return out


# ---------------------------------------------------------------- legs

def heartbeat_run(chrome, work, label, seconds, extra_args=(),
                  want_throttle=False, note_extra=""):
    """Boot headful, arm heartbeat, minimize, wait, read gaps."""
    def run():
        if not os.environ.get("DISPLAY") and not IS_MAC and os.name != "nt":
            return None, "SKIP: no display (headful only)", {"skipped": True}
        profile = os.path.join(work, label)
        origin = Origin()
        proc, port = boot(chrome, profile, headless=False,
                          log=os.path.join(work, f"chrome-{label}.log"),
                          url=origin.url("/hb"), extra_args=extra_args)
        try:
            cdp = attach(port, timeout=30)
            problem = arm_heartbeat(cdp, "/hb")
            if problem:
                return False, problem, None
            method = minimize_hidden(cdp, port)
            if method is None:
                return None, ("SKIP: no minimize form hid the page "
                              "(WM ignored; needs a real desktop)"), {
                    "skipped": True}
            time.sleep(seconds)
            still_hidden = hidden(cdp)
            read = cdp.eval(READ_GAPS_JS)
            restore(cdp)
            gaps = json.loads(read) if read else []
            # healthy cadence floor for no-flag legs; A/B legs EXPECT
            # ~1s (L1) or ~60s (L3) alignment, so their floor is the
            # clamped cadence -- a probe reading exactly ~1000ms gaps
            # is the positive signal, not starvation
            floor_s = (1.05 if want_throttle else 0.2)  # per-beat seconds
            min_b = max(5, int(seconds / floor_s * 0.5))
            if len(gaps) + 1 < min_b:
                return False, (f"probe starved: {len(gaps) + 1} beats in "
                               f"{seconds}s (min {min_b})"), {
                    "gaps": gaps, "label": label}
            ok, reason = classify(gaps)
            data = {"gaps": gaps, "label": label,
                    "minimized_via": method, "still_hidden": still_hidden}
            if want_throttle:
                if ok:
                    return False, (f"A/B VOID: clamping expected but probe "
                                   f"healthy ({reason}) -- reversal flag "
                                   f"ineffective or probe broken"), data
                return True, f"A/B positive: clamping observed ({reason})", data
            return ok, reason + note_extra, data
        finally:
            kill_group(proc)
    return run


def depth_leg(chrome, work, seconds):
    """5-minute line: sample before/after the mark inside one hidden
    session, verdict per half."""
    def run():
        if not os.environ.get("DISPLAY") and not IS_MAC and os.name != "nt":
            return None, "SKIP: no display (headful only)", {"skipped": True}
        profile = os.path.join(work, "depth")
        origin = Origin()
        proc, port = boot(chrome, profile, headless=False,
                          log=os.path.join(work, "chrome-depth.log"),
                          url=origin.url("/depth"))
        try:
            cdp = attach(port, timeout=30)
            problem = arm_heartbeat(cdp, "/depth")
            if problem:
                return False, problem, None
            method = minimize_hidden(cdp, port)
            if method is None:
                return None, "SKIP: no minimize form hid the page", {
                    "skipped": True}
            # early sample ends 30s before the line; late sample is the
            # remainder, so both halves share one continuous hidden run
            time.sleep(DEPTH_LINE_S - 30)
            early = json.loads(cdp.eval(READ_GAPS_JS) or "[]")
            time.sleep(seconds - (DEPTH_LINE_S - 30))
            late_full = json.loads(cdp.eval(READ_GAPS_JS) or "[]")
            still_hidden = hidden(cdp)
            restore(cdp)
            late = late_full[len(early):] if len(late_full) > len(early) \
                else late_full
            if len(early) + 1 < min_beats(DEPTH_LINE_S - 30):
                return False, (f"early probe starved: {len(early) + 1} "
                               f"beats"), {"early_gaps": early,
                                           "late_gaps": late}
            if len(late) + 1 < min_beats(seconds - DEPTH_LINE_S + 30):
                return False, (f"late probe starved: {len(late) + 1} "
                               f"beats"), {"early_gaps": early,
                                           "late_gaps": late}
            ok_e, why_e = classify(early)
            ok_l, why_l = classify(late)
            q = lambda gs: {"p50": quantiles(sorted(gs), 0.50),
                            "p99": quantiles(sorted(gs), 0.99),
                            "max": max(gs) if gs else None}
            return ok_e and ok_l, f"early[{why_e}] late[{why_l}]", {
                "early": q(early), "late": q(late),
                "minimized_via": method, "still_hidden": still_hidden,
                "early_gaps": early, "late_gaps": late}
        finally:
            kill_group(proc)
    return run


def occlusion_leg(chrome, work, seconds):
    """L2: second window fully covers the first; sub-second heartbeat."""
    def run():
        if not os.environ.get("DISPLAY") and not IS_MAC and os.name != "nt":
            return None, "SKIP: no display (headful only)", {"skipped": True}
        profile = os.path.join(work, "occluded")
        origin = Origin()
        proc, port = boot(chrome, profile, headless=False,
                          log=os.path.join(work, "chrome-occluded.log"),
                          url=origin.url("/victim"))
        try:
            cdp = attach(port, timeout=30)
            problem = arm_heartbeat(cdp, "/victim")
            if problem:
                return False, problem, None
            # a big new window opens front and covers the victim on the
            # single-screen WSLg/desktop default
            r = cdp.send("Target.createTarget", {
                "url": origin.url("/cover"), "newWindow": True,
                "width": 2560, "height": 1440})
            time.sleep(seconds)
            gaps = json.loads(cdp.eval(READ_GAPS_JS) or "[]")
            vis = cdp.eval("document.visibilityState")
            if len(gaps) + 1 < min_beats(seconds):
                return False, (f"probe starved: {len(gaps) + 1} beats in "
                               f"{seconds}s"), {"gaps": gaps}
            ok, reason = classify(gaps)
            return ok, f"occluded {reason} victim={vis}", {
                "gaps": gaps, "victim_visibility": vis}
        finally:
            kill_group(proc)
    return run


def reverse_both_leg(chrome, work, seconds):
    """Nesting proof (adjudicated 2026-10-15): both reversal flags
    together must show the kBackground -> kBackgroundIntensive phase
    transition at the 5-minute hidden line -- ~1s gaps before, ~60s
    gaps after. L1 alone (reverse-blink) and L3 alone (reverse-feature)
    bracket this leg; the three together are the living proof of the
    frame_scheduler_impl.cc nesting."""
    def run():
        if not os.environ.get("DISPLAY") and not IS_MAC and os.name != "nt":
            return None, "SKIP: no display (headful only)", {"skipped": True}
        profile = os.path.join(work, "rev-both")
        origin = Origin()
        proc, port = boot(chrome, profile, headless=False,
                          log=os.path.join(work, "chrome-rev-both.log"),
                          url=origin.url("/hb"),
                          extra_args=["--enable-blink-features="
                                      "TimerThrottlingForBackgroundTabs",
                                      "--enable-features="
                                      "IntensiveWakeUpThrottling"])
        try:
            cdp = attach(port, timeout=30)
            problem = arm_heartbeat(cdp, "/hb")
            if problem:
                return False, problem, None
            method = minimize_hidden(cdp, port)
            if method is None:
                return None, "SKIP: no minimize form hid the page", {
                    "skipped": True}
            # early sample ends 5s before the grace line; late sample is
            # the remainder, so the transition gap lands in "late"
            time.sleep(DEPTH_LINE_S - 5)
            early = json.loads(cdp.eval(READ_GAPS_JS) or "[]")
            time.sleep(seconds - (DEPTH_LINE_S - 5))
            late_full = json.loads(cdp.eval(READ_GAPS_JS) or "[]")
            still_hidden = hidden(cdp)
            restore(cdp)
            late = late_full[len(early):] if len(late_full) > len(early) \
                else late_full
            # the transition lands by PAGE clock, not by our read
            # clock (hidden-start offsets), so the verdict walks the
            # combined sequence: a ~1s alignment run first, then the
            # >=50s intensive gaps, in that order
            seq = early + late
            covered_ms = sum(seq)
            if covered_ms < 0.5 * seconds * 1000:
                return False, (f"probe starved: {covered_ms / 1000:.0f}s "
                               f"covered of {seconds}s"), {
                    "early_gaps": early, "late_gaps": late}
            l1_idx = l3_idx = None
            for i in range(len(seq) - 2):
                if all(950 <= g <= 2000 for g in seq[i:i + 3]):
                    l1_idx = i
                    break
            for i, g in enumerate(seq):
                if g >= L3_GAP_MS:
                    l3_idx = i
                    break
            l1_phase = l1_idx is not None
            l3_phase = l3_idx is not None
            ordered = (l1_phase and l3_phase and l1_idx < l3_idx)
            ok = ordered
            note = (f"1s-run@{l1_idx} intensive@{l3_idx} "
                    f"n_ge50s={sum(1 for g in seq if g >= L3_GAP_MS)} "
                    f"covered={covered_ms / 1000:.0f}s")
            return ok, (note + " phase-transition="
                        f"{'seen' if ok else 'missing'}"), {
                "early_gaps": early, "late_gaps": late,
                "minimized_via": method, "still_hidden": still_hidden}
        finally:
            kill_group(proc)
    return run


def priority_note_leg(chrome, work):
    """L3b, record-only: renderer /proc shape before/after minimize."""
    def run():
        if os.name == "nt" or IS_MAC:
            return None, ("SKIP: linux /proc shape only (win/mac QoS "
                          "spot-check runs their own)"), {"skipped": True}
        profile = os.path.join(work, "prio")
        origin = Origin()
        proc, port = boot(chrome, profile, headless=False,
                          log=os.path.join(work, "chrome-prio.log"),
                          url=origin.url("/prio"))
        try:
            cdp = attach(port, timeout=30)
            problem = arm_heartbeat(cdp, "/prio")
            if problem:
                return None, f"SKIP: {problem}", {"skipped": True}
            before = {pid: proc_priority_note(pid)
                      for pid in renderer_pids(profile)}
            method = minimize_hidden(cdp, port)
            if method is None:
                return None, "SKIP: no minimize form hid the page", {
                    "skipped": True}
            time.sleep(5)
            after = {pid: proc_priority_note(pid)
                     for pid in renderer_pids(profile)}
            restore(cdp)
            return True, "recorded (no linux verdict: kernel-shaped)", {
                "before": before, "after": after}
        finally:
            kill_group(proc)
    return run


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--chrome", required=True)
    ap.add_argument("--work", default=None)
    ap.add_argument("--main-seconds", type=int, default=390,
                    help="heartbeat-main duration (6.5 min default)")
    ap.add_argument("--depth-seconds", type=int, default=330,
                    help="depth leg hidden duration (crosses the 5-min "
                         "line; floor 330)")
    ap.add_argument("--reverse-seconds", type=int, default=120,
                    help="reverse-blink duration (L1 reverse is immediate)")
    ap.add_argument("--reverse-feature-seconds", type=int, default=330,
                    help="reverse-feature duration (must cross the 5-min "
                         "line; floor 330)")
    ap.add_argument("--reverse-both-seconds", type=int, default=390,
                    help="reverse-both duration (phase transition at the "
                         "5-min line; floor 390)")
    ap.add_argument("--occluded-seconds", type=int, default=120)
    ap.add_argument("--skip-reverse", action="store_true")
    ap.add_argument("--leg", default=None,
                    help="regex: run only matching legs")
    args = ap.parse_args()

    chrome = parse_chrome_arg(args.chrome)
    work = make_work(args.work, "cc-throttle-")
    gate = Gate(work)
    if args.leg:
        import re as _re
        _pat = _re.compile(args.leg)
        _orig = Gate.run
        def _filtered(self, name, fn):
            if _pat.search(name):
                return _orig(self, name, fn)
            self.results.append({"name": name, "ok": None, "skipped": True,
                                 "note": "SKIP: --leg filter", "data": None})
            return None
        Gate.run = _filtered
        print(f"leg filter: {args.leg}")
    gate.run("heartbeat-main",
             heartbeat_run(chrome, work, "main", args.main_seconds,
                           note_extra=" (no flags, minimized)"))
    gate.run("heartbeat-depth",
             depth_leg(chrome, work, max(330, args.depth_seconds)))
    gate.run("occluded-heartbeat",
             occlusion_leg(chrome, work, args.occluded_seconds))
    gate.run("priority-note", priority_note_leg(chrome, work))
    if not args.skip_reverse:
        gate.run("reverse-blink",
                 heartbeat_run(chrome, work, "rev-blink",
                               args.reverse_seconds,
                               extra_args=["--enable-blink-features="
                                           "TimerThrottlingForBackgroundTabs"],
                               want_throttle=True))
        gate.run("reverse-feature",
                 heartbeat_run(chrome, work, "rev-feat",
                               max(330, args.reverse_feature_seconds),
                               extra_args=["--enable-features="
                                           "IntensiveWakeUpThrottling"],
                               want_throttle=False,
                               note_extra=" (L3 unreachable with L1 "
                                          "closed: nesting, see "
                                          "reverse-both)"))
        gate.run("reverse-both",
                 reverse_both_leg(chrome, work,
                                  max(390, args.reverse_both_seconds)))
    return gate.finish("throttle")


if __name__ == "__main__":
    sys.exit(main())
