//! The Data, Video and System tools' cases.

use serde_json::Value;
use sharex_tools::video::{converter, gif_trimmer, thumbnailer, trimmer};
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
        "clipboard-preview" => {
            use sharex_tools::system::clipboard_viewer::{ClipboardSnapshot, ClipboardValue, Preview};
            let theirs = read_json(ctx, name);
            let dir = ctx.inputs.join("clipboard");
            let manifest: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
            let ours_dir = ctx.out.join(name);
            std::fs::create_dir_all(&ours_dir).unwrap();
            let mut out = Outcome::exact();
            let mut i = 0;
            for (entries, theirs) in manifest.as_array().unwrap().iter().zip(theirs["snapshots"].as_array().unwrap()) {
                let mut snap = ClipboardSnapshot::new();
                for e in entries.as_array().unwrap() {
                    let value = if let Some(t) = e["text"].as_str() {
                        ClipboardValue::Text(t.into())
                    } else if let Some(f) = e["files"].as_array() {
                        ClipboardValue::Files(f.iter().map(|x| x.as_str().unwrap().to_string()).collect())
                    } else {
                        ClipboardValue::Bytes(std::fs::read(dir.join(e["bytes"].as_str().unwrap())).unwrap())
                    };
                    snap.set(e["format"].as_str().unwrap(), value);
                }
                out = out.and(json_eq(&serde_json::json!(snap.formats()), &theirs["formats"]));
                for t in theirs["previews"].as_array().unwrap() {
                    let format = t["format"].as_str().unwrap();
                    let label = format!("{i} {format}: ");
                    out = out.and(match snap.preview(format) {
                        Ok(Preview::Image(b)) if t["image"].is_string() => {
                            let p = ours_dir.join(format!("{i}.png"));
                            std::fs::write(&p, imaging::encode_png(&b)).unwrap();
                            if serde_json::json!([b.width(), b.height()]) == serde_json::json!([t["width"], t["height"]]) {
                                compare::image_files(&label, &p, &ctx.sharex.join(name).join(t["image"].as_str().unwrap()), 0)
                            } else {
                                Outcome::fail(format!("{label}size"))
                            }
                        }
                        Ok(Preview::Text(s)) if t["text"].is_string() => {
                            if s == t["text"].as_str().unwrap() { Outcome::exact() } else { Outcome::fail(format!("{label}{s:?}")) }
                        }
                        Err(_) if t["error"].is_string() => Outcome::exact(),
                        r => Outcome::fail(format!("{label}ours {:?}, ShareX {t}", r.map(|p| matches!(p, Preview::Image(_))))),
                    });
                    i += 1;
                }
            }
            out
        }
        "bg-remove" => {
            use sharex_tools::background_remover::{self as br, Device, Model};
            if std::env::var_os("ORT_DYLIB_PATH").is_none() {
                return Outcome::skip("ORT_DYLIB_PATH isn't set (ShareX's onnxruntime.dll)");
            }
            let theirs = read_json(ctx, name);
            let path = ctx.inputs.join(c["model"].as_str().unwrap());
            let model = Model {
                file_name: path.file_name().unwrap().to_string_lossy().into_owned(),
                file_size: std::fs::metadata(&path).unwrap().len(),
                path,
            };
            let mut service = br::BackgroundRemovalService::new();
            let mut runs = Vec::new();
            let mut out = Outcome::exact();
            for (i, img) in c["images"].as_array().unwrap().iter().enumerate() {
                let source = br::load_image(&ctx.inputs.join(img.as_str().unwrap())).unwrap();
                match service.remove_background(&source, &model, Device::Cpu) {
                    Ok(r) => {
                        runs.push(serde_json::json!({"cached": r.session_cached, "device": r.execution_device}));
                        let sharex = compare::read_sxrb(&ctx.sharex.join(name).join(format!("{i}.sxrb"))).unwrap();
                        out = out.and(compare::bitmaps(&format!("image {i}"), &r.image, &sharex, 0));
                    }
                    Err(e) => return Outcome::fail(e),
                }
            }
            out.and(json_eq(&serde_json::json!({"display": model.display_name(), "runs": runs}), &theirs))
        }
        "inspect-styles" => {
            use sharex_tools::system::inspect_window::{format_rectangle, window_styles_to_string, Rect};
            let theirs = read_json(ctx, name);
            let strings: Vec<String> = c["values"].as_array().unwrap().iter().map(|v| window_styles_to_string(v.as_u64().unwrap() as u32)).collect();
            let rects: Vec<String> = c["rects"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| {
                    let v = |i: usize| a[i].as_i64().unwrap() as i32;
                    format_rectangle(Rect { x: v(0), y: v(1), width: v(2), height: v(3) })
                })
                .collect();
            json_eq(&serde_json::json!({"strings": strings, "rects": rects}), &serde_json::json!({"strings": theirs["strings"], "rects": theirs["rects"]}))
        }
        "video-trim-time" => {
            let ours = serde_json::json!({
                "format": c["seconds"].as_array().unwrap().iter().map(|v| trimmer::format_time(v.as_f64().unwrap())).collect::<Vec<_>>(),
                "parse": c["texts"].as_array().unwrap().iter().map(|v| trimmer::try_parse_time(v.as_str().unwrap())).collect::<Vec<_>>(),
            });
            json_eq(&ours, &read_json(ctx, name))
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
        "gif-trim" => {
            let theirs = read_json(ctx, name);
            let data = std::fs::read(ctx.inputs.join(c["file"].as_str().unwrap())).unwrap();
            let t = match gif_trimmer::GifLosslessTrimmer::parse(&data) {
                Ok(t) => t,
                Err(_) => return if theirs["error"].is_string() { Outcome::exact() } else { Outcome::fail("parse failed") },
            };
            let n = t.frame_count();
            let starts = t.starts();
            let times = [0.0, 0.05, 0.29, 0.3, 0.31, 1.0, 1.995, 2.11, 4.6, 99.0];
            let mut copies = Vec::new();
            let mut out = Outcome::exact();
            for first in 0..n {
                for end in first + 1..=n {
                    let can = t.can_copy_selection(first, end);
                    copies.push(serde_json::json!([first, end, can]));
                    if can {
                        let ours = t.copy(&data, first, end).unwrap();
                        out = out.and(compare::bytes(&format!("{first}-{end}"), &ours, &ctx.sharex.join(name).join(format!("{first}-{end}.gif"))));
                    }
                }
            }
            let ours = serde_json::json!({
                "frames": n,
                "loop": t.looping,
                "repeat": t.repeat_count,
                "delays": (0..n).map(|i| t.delay(i)).collect::<Vec<_>>(),
                "needsValidation": (0..n).map(|i| t.needs_visual_validation(i)).collect::<Vec<_>>(),
                "copies": copies,
                "frameAt": times.iter().map(|&x| gif_trimmer::frame_at(&starts, x)).collect::<Vec<_>>(),
                "nearest": times.iter().map(|&x| gif_trimmer::nearest_boundary(&starts, x, false)).collect::<Vec<_>>(),
                "nearestEnd": times.iter().map(|&x| gif_trimmer::nearest_boundary(&starts, x, true)).collect::<Vec<_>>(),
            });
            out.and(json_eq(&ours, &theirs))
        }
        "gif-reencode" => {
            let theirs = read_json(ctx, name);
            let dir = ctx.sharex.join(name);
            let ours_dir = ctx.out.join(name);
            std::fs::create_dir_all(&ours_dir).unwrap();
            let path = ctx.inputs.join(c["file"].as_str().unwrap());
            let data = std::fs::read(&path).unwrap();
            let image = gif_trimmer::AnimatedGif::decode(&data).unwrap();
            let mut out = Outcome::exact();
            for i in 0..image.frame_count() {
                let theirs = compare::read_sxrb(&dir.join(format!("frame{i}.sxrb"))).unwrap();
                out = out.and(compare::bitmaps(&format!("frame{i}"), &image.frame(i), &theirs, 0));
            }
            let doc = gif_trimmer::AnimatedGifTrimmerDocument::open(data).unwrap();
            let preview = c["preview"].as_u64().unwrap_or(640) as u32;
            for i in 0..doc.frame_count() {
                let p = ours_dir.join(format!("preview{i}.png"));
                std::fs::write(&p, doc.render_frame(i, preview)).unwrap();
                out = out.and(compare::image_files(&format!("preview{i}"), &p, &dir.join(format!("preview{i}.png")), 0));
            }
            let mut results = Vec::new();
            let src = path.to_string_lossy().to_string();
            for (n, e) in c["exports"].as_array().unwrap().iter().enumerate() {
                let v = |i: usize| e[i].as_i64().unwrap();
                let a = |i: usize, j: usize| e[i][j].as_i64().unwrap() as i32;
                let crop = gif_trimmer::CropRect { x: a(3, 0), y: a(3, 1), width: a(3, 2), height: a(3, 3) };
                let target = ours_dir.join(format!("export{n}.gif"));
                let _ = std::fs::remove_file(&target);
                let r = doc.export(
                    &src,
                    &target.to_string_lossy(),
                    v(0) as usize,
                    v(1) as usize,
                    e[2].as_bool().unwrap(),
                    crop,
                    (a(4, 0), a(4, 1)),
                    None,
                );
                match r {
                    Ok(ok) => {
                        results.push(serde_json::json!(ok));
                        if ok {
                            let ours = std::fs::read(&target).unwrap();
                            out = out.and(compare::bytes(&format!("export{n}"), &ours, &dir.join(format!("export{n}.gif"))));
                        }
                    }
                    Err(_) => results.push(serde_json::json!("ArgumentOutOfRangeException")),
                }
            }
            let ours = serde_json::json!({ "frames": image.frame_count(), "exports": results });
            out.and(json_eq(&ours, &theirs))
        }
        _ => Outcome::skip(format!("no validator for {tool}")),
    }
}
