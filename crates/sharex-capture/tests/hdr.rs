//! The HDR screenshot correction against ShareX's own `HDRScreenCapture`
//! (`tools/hdr-harness`): on the same synthetic FP16 and HDR10 frames, the
//! same tone curve (`ConvertToSrgb`), SDR reference, Windows tone mapper
//! result (WIC, or the same failure), both turned with the output
//! (`RotateOutput`), and the same corrected GDI capture
//! through Windows' tone mapper and through the fallback, byte for byte.
//! No screen is captured. `SHAREXRS_HDR_DIR=<harness out dir>` compares a
//! fresh harness run instead of the fixtures.
#![cfg(windows)]

use std::path::{Path, PathBuf};

use image::RgbaImage;
use serde_json::Value;
use sharex_capture::Rect;
use sharex_capture::hdr::{self, HdrFormat, HdrFrame, ModeRotation, OutputColor};
use sharex_capture::platform::windows_hdr::wic_tone_map;

fn dir() -> PathBuf {
    std::env::var_os("SHAREXRS_HDR_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hdr"))
}

fn bgra(image: &RgbaImage) -> Vec<u8> {
    image.as_raw().chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect()
}

fn rgba(bgra: &[u8], w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_raw(w, h, bgra.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect()).expect("size")
}

fn rect(v: &Value) -> Rect {
    let n = |i: usize| v[i].as_i64().unwrap() as i32;
    Rect::new(n(0), n(1), n(2), n(3))
}

#[derive(Default)]
struct Tally {
    same: usize,
    total: usize,
    failures: Vec<String>,
    /// Results of Windows' own tone mapper (WIC) that differ on a Windows
    /// build other than the fixtures' (reported, not failures).
    os_differences: Vec<String>,
    /// This Windows is the build the fixtures were recorded on.
    same_windows: bool,
}

/// The Windows build number (`ver`), e.g. 26300.
fn windows_build() -> Option<String> {
    let out = std::process::Command::new("cmd").args(["/c", "ver"]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let version = text.split("Version ").nth(1)?;
    version.trim().trim_end_matches(']').split('.').nth(2).map(str::to_owned)
}

impl Tally {
    fn check(&mut self, dir: &Path, name: &str, what: &str, ours: Vec<u8>) {
        let theirs = std::fs::read(dir.join(format!("{name}.{what}"))).unwrap();
        self.outcome(ours == theirs, || {
            let diff = ours.iter().zip(&theirs).filter(|(a, b)| a != b).count();
            if std::env::var_os("HDR_DEBUG").is_some() {
                for (i, (a, b)) in ours.chunks(4).zip(theirs.chunks(4)).enumerate() {
                    if a != b {
                        eprintln!("{name}.{what} px {i}: ours {a:?} sharex {b:?}");
                    }
                }
            }
            format!("{name}.{what}: {diff} bytes differ (lengths {} / {})", ours.len(), theirs.len())
        });
    }

    /// [`Self::check`] for an output of Windows' WIC tone mapper: exact on
    /// the fixtures' Windows build, reported elsewhere (CI's runners have
    /// another build, whose tone mapper rounds some pixels differently).
    fn check_os(&mut self, dir: &Path, name: &str, what: &str, ours: Vec<u8>) {
        if self.same_windows {
            return self.check(dir, name, what, ours);
        }
        let theirs = std::fs::read(dir.join(format!("{name}.{what}"))).unwrap();
        if ours != theirs {
            let diff = ours.iter().zip(&theirs).filter(|(a, b)| a != b).count();
            self.os_differences.push(format!("{name}.{what}: {diff} bytes differ"));
        }
    }

    fn outcome(&mut self, ok: bool, why: impl FnOnce() -> String) {
        self.total += 1;
        if ok {
            self.same += 1;
        } else {
            self.failures.push(why());
        }
    }
}

#[test]
fn hdr_correction_as_sharex() {
    let dir = dir();
    let cases: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("cases.json")).unwrap()).unwrap();
    let fixtures_build = std::fs::read_to_string(dir.join("wic-windows-build.txt")).unwrap();
    let mut t = Tally { same_windows: windows_build().as_deref() == Some(fixtures_build.trim()), ..Tally::default() };
    for c in cases.as_array().unwrap() {
        let name = c["Name"].as_str().unwrap();
        let (w, h) = (c["Width"].as_u64().unwrap() as u32, c["Height"].as_u64().unwrap() as u32);
        let data = std::fs::read(dir.join(format!("{name}.frame"))).unwrap();
        let frame = HdrFrame {
            format: if c["Format"] == "fp16" { HdrFormat::Rgba16Float } else { HdrFormat::R10G10B10A2 },
            width: w,
            height: h,
            row_pitch: c["RowPitch"].as_u64().unwrap() as usize,
            data: &data,
        };
        let white = c["White"].as_f64().unwrap() as f32;
        let peak = c["Peak"].as_f64().unwrap() as f32;
        let capture = rect(&c["Capture"]);
        let color = OutputColor {
            sdr_white_scale: white,
            peak_nits: peak,
            rotation: ModeRotation::from_dxgi(c["Rotation"].as_i64().unwrap() as i32),
            bounds: rect(&c["Bounds"]),
        };
        let gdi = std::fs::read(dir.join(format!("{name}.gdi"))).unwrap();

        t.check(&dir, name, "srgb", bgra(&hdr::convert_to_srgb(&frame, white, peak)));
        t.check(&dir, name, "ref", bgra(&hdr::convert_to_sdr_reference(&frame, white)));

        // Windows' tone mapper: the same pixels, or failing where ShareX's failed.
        let mut reference = hdr::convert_to_sdr_reference(&frame, white);
        hdr::rotate_output_premultiplied(&mut reference, color.rotation);
        t.check(&dir, name, "refrot", bgra(&reference));
        let sharex_wic_failed = dir.join(format!("{name}.wic.error.txt")).exists();
        match wic_tone_map(&frame) {
            Ok(mut img) => {
                t.check_os(&dir, name, "wic", bgra(&img));
                hdr::rotate_output_premultiplied(&mut img, color.rotation);
                t.check_os(&dir, name, "wicrot", bgra(&img));
            }
            Err(e) if t.same_windows => t.outcome(sharex_wic_failed, || format!("{name}.wic: ShareXrs's tone mapper failed ({e}), ShareX's didn't")),
            Err(e) => {
                if !sharex_wic_failed {
                    t.os_differences.push(format!("{name}.wic: this Windows's tone mapper failed ({e}), ShareX's didn't"));
                }
            }
        }

        for (what, tone_mapper) in [("final", true), ("fallback", false)] {
            let mut destination = rgba(&gdi, capture.w as u32, capture.h as u32);
            let output = if tone_mapper {
                hdr::captured_output(&frame, &color, wic_tone_map)
            } else {
                hdr::captured_output(&frame, &color, |_| Err("no tone mapper".into()))
            };
            let sharex_null = dir.join(format!("{name}.{what}.null")).exists();
            if let Some(o) = &output {
                hdr::copy_output_intersection(&mut destination, capture, o, color.bounds);
            }
            t.outcome(output.is_none() == sharex_null, || format!("{name}.{what}: CaptureOutput's result differs in being null"));
            if tone_mapper {
                t.check_os(&dir, name, what, bgra(&destination));
            } else {
                t.check(&dir, name, what, bgra(&destination));
            }
        }
    }
    eprintln!("HDR harness: {}/{} identical", t.same, t.total);
    if !t.os_differences.is_empty() {
        eprintln!(
            "Windows' tone mapper (WIC) on this build differs from the fixtures' build ({}):
{}",
            fixtures_build.trim(),
            t.os_differences.join("
")
        );
    }
    assert!(t.failures.is_empty(), "{}/{} identical:
{}", t.same, t.total, t.failures.join("
"));
}
