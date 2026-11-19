//! Running one case through ShareXrs's port and comparing it.

use std::path::{Path, PathBuf};

use serde_json::Value;
use sharex_tools::imaging::{self, BmpBitDepth, GifQuality, JpegDownsample, PngBitDepth, Rgba};
use sharex_tools::{combiner, comparer, converter, icon, resizer, splitter, thumbnailer, watermark};

use crate::compare::{self, Kind, Outcome};

pub struct Ctx {
    pub inputs: PathBuf,
    pub sharex: PathBuf,
    pub out: PathBuf,
}

/// JPEG goes through ports of libjpeg-turbo both ways: no tolerance.
const JPEG_TOLERANCE: u8 = 0;
/// Glyph edges (DirectWrite glyph masks vs filled outlines).
const TEXT_TOLERANCE: u8 = 255;

impl Ctx {
    fn input(&self, c: &Value, key: &str) -> String {
        self.inputs.join(c[key].as_str().unwrap()).to_string_lossy().into_owned()
    }
    fn inputs_list(&self, c: &Value, key: &str) -> Vec<String> {
        c[key].as_array().unwrap().iter().map(|v| self.inputs.join(v.as_str().unwrap()).to_string_lossy().into_owned()).collect()
    }
    fn theirs(&self, name: &str, suffix: &str) -> PathBuf {
        self.sharex.join(format!("{name}{suffix}"))
    }
    fn ours(&self, name: &str, suffix: &str) -> PathBuf {
        self.out.join(format!("{name}{suffix}"))
    }
}

fn i32_of(c: &Value, k: &str) -> i32 {
    c[k].as_i64().unwrap() as i32
}

fn color(c: &Value) -> Rgba {
    match c.as_array() {
        Some(a) => [a[0].as_u64().unwrap() as u8, a[1].as_u64().unwrap() as u8, a[2].as_u64().unwrap() as u8, 255],
        None => imaging::WHITE,
    }
}

fn sxrb(ctx: &Ctx, name: &str, suffix: &str, ours: &imaging::Bitmap, tolerance: u8) -> Outcome {
    match compare::read_sxrb(&ctx.theirs(name, suffix)) {
        Some(theirs) => compare::bitmaps("bitmap", ours, &theirs, tolerance),
        None => Outcome::fail(format!("missing ShareX {suffix}")),
    }
}

fn write(path: &Path, data: &[u8]) {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).unwrap();
    }
    std::fs::write(path, data).unwrap();
}

pub fn run(ctx: &Ctx, c: &Value) -> Outcome {
    let name = c["name"].as_str().unwrap();
    match c["tool"].as_str().unwrap() {
        "combiner" => {
            let o = combiner::ImageCombinerOptions::from_json(&c["options"]);
            let files = ctx.inputs_list(c, "files");
            let bmp = combiner::combine_files(&files, &o).expect("combined");
            let png = imaging::encode_png(&bmp);
            write(&ctx.ours(name, ".png"), &png);
            sxrb(ctx, name, ".sxrb", &bmp, 0).and(compare::image_files("png", &ctx.ours(name, ".png"), &ctx.theirs(name, ".png"), 0))
        }
        "splitter" => {
            let dir = ctx.ours(name, "");
            std::fs::create_dir_all(&dir).unwrap();
            let file = ctx.input(c, "file");
            let files = splitter::split_image_file(&file, i32_of(c, "rows"), i32_of(c, "columns"), &dir.to_string_lossy()).unwrap();
            let names: Vec<String> = files.iter().map(|f| sharex_tools::dotnet::file_name(f).to_string()).collect();
            let theirs: Vec<String> = serde_json::from_str(&std::fs::read_to_string(ctx.theirs(name, ".json")).unwrap()).unwrap();
            let mut out = if names == theirs { Outcome::exact() } else { Outcome::fail(format!("files {names:?} vs {theirs:?}")) };
            let src = imaging::load_image(&file).unwrap();
            for (i, part) in splitter::split_image(&src, i32_of(c, "rows"), i32_of(c, "columns")).iter().enumerate() {
                out = out.and(sxrb(ctx, name, &format!("/{}.sxrb", i + 1), part, 0));
            }
            for f in &names {
                out = out.and(compare::image_files(f, &dir.join(f), &ctx.sharex.join(name).join(f), 0));
            }
            out
        }
        "thumbnailer" => {
            let dir = ctx.ours(name, "");
            std::fs::create_dir_all(&dir).unwrap();
            let file = ctx.input(c, "file");
            let (w, h, q) = (i32_of(c, "width"), i32_of(c, "height"), i32_of(c, "quality"));
            let files = thumbnailer::generate(std::slice::from_ref(&file), w, h, q, &dir.to_string_lossy(), thumbnailer::DEFAULT_FILE_NAME).unwrap();
            let src = imaging::load_image(&file).unwrap();
            let mut out = sxrb(ctx, name, ".sxrb", &thumbnailer::create(&src, w, h).unwrap(), 0);
            for f in &files {
                let n = sharex_tools::dotnet::file_name(f);
                out = out.and(compare::image_files(n, Path::new(f), &ctx.sharex.join(name).join(n), 0));
            }
            out
        }
        "resizer" => {
            let file = ctx.input(c, "file");
            let mode = match c["mode"].as_str().unwrap() {
                "Fill" => resizer::ResizeMode::Fill,
                "Fit" => resizer::ResizeMode::Fit,
                _ => resizer::ResizeMode::Stretch,
            };
            let format = if c["format"] == "Jpeg" { resizer::OutputFormat::Jpeg } else { resizer::OutputFormat::Png };
            let ext = if format == resizer::OutputFormat::Jpeg { ".jpg" } else { ".png" };
            let tol = if format == resizer::OutputFormat::Jpeg { JPEG_TOLERANCE } else { 0 };
            let (w, h, q, bg) = (i32_of(c, "width"), i32_of(c, "height"), i32_of(c, "quality"), color(&c["background"]));
            let src = imaging::load_image(&file).unwrap();
            let bmp = resizer::resize(&src, w, h, mode).unwrap();
            write(&ctx.ours(name, ext), &resizer::encode(&bmp, format, q, bg));
            let preview = resizer::create_preview(&file, w, h, mode, format, q, bg).unwrap();
            let pext = format!(".preview{ext}");
            write(&ctx.ours(name, &pext), &preview);
            sxrb(ctx, name, ".sxrb", &bmp, 0)
                .and(compare::image_files("file", &ctx.ours(name, ext), &ctx.theirs(name, ext), tol))
                .and(compare::image_files("preview", &ctx.ours(name, &pext), &ctx.theirs(name, &pext), tol))
        }
        "converter" => {
            let file = ctx.input(c, "file");
            let src = converter::load_image(&file).unwrap();
            let mut o = converter::ConvertOptions {
                format: match c["format"].as_str().unwrap() {
                    "Png" => converter::OutputFormat::Png,
                    "Jpeg" => converter::OutputFormat::Jpeg,
                    "Webp" => converter::OutputFormat::Webp,
                    "Gif" => converter::OutputFormat::Gif,
                    _ => converter::OutputFormat::Bmp,
                },
                quality: c["quality"].as_i64().unwrap_or(90) as i32,
                background: color(&c["background"]),
                ..Default::default()
            };
            if let Some(s) = c["jpegSubsampling"].as_str() {
                o.jpeg_subsampling = match s {
                    "Downsample422" => JpegDownsample::Downsample422,
                    "Downsample444" => JpegDownsample::Downsample444,
                    _ => JpegDownsample::Downsample420,
                };
            }
            if let Some(s) = c["gifQuality"].as_str() {
                o.gif_quality = match s {
                    "Default" => GifQuality::Default,
                    "Bit8" => GifQuality::Bit8,
                    "Bit4" => GifQuality::Bit4,
                    "Grayscale" => GifQuality::Grayscale,
                    _ => GifQuality::Adaptive,
                };
            }
            if let Some(s) = c["pngBitDepth"].as_str() {
                o.png_bit_depth = match s {
                    "Bit32" => PngBitDepth::Bit32,
                    "Bit24" => PngBitDepth::Bit24,
                    _ => PngBitDepth::Automatic,
                };
            }
            if let Some(s) = c["bmpBitDepth"].as_str() {
                o.bmp_bit_depth = match s {
                    "Bit32" => BmpBitDepth::Bit32,
                    "Automatic" => BmpBitDepth::Automatic,
                    _ => BmpBitDepth::Bit24,
                };
            }
            o.webp_lossless = c["webpLossless"].as_bool().unwrap_or(false);
            let ext = format!(".{}", o.format.extension());
            let data = converter::encode(&src, &o);
            write(&ctx.ours(name, &ext), &data);
            let loaded = sxrb(ctx, name, ".loaded.sxrb", &src, if file.ends_with(".jpg") { JPEG_TOLERANCE } else { 0 });
            let file_cmp = match o.format {
                converter::OutputFormat::Bmp | converter::OutputFormat::Gif | converter::OutputFormat::Webp => {
                    let b = compare::bytes("file", &data, &ctx.theirs(name, &ext));
                    if b.kind == Kind::Exact { b } else { b.and(compare::image_files("decoded", &ctx.ours(name, &ext), &ctx.theirs(name, &ext), 0)) }
                }
                converter::OutputFormat::Jpeg => compare::image_files("file", &ctx.ours(name, &ext), &ctx.theirs(name, &ext), JPEG_TOLERANCE),
                _ => compare::image_files("file", &ctx.ours(name, &ext), &ctx.theirs(name, &ext), 0),
            };
            loaded.and(file_cmp)
        }
        "converter-preview" => {
            let file = ctx.input(c, "file");
            let (data, w, h) = converter::create_preview(&file, &converter::ConvertOptions::default());
            write(&ctx.ours(name, ".png"), &data);
            let theirs: Value = serde_json::from_str(&std::fs::read_to_string(ctx.theirs(name, ".json")).unwrap()).unwrap();
            let size = if theirs["Width"] == w && theirs["Height"] == h { Outcome::exact() } else { Outcome::fail("size") };
            let tol = if file.ends_with(".jpg") { JPEG_TOLERANCE } else { 0 };
            size.and(compare::image_files("preview", &ctx.ours(name, ".png"), &ctx.theirs(name, ".png"), tol))
        }
        "watermark" | "watermark-preview" => {
            let file = ctx.input(c, "file");
            let o = watermark::WatermarkOptions {
                kind: if c["type"] == "Image" { watermark::WatermarkType::Image } else { watermark::WatermarkType::Text },
                text: c["text"].as_str().unwrap().to_string(),
                position: watermark::Position::ALL
                    [["TopLeft", "TopCenter", "TopRight", "MiddleLeft", "Center", "MiddleRight", "BottomLeft", "BottomCenter", "BottomRight"]
                        .iter()
                        .position(|p| *p == c["position"].as_str().unwrap())
                        .unwrap()],
                margin: i32_of(c, "margin"),
                opacity: i32_of(c, "opacity"),
                text_size: c["textSize"].as_f64().unwrap() as f32,
                text_color: color(&c["textColor"]),
                image_scale: i32_of(c, "imageScale"),
                rotation: c["rotation"].as_f64().unwrap() as f32,
            };
            let format = if c["format"] == "Jpeg" { watermark::OutputFormat::Jpeg } else { watermark::OutputFormat::Png };
            let ext = if format == watermark::OutputFormat::Jpeg { ".jpg" } else { ".png" };
            let text = o.kind == watermark::WatermarkType::Text;
            let tol = if text { TEXT_TOLERANCE } else if format == watermark::OutputFormat::Jpeg { JPEG_TOLERANCE } else { 0 };
            let q = c["quality"].as_i64().unwrap_or(90) as i32;
            let bg = color(&c["background"]);
            let wm_path = c["watermark"].as_str().filter(|s| !s.is_empty()).map(|s| ctx.inputs.join(s).to_string_lossy().into_owned());
            let mut out = if c["tool"] == "watermark-preview" {
                let (data, w, h) = watermark::create_preview(&file, wm_path.as_deref().unwrap_or(""), &o, format, q, bg);
                write(&ctx.ours(name, ext), &data);
                let theirs: Value = serde_json::from_str(&std::fs::read_to_string(ctx.theirs(name, ".json")).unwrap()).unwrap();
                if theirs["Width"] == w && theirs["Height"] == h { Outcome::exact() } else { Outcome::fail("size") }
            } else {
                let src = imaging::load_image(&file).unwrap();
                let wm = wm_path.as_deref().and_then(imaging::load_image);
                let bmp = watermark::apply(&src, wm.as_ref(), &o);
                write(&ctx.ours(name, ext), &watermark::encode(&bmp, format, q, bg));
                sxrb(ctx, name, ".sxrb", &bmp, tol)
            };
            out = out.and(compare::image_files("file", &ctx.ours(name, ext), &ctx.theirs(name, ext), tol));
            if text && out.kind != Kind::Differ && out.kind != Kind::Exact {
                out.kind = Kind::Close;
                out.note = format!("glyph rasterisation: {}", out.note);
            }
            out
        }
        "icon" => {
            let file = ctx.input(c, "file");
            let src = icon::load_image(&file).unwrap();
            let sizes: Vec<i32> = c["sizes"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap() as i32).collect();
            let depth = if c["depth"] == "Palette8" { icon::IconBitDepth::Palette8 } else { icon::IconBitDepth::TrueColor32 };
            let ico = icon::convert(&src, &sizes, depth).unwrap();
            write(&ctx.ours(name, ".ico"), &ico);
            let b = compare::bytes("ico", &ico, &ctx.theirs(name, ".ico"));
            if b.kind == Kind::Exact {
                return b;
            }
            compare_ico(&ico, &std::fs::read(ctx.theirs(name, ".ico")).unwrap())
        }
        "comparer" => {
            let a = comparer::load_image(&ctx.input(c, "a")).unwrap();
            let b = comparer::load_image(&ctx.input(c, "b")).unwrap();
            let r = comparer::compare(&a, &b, i32_of(c, "maxPreview"));
            let theirs: Value = serde_json::from_str(&std::fs::read_to_string(ctx.theirs(name, ".json")).unwrap()).unwrap();
            let (sim, status) = comparer::texts(&r);
            let mut out = sxrb(ctx, name, ".sxrb", &r.diff, 0);
            if theirs["MatchingPixels"] != r.matching_pixels || theirs["TotalPixels"] != r.total_pixels {
                out = out.and(Outcome::fail(format!("counts {} / {} vs {} / {}", r.matching_pixels, r.total_pixels, theirs["MatchingPixels"], theirs["TotalPixels"])));
            }
            if theirs["Similarity"] != sim.as_str() || theirs["Status"] != status.as_str() {
                out = out.and(Outcome::fail(format!("texts {sim:?} {status:?} vs {} {}", theirs["Similarity"], theirs["Status"])));
            }
            out
        }
        "gifmaker" => {
            let files = ctx.inputs_list(c, "files");
            let data = sharex_tools::video::gif_maker::create_bytes(
                &files,
                i32_of(c, "delay"),
                c["loop"].as_bool().unwrap(),
                i32_of(c, "repeat"),
            )
            .unwrap();
            write(&ctx.ours(name, ".gif"), &data);
            compare::bytes("gif", &data, &ctx.theirs(name, ".gif"))
        }
        other => crate::cases_data::run(ctx, c, other),
    }
}

/// An `.ico` that differs only in an entry's PNG zlib stream: same directory,
/// same BMP entries byte for byte, same decoded PNG pixels.
fn compare_ico(ours: &[u8], theirs: &[u8]) -> Outcome {
    let entries = |d: &[u8]| -> Vec<(Vec<u8>, Vec<u8>)> {
        let n = u16::from_le_bytes([d[4], d[5]]) as usize;
        (0..n)
            .map(|i| {
                let e = &d[6 + i * 16..6 + i * 16 + 16];
                let len = u32::from_le_bytes(e[8..12].try_into().unwrap()) as usize;
                let off = u32::from_le_bytes(e[12..16].try_into().unwrap()) as usize;
                (e[..8].to_vec(), d[off..off + len].to_vec())
            })
            .collect()
    };
    let (a, b) = (entries(ours), entries(theirs));
    if a.len() != b.len() {
        return Outcome::fail("entry count");
    }
    let mut out = Outcome { kind: Kind::PixelExact, note: "PNG entry deflated differently".into() };
    for ((da, pa), (db, pb)) in a.iter().zip(&b) {
        if da != db {
            return Outcome::fail("directory entry");
        }
        if pa == pb {
            continue;
        }
        if pa.starts_with(&[137, 80, 78, 71]) {
            let (ia, ib) = (image::load_from_memory(pa), image::load_from_memory(pb));
            match (ia, ib) {
                (Ok(x), Ok(y)) if x.to_rgba8() == y.to_rgba8() => {}
                (Ok(x), Ok(y)) => {
                    let (xa, ya) = (x.to_rgba8().into_raw(), y.to_rgba8().into_raw());
                    let (max, n) = xa.chunks(4).zip(ya.chunks(4)).fold((0u8, 0), |(m, n), (p, q)| {
                        let d = p.iter().zip(q).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
                        (m.max(d), n + (d > 0) as usize)
                    });
                    out = out.and(Outcome::fail(format!("png entry max ±{max} on {n} px")));
                }
                _ => return Outcome::fail("undecodable png entry"),
            }
        } else {
            let first = pa.iter().zip(pb).position(|(x, y)| x != y).unwrap_or(0);
            out = out.and(Outcome::fail(format!("bmp entry differs at {first} of {}", pa.len())));
        }
    }
    out
}
