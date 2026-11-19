"""Benchmarks how long ShareX's and ShareXrs's windows take to open (Windows).

Measures, for one app at a time, from an action to a top-level window of the
app's process being (a) present in `EnumWindows`, (b) visible
(`IsWindowVisible` and not DWM-cloaked), (c) painted: a `PrintWindow`
(`PW_CLIENTONLY | PW_RENDERFULLCONTENT`) of its client area that isn't
uniform (>= 1% of sampled pixels differ from the most common colour), and
(d) visually complete: the first capture after which the client stops
changing (no capture differs from it by more than 0.3% of sampled pixels for
`--settle` seconds). For the image editor, (e) the first capture with the
image shown (colourful pixels >= half of the settled capture's).

A `PrintWindow` with `PW_RENDERFULLCONTENT` is slow (tens of ms; on the
benchmark machine ~50-80 ms for ShareXrs's windows and ~125-155 ms for
ShareX's, which present differently), so (c)-(e) are bounds: `painted` is the end of
the first capture that has it ("painted by") and `painted_after` the start
of the last capture that didn't (or the visible time). (a) and (b) come
from polling alone and have ~1-3 ms resolution.

The actions:
  launch  start the app (`CreateProcess`, via subprocess) and wait for its
          main window; `-ExitShareX` closes it afterwards
  click   left-click a point of the main window's client area (the sidebar
          item), timed from the button release, for Application settings or
          Task settings; the window is closed with WM_CLOSE afterwards
  editor  start a second copy with `-ImageEditor <png>`, which hands the file
          to the running one; the editor is closed with WM_CLOSE afterwards

Polling is `EnumWindows` every ~1 ms (`timeBeginPeriod(1)`); once the window
is visible, captures run back to back (each `PrintWindow` takes a few ms,
reported as the capture interval). Titles are read with
`InternalGetWindowText`, which sends no message to the app.

Only the apps' own windows are captured. Input is only sent after checking
that the window under the point is the app's main window, and the run pauses
while the user is using the machine (input not sent by this script).

Usage (results are appended as JSON lines; `report` summarises them):
  python scripts/bench-window-open.py list --pid 1234
  python scripts/bench-window-open.py shot --pid 1234 --title "^ShareX" --out x.png
  python scripts/bench-window-open.py bench --config scripts/bench-window-open.json --rounds 15 \
      --repeats 2 --png 1920x1080.png --out results.jsonl
  # the hotkey warning box (no -NoHotkeys; the box is closed the moment it shows)
  python scripts/bench-window-open.py bench --config scripts/bench-window-open.json --rounds 6 \
      --no-items --extra-args= --label box --out results.jsonl
  python scripts/bench-window-open.py report results.jsonl

The config's apps run alternately (the order flips every round). Each needs
a portable copy (a `Portable` file beside the exe) with `ShowStartScreen`
and `AutoCheckUpdate` off, and no other copy running: both apps are single
instance, so a copy someone else started would take the command lines (the
bench refuses to start then). Each record carries a wall-clock `at`, so
rounds hit by other load on the machine can be dropped.
"""

import argparse
import ctypes as C
import ctypes.wintypes as W
import json
import math
import os
import re
import subprocess
import sys
import time

import numpy as np

user32 = C.WinDLL("user32", use_last_error=True)
gdi32 = C.WinDLL("gdi32")
kernel32 = C.WinDLL("kernel32")
dwmapi = C.WinDLL("dwmapi")
winmm = C.WinDLL("winmm")

user32.SetProcessDpiAwarenessContext(C.c_void_p(-4))  # per-monitor v2: physical pixels
winmm.timeBeginPeriod(1)

EnumWindowsProc = C.WINFUNCTYPE(W.BOOL, W.HWND, W.LPARAM)
user32.EnumWindows.argtypes = [EnumWindowsProc, W.LPARAM]
user32.GetWindowThreadProcessId.argtypes = [W.HWND, C.POINTER(W.DWORD)]
user32.InternalGetWindowText.argtypes = [W.HWND, W.LPWSTR, C.c_int]
user32.GetClassNameW.argtypes = [W.HWND, W.LPWSTR, C.c_int]
user32.IsWindowVisible.argtypes = [W.HWND]
user32.IsWindow.argtypes = [W.HWND]
user32.GetWindowRect.argtypes = [W.HWND, C.POINTER(W.RECT)]
user32.GetClientRect.argtypes = [W.HWND, C.POINTER(W.RECT)]
user32.ClientToScreen.argtypes = [W.HWND, C.POINTER(W.POINT)]
user32.WindowFromPoint.argtypes = [W.POINT]
user32.WindowFromPoint.restype = W.HWND
user32.GetAncestor.argtypes = [W.HWND, W.UINT]
user32.GetAncestor.restype = W.HWND
user32.PrintWindow.argtypes = [W.HWND, W.HDC, W.UINT]
user32.PostMessageW.argtypes = [W.HWND, W.UINT, W.WPARAM, W.LPARAM]
user32.GetDC.argtypes = [W.HWND]
user32.GetDC.restype = W.HDC
user32.ReleaseDC.argtypes = [W.HWND, W.HDC]
user32.SetForegroundWindow.argtypes = [W.HWND]
user32.BringWindowToTop.argtypes = [W.HWND]
user32.AttachThreadInput.argtypes = [W.DWORD, W.DWORD, W.BOOL]
user32.GetForegroundWindow.restype = W.HWND
gdi32.CreateCompatibleDC.argtypes = [W.HDC]
gdi32.CreateCompatibleDC.restype = W.HDC
gdi32.CreateDIBSection.argtypes = [W.HDC, C.c_void_p, W.UINT, C.POINTER(C.c_void_p), W.HANDLE, W.DWORD]
gdi32.CreateDIBSection.restype = W.HBITMAP
gdi32.SelectObject.argtypes = [W.HDC, W.HGDIOBJ]
gdi32.SelectObject.restype = W.HGDIOBJ
gdi32.DeleteObject.argtypes = [W.HGDIOBJ]
gdi32.DeleteDC.argtypes = [W.HDC]
dwmapi.DwmGetWindowAttribute.argtypes = [W.HWND, W.DWORD, C.c_void_p, W.DWORD]
kernel32.OpenProcess.restype = W.HANDLE
kernel32.OpenProcess.argtypes = [W.DWORD, W.BOOL, W.DWORD]
kernel32.GetExitCodeProcess.argtypes = [W.HANDLE, C.POINTER(W.DWORD)]
kernel32.CloseHandle.argtypes = [W.HANDLE]

WM_CLOSE = 0x0010
GA_ROOT = 2
DWMWA_CLOAKED = 14
PW_CLIENTONLY_RENDERFULLCONTENT = 0x1 | 0x2


class LASTINPUTINFO(C.Structure):
    _fields_ = [("cbSize", W.UINT), ("dwTime", W.DWORD)]


class MOUSEINPUT(C.Structure):
    _fields_ = [("dx", W.LONG), ("dy", W.LONG), ("mouseData", W.DWORD), ("dwFlags", W.DWORD),
                ("time", W.DWORD), ("dwExtraInfo", C.c_size_t)]


class INPUT(C.Structure):
    class _U(C.Union):
        _fields_ = [("mi", MOUSEINPUT), ("pad", C.c_byte * 32)]
    _anonymous_ = ("u",)
    _fields_ = [("type", W.DWORD), ("u", _U)]


class BITMAPINFOHEADER(C.Structure):
    _fields_ = [("biSize", W.DWORD), ("biWidth", W.LONG), ("biHeight", W.LONG), ("biPlanes", W.WORD),
                ("biBitCount", W.WORD), ("biCompression", W.DWORD), ("biSizeImage", W.DWORD),
                ("biXPelsPerMeter", W.LONG), ("biYPelsPerMeter", W.LONG), ("biClrUsed", W.DWORD),
                ("biClrImportant", W.DWORD)]


user32.SendInput.argtypes = [W.UINT, C.POINTER(INPUT), C.c_int]
user32.GetLastInputInfo.argtypes = [C.POINTER(LASTINPUTINFO)]

now = time.perf_counter


# ---------------------------------------------------------------- windows

def window_info(hwnd):
    buf = C.create_unicode_buffer(512)
    user32.InternalGetWindowText(hwnd, buf, 512)
    title = buf.value
    user32.GetClassNameW(hwnd, buf, 512)
    cls = buf.value
    cloaked = W.DWORD(0)
    dwmapi.DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, C.byref(cloaked), 4)
    rc = W.RECT()
    user32.GetWindowRect(hwnd, C.byref(rc))
    return {"hwnd": hwnd, "title": title, "class": cls, "visible": bool(user32.IsWindowVisible(hwnd)),
            "cloaked": cloaked.value != 0, "rect": (rc.left, rc.top, rc.right - rc.left, rc.bottom - rc.top)}


def windows_of(pids):
    """Top-level windows of these processes, as (hwnd, pid), in z-order."""
    out = []
    pid = W.DWORD()

    def cb(hwnd, _):
        user32.GetWindowThreadProcessId(hwnd, C.byref(pid))
        if pid.value in pids:
            out.append((hwnd, pid.value))
        return True

    user32.EnumWindows(EnumWindowsProc(cb), 0)
    return out


def shown(info):
    w, h = info["rect"][2], info["rect"][3]
    return info["visible"] and not info["cloaked"] and w > 0 and h > 0


def pid_alive(pid):
    h = kernel32.OpenProcess(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
    if not h:
        return False
    code = W.DWORD()
    kernel32.GetExitCodeProcess(h, C.byref(code))
    kernel32.CloseHandle(h)
    return code.value == 259  # STILL_ACTIVE


# ---------------------------------------------------------------- capture

class Capturer:
    def __init__(self):
        self.size = None
        self.screen = user32.GetDC(None)
        self.dc = gdi32.CreateCompatibleDC(self.screen)
        self.bmp = None
        self.bits = C.c_void_p()

    def _ensure(self, w, h):
        if self.size == (w, h):
            return
        if self.bmp:
            gdi32.DeleteObject(self.bmp)
        bmi = BITMAPINFOHEADER(C.sizeof(BITMAPINFOHEADER), w, -h, 1, 32, 0, 0, 0, 0, 0, 0)
        buf = (C.c_byte * (C.sizeof(bmi) + 16))()
        C.memmove(buf, C.byref(bmi), C.sizeof(bmi))
        self.bmp = gdi32.CreateDIBSection(self.dc, buf, 0, C.byref(self.bits), None, 0)
        gdi32.SelectObject(self.dc, self.bmp)
        self.size = (w, h)

    def grab(self, hwnd):
        """The window's client area as an HxWx3 BGR array, or None."""
        rc = W.RECT()
        if not user32.GetClientRect(hwnd, C.byref(rc)):
            return None
        w, h = rc.right, rc.bottom
        if w <= 0 or h <= 0:
            return None
        self._ensure(w, h)
        C.memset(self.bits, 0, w * h * 4)
        if not user32.PrintWindow(hwnd, self.dc, PW_CLIENTONLY_RENDERFULLCONTENT):
            return None
        raw = (C.c_ubyte * (w * h * 4)).from_address(self.bits.value)
        return np.frombuffer(raw, np.uint8).reshape(h, w, 4)[:, :, :3].copy()


def stats(frame):
    """(non-mode share, colourful share) of a frame, from every 2nd pixel."""
    s = frame[::2, ::2].astype(np.int16)
    q = ((s[:, :, 0] >> 4) << 8) | ((s[:, :, 1] >> 4) << 4) | (s[:, :, 2] >> 4)
    counts = np.bincount(q.ravel(), minlength=4096)
    nonmode = 1.0 - counts.max() / q.size
    colourful = float(((s.max(axis=2) - s.min(axis=2)) > 80).mean())
    return float(nonmode), colourful


def differs(a, b):
    if a is None or b is None or a.shape != b.shape:
        return True
    d = np.abs(a[::2, ::2].astype(np.int16) - b[::2, ::2].astype(np.int16)).max(axis=2) > 16
    return d.mean() > 0.003


# ---------------------------------------------------------------- user guard

class Guard:
    """Pauses while someone else uses the machine (input this script didn't send)."""

    def __init__(self, idle_needed=60.0):
        self.idle_needed = idle_needed
        self.ours = None  # GetTickCount after our last input

    def _last(self):
        li = LASTINPUTINFO(C.sizeof(LASTINPUTINFO), 0)
        user32.GetLastInputInfo(C.byref(li))
        return li.dwTime

    def check(self):
        mask = 0xFFFFFFFF
        while True:
            last = self._last()
            tick = kernel32.GetTickCount() & mask
            # input after (our last input + 150 ms) wasn't ours
            after_ours = None if self.ours is None else (last - self.ours) & mask
            foreign = after_ours is None or (150 < after_ours < 0x80000000)
            idle = ((tick - last) & mask) / 1000.0
            if not foreign or idle >= self.idle_needed:
                return
            print(f"  [guard] user input {idle:.0f}s ago; waiting for {self.idle_needed:.0f}s idle", flush=True)
            time.sleep(5)

    def sent(self):
        self.ours = kernel32.GetTickCount() & 0xFFFFFFFF


GUARD = Guard()


def mouse(flags, x=0, y=0):
    inp = INPUT()
    inp.type = 0
    inp.mi = MOUSEINPUT(x, y, 0, flags, 0, 0)
    return inp


def move_to(x, y):
    vw, vh = user32.GetSystemMetrics(78), user32.GetSystemMetrics(79)
    vx, vy = user32.GetSystemMetrics(76), user32.GetSystemMetrics(77)
    ax = int(round((x - vx) * 65535 / (vw - 1)))
    ay = int(round((y - vy) * 65535 / (vh - 1)))
    arr = (INPUT * 1)(mouse(0x0001 | 0x8000 | 0x4000, ax, ay))  # MOVE | ABSOLUTE | VIRTUALDESK
    user32.SendInput(1, arr, C.sizeof(INPUT))


def send_button(down):
    arr = (INPUT * 1)(mouse(0x0002 if down else 0x0004))
    user32.SendInput(1, arr, C.sizeof(INPUT))


# ---------------------------------------------------------------- watching

def watch(pids, title_re, t0, existing=(), settle=1.5, timeout=60.0, dismiss_re=None, image=False,
          exclude_re=None):
    """Waits for a new matching window of `pids` and times it from `t0`."""
    rx = re.compile(title_re)
    ex = re.compile(exclude_re) if exclude_re else None
    dx = re.compile(dismiss_re) if dismiss_re else None
    existing = set(existing)
    rec = {"exists": None, "visible": None, "painted": None, "complete": None, "image": None,
           "dismissed": [], "hwnd": None, "title": None, "polls": 0}
    dismissed = set()
    hwnd = None
    deadline = t0 + timeout
    while now() < deadline:
        rec["polls"] += 1
        for h, _ in windows_of(pids):
            if h in existing:
                continue
            info = window_info(h)
            if dx and h not in dismissed and shown(info) and dx.search(info["title"]):
                t = now()
                user32.PostMessageW(h, WM_CLOSE, 0, 0)
                dismissed.add(h)
                rec["dismissed"].append({"title": info["title"], "t_ms": (t - t0) * 1000})
                continue
            if rx.search(info["title"]) and not (ex and ex.search(info["title"])):
                if rec["exists"] is None:
                    rec["exists"] = now() - t0
                if shown(info):
                    hwnd = h
                    rec["visible"] = now() - t0
                    rec["title"] = info["title"]
                    break
        if hwnd:
            break
        time.sleep(0.001)
    if not hwnd:
        rec["error"] = "timeout waiting for window"
        return rec
    rec["hwnd"] = hwnd
    cap = Capturer()
    prev = None
    last_change = None
    first_frame = None
    intervals = []
    durations = []
    frames = []  # (start, end, nonmode, colourful, changed)
    t_prev = None
    while now() < deadline:
        # keep dismissing late boxes (the hotkey warning can come after the main window)
        if dx:
            for h, _ in windows_of(pids):
                if h in dismissed or h == hwnd:
                    continue
                info = window_info(h)
                if shown(info) and dx.search(info["title"]):
                    t = now()
                    user32.PostMessageW(h, WM_CLOSE, 0, 0)
                    dismissed.add(h)
                    rec["dismissed"].append({"title": info["title"], "t_ms": (t - t0) * 1000})
        t_before = now()
        frame = cap.grab(hwnd)
        t = now()
        if frame is None:
            if not user32.IsWindow(hwnd):
                rec["error"] = "window closed during capture"
                return rec
            time.sleep(0.002)
            continue
        # The picture is from somewhere in [t_before, t]: a PrintWindow with
        # PW_RENDERFULLCONTENT takes tens of ms, so each paint time is kept as
        # bounds: "by" (this capture's end) and "after" (the start of the
        # last capture without it, or the visible time).
        ts, te = t_before - t0, t - t0
        if t_prev is not None:
            intervals.append(t - t_prev)
        t_prev = t
        durations.append(te - ts)
        nonmode, colourful = stats(frame)
        changed = differs(prev, frame)
        frames.append((ts, te, nonmode, colourful, changed))
        if first_frame is None:
            first_frame = te
        if rec["painted"] is None and nonmode >= 0.01:
            rec["painted"] = te
            rec["painted_after"] = frames[-2][0] if len(frames) > 1 else rec["visible"]
        if changed:
            last_change = (ts, te, frames[-2][0] if len(frames) > 1 else rec["visible"])
        prev = frame
        if rec["painted"] is not None and ts - last_change[1] >= settle:
            break
        time.sleep(0.002)
    else:
        rec["error"] = "never settled"
    if last_change:
        rec["complete"] = last_change[1]
        rec["complete_after"] = last_change[2]
    rec["first_capture"] = first_frame
    rec["final_nonmode"] = frames[-1][2] if frames else None
    rec["final_colourful"] = frames[-1][3] if frames else None
    if image and frames and frames[-1][3] >= 0.05:
        want = frames[-1][3] * 0.5
        i = next(i for i, f in enumerate(frames) if f[3] >= want)
        rec["image"] = frames[i][1]
        rec["image_after"] = frames[i - 1][0] if i else rec["visible"]
    rec["capture_interval_ms"] = float(np.median(intervals) * 1000) if intervals else None
    rec["capture_ms"] = float(np.median(durations) * 1000) if durations else None
    rec["captures"] = len(frames)
    rec["frame_size"] = None if prev is None else [int(prev.shape[1]), int(prev.shape[0])]
    for k in ("exists", "visible", "painted", "painted_after", "complete", "complete_after", "image", "image_after",
              "first_capture"):
        if rec.get(k) is not None:
            rec[k] = rec[k] * 1000
    rec["_last_frame"] = prev
    return rec


def close_window(hwnd, pids, title_re, timeout=15.0):
    info = window_info(hwnd)
    pid = W.DWORD()
    user32.GetWindowThreadProcessId(hwnd, C.byref(pid))
    if pid.value not in pids or not re.search(title_re, info["title"]):
        raise RuntimeError(f"refusing to close {info}")
    user32.PostMessageW(hwnd, WM_CLOSE, 0, 0)
    end = now() + timeout
    while now() < end:
        if not user32.IsWindow(hwnd) or not shown(window_info(hwnd)):
            return True
        time.sleep(0.01)
    return False


def bring_forward(hwnd):
    """Activates the app's own window without sending input (a process that
    isn't in the foreground can't, unless attached to the foreground thread)."""
    fg = user32.GetForegroundWindow()
    fg_thread = user32.GetWindowThreadProcessId(fg, None) if fg else 0
    me = kernel32.GetCurrentThreadId()
    attached = bool(fg_thread) and fg_thread != me and user32.AttachThreadInput(me, fg_thread, True)
    try:
        user32.BringWindowToTop(hwnd)
        user32.SetForegroundWindow(hwnd)
    finally:
        if attached:
            user32.AttachThreadInput(me, fg_thread, False)


def find_main(pids, title_re, exclude_re=None):
    for h, _ in windows_of(pids):
        info = window_info(h)
        if shown(info) and re.search(title_re, info["title"]) and not (exclude_re and re.search(exclude_re, info["title"])):
            return h
    return None


def last_startup_line(log_dir):
    """The last "Startup time: N ms" in the newest log file of a Logs folder."""
    try:
        files = sorted((os.path.join(log_dir, f) for f in os.listdir(log_dir)), key=os.path.getmtime)
    except OSError:
        return None
    if not files:
        return None
    with open(files[-1], encoding="utf-8", errors="replace") as f:
        lines = [ln for ln in f if "Startup time:" in ln]
    if not lines:
        return None
    m = re.search(r"Startup time:\s*(\d+)\s*ms", lines[-1])
    return (lines[-1].strip(), int(m.group(1)) if m else None)


# ---------------------------------------------------------------- actions

def save_png(frame, path):
    from PIL import Image
    Image.fromarray(frame[:, :, ::-1]).save(path)


def do_launch(app, log, shot=None):
    GUARD.check()
    before = last_startup_line(app["logs"]) if app.get("logs") else None
    exe = app["exe"]
    t0 = now()
    proc = subprocess.Popen([exe] + app.get("args", []), cwd=os.path.dirname(exe))
    t_spawned = now()
    rec = watch({proc.pid}, app["main_title"], t0, settle=app.get("settle", 1.5), timeout=90,
                dismiss_re=app.get("dismiss"), exclude_re=app.get("main_exclude"))
    frame = rec.pop("_last_frame", None)
    if shot and frame is not None:
        save_png(frame, shot)
    rec["spawn_ms"] = (t_spawned - t0) * 1000
    rec["pid"] = proc.pid
    if proc.poll() is not None:
        rec["error"] = f"the process exited ({proc.returncode}): another copy took the command line?"
    # late hotkey box: give it a moment, dismiss if it shows
    end = now() + 1.0
    while now() < end and app.get("dismiss"):
        for h, _ in windows_of({proc.pid}):
            info = window_info(h)
            if shown(info) and re.search(app["dismiss"], info["title"]) and h != rec.get("hwnd"):
                user32.PostMessageW(h, WM_CLOSE, 0, 0)
                rec["dismissed"].append({"title": info["title"], "t_ms": (now() - t0) * 1000, "late": True})
        time.sleep(0.05)
    after = last_startup_line(app["logs"]) if app.get("logs") else None
    if after and after != before:
        rec["log_startup_ms"] = after[1]
    return proc, rec


def running_copies(exe):
    """PIDs of processes running this executable (by image name)."""
    name = os.path.basename(exe)
    out = subprocess.run(["tasklist", "/FI", f"IMAGENAME eq {name}", "/FO", "CSV", "/NH"], capture_output=True,
                         text=True).stdout
    return [int(row.split('","')[1]) for row in out.splitlines() if row.startswith('"')]


def do_exit(app, proc):
    exe = app["exe"]
    if proc.poll() is not None:
        return "already gone"  # -ExitShareX would start a new copy
    subprocess.Popen([exe, "-ExitShareX"], cwd=os.path.dirname(exe)).wait(timeout=60)
    try:
        proc.wait(timeout=30)
        return "exited"
    except subprocess.TimeoutExpired:
        proc.kill()  # our own process, by handle
        proc.wait(timeout=10)
        return "killed"


def do_click(app, proc, item, shot=None):
    pids = {proc.pid}
    main = find_main(pids, app["main_title"], app.get("main_exclude"))
    if not main:
        raise RuntimeError("main window not found")
    cx, cy = app["items"][item]["at"]
    pt = W.POINT(cx, cy)
    user32.ClientToScreen(main, C.byref(pt))
    root = user32.GetAncestor(user32.WindowFromPoint(pt), GA_ROOT)
    if root != main:
        bring_forward(main)
        time.sleep(0.3)
        root = user32.GetAncestor(user32.WindowFromPoint(pt), GA_ROOT)
        if root != main:
            raise RuntimeError(f"point {pt.x},{pt.y} is over {window_info(root)} not the main window")
    existing = [h for h, _ in windows_of(pids)]
    GUARD.check()
    move_to(pt.x, pt.y)
    time.sleep(0.15)
    if user32.GetAncestor(user32.WindowFromPoint(pt), GA_ROOT) != main:
        raise RuntimeError("main window moved away from the point")
    send_button(True)
    time.sleep(0.05)
    t0 = now()
    send_button(False)
    GUARD.sent()
    title = app["items"][item]["title"]
    rec = watch(pids, title, t0, existing=existing, settle=app.get("settle", 1.5), timeout=30)
    frame = rec.pop("_last_frame", None)
    if shot and frame is not None:
        save_png(frame, shot)
    if rec.get("hwnd"):
        rec["closed"] = close_window(rec["hwnd"], pids, title)
        rec.pop("hwnd")
    return rec


def do_editor(app, proc, png, shot=None):
    pids = {proc.pid}
    main = find_main(pids, app["main_title"], app.get("main_exclude"))
    if main:
        bring_forward(main)  # as for the clicks: the app is in front when asked
        time.sleep(0.3)
    existing = [h for h, _ in windows_of(pids)]
    exe = app["exe"]
    GUARD.check()
    t0 = now()
    p2 = subprocess.Popen([exe, "-ImageEditor", png], cwd=os.path.dirname(exe))
    title = app["editor_title"]
    rec = watch(pids | {p2.pid}, title, t0, existing=existing, settle=app.get("settle", 1.5), timeout=60, image=True)
    frame = rec.pop("_last_frame", None)
    if shot and frame is not None:
        save_png(frame, shot)
    try:
        p2.wait(timeout=30)
        rec["second_copy_exit"] = p2.returncode
    except subprocess.TimeoutExpired:
        rec["second_copy_exit"] = "still running"
    if rec.get("hwnd"):
        rec["closed"] = close_window(rec["hwnd"], pids | {p2.pid}, title)
        rec.pop("hwnd")
        # a "save changes?" prompt would be a new window: report it
        time.sleep(0.4)
        extra = [window_info(h) for h, _ in windows_of(pids) if h not in existing]
        extra = [i for i in extra if shown(i)]
        if extra:
            rec["left_windows"] = [i["title"] for i in extra]
    return rec


# ---------------------------------------------------------------- commands

def cmd_list(a):
    for h, pid in windows_of({a.pid}):
        info = window_info(h)
        if a.all or shown(info):
            print(pid, hex(h), info)


def cmd_shot(a):
    h = find_main({a.pid}, a.title)
    if not h:
        sys.exit("no window")
    f = Capturer().grab(h)
    print(stats(f), f.shape)
    save_png(f, a.out)


def cmd_bench(a):
    cfg = json.load(open(a.config, encoding="utf-8"))
    apps = cfg["apps"]
    if a.only:
        apps = {k: v for k, v in apps.items() if k in a.only.split(",")}
    for app in apps.values():
        for k in ("exe", "logs"):
            if app.get(k):
                app[k] = os.path.normpath(os.path.expandvars(app[k]))
        if a.no_items:
            app["items"] = {}
            app.pop("editor_title", None)
        if a.extra_args is not None:
            app["args"] = a.extra_args.split() if a.extra_args else []
    print("DPI of the primary monitor:", user32.GetDpiForSystem(), flush=True)
    out = open(a.out, "a", encoding="utf-8")
    shots = a.shots

    def emit(rec):
        rec = {k: v for k, v in rec.items() if not k.startswith("_")}
        if a.label:
            rec["label"] = a.label
        rec["at"] = time.strftime("%Y-%m-%d %H:%M:%S")  # wall clock, to drop rounds hit by other load
        out.write(json.dumps(rec) + "\n")
        out.flush()
        brief = {k: (round(v, 1) if isinstance(v, float) else v) for k, v in rec.items()
                 if k in ("app", "what", "round", "visible", "painted", "complete", "image", "log_startup_ms", "error", "dismissed", "closed", "left_windows")}
        print(" ", json.dumps(brief), flush=True)

    names = list(apps)
    for r in range(a.start_round, a.start_round + a.rounds):
        order = names if r % 2 == 0 else names[::-1]
        for name in order:
            app = apps[name]
            # A copy we didn't start would take our command lines (single instance).
            others = running_copies(app["exe"])
            if others:
                sys.exit(f"{name} is already running (PIDs {others}); stopping so as not to drive someone else's copy")
            print(f"round {r} {name}", flush=True)
            shot = (lambda w: os.path.join(shots, f"{name}-{w}-r{r}.png")) if shots and r == a.start_round else (lambda w: None)
            proc, rec = do_launch(app, out, shot("main"))
            rec.update(app=name, what="launch", round=r, first_run=(r == 0 and a.first_run))
            emit(rec)
            if rec.get("error"):
                print("  launch failed; stopping this app's round", flush=True)
                print("  exit:", do_exit(app, proc), flush=True)
                continue
            time.sleep(a.gap)
            try:
                for item in app.get("items", {}):
                    for rep in range(a.repeats):
                        rec = do_click(app, proc, item, shot(f"{item}{rep}"))
                        rec.update(app=name, what=item, round=r, rep=rep)
                        emit(rec)
                        time.sleep(a.gap)
                if a.png and app.get("editor_title"):
                    for rep in range(a.repeats):
                        rec = do_editor(app, proc, a.png, shot(f"editor{rep}"))
                        rec.update(app=name, what="editor", round=r, rep=rep)
                        emit(rec)
                        time.sleep(a.gap)
            except Exception as e:  # report and continue with a clean exit
                emit({"app": name, "what": "error", "round": r, "error": repr(e)})
            print("  exit:", do_exit(app, proc), flush=True)
            time.sleep(a.gap)


def pct(vals, p):
    s = sorted(vals)
    return s[max(0, math.ceil(p / 100 * len(s)) - 1)]


def cmd_report(a):
    recs = [json.loads(ln) for f in a.files for ln in open(f, encoding="utf-8") if ln.strip()]
    groups = {}
    for r in recs:
        if r.get("what") == "error":
            continue
        sub = r.get("rep", 0) if r["what"] != "launch" else ("first" if r.get("first_run") else "warm")
        what = r["what"] + (f" [{r['label']}]" if r.get("label") else "")
        key = (what, sub, r["app"])
        groups.setdefault(key, []).append(r)
    metrics = ["visible", "painted_after", "painted", "complete_after", "complete", "image_after", "image",
               "log_startup_ms", "capture_ms"]
    print("| measurement | app | metric | median | p90 | min | max | n |")
    print("|---|---|---|---:|---:|---:|---:|---:|")
    for key in sorted(groups, key=lambda k: (k[0], str(k[1]), k[2])):
        rs = groups[key]
        for m in metrics:
            vals = [r[m] for r in rs if r.get(m) is not None]
            if not vals:
                continue
            label = f"{key[0]} ({key[1]})"
            if not key[0].startswith("launch"):
                label = f"{key[0]} ({'first in process' if key[1] == 0 else 'repeat'})"
            print(f"| {label} | {key[2]} | {m} | {np.median(vals):.0f} | {pct(vals, 90):.0f} | {min(vals):.0f} | {max(vals):.0f} | {len(vals)} |")
    errs = [r for r in recs if r.get("error")]
    if errs:
        print("\nerrors:")
        for r in errs:
            print(" ", r.get("app"), r.get("what"), r.get("round"), r["error"])
    ci = [r["capture_interval_ms"] for r in recs if r.get("capture_interval_ms")]
    if ci:
        print(f"\ncapture interval: median {np.median(ci):.1f} ms, p90 {pct(ci, 90):.1f} ms")
    polls = [r for r in recs if r.get("polls") and r.get("visible")]
    if polls:
        per = [r["visible"] / r["polls"] for r in polls]
        print(f"window poll interval before visible: median {np.median(per):.2f} ms")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("list")
    p.add_argument("--pid", type=int, required=True)
    p.add_argument("--all", action="store_true")
    p = sub.add_parser("shot")
    p.add_argument("--pid", type=int, required=True)
    p.add_argument("--title", default=".")
    p.add_argument("--out", required=True)
    p = sub.add_parser("bench")
    p.add_argument("--config", required=True)
    p.add_argument("--out", required=True)
    p.add_argument("--rounds", type=int, default=10)
    p.add_argument("--start-round", type=int, default=0)
    p.add_argument("--repeats", type=int, default=2, help="opens of each sub window per launch")
    p.add_argument("--gap", type=float, default=1.0, help="seconds between measurements")
    p.add_argument("--png", help="image for -ImageEditor")
    p.add_argument("--shots", help="folder for the first round's settled captures")
    p.add_argument("--first-run", action="store_true", help="mark round 0's launch as the first run")
    p.add_argument("--only", help="comma-separated app names from the config")
    p.add_argument("--no-items", action="store_true", help="launch timing only")
    p.add_argument("--extra-args", help="replace the apps' launch arguments (space-separated; '' for none)")
    p.add_argument("--label", default="", help="stored with every record (e.g. 'box' for runs with the hotkey box)")
    p = sub.add_parser("report")
    p.add_argument("files", nargs="+")
    a = ap.parse_args()
    {"list": cmd_list, "shot": cmd_shot, "bench": cmd_bench, "report": cmd_report}[a.cmd](a)


if __name__ == "__main__":
    main()
