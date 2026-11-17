"""Writes tools-harness/cases.json: every tool's parameter sets.

    python tools/tools-harness/make_cases.py tools/tools-harness/cases.json

Each case: {"name", "tool", ...parameters}. Input file names refer to the
inputs folder the harness generates (`{inputs}/...`).
"""

import itertools
import json
import sys

OPAQUE = ["opaque_97x61.png", "odd_33x77.png", "wide_301x41.png", "tall_23x157.png", "small_5x3.png"]
ALPHA = ["alpha_64x48.png", "alpha_odd_45x29.png"]
ALL_PNG = OPAQUE + ALPHA + ["tiny_1x1.png", "big_420x300.png"]
OTHER = ["photo_q90.jpg", "exif6.jpg", "exif3.jpg", "exif8.jpg", "palette.gif", "image24.bmp", "lossless.webp", "gray.png", "rgb16.png"]

cases = []


def add(name, tool, **kw):
    cases.append({"name": name, "tool": tool, **kw})


# ---------------------------------------------------------------- images

combine_sets = {
    "mix": ["opaque_97x61.png", "alpha_64x48.png", "odd_33x77.png"],
    "many": ["wide_301x41.png", "tall_23x157.png", "small_5x3.png", "alpha_odd_45x29.png", "opaque_97x61.png"],
}
for (set_name, files), orient, align, space, wrap, fill in itertools.product(
    combine_sets.items(), ["Horizontal", "Vertical"], ["LeftOrTop", "Center", "RightOrBottom"], [0, 7], [0, 2], [True, False]
):
    add(f"combine-{set_name}-{orient[0]}-{align}-s{space}-w{wrap}-{'fill' if fill else 'nofill'}", "combiner",
        files=files, options={"Orientation": orient, "Alignment": align, "Space": space, "WrapAfter": wrap,
                              "AutoFillBackground": fill})

for img, (rows, cols) in itertools.product(["opaque_97x61.png", "alpha_64x48.png", "big_420x300.png", "odd_33x77.png"],
                                           [(1, 2), (2, 1), (3, 3), (4, 5), (2, 7)]):
    add(f"split-{img.split('.')[0]}-{rows}x{cols}", "splitter", file=img, rows=rows, columns=cols)

for img, (w, h), q in itertools.product(["opaque_97x61.png", "alpha_64x48.png", "big_420x300.png", "tall_23x157.png", "exif6.jpg"],
                                        [(150, 150), (64, 32), (33, 77), (200, 50)], [90, 50]):
    add(f"thumb-{img.split('.')[0]}-{w}x{h}-q{q}", "thumbnailer", file=img, width=w, height=h, quality=q)

for img, mode, (w, h) in itertools.product(["opaque_97x61.png", "alpha_64x48.png", "big_420x300.png", "small_5x3.png", "tall_23x157.png"],
                                           ["Fill", "Fit", "Stretch"], [(100, 100), (50, 120), (301, 41), (1, 1), (97, 61), (1700, 900)]):
    add(f"resize-{img.split('.')[0]}-{mode}-{w}x{h}", "resizer", file=img, mode=mode, width=w, height=h, format="Png",
        quality=90, background=[255, 255, 255])
for img, mode in itertools.product(["alpha_64x48.png", "opaque_97x61.png"], ["Fill", "Fit", "Stretch"]):
    add(f"resize-jpeg-{img.split('.')[0]}-{mode}", "resizer", file=img, mode=mode, width=80, height=50, format="Jpeg",
        quality=80, background=[255, 0, 0])

conv_inputs = ["opaque_97x61.png", "alpha_64x48.png", "photo_q90.jpg", "exif6.jpg", "palette.gif", "image24.bmp", "lossless.webp",
               "gray.png", "rgb16.png", "big_420x300.png"]
for img in conv_inputs:
    stem = img.replace(".", "_")
    for depth in ["Automatic", "Bit32", "Bit24"]:
        add(f"conv-{stem}-png-{depth}", "converter", file=img, format="Png", pngBitDepth=depth, background=[30, 144, 255])
    for depth in ["Automatic", "Bit32", "Bit24"]:
        add(f"conv-{stem}-bmp-{depth}", "converter", file=img, format="Bmp", bmpBitDepth=depth, background=[30, 144, 255])
    for gq in ["Default", "Bit8", "Bit4", "Grayscale", "Adaptive"]:
        add(f"conv-{stem}-gif-{gq}", "converter", file=img, format="Gif", gifQuality=gq)
    for sub, q in itertools.product(["Downsample420", "Downsample422", "Downsample444"], [90, 40]):
        add(f"conv-{stem}-jpeg-{sub}-q{q}", "converter", file=img, format="Jpeg", jpegSubsampling=sub, quality=q,
            background=[255, 255, 255])
    add(f"conv-{stem}-webp-lossless", "converter", file=img, format="Webp", webpLossless=True, quality=75)
    add(f"conv-{stem}-webp-lossy", "converter", file=img, format="Webp", webpLossless=False, quality=90)
    add(f"conv-{stem}-preview", "converter-preview", file=img, format="Png")

positions = ["TopLeft", "TopCenter", "TopRight", "MiddleLeft", "Center", "MiddleRight", "BottomLeft", "BottomCenter", "BottomRight"]
for img, pos in itertools.product(["big_420x300.png", "alpha_64x48.png"], positions):
    add(f"wm-img-{img.split('.')[0]}-{pos}", "watermark", file=img, type="Image", watermark="logo_37x23.png", position=pos,
        margin=10, opacity=70, imageScale=25, rotation=0, text="", textSize=48, textColor=[255, 255, 255], format="Png")
for rot, op, scale in itertools.product([0, 30, -45, 90, 400], [100, 70, 0], [25, 100]):
    add(f"wm-img-rot{rot}-op{op}-sc{scale}", "watermark", file="big_420x300.png", type="Image", watermark="logo_37x23.png",
        position="Center", margin=24, opacity=op, imageScale=scale, rotation=rot, text="", textSize=48, textColor=[255, 255, 255],
        format="Png")
add("wm-img-jpeg", "watermark", file="big_420x300.png", type="Image", watermark="logo_37x23.png", position="BottomRight",
    margin=24, opacity=70, imageScale=25, rotation=0, text="", textSize=48, textColor=[255, 255, 255], format="Jpeg", quality=85,
    background=[0, 0, 0])
for text, size, rot, pos in [("Watermark", 48, 0, "BottomRight"), ("ShareX", 20, 0, "TopLeft"), ("Two\nlines", 30, 0, "Center"),
                             ("Rotated", 36, 30, "Center"), ("Small", 9, 0, "BottomLeft")]:
    add(f"wm-text-{text.split()[0]}-{size}-r{rot}", "watermark", file="big_420x300.png", type="Text", watermark="", position=pos,
        margin=12, opacity=70, imageScale=25, rotation=rot, text=text, textSize=size, textColor=[255, 255, 0], format="Png")
add("wm-preview-img", "watermark-preview", file="huge_1900x1100.png", type="Image", watermark="logo_37x23.png", position="BottomRight",
    margin=24, opacity=70, imageScale=25, rotation=15, text="", textSize=48, textColor=[255, 255, 255], format="Png")

for img, depth, sizes in itertools.product(["opaque_97x61.png", "alpha_64x48.png", "tall_23x157.png", "small_5x3.png", "big_420x300.png"],
                                           ["TrueColor32", "Palette8"],
                                           [[16, 32, 48, 64, 128, 256], [16], [256], [48, 16, 32, 16]]):
    add(f"icon-{img.split('.')[0]}-{depth}-{'_'.join(map(str, sizes))}", "icon", file=img, depth=depth, sizes=sizes)

compare_pairs = [("opaque_97x61.png", "opaque_97x61.png"), ("opaque_97x61.png", "opaque_97x61_dots.png"),
                 ("opaque_97x61.png", "odd_33x77.png"), ("alpha_64x48.png", "opaque_97x61.png"),
                 ("big_420x300.png", "big_420x300_dots.png"), ("photo_q90.jpg", "opaque_97x61.png"),
                 ("tiny_1x1.png", "wide_301x41.png")]
for (a, b), maxdim in itertools.product(compare_pairs, [2048, 50, 7]):
    add(f"compare-{a.split('.')[0]}-{b.split('.')[0]}-m{maxdim}", "comparer", a=a, b=b, maxPreview=maxdim)

gif_sets = {"two": ["opaque_97x61.png", "odd_33x77.png"], "alpha": ["alpha_64x48.png", "opaque_97x61.png", "small_5x3.png"],
            "big": ["big_420x300.png", "big_420x300_dots.png", "opaque_97x61.png"]}
for (set_name, files), delay, loop, repeat in itertools.product(gif_sets.items(), [100, 33], [True, False], [0, 3]):
    add(f"gifmaker-{set_name}-d{delay}-{'loop' if loop else 'once'}-r{repeat}", "gifmaker", files=files, delay=delay, loop=loop,
        repeat=repeat)

# ---------------------------------------------------------------- data

for algo in ["CRC32", "MD5", "SHA1", "SHA256", "SHA384", "SHA512"]:
    add(f"hash-{algo}", "hash", algorithm=algo)

indexer_variants = {
    "Txt": [{}, {"AddFooter": False}, {"ShowSizeInfo": False, "AddFooter": False},
            {"IndentationText": "--", "AddEmptyLineAfterFolders": True}, {"SkipHiddenFolders": False, "SkipHiddenFiles": False},
            {"SkipFiles": True}, {"MaxDepthLevel": 1}, {"MaxDepthLevel": 2}, {"BinaryUnits": True}],
    "Html": [{}, {"DisplayPath": True}, {"DisplayPath": True, "DisplayPathLimited": True},
             {"UseCustomCSSFile": True, "CustomCSSFilePath": "custom.css"}, {"ShowSizeInfo": False}, {"SkipFiles": True},
             {"AddFooter": False, "BinaryUnits": True}],
    "Xml": [{}, {"UseAttribute": False}, {"ShowSizeInfo": False}, {"SkipHiddenFiles": False, "SkipHiddenFolders": False}],
    "Json": [{}, {"CreateParseableJson": False}, {"ShowSizeInfo": False}, {"MaxDepthLevel": 1}, {"CreateParseableJson": False, "SkipFiles": True}],
}
for output, variants in indexer_variants.items():
    for i, v in enumerate(variants):
        for folder in ["tree", "empty", "sizes"]:
            add(f"index-{output}-{i}-{folder}", "indexer", folder=folder, settings={"Output": output, **v})

qr_texts = {"url": "https://getsharex.com", "hello": "Hello, World!", "numeric": "12345678901234567890",
            "alnum": "HELLO WORLD 123", "utf8": "é ü 日本語 😀", "multiline": "line1" + chr(10) + "line2", "long": "ShareX " * 70,
            "max": "a" * 2952, "toolong": "a" * 2953, "empty": ""}
for (key, text), size in itertools.product(qr_texts.items(), [64, 100, 177, 256, 400, 33]):
    add(f"qr-{key}-{size}", "qr", text=text, size=size)
for f, qr_only in itertools.product(["plain.png", "rotated90.png", "inverted.png", "translucent.png", "two.png", "code128.png",
                                     "ean13.png", "datamatrix.png", "dense.png", "small.png"], [False, True]):
    add(f"qrscan-{f.split('.')[0]}-{'qr' if qr_only else 'all'}", "qr-scan", file=f, qrOnly=qr_only)
add("qrscan-none-all", "qr-scan", file="../opaque_97x61.png", qrOnly=False)

json.dump(cases, open(sys.argv[1], "w", newline="\n"), indent=1)
print(len(cases), "cases")
