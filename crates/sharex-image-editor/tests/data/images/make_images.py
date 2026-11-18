"""Generates the test images the image, clone and smart eraser cases use.

    python make_images.py

Deterministic: every pixel is computed, no fonts, no randomness.
"""

import math
import os

from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))


def sticker():
    """64x48, straight alpha: a soft disc over a diagonal gradient, the corners transparent."""
    w, h = 64, 48
    im = Image.new("RGBA", (w, h))
    px = im.load()
    for y in range(h):
        for x in range(w):
            d = math.hypot((x + 0.5 - w / 2) / (w / 2), (y + 0.5 - h / 2) / (h / 2))
            a = max(0, min(255, int(round((1.15 - d) * 400))))
            r = (x * 255) // (w - 1)
            g = (y * 255) // (h - 1)
            b = 255 - ((x + y) * 255) // (w + h - 2)
            if (x // 8 + y // 8) % 2 == 0:
                r, g, b = 255 - r, 255 - g, b
            px[x, y] = (r, g, b, a)
    im.save(os.path.join(HERE, "sticker.png"))


def tile():
    """23x17, opaque, hard-edged blocks and lines (shows the sampling)."""
    w, h = 23, 17
    im = Image.new("RGBA", (w, h))
    px = im.load()
    for y in range(h):
        for x in range(w):
            if x % 5 == 0 or y % 4 == 0:
                c = (20, 20, 30)
            elif (x // 5 + y // 4) % 2:
                c = (240, 200, 40)
            else:
                c = (40, 120, 230)
            px[x, y] = c + (255,)
    im.save(os.path.join(HERE, "tile.png"))


def eraser_base():
    """200x150: horizontal stripes (left), vertical stripes (right), noise (bottom)."""
    w, h = 200, 150
    im = Image.new("RGBA", (w, h))
    px = im.load()
    for y in range(h):
        for x in range(w):
            if y < 75 and x < 100:
                c = ((y * 37) % 256, (y * 11 + 60) % 256, 200 - y)
            elif y < 75:
                c = (200 - (x - 100), (x * 23) % 256, (x * 7 + 30) % 256)
            else:
                v = (x * 73 + y * 151 + (x * y) % 97) % 256
                c = (v, (v * 3) % 256, 255 - v)
            px[x, y] = c + (255,)
    im.save(os.path.join(HERE, "eraser-base.png"))


def border():
    """120x90: a 15 px flat border (auto crop removes it) around a pattern."""
    w, h = 120, 90
    im = Image.new("RGBA", (w, h), (0xEE, 0xDD, 0xCC, 255))
    px = im.load()
    for y in range(15, h - 21):
        for x in range(18, w - 15):
            px[x, y] = ((x * 9) % 256, (y * 13) % 256, ((x + y) * 5) % 256, 255)
    # A pixel within the tolerance of the border colour stays border.
    px[5, 5] = (0xF0, 0xD8, 0xCA, 255)
    im.save(os.path.join(HERE, "border.png"))


if __name__ == "__main__":
    border()
    sticker()
    tile()
    eraser_base()
