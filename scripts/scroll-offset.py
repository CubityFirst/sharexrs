"""Finds how far a settings page was scrolled in a ShareX shot, so ShareXrs's
render can be scrolled the same way.

    python scripts/scroll-offset.py <sharex.png> <tall.png> [--columns X0,X1] [--rows Y0,Y1] [--max N]

<tall.png> is ShareXrs's render of the same page, unscrolled and tall enough to
show all of it (`--height 4000`). The rows Y0..Y1 of the shot (default: the
whole shot), in columns X0..X1 (default 240,900: the page, not the navigation),
are searched for in the tall render; prints the scroll offset (the best match's
top minus Y0) and how many pixels differ there by more than 40.
"""
import sys

from PIL import Image


def main():
    args = sys.argv[1:]
    if len(args) < 2:
        print(__doc__)
        sys.exit(2)
    shot = Image.open(args[0]).convert("RGB")
    tall = Image.open(args[1]).convert("RGB")
    x0, x1 = 240, 900
    y0, y1 = 0, shot.height
    max_offset = tall.height - shot.height
    i = 2
    while i < len(args):
        if args[i] == "--columns":
            x0, x1 = map(int, args[i + 1].split(","))
        elif args[i] == "--rows":
            y0, y1 = map(int, args[i + 1].split(","))
        elif args[i] == "--max":
            max_offset = int(args[i + 1])
        i += 2
    s = shot.load()
    t = tall.load()
    step = 3
    best = None
    for off in range(0, max(0, max_offset) + 1):
        bad = 0
        for y in range(y0, y1, step):
            ty = y + off
            if ty >= tall.height:
                bad += (x1 - x0) // step
                continue
            for x in range(x0, x1, step):
                a, b = s[x, y], t[x, ty]
                if abs(a[0] - b[0]) + abs(a[1] - b[1]) + abs(a[2] - b[2]) > 40:
                    bad += 1
            if best is not None and bad >= best[1]:
                break
        if best is None or bad < best[1]:
            best = (off, bad)
    print(f"offset {best[0]} ({best[1]} sampled pixels differ)")


if __name__ == "__main__":
    main()
