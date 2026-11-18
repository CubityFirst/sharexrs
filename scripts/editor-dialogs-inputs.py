"""The synthetic images the image editor's dialog shots use
(scripts/compare-editor-dialogs.sh): a 640x400 gradient with squares, and
the 200x120 image the Insert image dialog inserts.
    python scripts/editor-dialogs-inputs.py target/edtest
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw

out = Path(sys.argv[1] if len(sys.argv) > 1 else 'target/edtest')
out.mkdir(parents=True, exist_ok=True)
w, h = 640, 400
im = Image.new('RGB', (w, h))
px = im.load()
for y in range(h):
    for x in range(w):
        px[x, y] = (x * 255 // w, y * 255 // h, 160)
d = ImageDraw.Draw(im)
for i in range(0, w, 80):
    d.rectangle([i + 10, 20, i + 60, 70], fill=(255, 255, 255) if (i // 80) % 2 else (20, 20, 20))
im.save(out / 'test-640x400.png')
ins = Image.new('RGBA', (200, 120), (30, 200, 90, 255))
ImageDraw.Draw(ins).ellipse([20, 20, 180, 100], fill=(250, 240, 40, 255))
ins.save(out / 'insert-200x120.png')
