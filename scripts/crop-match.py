"""Crops a ShareXrs render to where a ShareX popup shot fits best in it.

    python scripts/crop-match.py <shot.png> <render.png> <out.png> [--near X,Y] [--radius R]

ShareX's popups (menus, dropdowns) are their own windows, so their shots are
the popup alone; ShareXrs draws them inside its window. This slides the shot
over the render (every position, or within R pixels of X,Y) and writes the
best-matching crop, the shot's size, for `compare.py --no-crop`. The shot's
outer 3 pixels (the popup window's rounded corners on black) don't count.
Prints the position and the share of sampled pixels that differ.
"""
import sys

import numpy as np
from PIL import Image


def main():
    args = sys.argv[1:]
    if len(args) < 3:
        print(__doc__)
        sys.exit(2)
    shot = np.asarray(Image.open(args[0]).convert("RGB"), dtype=np.int16)
    render_im = Image.open(args[1]).convert("RGB")
    render = np.asarray(render_im, dtype=np.int16)
    near, radius = None, None
    i = 3
    while i < len(args):
        if args[i] == "--near":
            near = tuple(map(int, args[i + 1].split(",")))
        elif args[i] == "--radius":
            radius = int(args[i + 1])
        i += 2
    sh, sw = shot.shape[:2]
    rh, rw = render.shape[:2]
    inner = shot[3:sh - 3:2, 3:sw - 3:2]
    x_range = range(0, rw - sw + 1)
    y_range = range(0, rh - sh + 1)
    if near and radius is not None:
        x_range = range(max(0, near[0] - radius), min(rw - sw, near[0] + radius) + 1)
        y_range = range(max(0, near[1] - radius), min(rh - sh, near[1] + radius) + 1)
    best = (None, None)
    for oy in y_range:
        for ox in x_range:
            win = render[oy + 3:oy + sh - 3:2, ox + 3:ox + sw - 3:2]
            bad = int((np.abs(win - inner).sum(axis=2) > 40).sum())
            if best[1] is None or bad < best[1]:
                best = ((ox, oy), bad)
    (ox, oy), bad = best
    render_im.crop((ox, oy, ox + sw, oy + sh)).save(args[2])
    total = inner.shape[0] * inner.shape[1]
    print(f"at {ox},{oy}: {100.0 * bad / total:.2f}% of sampled pixels differ")


if __name__ == "__main__":
    main()
