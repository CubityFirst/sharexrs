//! Prints the layout and outline ShareXrs computes for a string, in the
//! editor harness's `--text` format, to compare with Avalonia's.
use sharex_image_editor::render::text;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let size: f64 = a[2].parse().unwrap();
    let font = text::load_face(&a[1], a[3] == "1", false).expect("font");
    let l = text::layout(&font, &a[4], size, text::Align::Left, f64::INFINITY).unwrap();
    println!(
        "width {} height {} baseline {}",
        l.width(),
        l.height(),
        l.lines[0].baseline
    );
    let p = text::build_geometry(&font, &l, (0.0, 0.0)).unwrap();
    for (v, pts, _) in p.iter_verbs() {
        let s: Vec<String> = pts.iter().map(|p| format!("{},{}", p.x, p.y)).collect();
        println!("{v:?} {}", s.join(" "));
    }
}
