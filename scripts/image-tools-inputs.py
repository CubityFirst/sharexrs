"""The synthetic inputs of the Images tool sessions (scripts/image-tools-shots.ps1,
image-tools-behaviour.ps1, image-tools-compare.sh): gradients with shapes,
a translucent image, a stamp and a slightly edited copy for the comparer.

    python scripts/image-tools-inputs.py G:/scripts/sharexrs-ref/tools-ui/in
"""
import sys
from PIL import Image, ImageDraw
out = sys.argv[1]
def photo(w, h, seed):
    im = Image.new("RGB", (w, h))
    px = im.load()
    for y in range(h):
        for x in range(w):
            px[x, y] = ((x * 255 // w + seed * 40) % 256, (y * 255 // h + seed * 70) % 256, ((x + y) * 128 // (w + h) + seed * 90) % 256)
    d = ImageDraw.Draw(im)
    d.ellipse((w*0.2, h*0.2, w*0.55, h*0.75), fill=(240, 200, 40))
    d.rectangle((w*0.6, h*0.3, w*0.9, h*0.8), fill=(30, 60, 160), outline=(255, 255, 255), width=3)
    return im
photo(640, 400, 0).save(f"{out}/photo.png")
photo(480, 320, 1).save(f"{out}/landscape.png")
photo(300, 420, 2).save(f"{out}/portrait.png")
a = Image.new("RGBA", (320, 240), (0, 0, 0, 0))
d = ImageDraw.Draw(a)
d.ellipse((20, 20, 300, 220), fill=(220, 60, 90, 200))
d.rectangle((100, 80, 220, 160), fill=(40, 200, 120, 255))
a.save(f"{out}/alpha.png")
s = Image.new("RGBA", (160, 60), (0, 0, 0, 0))
d = ImageDraw.Draw(s)
d.rounded_rectangle((0, 0, 159, 59), 12, fill=(255, 255, 255, 230), outline=(20, 20, 20, 255), width=3)
d.rectangle((20, 20, 140, 40), fill=(200, 30, 30, 255))
s.save(f"{out}/stamp.png")
# a near copy of photo.png with a few changed pixels, for the comparer
p = photo(640, 400, 0)
d = ImageDraw.Draw(p)
d.rectangle((50, 300, 120, 360), fill=(255, 0, 0))
p.save(f"{out}/photo-edited.png")
