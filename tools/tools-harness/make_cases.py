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

for f in ["full.jpg", "progressive.jpg", "plain.jpg", "full.png", "full.gif", "be.tif", "le.tif", "image.bmp", "full.webp",
          "lossy.webp", "clip.mp4", "clip.mov", "clip.mkv", "clip.webm", "clip.avi", "clip.wmv"]:
    add(f"meta-{f.replace('.', '_')}", "metadata", file=f)
add("meta-icon", "metadata", file="../../sharex/icon-alpha_64x48-Palette8-16_32_48_64_128_256.ico")

# ---------------------------------------------------------------- video

vm_codecs = ["X264", "X265", "H264Nvenc", "HevcNvenc", "H264Amf", "HevcAmf", "H264Qsv", "HevcQsv", "Vp8", "Vp9", "Av1", "Xvid", "Gif",
             "Webp", "Apng"]
opt_codecs = ["x264", "x265", "h264_nvenc", "hevc_nvenc", "h264_amf", "hevc_amf", "h264_qsv", "hevc_qsv", "vp8", "vp9", "av1", "xvid",
              "gif", "webp", "apng"]
for (vm, opt), use_bitrate, (q, vq), inp, name in itertools.product(
        zip(vm_codecs, opt_codecs), [False, True], [(23, 22.5), (0, 3.5), (70, 70.0), (-5, -1.0)],
        ["C:\\videos\\in clip.mov", "C:\\videos\\anim.GIF"], ["out", "out.mkv"]):
    add(f"vargs-{opt}-{'br' if use_bitrate else 'q'}-{q}-{inp[-3:].lower()}-{name.replace('.', '_')}", "video-args",
        input=inp, folder="C:\\out dir", outName=name, codec=opt, vmCodec=vm, quality=q, vmQuality=vq, useBitrate=use_bitrate,
        bitrate=2500, vmBitrate=2500.7)
add("vargs-custom", "video-args", input="a.mp4", folder="C:\\o", outName="b", codec="x264", vmCodec="X264", quality=23, vmQuality=23.0,
    useBitrate=False, bitrate=3000, vmBitrate=3000.0, custom=True)
add("vargs-nofolder", "video-args", input="a.mp4", folder=None, outName="b", codec="vp9", vmCodec="Vp9", quality=30, vmQuality=30.0,
    useBitrate=False, bitrate=3000, vmBitrate=3000.0)
for (start, end), precise, (inp, out) in itertools.product([(0, 2), (1.5, 4.25), (0.1234567, 10.0000001), (12.75, 3600.5)], [False, True],
                                                          [("C:\\v\\in.mkv", "C:\\v\\out.mkv"), ("in clip.mp4", "out clip.mp4")]):
    add(f"vtrim-{start}-{end}-{'precise' if precise else 'copy'}-{out[-3:]}", "video-trim-args", input=inp, output=out, start=start,
        end=end, precise=precise)
for f in ["clip.mp4", "clip.mov", "clip.mkv", "clip.webm", "clip.avi", "clip.wmv"]:
    add(f"vinfo-{f.replace('.', '_')}", "video-outputinfo", file=f)
for i, (text, size) in enumerate([("  Duration: 00:01:02.50, start: 0.000000, bitrate: N/A\n    Stream #0:0: Video: gif, bgra, 320x240, 25 fps", 1000000),
                                  ("  Duration: 01:00:00.00, start: 0.0, bitrate: 1234.5 kb/s", 5), ("nothing", 10),
                                  ("  Duration: 00:00:00.00, bitrate: N/A\n  Stream #0:1(und): Video: h264 (High), yuv420p, 0x0", 99)]):
    add(f"vinfo-text-{i}", "video-outputinfo", text=text, size=size)
for name, opts in [("default", {}), ("jpeg-4", {"ImageFormat": "JPEG", "ThumbnailCount": 4, "ColumnCount": 2, "KeepScreenshots": True}),
                   ("plain", {"ThumbnailCount": 3, "AddVideoInfo": False, "AddTimestamp": False, "DrawShadow": False, "DrawBorder": False,
                              "MaxThumbnailWidth": 100, "KeepScreenshots": True}),
                   ("separate", {"ThumbnailCount": 2, "CombineScreenshots": False})]:
    add(f"vthumbs-{name}", "video-thumbs", file="long.mp4", options=opts)

# ---------------------------------------------------------------- system

for gray in [255, 127.5, 0, 300]:
    add(f"monitor-gray-{gray}", "monitor-test", mode="Grayscale", width=64, height=40, gray=gray)
add("monitor-rgb", "monitor-test", mode="Rgb", width=64, height=40, red=12.4, green=200.6, blue=99.5)
for direction, (start, end) in itertools.product(["Horizontal", "Vertical", "ForwardDiagonal", "BackwardDiagonal"],
                                                 [([255, 255, 255], [0, 0, 0]), ([255, 0, 0], [0, 64, 255])]):
    add(f"monitor-gradient-{direction}-{start[0]}{end[2]}", "monitor-test", mode="Gradient", width=257, height=129, direction=direction,
        start=start, end=end)
for pattern, size in itertools.product(["HorizontalLines", "VerticalLines", "Checkerboard"], [1, 5, 2.5, 13]):
    add(f"monitor-pattern-{pattern}-{size}", "monitor-test", mode="Pattern", width=97, height=61, pattern=pattern, patternSize=size)
for motion, offset in itertools.product(["VerticalBars", "HorizontalBars"], [0, 37.25, 133.7, 1000.5]):
    add(f"monitor-motion-{motion}-{offset}", "monitor-test", mode="Motion", width=301, height=167, motion=motion, offset=offset)

for f in ["trim.gif", "../sharex/gifmaker-alpha-d33-loop-r3.gif", "../sharex/gifmaker-big-d100-once-r0.gif", "meta/full.gif", "palette.gif"]:
    add(f"giftrim-{f.split('/')[-1].replace('.', '_')}", "gif-trim", file=f)

# The GIF trimmer's frames, preview and export (lossless, re-encode, crop, resize).
# Each export: [first, end exclusive, reencode, crop x/y/w/h, size w/h].
def full(w, h):
    return [0, 0, w, h]


reencode_sets = {
    "trim.gif": (60, 40, 5),
    "disp0.gif": (48, 36, 7),
    "disp1.gif": (48, 36, 7),
    "disp2.gif": (48, 36, 6),
    "../sharex/gifmaker-alpha-d33-loop-r3.gif": None,
}
for f, dims in reencode_sets.items():
    exports = []
    if dims:
        w, h, n = dims
        for first, end in [(0, n), (1, n), (2, n - 1), (n - 1, n), (3, n)]:
            exports.append([first, end, False, full(w, h), [w, h]])
            exports.append([first, end, True, full(w, h), [w, h]])
        exports += [
            [0, n, False, [5, 3, w - 12, h - 9], [w - 12, h - 9]],
            [1, n, False, full(w, h), [w * 2, h + 7]],
            [0, n - 2, True, [7, 2, w // 2, h // 2], [w // 3, h // 3]],
            [2, n, False, [0, 0, w // 2, h], [w, h * 3 // 2]],
            [0, 1, True, [w - 1, h - 1, 1, 1], [3, 3]],
            # out of range: ShareX throws
            [0, n, False, [0, 0, w + 1, h], [w, h]],
            [n - 1, n - 1, True, full(w, h), [w, h]],
        ]
    add(f"gifreencode-{f.split('/')[-1].replace('.', '_')}", "gif-reencode", file=f, exports=exports,
        preview=20 if f == "disp1.gif" else 640)

# FormatTime / TryParseTime (the trimmers' time boxes).
add("vtrim-time", "video-trim-time",
    seconds=[0, -5, 0.0004, 0.0005, 0.9999999, 1.5, 59.9995, 61.25, 3599.999, 3600, 86399.5, 360000.123, 1e-7,
             12345.6789, 0.1 + 0.2, 2.675, 0.001, 0.0019999],
    texts=["00:00:00.000", "1:02:03.5", " 01:02:03.25 ", "1:2:3", "01:60:00", "01:59:59.9999", "00:00:60", "-1:00:00",
           "+1:00:00", "1:00:00,5", "1:00", "1:00:00:00", "", "  ", "a:b:c", "1:00:.5", "1:00:5.", "1:00:.",
           "00001:05:07.125", "2147483647:00:00", "2147483648:00:00", "1:00:1e1", "1 :00:00", "1:00:00.5.5", "0:0:0",
           "99999999999999999999:00:00", "0000000000000000000001:00:00", "\t0:00:01\n", "0:00:59.99999999999"])

json.dump(cases, open(sys.argv[1], "w", newline="\n"), indent=1)
print(len(cases), "cases")
