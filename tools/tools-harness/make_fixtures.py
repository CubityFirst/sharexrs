"""Copies a small set of the harness's inputs and ShareX's outputs into
crates/sharex-tools/tests/fixtures, for the crate's CI tests (tests/sharex.rs).

    python tools/tools-harness/make_fixtures.py <work dir> crates/sharex-tools/tests/fixtures

Raw bitmaps (.sxrb) are stored zlib-compressed (.sxrb.z).
"""

import json
import os
import shutil
import sys
import zlib

work, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)

INPUTS = ["opaque_97x61.png", "alpha_64x48.png", "odd_33x77.png", "photo_q90.jpg", "exif6.jpg", "lossless.webp", "trim.gif", "disp1.gif",
          "meta/full.jpg", "meta/full.png"]
EXPECTED = [
    "thumb-exif6-64x32-q90/exif6_th.jpg",
    "conv-photo_q90_jpg-jpeg-Downsample422-q40.jpg",
    "conv-alpha_64x48_png-bmp-Bit32.bmp",
    "conv-alpha_64x48_png-gif-Adaptive.gif",
    "conv-photo_q90_jpg-png-Automatic.loaded.sxrb",
    "conv-lossless_webp-png-Automatic.loaded.sxrb",
    "gifmaker-two-d100-loop-r0.gif",
    "icon-alpha_64x48-Palette8-16.ico",
    "icon-opaque_97x61-TrueColor32-48_16_32_16.ico",
    "qr-utf8-100.svg",
    "qr-utf8-100.sxrb",
    "combine-mix-H-Center-s7-w2-nofill.sxrb",
    "resize-alpha_64x48-Fit-50x120.sxrb",
    "monitor-gradient-ForwardDiagonal-2550.png",
    "monitor-motion-VerticalBars-37.25.png",
    "giftrim-trim_gif/1-3.gif",
    "gifreencode-disp1_gif/frame4.sxrb",
    "gifreencode-disp1_gif/export3.gif",
    "gifreencode-disp1_gif/export10.gif",
    "gifreencode-disp1_gif/export12.gif",
    "meta-full_jpg.json",
    "meta-full_png.json",
    "vargs-nofolder.json",
    "vtrim-1.5-4.25-precise-mp4.json",
]

for f in INPUTS:
    shutil.copyfile(os.path.join(work, "inputs", f), os.path.join(out, "in-" + f.replace("/", "-")))
for f in EXPECTED:
    src = os.path.join(work, "sharex", f)
    name = "sharex-" + f.replace("/", "-")
    if f.endswith(".sxrb"):
        open(os.path.join(out, name + ".z"), "wb").write(zlib.compress(open(src, "rb").read(), 9))
    elif f.startswith("meta-"):
        # Only what's stable across checkouts: no File System times.
        j = json.load(open(src, encoding="utf-8"))
        j = {"read": [x for x in j["read"] if x[0] != "File System"]}
        json.dump(j, open(os.path.join(out, name), "w", encoding="utf-8", newline="\n"), ensure_ascii=False, indent=1)
    else:
        shutil.copyfile(src, os.path.join(out, name))
total = sum(os.path.getsize(os.path.join(out, x)) for x in os.listdir(out))
print(len(os.listdir(out)), "files,", total, "bytes")
