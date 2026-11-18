"""Writes the scrolling capture cases: synthetic frame sequences (a generated
"document" seen through a viewport at a list of scroll offsets, with sticky
headers/footers, a scroll bar, flashes and size changes), never real screen
content. Each case is a folder of PNG frames under
crates/sharex-capture/tests/fixtures/scrolling/<case>/, listed in cases.json.

Pure standard library (zlib + struct), deterministic.
"""

import json
import os
import struct
import zlib

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
OUT = os.path.join(ROOT, "crates", "sharex-capture", "tests", "fixtures", "scrolling")


class Rng:
    """A 32-bit LCG (Numerical Recipes), so every run makes the same frames."""

    def __init__(self, seed):
        self.s = seed & 0xFFFFFFFF

    def next(self):
        self.s = (self.s * 1664525 + 1013904223) & 0xFFFFFFFF
        return self.s >> 8

    def range(self, lo, hi):
        return lo + self.next() % (hi - lo)


def png(path, w, h, rows):
    raw = b"".join(b"\x00" + bytes(r) for r in rows)

    def chunk(t, d):
        c = struct.pack(">I", len(d)) + t + d
        return c + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
    data += chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(data)


def document(w, d, seed, kind="text"):
    """A tall page as a list of rows (RGBA bytearrays)."""
    rng = Rng(seed)
    rows = [bytearray([250, 250, 250, 255] * w) for _ in range(d)]
    if kind == "blank":
        return rows
    if kind == "repeat":
        for y in range(d):
            if y % 30 < 4:
                for x in range(w):
                    rows[y][x * 4:x * 4 + 4] = bytes([40, 90, 160, 255])
        return rows
    if kind == "noise":
        for y in range(d):
            for x in range(w):
                v = rng.next()
                rows[y][x * 4:x * 4 + 4] = bytes([v & 255, (v >> 8) & 255, (v >> 16) & 255, 255])
        return rows
    # "text": lines of word blocks, a coloured margin mark per line.
    line = 0
    y = 4
    while y < d:
        lh = 10 + rng.range(0, 6)
        mark = bytes([rng.range(0, 256), rng.range(0, 256), rng.range(0, 256), 255])
        x = 6
        while x < w - 8:
            ww = rng.range(4, 40)
            gap = rng.range(3, 9)
            c = rng.range(10, 120)
            col = bytes([c, c + rng.range(0, 40), c + rng.range(0, 60), 255])
            for yy in range(y, min(y + lh - 4, d)):
                for xx in range(x, min(x + ww, w - 8)):
                    rows[yy][xx * 4:xx * 4 + 4] = col
            x += ww + gap
        for yy in range(y, min(y + lh - 4, d)):
            for xx in range(0, min(3, w)):
                rows[yy][xx * 4:xx * 4 + 4] = mark
        y += lh + 4 + rng.range(0, 8)
        line += 1
    return rows


def fill(rows, x0, y0, x1, y1, color):
    for y in range(max(0, y0), min(len(rows), y1)):
        r = rows[y]
        w = len(r) // 4
        for x in range(max(0, x0), min(w, x1)):
            r[x * 4:x * 4 + 4] = color


def frames_for(c):
    w, h, d = c["w"], c["h"], c["d"]
    doc = document(w, d, c.get("seed", 1), c.get("kind", "text"))
    out = []
    for k, off in enumerate(c["offsets"]):
        fh = c.get("heights", {}).get(str(k), h)
        fw = c.get("widths", {}).get(str(k), w)
        rows = []
        for y in range(fh):
            src = doc[min(max(off + y, 0), d - 1)]
            if fw <= w:
                rows.append(bytearray(src[: fw * 4]))
            else:
                rows.append(bytearray(src + bytes([250, 250, 250, 255]) * (fw - w)))
        header = c.get("header", 0)
        if header:
            fill(rows, 0, 0, fw, header, bytes([30, 30, 36, 255]))
            fill(rows, 8, 6, 60, header - 6, bytes([220, 220, 230, 255]))
        footer = c.get("footer", 0)
        if footer:
            fill(rows, 0, fh - footer, fw, fh, bytes([36, 30, 30, 255]))
            if c.get("footer_varies"):
                # A counter: a block whose width changes every frame.
                fill(rows, 70, fh - footer + 6, 70 + 4 + (k % 9) * 6, fh - 6, bytes([240, 200, 60, 255]))
        if c.get("scrollbar"):
            track = 10
            thumb_h = max(12, fh * fh // d)
            top = (fh - thumb_h) * min(off, d - fh) // max(1, d - fh)
            fill(rows, fw - track, 0, fw, fh, bytes([225, 225, 225, 255]))
            fill(rows, fw - track + 2, top, fw - 2, top + thumb_h, bytes([120, 120, 120, 255]))
        if str(k) in c.get("flash", {}):
            # Every pixel inverted: no row of this frame matches the result.
            for r in rows:
                for i in range(0, len(r), 4):
                    r[i], r[i + 1], r[i + 2] = 255 - r[i], 255 - r[i + 1], 255 - r[i + 2]
        if str(k) in c.get("spinner", {}):
            # An animated block in the middle of the frame.
            s = k * 37 % 200
            fill(rows, w // 2 - 20, fh // 2 - 20, w // 2 + 20, fh // 2 + 20, bytes([s, 255 - s, 128, 255]))
        out.append((fw, fh, rows))
    return out


def steps(start, stop, step):
    v = list(range(start, stop, step))
    if not v or v[-1] != stop:
        v.append(stop)
    return v


def jitter(seed, stop, lo, hi):
    rng = Rng(seed)
    v = [0]
    while v[-1] < stop:
        v.append(min(stop, v[-1] + rng.range(lo, hi)))
    return v


CASES = [
    dict(name="basic", w=200, h=150, d=700, offsets=steps(0, 550, 60) + [550]),
    dict(name="small_step", w=200, h=150, d=400, offsets=steps(0, 250, 7) + [250]),
    dict(name="half_step", w=200, h=150, d=700, offsets=steps(0, 550, 75) + [550]),
    dict(name="big_step", w=200, h=150, d=900, offsets=steps(0, 750, 110) + [750]),
    dict(name="no_overlap", w=200, h=150, d=900, offsets=[0, 200, 400, 400]),
    dict(name="footer_auto", w=200, h=150, d=700, footer=30, offsets=steps(0, 550, 50) + [550]),
    dict(name="footer_no_auto", w=200, h=150, d=700, footer=30, auto=False, offsets=steps(0, 550, 50) + [550]),
    dict(name="footer_varies", w=200, h=150, d=700, footer=30, footer_varies=True, offsets=steps(0, 550, 50) + [550]),
    dict(name="header", w=200, h=150, d=700, header=25, offsets=steps(0, 550, 55) + [550]),
    dict(name="header_footer", w=240, h=180, d=900, header=30, footer=24, offsets=steps(0, 720, 64) + [720]),
    dict(name="scrollbar", w=200, h=150, d=700, scrollbar=True, offsets=steps(0, 550, 60) + [550]),
    dict(name="width_change", w=200, h=150, d=700, widths={"3": 220}, offsets=[0, 60, 120, 180, 240, 240]),
    dict(name="taller_frame", w=200, h=150, d=700, heights={"0": 100}, offsets=[0, 60, 120, 120]),
    dict(name="shorter_frame", w=200, h=150, d=700, heights={"2": 120}, offsets=[0, 60, 120, 180, 240, 240]),
    dict(name="blank", w=200, h=150, d=700, kind="blank", offsets=[0, 60, 120, 180]),
    dict(name="repeating", w=200, h=150, d=700, kind="repeat", offsets=[0, 45, 90, 135, 180, 180]),
    dict(name="partial_end", w=200, h=150, d=503, offsets=[0, 60, 120, 180, 240, 300, 353, 353]),
    dict(name="flash_best_guess", w=200, h=150, d=900, flash={"3": 1}, offsets=steps(0, 600, 60) + [600]),
    dict(name="spinner", w=200, h=150, d=900, spinner={str(k): 1 for k in range(12)}, offsets=steps(0, 600, 60) + [600]),
    dict(name="tall_footer", w=400, h=600, d=2400, footer=40, seed=7, offsets=steps(0, 1800, 170) + [1800]),
    dict(name="tall_header_scrollbar", w=400, h=600, d=2400, header=60, scrollbar=True, seed=9, offsets=steps(0, 1800, 240) + [1800]),
    dict(name="narrow", w=100, h=150, d=700, offsets=steps(0, 550, 40) + [550]),
    dict(name="tiny", w=40, h=30, d=200, offsets=steps(0, 170, 9) + [170]),
    dict(name="jitter", w=220, h=160, d=1200, seed=3, offsets=jitter(11, 1040, 20, 90) + [1040]),
    dict(name="single", w=200, h=150, d=700, offsets=[0, 0]),
    dict(name="noise", w=160, h=120, d=400, kind="noise", seed=5, offsets=steps(0, 280, 35) + [280]),
    dict(name="fail_keeps_result", w=200, h=150, d=900, offsets=[0, 60, 120, 600, 660, 660]),
    dict(name="scroll_up", w=200, h=150, d=700, offsets=[300, 240, 180, 120, 120]),
    dict(name="exhausted", w=200, h=150, d=700, offsets=[0, 60, 120, 180]),
    dict(name="best_guess_twice", w=200, h=150, d=900, flash={"2": 1, "4": 1}, offsets=steps(0, 600, 70) + [600]),
]


def main():
    os.makedirs(OUT, exist_ok=True)
    listing = []
    for c in CASES:
        folder = os.path.join(OUT, c["name"])
        os.makedirs(folder, exist_ok=True)
        for f in os.listdir(folder):
            if f.startswith("frame-"):
                os.remove(os.path.join(folder, f))
        names = []
        for k, (fw, fh, rows) in enumerate(frames_for(c)):
            n = f"frame-{k:02}.png"
            png(os.path.join(folder, n), fw, fh, rows)
            names.append(n)
        listing.append({"name": c["name"], "auto_ignore_bottom_edge": c.get("auto", True), "frames": names})
    with open(os.path.join(OUT, "cases.json"), "w", newline="\n") as f:
        json.dump({"cases": listing}, f, indent=1)
        f.write("\n")
    print(f"{len(listing)} cases in {OUT}")


if __name__ == "__main__":
    main()
