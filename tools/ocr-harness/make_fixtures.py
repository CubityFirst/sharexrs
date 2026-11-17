"""Copies a small set of the OCR harness's inputs and ShareX's outputs into
crates/sharex-ocr/tests/fixtures, for the crate's CI tests (tests/sharex.rs),
which check everything that runs without ONNX Runtime and the models.

    python tools/ocr-harness/make_fixtures.py <cases dir> <work dir> crates/sharex-ocr/tests/fixtures

- results.json.z: every case's ShareX result, trimmed (sizes, boxes, raw texts,
  crops, regions, lines, the three texts), for the line grouping test.
- inputs/<case>.png, <case>.resized.z, <case>.crop_<i>.z: test images with
  ShareX's detection input and line crops (raw BGRA), for the preprocessing test.
- <case>.prob.z: ShareX's probability maps (little-endian floats), for the box
  extraction test.

Raw files are zlib-compressed.
"""

import json
import os
import shutil
import sys
import zlib

cases_dir, work, out = sys.argv[1], sys.argv[2], sys.argv[3]
sharex = os.path.join(work, "sharex")

# Upscaled (Catmull-Rom) and shrunk (mipmapped) crops, rotated, vertical and skewed
# lines, and a detection input shrunk between two mipmap levels.
PREPROCESS = ["tiny_image", "large_72", "giant_text", "rotated_15", "rotated_90", "skewed", "vertical_cjk", "very_wide_short"]
# Large files that add nothing the other cases don't test: giant_text's detection
# input, very_wide_short's 3200 px line crop.
NO_RESIZED = {"giant_text"}
NO_CROPS = {"very_wide_short"}
PROBABILITY = ["tiny_image", "rotated_90"]

KEEP = ["width", "height", "detInputWidth", "detInputHeight", "mapWidth", "mapHeight", "text", "textSingleLine", "textNoParagraphs"]

if os.path.isdir(out):
    shutil.rmtree(out)
os.makedirs(os.path.join(out, "inputs"))


def z(src, dst):
    with open(src, "rb") as f:
        data = f.read()
    with open(dst, "wb") as f:
        f.write(zlib.compress(data, 9))


results = {}
for name in sorted(os.listdir(sharex)):
    with open(os.path.join(sharex, name, "result.json"), encoding="utf-8") as f:
        r = json.load(f)
    t = {k: r[k] for k in KEEP}
    t["boxes"] = [b["bits"] for b in r["boxes"]]
    t["texts"] = [{"text": x["text"], "scoreBits": x["scoreBits"]} for x in r.get("texts", [])]
    t["crops"] = r.get("crops", [])
    t["regions"] = [{"text": x["text"], "scoreBits": x["scoreBits"], "pointBits": x["pointBits"]} for x in r["regions"]]
    t["lines"] = [{"text": x["text"], "startsParagraph": x["startsParagraph"], "heightBits": x["heightBits"], "boundsBits": x["bounds"]["bits"]} for x in r["lines"]]
    results[name] = t
with open(os.path.join(out, "results.json.z"), "wb") as f:
    f.write(zlib.compress(json.dumps(results, ensure_ascii=False, separators=(",", ":")).encode("utf-8"), 9))

for name in PREPROCESS:
    shutil.copy(os.path.join(cases_dir, name + ".png"), os.path.join(out, "inputs", name + ".png"))
    d = os.path.join(sharex, name)
    if name not in NO_RESIZED:
        z(os.path.join(d, "resized.bgra"), os.path.join(out, name + ".resized.z"))
    for i in range(0 if name in NO_CROPS else len(results[name]["crops"])):
        z(os.path.join(d, f"crop_{i}.bgra"), os.path.join(out, f"{name}.crop_{i}.z"))
for name in PROBABILITY:
    z(os.path.join(sharex, name, "prob.f32"), os.path.join(out, name + ".prob.z"))

total = sum(os.path.getsize(os.path.join(dp, f)) for dp, _, fs in os.walk(out) for f in fs)
print(f"{len(results)} cases, {total // 1024} KB in {out}")
