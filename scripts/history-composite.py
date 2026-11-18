"""Pastes the popups (context menus) captured by history-shots.ps1 onto the
window shot they belong to, where they were on screen, so the result can be
compared with a `--render` of the same state (ShareXrs draws menus inside
the window).

    python scripts/history-composite.py <window png> <popups prefix> <out png>

<popups prefix> names `<prefix>.popups.txt` (the window's client origin,
then each popup's, top first) and `<prefix>.popup<N>.png`.
"""
import sys

from PIL import Image


def main():
    base = Image.open(sys.argv[1]).convert("RGB")
    prefix = sys.argv[2]
    lines = [l.strip() for l in open(prefix + ".popups.txt") if l.strip()]
    cx, cy = map(int, lines[0].split(","))
    popups = [tuple(map(int, l.split(","))) for l in lines[1:]]
    # Bottom-most first, so the top popup ends up on top.
    for i in reversed(range(len(popups))):
        px, py = popups[i]
        im = Image.open(f"{prefix}.popup{i}.png").convert("RGB")
        base.paste(im, (px - cx, py - cy))
    base.save(sys.argv[3])


if __name__ == "__main__":
    main()
