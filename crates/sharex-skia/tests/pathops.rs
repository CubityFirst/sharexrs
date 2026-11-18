//! `pathops::op` against SkiaSharp's `SKPath.Op`, run by
//! `tools/pathops-harness` with the libSkiaSharp ShareX ships: the same
//! inputs (exact float bits) must give the same result path, verb for verb
//! and bit for bit.

use sharex_skia::geom::Point;
use sharex_skia::path::{FillType, Path, Verb};
use sharex_skia::pathops::{PathOp, op};

fn f(hex: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(hex, 16).unwrap())
}

fn parse(text: &str) -> Path {
    let t: Vec<&str> = text.split_whitespace().collect();
    let mut p = Path::new();
    p.fill_type = match t[0] {
        "evenodd" => FillType::EvenOdd,
        _ => FillType::Winding,
    };
    let mut i = 1;
    while i < t.len() {
        let n = |k: usize| f(t[i + 1 + k]);
        match t[i] {
            "M" => {
                p.move_to(n(0), n(1));
                i += 3;
            }
            "L" => {
                p.line_to(n(0), n(1));
                i += 3;
            }
            "Q" => {
                p.quad_to(n(0), n(1), n(2), n(3));
                i += 5;
            }
            "K" => {
                p.conic_to(n(0), n(1), n(2), n(3), n(4));
                i += 6;
            }
            "C" => {
                p.cubic_to(n(0), n(1), n(2), n(3), n(4), n(5));
                i += 7;
            }
            "Z" => {
                p.close();
                i += 1;
            }
            v => panic!("verb {v}"),
        }
    }
    p
}

/// The path in the harness's format (`SkPath::RawIter`).
fn dump(p: &Path) -> String {
    let bits = |v: f32| format!("{:08x}", v.to_bits());
    let pt = |q: Point| format!(" {} {}", bits(q.x), bits(q.y));
    let mut s = String::from(if p.fill_type == FillType::EvenOdd { "evenodd" } else { "winding" });
    let (mut pi, mut wi) = (0usize, 0usize);
    for &v in &p.verbs {
        match v {
            Verb::Move => {
                s += " M";
                s += &pt(p.points[pi]);
                pi += 1;
            }
            Verb::Line => {
                s += " L";
                s += &pt(p.points[pi]);
                pi += 1;
            }
            Verb::Quad => {
                s += " Q";
                s += &pt(p.points[pi]);
                s += &pt(p.points[pi + 1]);
                pi += 2;
            }
            Verb::Conic => {
                s += " K";
                s += &pt(p.points[pi]);
                s += &pt(p.points[pi + 1]);
                s += &format!(" {}", bits(p.weights[wi]));
                pi += 2;
                wi += 1;
            }
            Verb::Cubic => {
                s += " C";
                for k in 0..3 {
                    s += &pt(p.points[pi + k]);
                }
                pi += 3;
            }
            Verb::Close => s += " Z",
        }
    }
    s
}

fn op_of(name: &str) -> PathOp {
    match name {
        "union" => PathOp::Union,
        "intersect" => PathOp::Intersect,
        "difference" => PathOp::Difference,
        "xor" => PathOp::Xor,
        _ => PathOp::ReverseDifference,
    }
}

#[test]
fn op_matches_skiasharp() {
    let data = include_str!("data/pathops/skiasharp.txt");
    let lines: Vec<&str> = data.lines().collect();
    let mut failures = Vec::new();
    let mut count = 0;
    for chunk in lines.chunks(4) {
        let head: Vec<&str> = chunk[0].split_whitespace().collect();
        let (name, opname) = (head[1], head[2]);
        // Cubic roots go through acos/cos/cbrt: exact only with the C runtime
        // libSkiaSharp uses on Windows.
        if !cfg!(windows) && name.contains("cubic") {
            continue;
        }
        let a = parse(chunk[1].strip_prefix("A ").unwrap());
        let b = parse(chunk[2].strip_prefix("B ").unwrap());
        assert_eq!(dump(&a), chunk[1][2..], "{name}: input A round trip");
        assert_eq!(dump(&b), chunk[2][2..], "{name}: input B round trip");
        let expected = chunk[3].strip_prefix("R ").unwrap();
        let got = match op(&a, &b, op_of(opname)) {
            Some(r) => dump(&r),
            None => "FAIL".to_string(),
        };
        count += 1;
        if got != expected {
            failures.push(format!("{name} {opname}\n  skia: {expected}\n  ours: {got}"));
        }
    }
    assert!(failures.is_empty(), "{} of {count} differ:\n{}", failures.len(), failures.join("\n"));
}
