# Path ops harness

Runs SkiaSharp's `SKPath.Op` with the `SkiaSharp.dll` and `libSkiaSharp.dll`
of the ShareX reference build (`G:\scripts\sharexrs-ref\ShareX-build`;
override with `-p:ShareXBuild=<dir>`), headless (no window, no UI), and
writes each case's two input paths and the result with their exact float
bits. `crates/sharex-skia/tests/pathops.rs` replays the inputs through
`sharex_skia::pathops::op` and requires the same result, verb for verb and
bit for bit.

## Run

```
python tools/pathops-harness/gen_cases.py        # writes cases.txt (run from tools/pathops-harness)
dotnet build tools/pathops-harness -c Release -o G:/scripts/sharexrs-ref/pathops-harness
dotnet G:/scripts/sharexrs-ref/pathops-harness/PathOpsHarness.dll tools/pathops-harness/cases.txt crates/sharex-skia/tests/data/pathops/skiasharp.txt
cp tools/pathops-harness/cases.txt crates/sharex-skia/tests/data/pathops/cases.txt
cargo test -p sharex-skia --test pathops
```

## Cases

`cases.txt`, one per line: `name | op | pathA | pathB`. `op` is `union`,
`intersect`, `difference`, `xor` or `reversedifference`. A path is tokens:
`winding`/`evenodd` (fill type), `M x y`, `L x y`, `Q x1 y1 x y`,
`K x1 y1 x y w` (conic), `C x1 y1 x2 y2 x y`, `Z`,
`A rx ry rotation large sweep x y` (`SKPath.ArcTo`, sweep 1 = clockwise),
`oval l t r b`, `ovalccw l t r b`, `rect l t r b`, `circle cx cy r`.

`gen_cases.py` writes steps (a circle and a tail meeting it at its tangent
points, as `StepControl` builds them), speech balloons (a rounded rectangle
from `ArcTo`s and a tail leaving the body's rectangle, as
`SpeechBalloonControl`), generic shapes (overlapping and coincident
rectangles, ovals, quads, cubics, a cubic loop, stars with both fill types,
every op) and 200 seeded random pairs.

## Output

```
case <name> <op>
A <fill> M xxxxxxxx yyyyyyyy L ... K x1 y1 x y w ... Z
B ...
R ...            (or R FAIL when SKPath.Op returns null)
```

Coordinates and weights are the floats' bits in hex; the verbs are
`SkPath::RawIter`'s.
