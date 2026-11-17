"""Compares a screenshot of ShareX with ShareXrs's rendering of the same screen.

    python scripts/compare.py <sharex.png> <sharexrs.png> <out dir> [--threshold 40]

Writes to <out dir>:
  side-by-side.png  ShareX left, ShareXrs right, same scale
  diff.png          ShareXrs dimmed, with pixels that differ by more than the
                    threshold in red
  overlay.png       50/50 blend, to spot offsets (doubled edges)
and prints how much differs, and the bounding boxes of the differing areas.

winshot.ps1 captures the client area, so both images start at the window's
first pixel. Older -Frame captures with the 1px DWM border (a black or
#202020 edge) have it cropped off first.
"""
import sys
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw


def crop_dwm_border(im: Image.Image) -> Image.Image:
    """Drops the outer rows/columns that aren't part of the window: the
    DWM border (a uniform line) and black capture padding."""
    px = im.load()
    w, h = im.size

    def uniform_col(x):
        first = px[x, h // 2]
        return all(px[x, y] == first for y in range(h // 4, 3 * h // 4, 7)) and (first[0] < 40 or first == px[x, 0])

    def uniform_row(y):
        first = px[w // 2, y]
        return all(px[x, y] == first for x in range(w // 4, 3 * w // 4, 7))

    left = 0
    while left < 3 and uniform_col(left) and px[left, h // 2][:3] == (0, 0, 0):
        left += 1
    right = w
    while right > w - 3 and px[right - 1, h // 2][:3] == (0, 0, 0):
        right -= 1
    top = 0
    while top < 3 and uniform_row(top) and px[w // 2, top][:3] in ((32, 32, 32), (0, 0, 0)):
        top += 1
    bottom = h
    while bottom > h - 3 and px[w // 2, bottom - 1][:3] == (0, 0, 0):
        bottom -= 1
    return im.crop((left, top, right, bottom))


def boxes(mask: Image.Image, cell=16):
    """Groups differing pixels into rough rectangles, cell by cell."""
    w, h = mask.size
    px = mask.load()
    hot = set()
    for cy in range(0, h, cell):
        for cx in range(0, w, cell):
            n = sum(1 for y in range(cy, min(cy + cell, h)) for x in range(cx, min(cx + cell, w)) if px[x, y])
            if n > cell:  # more than a stray glyph edge
                hot.add((cx // cell, cy // cell))
    seen, out = set(), []
    for c in sorted(hot):
        if c in seen:
            continue
        stack, xs, ys = [c], [], []
        seen.add(c)
        while stack:
            x, y = stack.pop()
            xs.append(x)
            ys.append(y)
            for n in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
                if n in hot and n not in seen:
                    seen.add(n)
                    stack.append(n)
        out.append((min(xs) * cell, min(ys) * cell, (max(xs) + 1) * cell, (max(ys) + 1) * cell, len(xs)))
    return sorted(out, key=lambda b: -b[4])


def main():
    args = sys.argv[1:]
    threshold = 40
    if "--threshold" in args:
        i = args.index("--threshold")
        threshold = int(args[i + 1])
        del args[i : i + 2]
    a_path, b_path, out = args
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    a = crop_dwm_border(Image.open(a_path).convert("RGB"))
    b = Image.open(b_path).convert("RGB")
    print(f"ShareX   {a.size[0]}x{a.size[1]} (after cropping the DWM border)")
    print(f"ShareXrs {b.size[0]}x{b.size[1]}")
    w, h = max(a.size[0], b.size[0]), max(a.size[1], b.size[1])
    A = Image.new("RGB", (w, h), (255, 0, 255))
    A.paste(a)
    B = Image.new("RGB", (w, h), (255, 0, 255))
    B.paste(b)

    side = Image.new("RGB", (w * 2 + 10, h + 24), (128, 128, 128))
    side.paste(A, (0, 24))
    side.paste(B, (w + 10, 24))
    d = ImageDraw.Draw(side)
    d.text((6, 6), "ShareX", fill=(255, 255, 255))
    d.text((w + 16, 6), "ShareXrs", fill=(255, 255, 255))
    side.save(out / "side-by-side.png")

    Image.blend(A, B, 0.5).save(out / "overlay.png")

    diff = ImageChops.difference(A, B).convert("L")
    mask = diff.point(lambda v: 255 if v > threshold else 0)
    red = Image.new("RGB", (w, h), (255, 0, 0))
    base = Image.blend(B, Image.new("RGB", (w, h), (0, 0, 0)), 0.6)
    Image.composite(red, base, mask).save(out / "diff.png")

    n = sum(1 for v in mask.get_flattened_data() if v)
    print(f"{n} of {w * h} pixels differ by more than {threshold} ({100 * n / (w * h):.2f}%)")
    for x0, y0, x1, y1, cells in boxes(mask)[:15]:
        print(f"  differs around x {x0}-{x1}, y {y0}-{y1} ({cells} cells)")


if __name__ == "__main__":
    main()
