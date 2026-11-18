"""The synthetic image the Pin to screen sessions pin (from the clipboard or
a file): stripes, a 1px checker and a gradient, so scaling and placement
show pixel for pixel.

    python scripts/screen-tools-inputs.py <out dir>
"""
import sys
from PIL import Image, ImageDraw
out = sys.argv[1]
w, h = 240, 150
im = Image.new("RGB", (w, h))
px = im.load()
for y in range(h):
    for x in range(w):
        px[x, y] = (x * 255 // (w - 1), y * 255 // (h - 1), 128)
d = ImageDraw.Draw(im)
for y in range(20, 60):
    for x in range(20, 80):
        px[x, y] = (255, 255, 255) if (x + y) % 2 else (0, 0, 0)
d.rectangle((100, 20, 220, 60), fill=(240, 200, 40), outline=(20, 20, 20), width=2)
d.ellipse((30, 80, 130, 140), fill=(30, 60, 160))
d.line((140, 80, 230, 140), fill=(255, 255, 255), width=3)
im.save(f"{out}/pin.png")
