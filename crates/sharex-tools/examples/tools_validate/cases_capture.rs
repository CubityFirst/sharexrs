//! The `capture` cases: `TaskHelpers.PrepareImage` (ShareX's, through the
//! harness) against `sharex_tools::capture_image::prepare_image` over the
//! same image and `TaskSettingsImage`, as the app calls it: the image the
//! task holds (straight RGBA) made a bitmap again.

use serde_json::Value;
use sharex_settings::TaskSettingsImage;
use sharex_tools::capture_image;
use sharex_tools::imaging::{self, Bitmap};

use crate::cases::Ctx;
use crate::compare::{self, Outcome};

/// The PNG's IHDR colour type (2 RGB, 6 RGBA).
fn png_color_type(png: &[u8]) -> Option<u8> {
    png.get(25).copied()
}

pub fn run(ctx: &Ctx, c: &Value) -> Outcome {
    let name = c["name"].as_str().unwrap();
    let theirs: Value = match std::fs::read_to_string(ctx.sharex.join(format!("{name}.json"))) {
        Ok(t) => serde_json::from_str(&t).unwrap(),
        Err(_) => return Outcome::fail("ShareX output missing"),
    };
    let their_format = theirs["Format"].as_str().unwrap_or_default().to_string();
    let their_len = theirs["Length"].as_u64().unwrap_or_default();

    // TaskSettingsImage's defaults with the case's fields over them.
    let mut settings = serde_json::to_value(TaskSettingsImage::default()).unwrap();
    for (k, v) in c["settings"].as_object().unwrap() {
        settings[k] = v.clone();
    }
    let settings: TaskSettingsImage = serde_json::from_value(settings).unwrap();
    let strip = c["strip"].as_bool().unwrap_or(false);

    let src = imaging::load_image(ctx.inputs.join(c["file"].as_str().unwrap())).expect("input");
    let mut rgba = src.to_straight();
    if c["kind"] == "capture" {
        // `SetOpaque`.
        for p in rgba.chunks_exact_mut(4) {
            p[3] = 255;
        }
    }
    let bmp = Bitmap::from_straight(src.width(), src.height(), &rgba);
    let d = capture_image::prepare_image(&bmp, &settings, strip);
    let ext = capture_image::extension(&d.format);
    let ours_path = ctx.out.join(format!("{name}.{ext}"));
    std::fs::write(&ours_path, &d.data).unwrap();
    let our_format = format!("{:?}", d.format);
    if our_format != their_format {
        return Outcome::fail(format!("format {our_format} ({} bytes) vs ShareX {their_format} ({their_len} bytes)", d.data.len()));
    }
    let theirs_path = ctx.sharex.join(format!("{name}.{ext}"));
    if ext != "png" {
        return compare::bytes(ext, &d.data, &theirs_path);
    }
    // PNG: the same pixels and colour type; the zlib stream (so the size)
    // is zlib-rs's, not Skia's.
    let mut out = compare::image_files("png", &ours_path, &theirs_path, 0);
    let their_png = std::fs::read(&theirs_path).unwrap_or_default();
    if png_color_type(&d.data) != png_color_type(&their_png) {
        out = out.and(Outcome::fail(format!("colour type {:?} vs ShareX {:?}", png_color_type(&d.data), png_color_type(&their_png))));
    }
    if out.kind == compare::Kind::PixelExact {
        out.note = format!("size {} vs ShareX {} ({:+.1}%)", d.data.len(), their_len, (d.data.len() as f64 / their_len as f64 - 1.0) * 100.0);
    }
    out
}
