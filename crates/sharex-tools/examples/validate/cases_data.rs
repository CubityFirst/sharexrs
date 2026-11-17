//! The Data, Video and System tools' cases.

use serde_json::Value;
use sharex_tools::video::{converter, thumbnailer, trimmer};
use sharex_tools::{hash, imaging, indexer, metadata, monitor_test, qr};

use crate::cases::Ctx;
use crate::compare::{self, Outcome};

/// The indexers' footers carry the time they ran: mask it.
fn mask_time(s: &str) -> String {
    let re = regex::Regex::new(r"\d{4}-\d{2}-\d{2} at \d{2}:\d{2}:\d{2} UTC").unwrap();
    re.replace_all(s, "<time>").into_owned()
}

fn read_json(ctx: &Ctx, name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(ctx.sharex.join(format!("{name}.json"))).unwrap()).unwrap()
}

/// JSON equality, with numbers compared as numbers.
fn json_eq(ours: &Value, theirs: &Value) -> Outcome {
    fn norm(v: &Value) -> Value {
        match v {
            Value::Number(n) => Value::from(n.as_f64().unwrap()),
            Value::Array(a) => Value::Array(a.iter().map(norm).collect()),
            Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), norm(v))).collect()),
            other => other.clone(),
        }
    }
    if norm(ours) == norm(theirs) { Outcome::exact() } else { Outcome::fail(format!("{ours} vs ShareX {theirs}")) }
}

pub fn run(ctx: &Ctx, c: &Value, tool: &str) -> Outcome {
    let name = c["name"].as_str().unwrap();
    match tool {
        "hash" => {
            let algo = match c["algorithm"].as_str().unwrap() {
                "CRC32" => hash::HashAlgorithm::Crc32,
                "MD5" => hash::HashAlgorithm::Md5,
                "SHA1" => hash::HashAlgorithm::Sha1,
                "SHA256" => hash::HashAlgorithm::Sha256,
                "SHA384" => hash::HashAlgorithm::Sha384,
                _ => hash::HashAlgorithm::Sha512,
            };
            let theirs: Value = serde_json::from_str(&std::fs::read_to_string(ctx.sharex.join(format!("{name}.json"))).unwrap()).unwrap();
            let cancel = std::sync::atomic::AtomicBool::new(false);
            let mut out = Outcome::exact();
            for (file, expected) in theirs.as_object().unwrap() {
                let path = ctx.inputs.join("hash").join(file);
                let ours = hash::hash_file(&path.to_string_lossy(), algo, |_| {}, &cancel);
                let ours = ours.map(Value::String).unwrap_or(Value::Null);
                if &ours != expected {
                    out = out.and(Outcome::fail(format!("{file}: {ours} vs {expected}")));
                }
            }
            out
        }
        "indexer" => {
            let settings = {
                let mut v = c["settings"].clone();
                if let Some(css) = v["CustomCSSFilePath"].as_str() {
                    v["CustomCSSFilePath"] = Value::String(ctx.inputs.join(css).to_string_lossy().into_owned());
                }
                let binary = v["BinaryUnits"].as_bool().unwrap_or(false);
                indexer::IndexerSettings::from_json(&v, binary)
            };
            let folder = ctx.inputs.join("folders").join(c["folder"].as_str().unwrap());
            let ours = indexer::index(&folder.to_string_lossy(), &settings);
            std::fs::write(ctx.out.join(format!("{name}.txt")), &ours).unwrap();
            let theirs = std::fs::read_to_string(ctx.sharex.join(format!("{name}.txt"))).unwrap();
            let (a, b) = (mask_time(&ours), mask_time(&theirs));
            let tmp = ctx.out.join(format!("{name}.theirs-masked.txt"));
            std::fs::write(&tmp, &b).unwrap();
            let r = compare::text("index", &a, &tmp);
            let _ = std::fs::remove_file(&tmp);
            r
        }
        "qr" => {
            let text = c["text"].as_str().unwrap();
            let size = c["size"].as_i64().unwrap() as i32;
            let theirs: Value = serde_json::from_str(&std::fs::read_to_string(ctx.sharex.join(format!("{name}.json"))).unwrap()).unwrap();
            let ours = qr::generate(text, size);
            if theirs["null"].as_bool().unwrap() {
                return if ours.is_none() { Outcome::exact() } else { Outcome::fail("ShareX made no QR code") };
            }
            let Some(bmp) = ours else { return Outcome::fail("no QR code") };
            let mut out = match compare::read_sxrb(&ctx.sharex.join(format!("{name}.sxrb"))) {
                Some(t) => compare::bitmaps("bitmap", &bmp, &t, 0),
                None => Outcome::fail("missing sxrb"),
            };
            let svg = qr::generate_svg_file(text, size).unwrap_or_default();
            std::fs::write(ctx.out.join(format!("{name}.svg")), &svg).unwrap();
            out = out.and(compare::bytes("svg", &svg, &ctx.sharex.join(format!("{name}.svg"))));
            out
        }
        "qr-scan" => {
            let bmp = imaging::load_image(ctx.inputs.join("qr").join(c["file"].as_str().unwrap())).unwrap();
            let ours = qr::scan(&bmp, c["qrOnly"].as_bool().unwrap());
            let ours = match ours {
                Some(v) => Value::Array(v.into_iter().map(Value::String).collect()),
                None => Value::Null,
            };
            let theirs: Value = serde_json::from_str(&std::fs::read_to_string(ctx.sharex.join(format!("{name}.json"))).unwrap()).unwrap();
            if ours == theirs { Outcome::exact() } else { Outcome::fail(format!("{ours} vs ShareX {theirs}")) }
        }
        "metadata" => {
            let src = ctx.inputs.join("meta").join(c["file"].as_str().unwrap());
            let src_s = src.to_string_lossy().into_owned();
            let theirs: Value = serde_json::from_str(&std::fs::read_to_string(ctx.sharex.join(format!("{name}.json"))).unwrap()).unwrap();
            let as_json = |r: Result<Vec<metadata::MetadataValue>, String>, mask_fs: bool| -> Value {
                match r {
                    Ok(items) => Value::Array(
                        items
                            .iter()
                            .filter(|m| !(mask_fs && m.group == "File System"))
                            .map(|m| Value::Array(vec![m.group.clone().into(), m.tag.clone().into(), m.value.clone().into()]))
                            .collect(),
                    ),
                    Err(_) => Value::String("error".into()),
                }
            };
            let strip_fs = |v: &Value| -> Value {
                match v.as_array() {
                    Some(a) => Value::Array(a.iter().filter(|x| x[0] != "File System").cloned().collect()),
                    None => Value::String(if v.as_str().is_some_and(|s| s.starts_with("error")) { "error".into() } else { v.to_string() }),
                }
            };
            let mut out = Outcome::exact();
            let ours = as_json(metadata::read(&src_s), false);
            let expected = if theirs["read"].is_array() { theirs["read"].clone() } else { Value::String("error".into()) };
            if ours != expected {
                let (a, b) = (ours.as_array().cloned().unwrap_or_default(), expected.as_array().cloned().unwrap_or_default());
                let first = a.iter().zip(&b).position(|(x, y)| x != y);
                out = out.and(Outcome::fail(match first {
                    Some(i) => format!("read #{i}: {} vs ShareX {}", a[i], b[i]),
                    None => format!("read: {} entries vs ShareX {} (next: {:?} / {:?})", a.len(), b.len(), a.get(b.len()), b.get(a.len())),
                }));
            }
            if let Ok(items) = metadata::read(&src_s) {
                let text = metadata::build_text(&items);
                if theirs["text"] != text.as_str() {
                    out = out.and(Outcome::fail("copy-all text differs"));
                }
            }
            if theirs["canStrip"] != metadata::can_strip(&src_s) {
                out = out.and(Outcome::fail("CanStripMetadata differs"));
            }
            let ext = sharex_tools::dotnet::extension(&src_s).to_string();
            let copy = ctx.out.join(format!("{name}{ext}"));
            std::fs::copy(&src, &copy).unwrap();
            let r = metadata::strip(&copy.to_string_lossy());
            match (theirs["strip"].as_str(), r) {
                (Some("ok"), Ok(())) => {
                    out = out.and(compare::bytes("stripped", &std::fs::read(&copy).unwrap(), &ctx.sharex.join(format!("{name}{ext}"))));
                    let after = as_json(metadata::read(&copy.to_string_lossy()), true);
                    if after != strip_fs(&theirs["stripped"]) {
                        out = out.and(Outcome::fail("metadata after stripping differs"));
                    }
                }
                (Some("ok"), Err(e)) => out = out.and(Outcome::fail(format!("strip failed: {e}"))),
                (Some(_), Ok(())) => out = out.and(Outcome::fail(format!("strip succeeded where ShareX threw {}", theirs["strip"]))),
                _ => {}
            }
            out
        }
        "video-args" => {
            let theirs = read_json(ctx, name);
            let codec_of = |names: &[&str], key: &str| converter::Codec::ALL[names.iter().position(|n| *n == c[key].as_str().unwrap()).unwrap()];
            let o = converter::VideoConverterOptions {
                input_file_path: c["input"].as_str().map(str::to_string),
                output_folder_path: c["folder"].as_str().map(str::to_string),
                output_file_name: c["outName"].as_str().map(str::to_string),
                video_codec: codec_of(&converter::Codec::NAMES, "codec"),
                video_quality: c["quality"].as_i64().unwrap() as i32,
                video_quality_use_bitrate: c["useBitrate"].as_bool().unwrap(),
                video_quality_bitrate: c["bitrate"].as_i64().unwrap() as i32,
                use_custom_arguments: c["custom"].as_bool().unwrap_or(false),
                custom_arguments: "-i custom".into(),
                auto_open_folder: true,
            };
            let vm_names = ["X264", "X265", "H264Nvenc", "HevcNvenc", "H264Amf", "HevcAmf", "H264Qsv", "HevcQsv", "Vp8", "Vp9", "Av1", "Xvid", "Gif", "Webp", "Apng"];
            let w = converter::ConverterWindow {
                input_file_path: c["input"].as_str().unwrap_or("").into(),
                output_folder_path: c["folder"].as_str().unwrap_or("").into(),
                output_file_name: c["outName"].as_str().unwrap_or("").into(),
                codec: codec_of(&vm_names, "vmCodec"),
                use_bitrate: c["useBitrate"].as_bool().unwrap(),
                video_quality: c["vmQuality"].as_f64().unwrap(),
                video_bitrate: c["vmBitrate"].as_f64().unwrap(),
            };
            let ours = serde_json::json!({
                "optionsArgs": o.arguments(),
                "optionsOutput": o.output_file_path(),
                "windowArgs": w.build_arguments(),
                "windowOutput": w.output_file_path(),
            });
            json_eq(&ours, &theirs)
        }
        "video-trim-args" => {
            let ours = trimmer::build_trim_arguments(
                c["input"].as_str().unwrap(),
                c["output"].as_str().unwrap(),
                c["start"].as_f64().unwrap(),
                c["end"].as_f64().unwrap(),
                c["precise"].as_bool().unwrap(),
            );
            json_eq(&serde_json::json!(ours), &read_json(ctx, name))
        }
        "video-outputinfo" => {
            let text = std::fs::read_to_string(ctx.sharex.join(format!("{name}.txt"))).unwrap();
            let theirs = read_json(ctx, name);
            let size = theirs["FileSize"].as_u64().unwrap();
            let i = converter::output_info(&text, size);
            let ours = serde_json::json!({"FileSize": i.file_size, "Bitrate": i.average_bitrate_kbps, "Width": i.width, "Height": i.height});
            json_eq(&ours, &theirs)
        }
        "video-thumbs" => {
            let theirs = read_json(ctx, name);
            let dir = ctx.out.join(name);
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let mut o = thumbnailer::VideoThumbnailOptions::from_json(&c["options"]);
            o.default_output_directory = Some(dir.to_string_lossy().into_owned());
            o.output_location = thumbnailer::OutputLocation::DefaultFolder;
            let ffmpeg = std::path::Path::new(r"G:\scripts\sharexrs-ref\ShareX-22.4.0-portable\ffmpeg.exe");
            let media = ctx.inputs.join("meta").join(c["file"].as_str().unwrap()).to_string_lossy().into_owned();
            let result = thumbnailer::take_thumbnails(ffmpeg, &media, &o, || 0.5, |_, _| {}).unwrap().unwrap_or_default();
            let mut files: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
            files.sort();
            let info = sharex_screen_recording::cli::FFmpegCliManager::new(ffmpeg).get_video_info(&media).map(|v| thumbnailer::video_info_text(&v));
            let ours = serde_json::json!({
                "files": files,
                "result": result.iter().map(|r| sharex_tools::dotnet::file_name(&r.file_path).to_string()).collect::<Vec<_>>(),
                "info": info,
            });
            let mut out = json_eq(&ours, &theirs);
            for f in &files {
                let tol = if f.contains("_Thumbnail") && o.add_video_info | o.add_timestamp { 255 } else { 0 };
                let r = compare::image_files(f, &dir.join(f), &ctx.sharex.join(name).join(f), tol);
                out = out.and(if tol > 0 && r.kind != crate::compare::Kind::Differ && r.kind != crate::compare::Kind::Exact {
                    Outcome { kind: crate::compare::Kind::Close, note: format!("text glyphs: {}", r.note) }
                } else {
                    r
                });
            }
            out
        }
        "monitor-test" => {
            use monitor_test::*;
            let rgb = |k: &str, d: [u8; 4]| c[k].as_array().map(|a| [a[0].as_u64().unwrap() as u8, a[1].as_u64().unwrap() as u8, a[2].as_u64().unwrap() as u8, 255]).unwrap_or(d);
            let t = MonitorTest {
                mode: match c["mode"].as_str().unwrap() {
                    "Grayscale" => Mode::Grayscale,
                    "Rgb" => Mode::Rgb,
                    "Gradient" => Mode::Gradient,
                    "Pattern" => Mode::Pattern,
                    _ => Mode::Motion,
                },
                gray: c["gray"].as_f64().unwrap_or(255.0),
                red: c["red"].as_f64().unwrap_or(255.0),
                green: c["green"].as_f64().unwrap_or(0.0),
                blue: c["blue"].as_f64().unwrap_or(0.0),
                gradient_start: rgb("start", imaging::WHITE),
                gradient_end: rgb("end", imaging::BLACK),
                gradient_direction: match c["direction"].as_str().unwrap_or("Vertical") {
                    "Horizontal" => GradientDirection::Horizontal,
                    "ForwardDiagonal" => GradientDirection::ForwardDiagonal,
                    "BackwardDiagonal" => GradientDirection::BackwardDiagonal,
                    _ => GradientDirection::Vertical,
                },
                pattern: match c["pattern"].as_str().unwrap_or("HorizontalLines") {
                    "VerticalLines" => Pattern::VerticalLines,
                    "Checkerboard" => Pattern::Checkerboard,
                    _ => Pattern::HorizontalLines,
                },
                pattern_size: c["patternSize"].as_f64().unwrap_or(5.0),
                motion_direction: if c["motion"] == "HorizontalBars" { MotionDirection::HorizontalBars } else { MotionDirection::VerticalBars },
                motion_speed: 500.0,
            };
            let bmp = t.render(c["width"].as_u64().unwrap() as u32, c["height"].as_u64().unwrap() as u32, c["offset"].as_f64().unwrap_or(0.0));
            let path = ctx.out.join(format!("{name}.png"));
            std::fs::write(&path, imaging::encode_png(&bmp)).unwrap();
            compare::image_files("pattern", &path, &ctx.sharex.join(format!("{name}.png")), 0)
        }
        _ => Outcome::skip(format!("no validator for {tool}")),
    }
}
