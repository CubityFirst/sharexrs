//! Scrolling capture's engine against ShareX's own `ScrollingCaptureManager`
//! (`tools/scrolling-harness`): the same synthetic frames give the same
//! status, the same number of frames and scroll steps, the same result at
//! every step and the same final image, pixel for pixel.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use image::RgbaImage;
use serde_json::Value;
use sharex_capture::scrolling::{ScrollMethod, ScrollTarget, ScrollingCaptureOptions, Stitcher, compare_images, run_capture};

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scrolling")
}

fn load(p: &Path) -> RgbaImage {
    image::open(p).unwrap_or_else(|e| panic!("{}: {e}", p.display())).to_rgba8()
}

struct Frames<'a> {
    frames: Vec<RgbaImage>,
    next: usize,
    stop: &'a AtomicBool,
}

impl ScrollTarget for Frames<'_> {
    fn capture(&mut self) -> Option<RgbaImage> {
        let f = self.frames.get(self.next).cloned();
        self.next += 1;
        if self.next >= self.frames.len() {
            self.stop.store(true, Ordering::SeqCst);
        }
        f
    }

    fn scroll(&mut self, _method: ScrollMethod, _amount: i32) {}

    fn sleep(&mut self, _ms: i32) {}
}

#[test]
fn stitching_matches_sharex() {
    let cases: Value = serde_json::from_str(&std::fs::read_to_string(dir().join("cases.json")).unwrap()).unwrap();
    let results: Value = serde_json::from_str(&std::fs::read_to_string(dir().join("results-sharex.json")).unwrap()).unwrap();
    let mut failures = Vec::new();
    let mut n = 0;
    for c in cases["cases"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let folder = dir().join(name);
        let frames: Vec<RgbaImage> = c["frames"].as_array().unwrap().iter().map(|f| load(&folder.join(f.as_str().unwrap()))).collect();
        let auto = c["auto_ignore_bottom_edge"].as_bool().unwrap();
        let expected = &results[name];

        // The loop.
        let stop = AtomicBool::new(false);
        let mut target = Frames { frames: frames.clone(), next: 0, stop: &stop };
        let options = ScrollingCaptureOptions { auto_ignore_bottom_edge: auto, ..Default::default() };
        let out = run_capture(&options, &mut target, &stop);
        let got = (out.status.name(), out.frames as i64, out.scrolls as i64);
        let want = (expected["status"].as_str().unwrap(), expected["frames"].as_i64().unwrap(), expected["scrolls"].as_i64().unwrap());
        if got != want {
            failures.push(format!("{name}: loop {got:?}, ShareX {want:?}"));
        }
        let sharex_png = folder.join("result-sharex.png");
        match (&out.result, sharex_png.exists()) {
            (Some(r), true) => {
                if !compare_images(r, &load(&sharex_png)) {
                    failures.push(format!("{name}: result {:?} differs from ShareX's", r.dimensions()));
                }
            }
            (None, false) => {}
            (r, _) => failures.push(format!("{name}: result {:?}, ShareX has one: {}", r.as_ref().map(|r| r.dimensions()), sharex_png.exists())),
        }

        // Step by step.
        let mut s = Stitcher::new(auto);
        s.status = sharex_capture::scrolling::ScrollingCaptureStatus::Failed;
        for step in expected["steps"].as_array().unwrap() {
            let k = step["frame"].as_u64().unwrap() as usize;
            match step["outcome"].as_str().unwrap() {
                "identical" => {
                    if !compare_images(&frames[k], &frames[k - 1]) {
                        failures.push(format!("{name}: frame {k} should equal the one before"));
                    }
                }
                outcome => {
                    let ok = s.add(&frames[k]);
                    let status = step["status"].as_str().unwrap();
                    let size = s.result.as_ref().map(|r| (r.width() as i64, r.height() as i64));
                    let want_size = step.get("width").map(|w| (w.as_i64().unwrap(), step["height"].as_i64().unwrap()));
                    if ok != (outcome == "combined") || s.status.name() != status || (ok && size != want_size) {
                        failures.push(format!("{name}: frame {k}: {ok} {} {size:?}, ShareX {outcome} {status} {want_size:?}", s.status.name()));
                    }
                }
            }
        }
        n += 1;
    }
    assert!(failures.is_empty(), "{} of {n} cases differ:\n{}", failures.len(), failures.join("\n"));
    assert_eq!(n, 30);
}
