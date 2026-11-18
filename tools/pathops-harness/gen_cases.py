"""Writes cases.txt for PathOpsHarness: step-like (circle + tail), balloon-like
(rounded rectangle + tail) and generic shapes, mostly unions."""
import math

out = []
def f(v):
    return f"{v:.4f}".rstrip('0').rstrip('.')

def tri(a, b, c, fill="evenodd"):
    return f"{fill} M {f(a[0])} {f(a[1])} L {f(b[0])} {f(b[1])} L {f(c[0])} {f(c[1])} Z"

def step_tail(cx, cy, r, tip):
    dx, dy = tip[0] - cx, tip[1] - cy
    ln = math.hypot(dx, dy)
    nx, ny = dx / ln, dy / ln
    px, py = -ny, nx
    proj = r * r / ln
    off = r * math.sqrt(ln * ln - r * r) / ln
    tcx, tcy = cx + nx * proj, cy + ny * proj
    return (tcx + px * off, tcy + py * off), tip, (tcx - px * off, tcy - py * off)

# step: circle + tangent tail, as StepControl builds them (local coordinates)
for i, (r, dist, ang) in enumerate([
    (21.0, 2.2, 0), (21.0, 2.2, 30), (21.0, 2.5, 45), (21.0, 3.0, 90), (21.0, 2.0, 135),
    (21.0, 2.8, 180), (21.0, 2.4, 225), (21.0, 3.5, 270), (21.0, 2.1, 315), (18.5, 4.0, 200),
    (30.0, 1.6, 60), (12.0, 6.0, 110), (25.5, 2.9, 333), (40.0, 1.3, 10),
]):
    cx, cy = r + 0.5 + (i % 3), r + 0.25 * (i % 4)
    a = math.radians(ang)
    tip = (cx + math.cos(a) * r * dist, cy + math.sin(a) * r * dist)
    s, t, e = step_tail(cx, cy, r, tip)
    out.append(f"step{i} | union | oval {f(cx - r)} {f(cy - r)} {f(cx + r)} {f(cy + r)} | {tri(s, t, e)}")

def rrect(l, t, r, b, rad):
    if rad <= 0:
        return f"evenodd M {f(l)} {f(t)} L {f(r)} {f(t)} L {f(r)} {f(b)} L {f(l)} {f(b)} Z"
    return (f"evenodd M {f(l + rad)} {f(t)} L {f(r - rad)} {f(t)} A {f(rad)} {f(rad)} 0 0 1 {f(r)} {f(t + rad)} "
            f"L {f(r)} {f(b - rad)} A {f(rad)} {f(rad)} 0 0 1 {f(r - rad)} {f(b)} "
            f"L {f(l + rad)} {f(b)} A {f(rad)} {f(rad)} 0 0 1 {f(l)} {f(b - rad)} "
            f"L {f(l)} {f(t + rad)} A {f(rad)} {f(rad)} 0 0 1 {f(l + rad)} {f(t)} Z")

def exit_point(l, t, r, b, s, e):
    # where the segment s->e leaves the rectangle
    best = None
    for (axis, val) in ((0, l), (0, r), (1, t), (1, b)):
        d = e[axis] - s[axis]
        if d == 0:
            continue
        u = (val - s[axis]) / d
        if 0 <= u <= 1:
            p = (s[0] + (e[0] - s[0]) * u, s[1] + (e[1] - s[1]) * u)
            if l - 1e-4 <= p[0] <= r + 1e-4 and t - 1e-4 <= p[1] <= b + 1e-4:
                if best is None or u > best[0]:
                    best = (u, p)
    return best[1]

# balloon: rounded rectangle + tail, as SpeechBalloonControl builds them
for i, (w, h, rad, tip) in enumerate([
    (200, 100, 10, (100, 140)), (200, 100, 10, (180, 150)), (200, 100, 10, (-30, 50)),
    (200, 100, 10, (240, 20)), (200, 100, 20, (100, -40)), (150, 80, 0, (60, 130)),
    (150, 80, 40, (170, 110)), (120, 120, 10, (130, 150)), (200, 100, 10, (5, 160)),
    (300, 60, 15, (290, 110)),
]):
    l, t, r, b = 0.0, 0.0, float(w), float(h)
    cx, cy = w / 2, h / 2
    dx, dy = tip[0] - cx, tip[1] - cy
    ln = math.hypot(dx, dy)
    nx, ny = dx / ln, dy / ln
    tw = min(0.3 * (w + h) / 2, min(w, h))
    px, py = -ny * tw / 2, nx * tw / 2
    s = exit_point(l, t, r, b, (cx + px, cy + py), tip)
    e = exit_point(l, t, r, b, (cx - px, cy - py), tip)
    out.append(f"balloon{i} | union | {rrect(l, t, r, b, rad)} | {tri(s, tip, e)}")

generic = [
    ("ovals", "oval 0 0 100 100", "oval 50 20 150 120"),
    ("ovalccw", "ovalccw 0 0 100 100", "oval 50 20 150 120"),
    ("oval_rect", "oval 0 0 100 60", "rect 40 30 140 90"),
    ("rects_shared_edge", "rect 0 0 50 50", "rect 50 0 100 50"),
    ("rects_overlap", "rect 0 0 60 60", "rect 30 30 90 90"),
    ("rect_inside", "rect 0 0 100 100", "rect 20 20 40 40"),
    ("disjoint", "rect 0 0 10 10", "oval 50 50 70 80"),
    ("tri_tri", "evenodd M 0 0 L 100 10 L 20 90 Z", "evenodd M 50 -20 L 90 80 L -10 60 Z"),
    ("star_winding", "winding M 50 0 L 79 90 L 2 35 L 98 35 L 21 90 Z", "rect 40 40 60 60"),
    ("star_evenodd", "evenodd M 50 0 L 79 90 L 2 35 L 98 35 L 21 90 Z", "oval 30 30 70 70"),
    ("quads", "M 0 0 Q 50 -40 100 0 Q 140 50 100 100 Q 50 140 0 100 Q -40 50 0 0 Z", "oval 60 60 160 160"),
    ("cubics", "M 0 0 C 30 -30 70 -30 100 0 C 130 30 130 70 100 100 C 70 130 30 130 0 100 C -30 70 -30 30 0 0 Z",
     "M 50 50 C 80 20 120 20 150 50 C 180 80 180 120 150 150 C 120 180 80 180 50 150 Z"),
    ("cubic_loop", "M 0 50 C 150 -50 150 150 0 50 Z", "rect 40 30 80 70"),
    ("circle_tangent", "circle 50 50 50", "rect 100 0 150 100"),
    ("same_oval", "oval 0 0 100 100", "oval 0 0 100 100"),
    ("arc_shape", "M 0 0 A 50 50 0 0 1 100 0 A 50 50 0 0 1 0 0 Z", "rect 25 -60 75 10"),
]
for name, a, b in generic:
    out.append(f"{name} | union | {a} | {b}")
for opname in ("intersect", "difference", "xor", "reversedifference"):
    for name, a, b in (generic[0], generic[2], generic[4], generic[7], generic[11]):
        out.append(f"{name}_{opname} | {opname} | {a} | {b}")
    out.append(f"step3_{opname} | {opname} | " + out[3].split(" | ", 2)[2])

# random shapes, seeded: many intersections, every op
import random
rng = random.Random(119)

def rnd_shape():
    k = rng.randrange(6)
    x, y = rng.uniform(-50, 150), rng.uniform(-50, 150)
    w, h = rng.uniform(5, 160), rng.uniform(5, 160)
    if k == 0:
        return f"oval {f(x)} {f(y)} {f(x + w)} {f(y + h)}"
    if k == 1:
        return rrect(x, y, x + w, y + h, rng.choice([0, 5, 12.5, min(w, h) / 2]))
    if k == 2:
        pts = [(rng.uniform(-50, 200), rng.uniform(-50, 200)) for _ in range(rng.randrange(3, 7))]
        fill = rng.choice(["evenodd", "winding"])
        return fill + " M " + " L ".join(f"{f(a)} {f(b)}" for a, b in pts) + " Z"
    if k == 3:
        c = [rng.uniform(-50, 200) for _ in range(8)]
        return f"M {f(c[0])} {f(c[1])} C {f(c[2])} {f(c[3])} {f(c[4])} {f(c[5])} {f(c[6])} {f(c[7])} Z"
    if k == 4:
        c = [rng.uniform(-50, 200) for _ in range(6)]
        return f"M {f(c[0])} {f(c[1])} Q {f(c[2])} {f(c[3])} {f(c[4])} {f(c[5])} Z"
    r = rng.uniform(5, 60)
    a = rng.uniform(0, 2 * math.pi)
    s, t, e = step_tail(x, y, r, (x + r * 2.5 * math.cos(a), y + r * 2.5 * math.sin(a)))
    return (f"oval {f(x - r)} {f(y - r)} {f(x + r)} {f(y + r)} "
            f"M {f(s[0])} {f(s[1])} L {f(t[0])} {f(t[1])} L {f(e[0])} {f(e[1])} Z")

for i in range(200):
    opname = rng.choice(["union", "union", "union", "intersect", "difference", "xor", "reversedifference"])
    out.append(f"random{i} | {opname} | {rnd_shape()} | {rnd_shape()}")

with open("cases.txt", "w", newline="\n") as fh:
    fh.write("# generated by gen_cases.py\n")
    fh.write("\n".join(out) + "\n")
print(len(out), "cases")
