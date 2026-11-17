"""Compares two saved captures pixel for pixel (RGBA): how many pixels
differ, the largest channel difference, and where (50px cells, offset by
--origin, the capture's place on screen).

usage: python scripts/png-diff.py a.png b.png [--origin X,Y] [--diff out.png]
"""
import sys
from collections import Counter

from PIL import Image


def main():
    a_path, b_path = sys.argv[1], sys.argv[2]
    args = sys.argv[3:]
    ox, oy = map(int, args[args.index('--origin') + 1].split(',')) if '--origin' in args else (0, 0)
    a = Image.open(a_path).convert('RGBA')
    b = Image.open(b_path).convert('RGBA')
    print(f'{a_path}: {a.size}, {b_path}: {b.size}')
    if a.size != b.size:
        print('sizes differ')
        sys.exit(1)
    pa, pb = a.load(), b.load()
    w, h = a.size
    diff = [(x, y) for y in range(h) for x in range(w) if pa[x, y] != pb[x, y]]
    print(f'{len(diff)} of {w * h} pixels differ ({100 * len(diff) / (w * h):.4f}%)')
    if diff:
        md = max(max(abs(p - q) for p, q in zip(pa[x, y], pb[x, y])) for x, y in diff)
        print(f'largest channel difference {md}')
        cells = Counter(((x + ox) // 50 * 50, (y + oy) // 50 * 50) for x, y in diff)
        print('screen cells:', sorted(cells.items(), key=lambda t: -t[1])[:16])
        if '--diff' in args:
            out = Image.new('RGB', (w, h), 'black')
            po = out.load()
            for x, y in diff:
                po[x, y] = (255, 0, 255)
            out.save(args[args.index('--diff') + 1])


main()
