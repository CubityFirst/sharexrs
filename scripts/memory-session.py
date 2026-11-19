"""Measures a ShareXrs (or ShareX) copy's memory at the points of
docs/PORTING.md's memory table (Windows):

  idle-main     the main window shown, idle (`--idle` s after it showed)
  idle-tray     the main window closed to the tray, idle
  capture-1     after the first region capture
  capture-N     after `--captures` region captures
  cycles-N      (`--phase cycles`, its own start, without the test pattern:
                nothing is captured, and the pattern would cover the main
                window) after N cycles of opening and closing Application
                settings (a click on the main window's sidebar item) and the
                image editor (`-ImageEditor <png>`), sampled every
                `--sample-every` cycles

Each point is `scripts/memory.ps1` (working set / private / peak, GPU
dedicated and shared) plus, when the copy is a `memory-profile` build
started with `--memprof`, its last report line (heaps, wgpu allocator).

Region captures go through `bench-region-capture.py`'s `one_capture`: the
test pattern must cover the screen (`sharexrs --test-pattern`, a separate
copy given as `--pattern-exe`), every overlay is checked against it, and the
saved files stay in the copy's screenshots folder for the caller to compare
and delete. Only the app's own windows are captured or clicked.

  python scripts/memory-session.py --exe %TEMP%/m/base/sharexrs.exe --pattern-exe %TEMP%/m/pattern/p.exe \
      --saved %TEMP%/m/saved-base --png %TEMP%/m/editor.png --out base.jsonl --label base
"""

import argparse
import importlib.util
import json
import os
import subprocess
import sys
import time

_here = os.path.dirname(os.path.abspath(__file__))


def _load(name, file):
    spec = importlib.util.spec_from_file_location(name, os.path.join(_here, file))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


rc = _load("bench_region_capture", "bench-region-capture.py")
bw = rc.bw


def memory(pid, label):
    out = subprocess.run(["powershell", "-NoProfile", "-File", os.path.join(_here, "memory.ps1"), "-Id", str(pid),
                          "-Label", label], capture_output=True, text=True).stdout.strip()
    return json.loads(out.splitlines()[-1])


def memprof_last(path):
    if not path or not os.path.exists(path):
        return None
    with open(path, encoding="utf-8") as f:
        lines = [ln for ln in f if ln.strip()]
    return json.loads(lines[-1]) if lines else None


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--exe", required=True)
    ap.add_argument("--phase", choices=["captures", "cycles"], default="captures")
    ap.add_argument("--pattern-exe", required=True)
    ap.add_argument("--saved", required=True, help="the copy's screenshots folder")
    ap.add_argument("--png", required=True, help="an image for the editor cycles")
    ap.add_argument("--out", required=True)
    ap.add_argument("--label", required=True)
    ap.add_argument("--idle", type=float, default=8.0)
    ap.add_argument("--settle", type=float, default=4.0, help="seconds before each later sample")
    ap.add_argument("--captures", type=int, default=15)
    ap.add_argument("--cycles", type=int, default=50)
    ap.add_argument("--sample-every", type=int, default=10)
    ap.add_argument("--memprof", help="SHAREXRS_MEMPROF for a memory-profile build")
    ap.add_argument("--env", action="append", default=[], help="NAME=VALUE for the app")
    ap.add_argument("--keep", action="store_true", help="leave the app running at the end (prints its PID)")
    a = ap.parse_args()

    exe = os.path.normpath(os.path.expandvars(a.exe))
    pattern_exe = os.path.normpath(os.path.expandvars(a.pattern_exe))
    saved = os.path.normpath(os.path.expandvars(a.saved))
    png = os.path.normpath(os.path.expandvars(a.png))
    folder = os.path.dirname(exe)
    app = {"exe": exe, "logs": os.path.join(folder, "ShareXrs", "Logs"), "saved": saved, "args": ["-NoHotkeys"],
           "main_title": r"^ShareX(rs)? \d", "dismiss": "Hotkey registration failed",
           "overlay_title": "(?i)region capture",
           "items": {"appsettings": {"at": [133, 354], "title": "Application settings"}},
           "editor_title": "(?i)image editor", "settle": 1.0}
    pids = bw.running_copies(pattern_exe)
    if a.phase == "captures":
        if len(pids) != 1:
            sys.exit(f"start the test pattern first ({pattern_exe} --test-pattern --seconds 7200); running: {pids}")
        pattern_pid = pids[0]
        rc.check_pattern(pattern_pid)
    elif pids:
        sys.exit(f"the cycles need the main window uncovered: stop the test pattern ({pids})")
    if bw.running_copies(exe):
        sys.exit(f"{exe} already running")

    out = open(a.out, "a", encoding="utf-8")

    def sample(proc, point, **extra):
        m = memory(proc.pid, point)
        rec = {"label": a.label, "point": point, **m, **extra}
        prof = memprof_last(a.memprof)
        if prof:
            rec["memprof"] = prof
        out.write(json.dumps(rec) + "\n")
        out.flush()
        print(f"{a.label} {point}: WS {m['WorkingSet']} / private {m['Private']} / peak {m['PeakWS']}, "
              f"GPU {m['GpuDedicated']} (shared {m['GpuShared']})", flush=True)

    env_backup = {}
    for kv in ([f"SHAREXRS_MEMPROF={a.memprof}"] if a.memprof else []) + a.env:
        k, v = kv.split("=", 1)
        env_backup[k] = os.environ.get(k)
        os.environ[k] = v
    try:
        proc, rec = bw.do_launch({**app, "settle": 1.0}, None)
    finally:
        for k, v in env_backup.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
    if rec.get("error"):
        sys.exit(f"launch: {rec['error']}")
    log = rc.LogTail(app["logs"])
    try:
        time.sleep(a.idle)
        sample(proc, "idle-main")
        if a.phase == "cycles":
            for n in range(1, a.cycles + 1):
                r = bw.do_click(app, proc, "appsettings")
                if r.get("error") or not r.get("closed"):
                    raise SystemExit(f"settings cycle {n}: {r}")
                r = bw.do_editor(app, proc, png)
                if r.get("error") or not r.get("closed") or r.get("left_windows"):
                    raise SystemExit(f"editor cycle {n}: {r}")
                if n % a.sample_every == 0 or n == a.cycles:
                    time.sleep(a.settle)
                    sample(proc, f"cycles-{n}")
            return
        main_hwnd = bw.find_main({proc.pid}, app["main_title"])
        bw.close_window(main_hwnd, {proc.pid}, app["main_title"])
        time.sleep(a.idle)
        sample(proc, "idle-tray")
        matcher = rc.PatternMatcher()
        for n in range(1, a.captures + 1):
            r = rc.one_capture(app, proc, pattern_pid, matcher, log)
            if r.get("error"):
                raise SystemExit(f"capture {n}: {r['error']}")
            time.sleep(0.5)
            if n == 1:
                time.sleep(a.settle)
                sample(proc, "capture-1", visible=r.get("visible"), file_done=r.get("file_done"))
        time.sleep(a.settle)
        sample(proc, f"capture-{a.captures}")
    finally:
        if a.keep:
            print("left running: PID", proc.pid)
            return
        for h, i in rc.overlay_candidates({proc.pid}, app["overlay_title"]).items():
            if bw.shown(i):
                bw.user32.PostMessageW(h, bw.WM_CLOSE, 0, 0)
        time.sleep(0.3)
        print("exit:", bw.do_exit(app, proc))


if __name__ == "__main__":
    main()
