"""Checks how much of a screen grab is `sharexrs --test-pattern`'s picture.

    python scripts/pattern-check.py shot.png [--origin X,Y] [--min 0.5] [--delete]

Prints the share of pixels that match the pattern exactly. With --delete,
removes the file when fewer than --min of them match: a grab that isn't
mostly the pattern may show other apps, and must not be kept. Dimmed
pattern pixels (a capture overlay's 20% black) count as matching.
"""

import os
import sys

from PIL import Image


def pattern(x, y, alt=False):
    if x % 64 == 0 or y % 64 == 0:
        return (255, 255, 255)
    checker = ((x // 16) + (y // 16)) % 2 == 0
    r, g = x % 256, y % 256
    if alt:
        return (255 - r, 255 - g, 160 if checker else 0)
    return (r, g, 200 if checker else 70)


def dim(c, alpha=51):
    return tuple((v * (255 - alpha) + 127) // 255 for v in c)


def close(a, b, tol=2):
    return all(abs(p - q) <= tol for p, q in zip(a, b))


def main():
    args = sys.argv[1:]
    path = args[0]
    ox, oy = 0, 0
    minimum = 0.5
    delete = "--delete" in args
    if "--origin" in args:
        ox, oy = map(int, args[args.index("--origin") + 1].split(","))
    if "--min" in args:
        minimum = float(args[args.index("--min") + 1])
    img = Image.open(path).convert("RGB")
    w, h = img.size
    px = img.load()
    match = 0
    total = 0
    for y in range(0, h, 3):
        for x in range(0, w, 3):
            total += 1
            c = px[x, y]
            p = pattern(x + ox, y + oy)
            q = pattern(x + ox, y + oy, True)
            if close(c, p) or close(c, dim(p)) or close(c, q) or close(c, dim(q)):
                match += 1
    share = match / total
    print(f"{path}: {share:.4f} of sampled pixels are the test pattern")
    if delete and share < minimum:
        os.remove(path)
        print(f"deleted {path}: not mostly the test pattern")
        sys.exit(2)


if __name__ == "__main__":
    main()
