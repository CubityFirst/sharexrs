"""Synthetic files for validating the history windows and the OCR window
against ShareX: test-pattern images (no screen content), a text file, an
archive and a text image for OCR, in <out>/files.

    python scripts/history-fixture.py G:/scripts/sharexrs-ref/history-fixture

Then write History.db over them with
    cargo run -p sharex-history --example sharex_history_fixture -- <out>
and copy it into both apps' profiles.
"""
import sys
import zipfile
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

SEGOE = "C:/Windows/Fonts/segoeui.ttf"


def font(size):
    try:
        return ImageFont.truetype(SEGOE, size)
    except OSError:
        return ImageFont.load_default()


# (name, size, background, accent) of each pattern.
PATTERNS = [
    ("pattern-01.png", (640, 400), (32, 96, 160), (250, 200, 60)),
    ("pattern-02.png", (480, 640), (160, 48, 64), (240, 240, 240)),
    ("pattern-03.png", (1200, 300), (24, 120, 80), (255, 255, 255)),
    ("pattern-04.png", (800, 600), (90, 60, 140), (120, 220, 255)),
    ("pattern-05.png", (300, 300), (230, 140, 30), (30, 30, 30)),
    ("pattern-06.png", (1024, 576), (40, 40, 48), (255, 90, 90)),
    ("pattern-07.png", (500, 350), (200, 200, 210), (40, 80, 200)),
    ("pattern-08.png", (720, 480), (16, 140, 150), (250, 250, 120)),
    ("pattern-09.png", (360, 640), (120, 30, 120), (255, 200, 240)),
    ("pattern-10.png", (900, 400), (70, 110, 40), (230, 255, 200)),
    ("pattern-11.png", (640, 640), (20, 60, 110), (255, 160, 60)),
    ("pattern-12.png", (1280, 720), (150, 150, 30), (20, 20, 80)),
]


def pattern(size, bg, fg, label):
    w, h = size
    im = Image.new("RGB", size, bg)
    d = ImageDraw.Draw(im)
    step = max(20, min(w, h) // 10)
    for x in range(0, w, step):
        d.line([(x, 0), (x, h)], fill=tuple(min(255, c + 30) for c in bg), width=1)
    for y in range(0, h, step):
        d.line([(0, y), (w, y)], fill=tuple(min(255, c + 30) for c in bg), width=1)
    d.ellipse([w * 0.1, h * 0.15, w * 0.45, h * 0.85], outline=fg, width=max(3, step // 6))
    d.rectangle([w * 0.55, h * 0.2, w * 0.9, h * 0.6], fill=fg)
    for i in range(8):
        c = tuple((v * (i + 2)) % 256 for v in fg)
        d.rectangle([w * 0.55 + i * w * 0.044, h * 0.68, w * 0.55 + (i + 1) * w * 0.044, h * 0.8], fill=c)
    d.text((step // 2, step // 4), label, fill=fg, font=font(max(14, step // 2)))
    return im


def main():
    out = Path(sys.argv[1]) / "files"
    out.mkdir(parents=True, exist_ok=True)
    for name, size, bg, fg in PATTERNS:
        pattern(size, bg, fg, name.split(".")[0]).save(out / name)
    pattern((800, 500), (60, 60, 60), (255, 220, 120), "photo").save(out / "photo-01.jpg", quality=90)
    frames = [pattern((240, 180), (30 + i * 40, 60, 90), (255, 255, 255), f"frame {i}") for i in range(3)]
    frames[0].save(out / "animation-01.gif", save_all=True, append_images=frames[1:], duration=300, loop=0)
    (out / "notes-01.txt").write_text("Synthetic notes for the history windows.\r\nLine two.\r\n", encoding="utf-8")
    with zipfile.ZipFile(out / "archive-01.zip", "w") as z:
        z.writestr("readme.txt", "synthetic archive")

    # The OCR sample: a heading and two paragraphs, black on white.
    im = Image.new("RGB", (760, 330), (255, 255, 255))
    d = ImageDraw.Draw(im)
    d.text((30, 24), "Release notes", fill=(0, 0, 0), font=font(30))
    lines = [
        "This version adds text recognition that runs",
        "entirely on your computer, with no upload.",
        "",
        "Select a region of the screen or open an image,",
        "then copy the recognized text with one click.",
    ]
    y = 90
    for line in lines:
        if line:
            d.text((30, y), line, fill=(20, 20, 20), font=font(22))
        y += 42
    im.save(out / "ocr-sample.png")
    print(f"wrote {len(list(out.iterdir()))} files to {out}")


if __name__ == "__main__":
    main()
