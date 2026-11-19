"""Benchmarks ShareX's and ShareXrs's region capture (Windows).

For each app (alternately), per app start: wait until it's idle, then run
`--per-start` region captures, each timed from the trigger (a second copy
started with `-RectangleRegion`, which hands the command line to the running
one) to:
  received   the running copy logging "Arguments received" (its log's ms
             timestamps), so the second copy's start can be told apart
  created    a new overlay window existing, still hidden (ShareXrs creates
             it hidden and paints it before showing it)
  visible    the overlay window shown (EnumWindows polling, ~1-3 ms)
  painted    a PrintWindow of the overlay showing the frozen screen, i.e.
             `sharexrs --test-pattern` (kept as bounds: `painted` is the end
             of the first capture with it, `painted_after` the start of the
             last one without; a full-screen PrintWindow takes 100+ ms)
then drags a fixed region (400,300 -> 800,600, physical pixels) and times,
from the button release, the overlay closing and the saved file appearing
in the task's screenshots folder (`file_seen`) and being complete
(`file_done`: the PNG's IEND chunk is written).

Safety: `sharexrs --test-pattern` must cover the screen for the whole run,
so the apps only ever freeze the pattern. The bench refuses to start
without it, checks it before every trigger, and checks every overlay
capture against the pattern: a capture that isn't mostly the pattern stops
the run. Captures stay in memory; nothing is written except with `verify`,
which saves one overlay capture, runs `scripts/pattern-check.py --delete`
on it and deletes it anyway. Saved region files are never opened beyond
their PNG header (size) and end (IEND), and are deleted by the caller.

Each app needs a portable copy whose default task saves to a folder only
(AfterCaptureJob SaveImageToFile, OverrideScreenshotsFolder), no sound or
toast, and no other copy running (single instance).

  python scripts/bench-region-capture.py verify --config scripts/bench-region-capture.json --app sharex
  python scripts/bench-region-capture.py bench --config scripts/bench-region-capture.json --rounds 8 --out r.jsonl
  python scripts/bench-region-capture.py report r.jsonl
"""

import argparse
import ctypes as C
import datetime
import importlib.util
import json
import math
import os
import re
import subprocess
import sys
import time

import numpy as np

_here = os.path.dirname(os.path.abspath(__file__))
_spec = importlib.util.spec_from_file_location("bench_window_open", os.path.join(_here, "bench-window-open.py"))
bw = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(bw)
W, user32, now = bw.W, bw.user32, bw.now

GA_ROOT = 2


# ---------------------------------------------------------------- the pattern

def expected_pattern(x0, y0, w, h, step):
    """`test_pattern::pixel` (not the alt variant) sampled every `step`
    pixels of a w x h area at screen (x0, y0), as RGB int16."""
    ys, xs = np.mgrid[0:h:step, 0:w:step]
    sx, sy = xs + x0, ys + y0
    checker = ((sx // 16) + (sy // 16)) % 2 == 0
    r, g = sx % 256, sy % 256
    b = np.where(checker, 200, 70)
    p = np.stack([r, g, b], axis=2)
    p[(sx % 64 == 0) | (sy % 64 == 0)] = 255
    return p.astype(np.int16)


def dim(p, alpha=51):
    return ((p * (255 - alpha) + 127) // 255).astype(np.int16)


class PatternMatcher:
    """Share of a capture's sampled pixels that are the pattern (or the
    pattern under an overlay's 20% black), as pattern-check.py counts."""

    def __init__(self, step=7):
        self.step = step
        self.cache = {}

    def share(self, frame_bgr, origin):
        h, w = frame_bgr.shape[:2]
        key = (origin, w, h)
        if key not in self.cache:
            p = expected_pattern(origin[0], origin[1], w, h, self.step)
            self.cache = {key: (p, dim(p))}
        p, d = self.cache[key]
        s = frame_bgr[::self.step, ::self.step, ::-1].astype(np.int16)
        ok = (np.abs(s - p).max(axis=2) <= 2) | (np.abs(s - d).max(axis=2) <= 2)
        return float(ok.mean())


def pattern_windows(pid):
    return [h for h, _ in bw.windows_of({pid}) if bw.shown(bw.window_info(h))]


def check_pattern(pid):
    """The test pattern is running and shown, and nothing visible is above it
    other than the apps' overlays (checked by the caller's captures)."""
    if not bw.pid_alive(pid):
        raise RuntimeError("the test pattern isn't running")
    wins = pattern_windows(pid)
    if not wins:
        raise RuntimeError("the test pattern has no window shown")
    return wins


# ---------------------------------------------------------------- logs

LOG_TS = re.compile(r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d{3}) - (.*)$")


class LogTail:
    """New lines of an app's newest log file since `mark()`."""

    def __init__(self, folder):
        self.folder = folder
        self.path = None
        self.pos = 0

    def mark(self):
        files = [os.path.join(self.folder, f) for f in os.listdir(self.folder)] if os.path.isdir(self.folder) else []
        self.path = max(files, key=os.path.getmtime) if files else None
        self.pos = os.path.getsize(self.path) if self.path else 0

    def lines(self):
        if not self.path:
            self.mark()
            self.pos = 0
            if not self.path:
                return []
        with open(self.path, "rb") as f:
            f.seek(self.pos)
            data = f.read()
        out = []
        for ln in data.decode("utf-8", "replace").splitlines():
            m = LOG_TS.match(ln)
            if m:
                ts = datetime.datetime.strptime(m.group(1), "%Y-%m-%d %H:%M:%S.%f").timestamp()
                out.append((ts, m.group(2)))
        return out


# ---------------------------------------------------------------- input

def root_at(x, y):
    return user32.GetAncestor(user32.WindowFromPoint(W.POINT(x, y)), GA_ROOT)


def drag(overlay, start, end, steps=20):
    """Left-drags start -> end over the overlay; returns the release time."""
    if root_at(*start) != overlay:
        raise RuntimeError(f"{start} isn't over the overlay ({bw.window_info(root_at(*start))})")
    bw.GUARD.check()
    bw.move_to(*start)
    time.sleep(0.12)
    if root_at(*start) != overlay:
        raise RuntimeError("the overlay went away from under the pointer")
    bw.send_button(True)
    time.sleep(0.05)
    for i in range(1, steps + 1):
        bw.move_to(start[0] + (end[0] - start[0]) * i // steps, start[1] + (end[1] - start[1]) * i // steps)
        time.sleep(0.012)
    time.sleep(0.1)
    t = now()
    bw.send_button(False)
    bw.GUARD.sent()
    return t


# ---------------------------------------------------------------- one capture

def overlay_candidates(pids, title_re):
    out = {}
    for h, _ in bw.windows_of(pids):
        info = bw.window_info(h)
        if re.search(title_re, info["title"]):
            out[h] = info
    return out


def client_origin(hwnd):
    pt = W.POINT(0, 0)
    user32.ClientToScreen(hwnd, C.byref(pt))
    return pt.x, pt.y


def png_info(path):
    """(width, height, complete) from a PNG's header and last bytes."""
    try:
        with open(path, "rb") as f:
            head = f.read(24)
            f.seek(0, 2)
            size = f.tell()
            if size < 12:
                return None, None, False
            f.seek(size - 12)
            tail = f.read(12)
    except OSError:
        return None, None, False
    if len(head) < 24 or head[:8] != b"\x89PNG\r\n\x1a\n":
        return None, None, False
    w, h = int.from_bytes(head[16:20], "big"), int.from_bytes(head[20:24], "big")
    return w, h, tail[4:8] == b"IEND"


def one_capture(app, proc, pattern_pid, matcher, log, save=True, keep_capture=None, timeout=15.0):
    pids = {proc.pid}
    check_pattern(pattern_pid)
    before = overlay_candidates(pids, app["overlay_title"])
    shown_before = {h for h, i in before.items() if bw.shown(i)}
    if shown_before:
        raise RuntimeError("an overlay is already open")
    folder = app["saved"]
    files_before = set(os.listdir(folder))
    log.mark()
    bw.GUARD.check()
    wall0 = time.time()
    t0 = now()
    trigger = subprocess.Popen([app["exe"], "-RectangleRegion"], cwd=os.path.dirname(app["exe"]))
    rec = {"created": None, "visible": None, "painted": None, "painted_after": None, "reused_window": None}
    hwnd = None
    deadline = t0 + timeout
    polls = 0
    while now() < deadline:
        polls += 1
        for h, _ in bw.windows_of(pids):
            info = bw.window_info(h)
            if not re.search(app["overlay_title"], info["title"]):
                continue
            # a new overlay window, still hidden (ShareXrs paints before showing)
            if rec["created"] is None and h not in before:
                rec["created"] = (now() - t0) * 1000
            if bw.shown(info):
                hwnd = h
                break
        if hwnd:
            rec["visible"] = (now() - t0) * 1000
            rec["reused_window"] = hwnd in before  # ShareX warms its window up hidden
            break
        time.sleep(0.001)
    rec["polls"] = polls
    if not hwnd:
        rec["error"] = "no overlay"
        return rec
    rec["overlay_rect"] = bw.window_info(hwnd)["rect"]
    cap = bw.Capturer()
    prev_start = rec["visible"] / 1000
    durations = []
    best = 0.0
    while now() < deadline:
        ts = now()
        frame = cap.grab(hwnd)
        te = now()
        if frame is None:
            if not user32.IsWindow(hwnd):
                rec["error"] = "overlay closed during capture"
                return rec
            continue
        durations.append(te - ts)
        share = matcher.share(frame, client_origin(hwnd))
        best = max(best, share)
        if share >= 0.5:
            rec["painted"] = (te - t0) * 1000
            rec["painted_after"] = prev_start * 1000
            rec["pattern_share"] = share
            if keep_capture:
                bw.save_png(frame, keep_capture)
            break
        prev_start = ts - t0
        time.sleep(0.002)
    rec["capture_ms"] = float(np.median(durations) * 1000) if durations else None
    if rec["painted"] is None:
        # Not the pattern: something else may be frozen on it. Stop everything.
        rec["error"] = f"overlay never showed the pattern (best share {best:.3f})"
        user32.PostMessageW(hwnd, bw.WM_CLOSE, 0, 0)
        return rec
    # when the running copy got the command line
    for ts, msg in log.lines():
        if msg.startswith("Arguments received") and "RectangleRegion" in msg and ts >= wall0 - 0.05:
            rec["received"] = (ts - wall0) * 1000
            break
    try:
        trigger.wait(timeout=10)
    except subprocess.TimeoutExpired:
        rec["trigger_copy"] = "still running"
    if not save:
        return rec | {"_hwnd": hwnd}
    time.sleep(0.3)
    t_rel = drag(hwnd, tuple(app.get("drag_from", (400, 300))), tuple(app.get("drag_to", (800, 600))))
    end = t_rel + timeout
    seen = None
    path = None
    while now() < end:
        if rec.get("overlay_closed") is None and (not user32.IsWindow(hwnd) or not bw.shown(bw.window_info(hwnd))):
            rec["overlay_closed"] = (now() - t_rel) * 1000
        if path is None:
            new = [f for f in os.listdir(folder) if f not in files_before]
            if new:
                path = os.path.join(folder, new[0])
                seen = now()
                rec["file_seen"] = (seen - t_rel) * 1000
        if path:
            w, h, done = png_info(path)
            if done:
                rec["file_done"] = (now() - t_rel) * 1000
                rec["file_size"] = [w, h]
                rec["file"] = os.path.basename(path)
                break
        time.sleep(0.001)
    else:
        rec["error"] = "no saved file"
    end = now() + 3
    while rec.get("overlay_closed") is None and now() < end:
        if not user32.IsWindow(hwnd) or not bw.shown(bw.window_info(hwnd)):
            rec["overlay_closed"] = (now() - t_rel) * 1000
        time.sleep(0.001)
    return rec


# ---------------------------------------------------------------- commands

def load_config(path):
    cfg = json.load(open(path, encoding="utf-8"))
    for app in cfg["apps"].values():
        for k in ("exe", "logs", "saved"):
            app[k] = os.path.normpath(os.path.expandvars(app[k]))
    cfg["pattern_exe"] = os.path.normpath(os.path.expandvars(cfg["pattern_exe"]))
    return cfg


def find_pattern(cfg):
    pids = bw.running_copies(cfg["pattern_exe"])
    if len(pids) != 1:
        sys.exit(f"start the test pattern first ({cfg['pattern_exe']} --test-pattern --seconds 7200); running: {pids}")
    check_pattern(pids[0])
    return pids[0]


def start_app(app):
    others = bw.running_copies(app["exe"])
    if others:
        raise SystemExit(f"{app['exe']} already running (PIDs {others})")
    log = LogTail(app["logs"])
    log.mark()
    launch = {"exe": app["exe"], "args": app.get("args", []), "main_title": app["main_title"],
              "dismiss": app.get("dismiss"), "settle": 1.0, "logs": app["logs"]}
    proc, rec = bw.do_launch(launch, None)
    rec.pop("_last_frame", None)
    if rec.get("error"):
        bw.do_exit(launch, proc)
        raise RuntimeError(f"launch: {rec['error']}")
    return proc, rec, log


def stop_app(app, proc):
    # an overlay left open would hold the exit
    for h, i in overlay_candidates({proc.pid}, app["overlay_title"]).items():
        if bw.shown(i):
            user32.PostMessageW(h, bw.WM_CLOSE, 0, 0)
    time.sleep(0.3)
    return bw.do_exit({"exe": app["exe"]}, proc)


def cmd_verify(a):
    cfg = load_config(a.config)
    pattern_pid = find_pattern(cfg)
    app = cfg["apps"][a.app]
    proc, _, log = start_app(app)
    try:
        time.sleep(a.idle)
        shot = os.path.join(os.environ["TEMP"], f"overlay-check-{a.app}.png")
        rec = one_capture(app, proc, pattern_pid, PatternMatcher(), log, save=True, keep_capture=shot)
        rec.pop("_hwnd", None)
        print(json.dumps(rec))
        if os.path.exists(shot):
            check = subprocess.run([sys.executable, os.path.join(_here, "pattern-check.py"), shot, "--origin",
                                    ",".join(map(str, rec["overlay_rect"][:2])), "--delete"],
                                   capture_output=True, text=True)
            print("overlay capture:", check.stdout.strip(), check.stderr.strip())
            if os.path.exists(shot):
                os.remove(shot)
            print("overlay capture deleted:", not os.path.exists(shot))
        if rec.get("file"):
            saved = os.path.join(app["saved"], rec["file"])
            check = subprocess.run([sys.executable, os.path.join(_here, "pattern-check.py"), saved, "--origin",
                                    "{},{}".format(*app.get("drag_from", (400, 300))), "--delete", "--min", "0.9"],
                                   capture_output=True, text=True)
            print("saved region:", check.stdout.strip(), check.stderr.strip(), "kept:", os.path.exists(saved))
    finally:
        print("exit:", stop_app(app, proc))


def cmd_bench(a):
    cfg = load_config(a.config)
    pattern_pid = find_pattern(cfg)
    names = [n for n in cfg["apps"] if not a.only or n in a.only.split(",")]
    out = open(a.out, "a", encoding="utf-8")
    matcher = PatternMatcher()

    def emit(rec):
        rec = {k: v for k, v in rec.items() if not k.startswith("_")}
        rec["at"] = time.strftime("%Y-%m-%d %H:%M:%S")
        out.write(json.dumps(rec) + "\n")
        out.flush()
        brief = {k: (round(v) if isinstance(v, float) else v) for k, v in rec.items()
                 if k in ("app", "what", "round", "n", "received", "created", "visible", "painted", "overlay_closed", "file_seen",
                          "file_done", "file_size", "error")}
        print(" ", json.dumps(brief), flush=True)

    for r in range(a.start_round, a.start_round + a.rounds):
        for name in (names if r % 2 == 0 else names[::-1]):
            app = cfg["apps"][name]
            print(f"round {r} {name}", flush=True)
            proc, launch, log = start_app(app)
            emit({"app": name, "what": "launch", "round": r, "visible": launch.get("visible"),
                  "painted": launch.get("painted")})
            try:
                time.sleep(a.idle)  # past ShareX's region capture warm-up (~2.4 s after start)
                for n in range(a.per_start):
                    rec = one_capture(app, proc, pattern_pid, matcher, log)
                    rec.update(app=name, what="first" if n == 0 else "repeat", round=r, n=n)
                    emit(rec)
                    if rec.get("error"):
                        if "pattern" in rec["error"]:
                            raise SystemExit("stopping: an overlay didn't show the test pattern")
                        break
                    time.sleep(a.gap)
            finally:
                print("  exit:", stop_app(app, proc), flush=True)
            time.sleep(a.gap)


def cmd_report(a):
    recs = [json.loads(ln) for f in a.files for ln in open(f, encoding="utf-8") if ln.strip()]
    metrics = ["received", "created", "visible", "painted_after", "painted", "overlay_closed", "file_seen", "file_done", "capture_ms"]
    groups = {}
    for r in recs:
        groups.setdefault((r["what"], r["app"]), []).append(r)
    print("| measurement | app | metric | median | p90 | min | max | n |")
    print("|---|---|---|---:|---:|---:|---:|---:|")
    for (what, app), rs in sorted(groups.items()):
        for m in metrics:
            vals = [r[m] for r in rs if r.get(m) is not None]
            if vals:
                s = sorted(vals)
                p90 = s[max(0, math.ceil(0.9 * len(s)) - 1)]
                print(f"| {what} | {app} | {m} | {np.median(vals):.0f} | {p90:.0f} | {min(vals):.0f} | {max(vals):.0f} | {len(vals)} |")
        inapp = [r["visible"] - r["received"] for r in rs if r.get("visible") is not None and r.get("received") is not None]
        if inapp:
            s = sorted(inapp)
            p90 = s[max(0, math.ceil(0.9 * len(s)) - 1)]
            print(f"| {what} | {app} | visible - received | {np.median(inapp):.0f} | {p90:.0f} | {min(inapp):.0f} | {max(inapp):.0f} | {len(inapp)} |")
    errs = [r for r in recs if r.get("error")]
    for r in errs:
        print("error:", r.get("app"), r.get("what"), r.get("round"), r["error"])
    sizes = {tuple(r["file_size"]) for r in recs if r.get("file_size")}
    print("saved sizes:", sizes, " reused overlay windows:",
          {app: sum(1 for r in recs if r["app"] == app and r.get("reused_window")) for app in {r["app"] for r in recs}})


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("verify")
    p.add_argument("--config", required=True)
    p.add_argument("--app", required=True)
    p.add_argument("--idle", type=float, default=5.0)
    p = sub.add_parser("bench")
    p.add_argument("--config", required=True)
    p.add_argument("--out", required=True)
    p.add_argument("--rounds", type=int, default=8)
    p.add_argument("--start-round", type=int, default=1)
    p.add_argument("--per-start", type=int, default=3, help="captures per app start (the first, then repeats)")
    p.add_argument("--idle", type=float, default=5.0, help="seconds after the main window before the first capture")
    p.add_argument("--gap", type=float, default=1.0)
    p.add_argument("--only")
    p = sub.add_parser("report")
    p.add_argument("files", nargs="+")
    a = ap.parse_args()
    {"verify": cmd_verify, "bench": cmd_bench, "report": cmd_report}[a.cmd](a)


if __name__ == "__main__":
    main()
