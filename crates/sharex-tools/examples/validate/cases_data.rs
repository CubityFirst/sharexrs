//! The Data, Video and System tools' cases.

use serde_json::Value;
use sharex_tools::{hash, imaging, indexer, metadata, qr};

use crate::cases::Ctx;
use crate::compare::{self, Outcome};

/// The indexers' footers carry the time they ran: mask it.
fn mask_time(s: &str) -> String {
    let re = regex::Regex::new(r"\d{4}-\d{2}-\d{2} at \d{2}:\d{2}:\d{2} UTC").unwrap();
    re.replace_all(s, "<time>").into_owned()
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
        _ => Outcome::skip(format!("no validator for {tool}")),
    }
}
