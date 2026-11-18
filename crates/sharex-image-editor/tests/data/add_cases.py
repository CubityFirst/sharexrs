"""Adds (or replaces) the cases for the image, cursor, emoji, smart eraser
and clone annotations and EditorCore's image operations in cases.json.

    python add_cases.py
"""

import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))


def p(x, y):
    return {"X": x, "Y": y}


def ann(t, s, e, **kw):
    a = {"$type": t, "StartPoint": p(*s), "EndPoint": p(*e)}
    a.update(kw)
    return a


STICKER = "images/sticker.png"
TILE = "images/tile.png"
ERASER = "images/eraser-base.png"

CASES = [
    # Image (ImageAnnotation.LoadImage; Stretch.Fill).
    {"name": "image-1to1", "annotations": [ann("Image", (40, 30), (104, 78), ImagePath=STICKER)]},
    {"name": "image-fractional", "annotations": [ann("Image", (40.4, 30.7), (104.4, 78.7), ImagePath=STICKER)]},
    {"name": "image-scaled-up", "annotations": [ann("Image", (20, 20), (135, 105), ImagePath=TILE)]},
    {"name": "image-scaled-down", "annotations": [ann("Image", (200, 150), (232, 174), ImagePath=STICKER)]},
    {"name": "image-stretched-reversed", "annotations": [ann("Image", (300, 250), (180.5, 160.25), ImagePath=STICKER)]},
    {"name": "image-rotated", "annotations": [ann("Image", (150, 100), (214, 148), ImagePath=STICKER, RotationAngle=30)]},
    {"name": "image-tiny", "annotations": [ann("Image", (10.6, 280.2), (10.9, 280.4), ImagePath=TILE)]},
    {"name": "image-over-shapes", "annotations": [
        ann("Rectangle", (30, 20), (160, 120), FillColor="#8000A0FF"),
        ann("Image", (60, 40), (124, 88), ImagePath=STICKER),
        ann("Ellipse", (100, 60), (200, 140)),
    ]},
    # Cursor (WindowsCursorBitmapRenderer; the Image's default Stretch.Uniform).
    {"name": "cursor-bundled", "annotations": [ann("Cursor", (100, 100), (132, 132), CursorType="PanNE")]},
    {"name": "cursor-bundled-uniform", "annotations": [ann("Cursor", (200, 60), (250, 100), CursorType="NoMove2D")]},
    {"name": "cursor-bundled-rotated", "annotations": [ann("Cursor", (60, 200), (92, 232), CursorType="PanSW", RotationAngle=45)]},
    {"name": "cursor-bundled-fractional", "annotations": [ann("Cursor", (240.5, 180.25), (272.5, 212.25), CursorType="HSplit")]},
    {"name": "cursor-system-default", "systemCursor": "Default",
     "annotations": [ann("Cursor", (300, 40), (332, 72), CursorType="Default")]},
    {"name": "cursor-system-ibeam", "systemCursor": "IBeam",
     "annotations": [ann("Cursor", (300, 120), (348, 168), CursorType="IBeam")]},
    {"name": "cursor-system-hand", "systemCursor": "Hand",
     "annotations": [ann("Cursor", (20, 240), (52, 272), CursorType="Hand", RotationAngle=-20)]},
    # Smart eraser (SmartEraserAnnotation.ApplyFill).
    {"name": "eraser-solid", "annotations": [ann("SmartEraser", (60, 60), (160, 120), FillColor="#FF336699")]},
    {"name": "eraser-solid-rotated", "annotations": [ann("SmartEraser", (200, 40), (330, 110), FillColor="#C0FFAA00", RotationAngle=20)]},
    {"name": "eraser-edges-json", "annotations": [
        ann("SmartEraser", (20, 20), (60, 26), FillMode="StretchHorizontally",
            EdgePixels=[0xFFFF0000, 0xFF00FF00, 0xFF0000FF, 0xFFFFFFFF, 0xFF000000, 0x80FF00FF]),
        ann("SmartEraser", (100.5, 200.5), (300.5, 260.5), FillMode="StretchVertically",
            EdgePixels=[0xFF000000 | (i * 0x0A0B0C) & 0xFFFFFF for i in range(20)]),
    ]},
    {"name": "eraser-fill-horizontal", "base": ERASER, "configureFill": True,
     "annotations": [ann("SmartEraser", (20, 10), (80, 60))]},
    {"name": "eraser-fill-vertical", "base": ERASER, "configureFill": True,
     "annotations": [ann("SmartEraser", (120, 10), (180, 60))]},
    {"name": "eraser-fill-solid", "base": ERASER, "configureFill": True,
     "annotations": [ann("SmartEraser", (40, 90), (160, 140), FillColor="#FF808080")]},
    {"name": "eraser-fill-clamped", "base": ERASER, "configureFill": True,
     "annotations": [ann("SmartEraser", (150, 40), (230, 70))]},
    {"name": "eraser-fill-fractional", "base": ERASER, "configureFill": True,
     "annotations": [ann("SmartEraser", (20.6, 10.3), (80.2, 60.9))]},
    {"name": "eraser-fill-reversed", "base": ERASER, "configureFill": True,
     "annotations": [ann("SmartEraser", (90, 70), (10, 5))]},
]

BORDER = "images/border.png"

# A few annotations the image operations move, cut and drop.
SHAPES = [
    ann("Rectangle", (40, 50), (140, 120)),
    ann("Ellipse", (300, 200), (380, 280), StrokeColor="#FF2080F0"),
    ann("Arrow", (120, 240), (260, 60), StrokeWidth=5),
    ann("Freehand", (0, 0), (0, 0), Points=[p(30, 250), p(80, 230), p(150, 260), p(220, 215), p(330, 250)],
        StrokeColor="#FF00A050"),
    ann("SpeechBalloon", (290, 30), (380, 90), Text="", TailPointInitialized=True, TailPoint=p(250, 150)),
]
EFFECTS = [
    ann("Blur", (40, 40), (140, 120), Amount=6),
    ann("Pixelate", (180, 40), (300, 130), Amount=12),
    ann("Highlight", (60, 180), (220, 240)),
    ann("Spotlight", (250, 160), (370, 270), IsEllipse=True),
]


def op(name, **kw):
    o = {"op": name}
    o.update(kw)
    return o


CASES += [
    # Crop (EditorCore.Crop / PerformCrop).
    {"name": "op-crop", "annotations": SHAPES, "ops": [op("crop", rect=[30.7, 40.2, 250.9, 210.5])]},
    {"name": "op-crop-effects", "annotations": EFFECTS, "ops": [op("crop", rect=[50, 30, 350, 250])]},
    {"name": "op-crop-undo", "annotations": SHAPES, "ops": [op("crop", rect=[30, 40, 250, 210]), op("undo")]},
    {"name": "op-crop-undo-redo", "annotations": SHAPES,
     "ops": [op("crop", rect=[30, 40, 250, 210]), op("undo"), op("redo")]},
    {"name": "op-crop-whole", "annotations": SHAPES, "ops": [op("crop", rect=[-10, -10, 500, 400])]},
    {"name": "op-perform-crop", "annotations": [ann("Crop", (60, 40), (300.6, 220.4))] + SHAPES,
     "ops": [op("performCrop")]},
    {"name": "op-perform-crop-undo", "annotations": [ann("Crop", (60, 40), (300, 220))] + SHAPES,
     "ops": [op("performCrop"), op("undo")]},
    # Cut out (EditorCore.CutOut / PerformCutOut, CutOutAnnotation.AdjustAnnotation).
    {"name": "op-cutout-vertical", "annotations": SHAPES + [ann("Rectangle", (160, 140), (190, 180))],
     "ops": [op("cutOut", start=150, end=200, vertical=True)]},
    {"name": "op-cutout-horizontal", "annotations": SHAPES + [ann("Rectangle", (160, 125), (190, 145))],
     "ops": [op("cutOut", start=170, end=120, vertical=False)]},
    {"name": "op-cutout-effects", "annotations": EFFECTS, "ops": [op("cutOut", start=100, end=160, vertical=True)]},
    {"name": "op-cutout-undo", "annotations": SHAPES,
     "ops": [op("cutOut", start=150, end=200, vertical=True), op("undo")]},
    {"name": "op-perform-cutout", "annotations": SHAPES,
     "ops": [op("performCutOut", rect=[140.3, 0, 181.2, 300], vertical=True)]},
    {"name": "op-perform-cutout-horizontal", "annotations": SHAPES,
     "ops": [op("performCutOut", rect=[0, 101.5, 400, 150.5], vertical=False)]},
    # Rotate and flip.
    {"name": "op-rotate90cw", "annotations": SHAPES, "ops": [op("rotate90cw")]},
    {"name": "op-rotate90ccw", "annotations": SHAPES, "ops": [op("rotate90ccw")]},
    {"name": "op-rotate180", "annotations": SHAPES, "ops": [op("rotate180")]},
    {"name": "op-rotate-custom", "annotations": SHAPES[:3], "ops": [op("rotate", angle=30)]},
    {"name": "op-rotate-custom-clipped", "annotations": SHAPES[:3], "ops": [op("rotate", angle=-20, autoResize=False)]},
    {"name": "op-flip-horizontal", "annotations": SHAPES + [ann("Rectangle", (200, 30), (280, 90), RotationAngle=25)],
     "ops": [op("flipH")]},
    {"name": "op-flip-vertical", "annotations": SHAPES + [ann("Rectangle", (200, 30), (280, 90), RotationAngle=25)],
     "ops": [op("flipV")]},
    # Resize, resize canvas, auto crop, auto-expand.
    {"name": "op-resize-down", "annotations": SHAPES, "ops": [op("resize", width=300, height=200)]},
    {"name": "op-resize-up", "annotations": SHAPES[:3], "ops": [op("resize", width=500, height=360)]},
    {"name": "op-resize-text", "fonts": ["Segoe UI"],
     "annotations": [ann("Text", (40, 60), (300, 140), Text="Resized", TextColor="#FF0000FF", FontSize=40)],
     "ops": [op("resize", width=300, height=240)]},
    {"name": "op-resize-canvas", "annotations": SHAPES,
     "ops": [op("resizeCanvas", top=10, right=20, bottom=30, left=40, color="#FF00FF00")]},
    {"name": "op-resize-canvas-crop", "annotations": SHAPES,
     "ops": [op("resizeCanvas", top=-20, right=15, bottom=-10, left=-30, color="#80FF0000")]},
    {"name": "op-auto-crop", "base": BORDER, "annotations": [ann("Rectangle", (20, 20), (60, 50), StrokeWidth=2)],
     "ops": [op("autoCrop", tolerance=10)]},
    {"name": "op-auto-crop-strict", "base": BORDER, "annotations": [], "ops": [op("autoCrop", tolerance=0)]},
    {"name": "op-auto-expand", "annotations": [ann("Rectangle", (-30, -20), (100, 80)), ann("Ellipse", (350, 250), (430, 320))],
     "ops": [op("autoExpand", margin=10, color="#FFFFFFFF")]},
    # Flatten, clone, history.
    {"name": "op-flatten", "annotations": SHAPES + EFFECTS[:1], "ops": [op("flatten"), op("crop", rect=[20, 20, 380, 280])]},
    {"name": "op-flatten-undo", "annotations": SHAPES, "ops": [op("flatten"), op("undo")]},
    {"name": "op-clone", "annotations": SHAPES[:1],
     "ops": [op("clone", rect=[30.5, 20.25, 150.75, 140.5]), op("resize", width=320, height=250)]},
    {"name": "op-clone-clipped", "annotations": [],
     "ops": [op("clone", rect=[-20, 250, 60, 330]), op("flipV")]},
    {"name": "op-history-trim", "annotations": SHAPES[:3],
     "ops": [op("rotate90cw")] * 7 + [op("undo")] * 7},
    {"name": "op-history-mixed", "annotations": SHAPES[:2],
     "ops": [op("crop", rect=[10, 10, 390, 290]), op("flipH"), op("undo"), op("cutOut", start=50, end=80, vertical=False),
             op("undo"), op("undo"), op("redo")]},
]


def main():
    path = os.path.join(HERE, "cases.json")
    with open(path, encoding="utf-8") as f:
        data = json.load(f)
    names = {c["name"] for c in CASES}
    data["cases"] = [c for c in data["cases"] if c["name"] not in names] + CASES
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(json.dumps(data, indent=1, ensure_ascii=False))


if __name__ == "__main__":
    main()
